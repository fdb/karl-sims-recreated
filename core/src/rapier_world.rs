//! Rapier physics backend for the World.
//!
//! This module is compiled only when the `rapier-physics` feature is enabled.
//! It defines [`RapierState`], which is embedded as an `Option` in [`World`]
//! and lazily initialised on the first `step()` call.
//!
//! ## Sync protocol (each sub-step)
//! 1. Pre-step: read `world.torques` → apply as body torques (Newton 3rd law pair).
//! 2. Pre-step: apply water drag forces if `water_enabled`.
//! 3. Run Rapier `PhysicsPipeline::step`.
//! 4. Post-step: read Rapier body transforms → `world.transforms`.
//! 5. Post-step: compute relative body rotations → `world.joints[i].angles/velocities`.

use glam::{DAffine3, DMat3, DQuat, DVec3};
use rapier3d_f64::prelude::*;

use crate::body::RigidBody;
use crate::joint::{Joint, JointType};

// ── Coordinate conversion helpers ────────────────────────────────────────────
// rapier3d-f64 0.32 uses glamx types internally (Pose3 = DPose3, Vec3 = DVec3,
// Rot3 = DRot3 = DQuat), so conversion to/from our glam types is near-trivial.

#[inline]
fn affine_to_pose(tf: &DAffine3) -> Pose3 {
    Pose3::from_parts(tf.translation, DQuat::from_mat3(&tf.matrix3))
}

#[inline]
fn pose_to_affine(pose: &Pose3) -> DAffine3 {
    DAffine3 {
        matrix3: DMat3::from_quat(pose.rotation),
        translation: pose.translation,
    }
}

// ── RapierState ───────────────────────────────────────────────────────────────

pub struct RapierState {
    pipeline: PhysicsPipeline,
    integration_params: IntegrationParameters,
    islands: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    pub bodies: RigidBodySet,
    pub colliders: ColliderSet,
    impulse_joints: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccd_solver: CCDSolver,

    /// Rapier handle for each entry in `world.bodies` (same index).
    pub body_handles: Vec<RigidBodyHandle>,
    /// Rapier handle for each entry in `world.joints` (same index).
    pub joint_handles: Vec<ImpulseJointHandle>,
}

impl RapierState {
    /// Build Rapier state from the current (fully-developed) World.
    ///
    /// Called once on the first `step()`. At this point `world.transforms` has
    /// been set by `forward_kinematics()`, and `world.bodies`/`world.joints`
    /// are fully populated.
    pub fn build(
        world_bodies: &[RigidBody],
        world_joints: &[Joint],
        world_transforms: &[DAffine3],
        root_idx: usize,
        gravity: DVec3,
        ground_enabled: bool,
        water_enabled: bool,
    ) -> Self {
        let mut bodies = RigidBodySet::new();
        let mut colliders = ColliderSet::new();
        let mut impulse_joints = ImpulseJointSet::new();
        let multibody_joints = MultibodyJointSet::new();

        // ── Create rigid bodies and box colliders ─────────────────────────────
        let mut body_handles: Vec<RigidBodyHandle> = Vec::with_capacity(world_bodies.len());

        for (i, body) in world_bodies.iter().enumerate() {
            let pose = affine_to_pose(&world_transforms[i]);
            let he = body.half_extents;

            let rb = RigidBodyBuilder::dynamic()
                .pose(pose)
                .linear_damping(if water_enabled { 2.0 } else { 0.0 })
                .angular_damping(if water_enabled { 2.0 } else { 0.0 })
                .build();
            let handle = bodies.insert(rb);
            body_handles.push(handle);

            let collider = ColliderBuilder::cuboid(he.x, he.y, he.z)
                .density(body.mass / (he.x * he.y * he.z * 8.0))
                .restitution(0.1)
                .friction(0.8)
                .build();
            colliders.insert_with_parent(collider, handle, &mut bodies);
        }

        // ── Ground plane ──────────────────────────────────────────────────────
        // Use a large flat box with its top surface at y=0 rather than a halfspace,
        // since halfspace requires a Unit<Vector> which is harder to construct.
        if ground_enabled {
            let ground = ColliderBuilder::cuboid(1000.0, 0.1, 1000.0)
                .translation(DVec3::new(0.0, -0.1, 0.0))
                .friction(0.8)
                .restitution(0.1)
                .build();
            colliders.insert(ground);
        }

        // ── Joints ────────────────────────────────────────────────────────────
        let mut joint_handles: Vec<ImpulseJointHandle> = Vec::with_capacity(world_joints.len());

        for joint in world_joints.iter() {
            let ph = body_handles[joint.parent_idx];
            let ch = body_handles[joint.child_idx];

            let handle = match joint.joint_type {
                JointType::Rigid => {
                    let jd = FixedJointBuilder::new()
                        .local_anchor1(joint.parent_anchor)
                        .local_anchor2(joint.child_anchor)
                        .build();
                    impulse_joints.insert(ph, ch, jd, true)
                }
                JointType::Revolute | JointType::Twist => {
                    let axis = joint.axis;
                    let jd = RevoluteJointBuilder::new(axis)
                        .local_anchor1(joint.parent_anchor)
                        .local_anchor2(joint.child_anchor)
                        .limits([joint.angle_min[0], joint.angle_max[0]])
                        .build();
                    impulse_joints.insert(ph, ch, jd, true)
                }
                JointType::Spherical => {
                    let jd = SphericalJointBuilder::new()
                        .local_anchor1(joint.parent_anchor)
                        .local_anchor2(joint.child_anchor)
                        .build();
                    impulse_joints.insert(ph, ch, jd, true)
                }
                JointType::Universal | JointType::BendTwist | JointType::TwistBend => {
                    // 2-DOF joints: approximate as Spherical for now.
                    let jd = SphericalJointBuilder::new()
                        .local_anchor1(joint.parent_anchor)
                        .local_anchor2(joint.child_anchor)
                        .build();
                    impulse_joints.insert(ph, ch, jd, true)
                }
            };
            joint_handles.push(handle);
        }

        // ── Integration parameters ────────────────────────────────────────────
        let mut integration_params = IntegrationParameters::default();
        integration_params.dt = 1.0 / 60.0; // will be overridden at step time

        let _ = (root_idx, gravity, water_enabled); // used via body setup above

        Self {
            pipeline: PhysicsPipeline::new(),
            integration_params,
            islands: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            bodies,
            colliders,
            impulse_joints,
            multibody_joints,
            ccd_solver: CCDSolver::new(),
            body_handles,
            joint_handles,
        }
    }

    /// Run one physics frame of duration `dt`.
    pub fn step(
        &mut self,
        dt: f64,
        gravity: DVec3,
        world_joints: &mut Vec<Joint>,
        world_torques: &[JointTorques],
        world_transforms: &mut Vec<DAffine3>,
        water_enabled: bool,
        water_viscosity: f64,
        world_bodies: &[RigidBody],
    ) {
        self.integration_params.dt = dt;

        // ── Pre-step: apply joint torques as body force/torque pairs ──────────
        self.apply_torques(world_joints, world_torques, world_transforms);

        // ── Pre-step: water drag ──────────────────────────────────────────────
        if water_enabled {
            self.apply_water_drag(water_viscosity, world_bodies, world_transforms);
        }

        // ── Rapier step ───────────────────────────────────────────────────────
        self.pipeline.step(
            gravity,
            &self.integration_params,
            &mut self.islands,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd_solver,
            &(),
            &(),
        );

        // ── Post-step: sync transforms back to World ──────────────────────────
        self.sync_transforms(world_transforms);

        // ── Post-step: compute joint angles from relative body orientations ───
        self.sync_joint_angles(world_joints, world_transforms);
    }

    fn apply_torques(
        &mut self,
        world_joints: &[Joint],
        world_torques: &[JointTorques],
        _world_transforms: &[DAffine3],
    ) {
        for (ji, joint) in world_joints.iter().enumerate() {
            if ji >= world_torques.len() {
                continue;
            }
            let torques = world_torques[ji];
            let axes = joint.dof_axes_glam();

            for (dof, axis) in axes.iter().enumerate() {
                let tau: f64 = torques[dof];
                if tau.abs() < 1e-12 {
                    continue;
                }
                // Rotate local joint axis into world space via parent body orientation.
                let parent_rot = body_rotation(&self.bodies, &self.body_handles, joint.parent_idx);
                let world_axis = parent_rot * *axis;
                let torque_vec = world_axis * tau;

                // Newton's 3rd law: equal and opposite on parent and child.
                if let Some(rb) = self.bodies.get_mut(self.body_handles[joint.child_idx]) {
                    rb.add_torque(torque_vec, true);
                }
                if let Some(rb) = self.bodies.get_mut(self.body_handles[joint.parent_idx]) {
                    rb.add_torque(-torque_vec, true);
                }
            }
        }
    }

    fn apply_water_drag(
        &mut self,
        viscosity: f64,
        world_bodies: &[RigidBody],
        world_transforms: &[DAffine3],
    ) {
        for (i, handle) in self.body_handles.iter().enumerate() {
            let he = world_bodies[i].half_extents;
            let (linvel, angvel) = match self.bodies.get(*handle) {
                Some(rb) => (rb.linvel(), rb.angvel()),
                None => continue,
            };

            // Six faces: ±X, ±Y, ±Z with areas and face normals in body frame.
            let faces: [(DVec3, f64); 6] = [
                (DVec3::X,     4.0 * he.y * he.z),
                (DVec3::NEG_X, 4.0 * he.y * he.z),
                (DVec3::Y,     4.0 * he.x * he.z),
                (DVec3::NEG_Y, 4.0 * he.x * he.z),
                (DVec3::Z,     4.0 * he.x * he.y),
                (DVec3::NEG_Z, 4.0 * he.x * he.y),
            ];

            let rot = world_transforms[i].matrix3;
            let mut total_force = DVec3::ZERO;
            let mut total_torque = DVec3::ZERO;

            for (local_normal, area) in faces.iter() {
                let world_normal = rot * *local_normal;
                let face_extent = if local_normal.x.abs() > 0.5 {
                    he.x
                } else if local_normal.y.abs() > 0.5 {
                    he.y
                } else {
                    he.z
                };
                let r = rot * (*local_normal * face_extent);
                let v_face = linvel + angvel.cross(r);
                let v_normal = v_face.dot(world_normal);
                if v_normal > 0.0 {
                    let drag = -viscosity * v_normal * area;
                    let f = world_normal * drag;
                    total_force += f;
                    total_torque += r.cross(f);
                }
            }

            if let Some(rb) = self.bodies.get_mut(*handle) {
                rb.add_force(total_force, true);
                rb.add_torque(total_torque, true);
            }
        }
    }

    fn sync_transforms(&self, world_transforms: &mut Vec<DAffine3>) {
        for (i, handle) in self.body_handles.iter().enumerate() {
            if let Some(rb) = self.bodies.get(*handle) {
                world_transforms[i] = pose_to_affine(rb.position());
            }
        }
    }

    fn sync_joint_angles(&self, world_joints: &mut Vec<Joint>, world_transforms: &[DAffine3]) {
        for joint in world_joints.iter_mut() {
            let parent_tf = &world_transforms[joint.parent_idx];
            let child_tf = &world_transforms[joint.child_idx];

            let q_parent = DQuat::from_mat3(&parent_tf.matrix3);
            let q_child = DQuat::from_mat3(&child_tf.matrix3);
            let q_rel = q_parent.conjugate() * q_child;

            let omega_child = self
                .bodies
                .get(self.body_handles[joint.child_idx])
                .map(|rb| rb.angvel())
                .unwrap_or(DVec3::ZERO);
            let omega_parent = self
                .bodies
                .get(self.body_handles[joint.parent_idx])
                .map(|rb| rb.angvel())
                .unwrap_or(DVec3::ZERO);
            let omega_rel_parent = q_parent.conjugate() * (omega_child - omega_parent);

            match joint.joint_type {
                JointType::Rigid => {}
                JointType::Revolute | JointType::Twist => {
                    joint.angles[0] = angle_about_axis(q_rel, joint.axis);
                    joint.velocities[0] = omega_rel_parent.dot(joint.axis);
                }
                JointType::Universal | JointType::BendTwist | JointType::TwistBend => {
                    joint.angles[0] = angle_about_axis(q_rel, joint.axis);
                    joint.angles[1] = angle_about_axis(q_rel, joint.secondary_axis);
                    joint.velocities[0] = omega_rel_parent.dot(joint.axis);
                    joint.velocities[1] = omega_rel_parent.dot(joint.secondary_axis);
                }
                JointType::Spherical => {
                    let third = joint.axis.cross(joint.secondary_axis).normalize_or_zero();
                    joint.angles[0] = angle_about_axis(q_rel, joint.axis);
                    joint.angles[1] = angle_about_axis(q_rel, joint.secondary_axis);
                    joint.angles[2] = angle_about_axis(q_rel, third);
                    joint.velocities[0] = omega_rel_parent.dot(joint.axis);
                    joint.velocities[1] = omega_rel_parent.dot(joint.secondary_axis);
                    joint.velocities[2] = omega_rel_parent.dot(third);
                }
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Extract rotation angle about `axis` from quaternion `q`.
/// Uses 2 * atan2(q.vector · axis, q.scalar).
fn angle_about_axis(q: DQuat, axis: DVec3) -> f64 {
    let axis = axis.normalize_or_zero();
    if axis.length_squared() < 1e-10 {
        return 0.0;
    }
    let v_proj = DVec3::new(q.x, q.y, q.z).dot(axis);
    2.0 * f64::atan2(v_proj, q.w)
}

fn body_rotation(
    bodies: &RigidBodySet,
    body_handles: &[RigidBodyHandle],
    idx: usize,
) -> DQuat {
    bodies
        .get(body_handles[idx])
        .map(|rb| *rb.rotation())
        .unwrap_or(DQuat::IDENTITY)
}

// ── Type aliases ──────────────────────────────────────────────────────────────

/// Per-joint torque array: [dof0, dof1, dof2].
type JointTorques = [f64; 3];

// ── DOF axes trait (returns glam DVec3 axes for each DOF) ─────────────────────

trait DofAxesGlam {
    fn dof_axes_glam(&self) -> Vec<DVec3>;
}

impl DofAxesGlam for Joint {
    fn dof_axes_glam(&self) -> Vec<DVec3> {
        match self.joint_type {
            JointType::Rigid => vec![],
            JointType::Revolute | JointType::Twist => vec![self.axis],
            JointType::Universal | JointType::BendTwist | JointType::TwistBend => {
                vec![self.axis, self.secondary_axis]
            }
            JointType::Spherical => {
                let third = self.axis.cross(self.secondary_axis).normalize_or_zero();
                vec![self.axis, self.secondary_axis, third]
            }
        }
    }
}

// ── Fitness helpers ───────────────────────────────────────────────────────────

/// Evaluate swimming fitness using the Rapier physics backend.
///
/// Mirrors `fitness::evaluate_swimming_fitness` but routes `world.step()`
/// through Rapier by setting `world.use_rapier = true` after creature creation.
pub fn evaluate_swimming_fitness_rapier(
    genome: &crate::genotype::GenomeGraph,
    config: &crate::fitness::FitnessConfig,
) -> crate::fitness::FitnessResult {
    use crate::creature::Creature;
    use crate::fitness::FitnessResult;

    let mut creature = Creature::from_genome(genome.clone());
    creature.world.water_enabled = true;
    creature.world.water_viscosity = 2.0;
    creature.world.gravity = DVec3::ZERO;
    creature.world.use_rapier = true;

    if creature.world.bodies.len() > config.max_parts {
        return FitnessResult {
            score: 0.0,
            distance: 0.0,
            max_displacement: 0.0,
            terminated_early: true,
        };
    }

    let dt = config.dt;
    let total_steps = (config.sim_duration / dt).round() as usize;
    let early_check_step = (config.early_termination_time / dt).round() as usize;
    let initial_pos = creature.world.transforms[creature.world.root].translation;
    let mut max_displacement: f64 = 0.0;

    for step in 0..total_steps {
        creature.step(dt);
        let pos = creature.world.transforms[creature.world.root].translation;
        let disp = (pos - initial_pos).length();
        if !disp.is_finite() || disp > 1000.0 {
            return FitnessResult {
                score: 0.0,
                distance: 0.0,
                max_displacement: 0.0,
                terminated_early: true,
            };
        }
        max_displacement = max_displacement.max(disp);
        if step + 1 == early_check_step && disp < config.min_movement {
            return FitnessResult {
                score: 0.0,
                distance: 0.0,
                max_displacement,
                terminated_early: true,
            };
        }
    }

    let final_pos = creature.world.transforms[creature.world.root].translation;
    let distance = (final_pos - initial_pos).length();
    let score = distance * 0.7 + max_displacement * 0.3;
    FitnessResult {
        score,
        distance,
        max_displacement,
        terminated_early: false,
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::RigidBody;
    use crate::joint::Joint;

    fn make_single_body_world() -> (Vec<RigidBody>, Vec<Joint>, Vec<DAffine3>) {
        let body = RigidBody::new(DVec3::splat(0.5));
        let tf = DAffine3 {
            matrix3: DMat3::IDENTITY,
            translation: DVec3::new(0.0, 5.0, 0.0),
        };
        (vec![body], vec![], vec![tf])
    }

    #[test]
    fn single_body_falls_under_gravity() {
        let (bodies, joints, mut transforms) = make_single_body_world();
        let gravity = DVec3::new(0.0, -9.81, 0.0);

        let mut state = RapierState::build(
            &bodies, &joints, &transforms, 0, gravity,
            false, false,
        );

        let mut joints_mut = joints.clone();
        let torques: Vec<JointTorques> = vec![];

        for _ in 0..60 {
            state.step(
                1.0 / 60.0, gravity,
                &mut joints_mut, &torques,
                &mut transforms, false, 0.0, &bodies,
            );
        }

        // After 1s under gravity: y ≈ 5.0 - 0.5 * 9.81 ≈ 0.095
        let y = transforms[0].translation.y;
        assert!(y < 4.0, "body should have fallen; y={y:.3}");
        assert!(y > -5.0, "body should not have fallen through floor; y={y:.3}");
    }

    #[test]
    fn body_lands_on_ground() {
        let (bodies, joints, mut transforms) = make_single_body_world();
        let gravity = DVec3::new(0.0, -9.81, 0.0);

        let mut state = RapierState::build(
            &bodies, &joints, &transforms, 0, gravity,
            true, false,
        );

        let mut joints_mut = joints.clone();
        let torques: Vec<JointTorques> = vec![];

        // Step 5 seconds — enough to fall 5m and settle
        for _ in 0..300 {
            state.step(
                1.0 / 60.0, gravity,
                &mut joints_mut, &torques,
                &mut transforms, false, 0.0, &bodies,
            );
        }

        // Body half-extent is 0.5, ground top surface at y=0.
        // Body should rest at y ≈ 0.5 ± bounce.
        let y = transforms[0].translation.y;
        assert!(y < 1.5, "body should have landed; y={y:.3}");
        assert!(y > -0.5, "body should not be below ground; y={y:.3}");
    }

    // ── Milestone (d): fitness comparison against Featherstone ───────────────

    /// Run both Featherstone and Rapier fitness evals on the same genome,
    /// print the numbers, and assert basic sanity (non-NaN, non-negative).
    ///
    /// Uses a short sim (2s) for speed. The purpose is to verify Rapier
    /// produces finite, plausible fitness numbers — not that it matches
    /// Featherstone exactly (different physics → different numbers are expected).
    #[test]
    fn rapier_vs_featherstone_water_fitness() {
        use crate::fitness::{evaluate_swimming_fitness, FitnessConfig};
        use crate::genotype::GenomeGraph;
        use rand::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        let config = FitnessConfig {
            sim_duration: 2.0,
            ..Default::default()
        };

        // Test across several seeds to get a statistical sample, not just one lucky creature.
        let seeds = [7u64, 15, 23, 42, 77];
        let mut rapier_scores = Vec::new();
        let mut feather_scores = Vec::new();
        let mut rapier_nans = 0usize;

        for seed in seeds {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let genome = GenomeGraph::random(&mut rng);

            // Featherstone baseline
            let f_result = evaluate_swimming_fitness(&genome, &config);
            feather_scores.push(f_result.score);

            // Rapier
            let r_result = evaluate_swimming_fitness_rapier(&genome, &config);
            if !r_result.score.is_finite() {
                rapier_nans += 1;
            } else {
                rapier_scores.push(r_result.score);
            }

            eprintln!(
                "seed {:3}: feather={:.4} (early={}) | rapier={:.4} (early={})",
                seed,
                f_result.score, f_result.terminated_early,
                r_result.score, r_result.terminated_early,
            );
        }

        // Sanity checks
        assert_eq!(rapier_nans, 0, "Rapier produced NaN/Inf scores on {rapier_nans} creatures");
        assert!(
            rapier_scores.iter().all(|&s| s >= 0.0),
            "Rapier produced negative scores: {rapier_scores:?}"
        );

        let rapier_nonzero = rapier_scores.iter().filter(|&&s| s > 0.01).count();
        let feather_nonzero = feather_scores.iter().filter(|&&s| s > 0.01).count();
        eprintln!(
            "Non-zero: feather={}/{} rapier={}/{}",
            feather_nonzero, seeds.len(),
            rapier_nonzero, seeds.len()
        );
    }

    /// Rapier fitness evaluation should complete without panics on many random genomes.
    #[test]
    fn rapier_fitness_no_panic() {
        use super::evaluate_swimming_fitness_rapier;
        use crate::fitness::FitnessConfig;
        use crate::genotype::GenomeGraph;
        use rand::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        let config = FitnessConfig {
            sim_duration: 1.0,
            ..Default::default()
        };
        for seed in 0..10u64 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let genome = GenomeGraph::random(&mut rng);
            let result = evaluate_swimming_fitness_rapier(&genome, &config);
            assert!(result.score.is_finite() || result.terminated_early,
                "seed {seed}: Rapier produced non-finite score and didn't terminate early");
        }
    }
}
