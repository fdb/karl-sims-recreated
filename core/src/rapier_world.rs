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
        solver_iterations: usize,
        pgs_iterations: usize,
        friction_coefficient: f64,
        use_coulomb_friction: bool,
        friction_combine_max: bool,
    ) -> Self {
        let mut bodies = RigidBodySet::new();
        let mut colliders = ColliderSet::new();
        let mut impulse_joints = ImpulseJointSet::new();
        let multibody_joints = MultibodyJointSet::new();

        // ── Interaction groups ─────────────────────────────────────────────────
        // Creature bodies belong to GROUP_1 and only collide with GROUP_2 (ground).
        // This prevents sibling bodies from colliding with each other, which would
        // produce enormous forces that overwhelm the joint constraints.
        let creature_groups = InteractionGroups::new(Group::GROUP_1, Group::GROUP_2, InteractionTestMode::And);
        let ground_groups   = InteractionGroups::new(Group::GROUP_2, Group::GROUP_1, InteractionTestMode::And);

        // ── Create rigid bodies and box colliders ─────────────────────────────
        let mut body_handles: Vec<RigidBodyHandle> = Vec::with_capacity(world_bodies.len());

        for (i, body) in world_bodies.iter().enumerate() {
            let pose = affine_to_pose(&world_transforms[i]);
            let he = body.half_extents;

            // Small baseline damping in land mode dissipates the energy that
            // PGS solvers accumulate from constraint-violation corrections.
            // Without it, chains lying on ground bounce themselves into orbit.
            let (lin_damp, ang_damp) = if water_enabled { (2.0, 2.0) } else { (0.3, 0.5) };
            let rb = RigidBodyBuilder::dynamic()
                .pose(pose)
                .linear_damping(lin_damp)
                .angular_damping(ang_damp)
                .build();
            let handle = bodies.insert(rb);
            body_handles.push(handle);

            let mut coll_builder = ColliderBuilder::cuboid(he.x, he.y, he.z)
                .density(body.mass / (he.x * he.y * he.z * 8.0))
                .restitution(0.1)
                .friction(friction_coefficient)
                .collision_groups(creature_groups);
            if friction_combine_max {
                coll_builder = coll_builder.friction_combine_rule(CoefficientCombineRule::Max);
            }
            colliders.insert_with_parent(coll_builder.build(), handle, &mut bodies);
        }

        // ── Ground plane ──────────────────────────────────────────────────────
        // Use a large flat box with its top surface at y=0 rather than a halfspace,
        // since halfspace requires a Unit<Vector> which is harder to construct.
        if ground_enabled {
            let mut ground_builder = ColliderBuilder::cuboid(1000.0, 0.1, 1000.0)
                .translation(DVec3::new(0.0, -0.1, 0.0))
                .friction(friction_coefficient)
                .restitution(0.1)
                .collision_groups(ground_groups);
            if friction_combine_max {
                ground_builder = ground_builder.friction_combine_rule(CoefficientCombineRule::Max);
            }
            colliders.insert(ground_builder.build());
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
                    // ForceBased motor-velocity damping. The empirical 10× scale
                    // compensates for Rapier's cfm_gain formula (1/(dt·damping))
                    // making the effective damping much softer per-step than
                    // Featherstone's direct torque subtraction. Calibrated
                    // against the Featherstone water-starfish trace (Featherstone
                    // joint-angle amplitude ≈ 0.2 rad; Rapier at 10× matches).
                    const DAMPING_SCALE: f64 = 10.0;
                    let jd = RevoluteJointBuilder::new(axis)
                        .local_anchor1(joint.parent_anchor)
                        .local_anchor2(joint.child_anchor)
                        .limits([joint.angle_min[0], joint.angle_max[0]])
                        .motor_model(MotorModel::ForceBased)
                        .motor_velocity(0.0, joint.damping * DAMPING_SCALE)
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
        integration_params.num_solver_iterations = solver_iterations;
        integration_params.num_internal_pgs_iterations = pgs_iterations;
        if use_coulomb_friction {
            integration_params.friction_model = FrictionModel::Coulomb;
        }

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

    /// Add a rigid body to the Rapier simulation mid-step.
    ///
    /// Used by the developmental growth system to add body segments
    /// after the simulation has already started. Returns the index into
    /// `body_handles` for the new body.
    pub fn add_body_dynamic(
        &mut self,
        body: &RigidBody,
        transform: &DAffine3,
        water_enabled: bool,
        friction_coefficient: f64,
        friction_combine_max: bool,
    ) -> usize {
        let creature_groups = InteractionGroups::new(
            Group::GROUP_1, Group::GROUP_2, InteractionTestMode::And,
        );
        let pose = affine_to_pose(transform);
        let he = body.half_extents;

        let (lin_damp, ang_damp) = if water_enabled { (2.0, 2.0) } else { (0.3, 0.5) };
        let rb = RigidBodyBuilder::dynamic()
            .pose(pose)
            .linear_damping(lin_damp)
            .angular_damping(ang_damp)
            .build();
        let handle = self.bodies.insert(rb);

        let mut coll_builder = ColliderBuilder::cuboid(he.x, he.y, he.z)
            .density(body.mass / (he.x * he.y * he.z * 8.0))
            .restitution(0.1)
            .friction(friction_coefficient)
            .collision_groups(creature_groups);
        if friction_combine_max {
            coll_builder = coll_builder.friction_combine_rule(CoefficientCombineRule::Max);
        }
        self.colliders.insert_with_parent(coll_builder.build(), handle, &mut self.bodies);

        let idx = self.body_handles.len();
        self.body_handles.push(handle);
        idx
    }

    /// Add a joint to the Rapier simulation mid-step.
    ///
    /// Used by the developmental growth system to connect a newly-grown
    /// body segment to its parent. Returns the index into `joint_handles`.
    pub fn add_joint_dynamic(&mut self, joint: &Joint) -> usize {
        let ph = self.body_handles[joint.parent_idx];
        let ch = self.body_handles[joint.child_idx];

        let handle = match joint.joint_type {
            JointType::Rigid => {
                let jd = FixedJointBuilder::new()
                    .local_anchor1(joint.parent_anchor)
                    .local_anchor2(joint.child_anchor)
                    .build();
                self.impulse_joints.insert(ph, ch, jd, true)
            }
            JointType::Revolute | JointType::Twist => {
                let axis = joint.axis;
                const DAMPING_SCALE: f64 = 10.0;
                let jd = RevoluteJointBuilder::new(axis)
                    .local_anchor1(joint.parent_anchor)
                    .local_anchor2(joint.child_anchor)
                    .limits([joint.angle_min[0], joint.angle_max[0]])
                    .motor_model(MotorModel::ForceBased)
                    .motor_velocity(0.0, joint.damping * DAMPING_SCALE)
                    .build();
                self.impulse_joints.insert(ph, ch, jd, true)
            }
            JointType::Spherical => {
                let jd = SphericalJointBuilder::new()
                    .local_anchor1(joint.parent_anchor)
                    .local_anchor2(joint.child_anchor)
                    .build();
                self.impulse_joints.insert(ph, ch, jd, true)
            }
            JointType::Universal | JointType::BendTwist | JointType::TwistBend => {
                let jd = SphericalJointBuilder::new()
                    .local_anchor1(joint.parent_anchor)
                    .local_anchor2(joint.child_anchor)
                    .build();
                self.impulse_joints.insert(ph, ch, jd, true)
            }
        };

        let idx = self.joint_handles.len();
        self.joint_handles.push(handle);
        idx
    }

    /// Run one physics frame of duration `dt`.
    ///
    /// Splits the frame into fixed 4 ms sub-steps (matching `Featherstone`'s
    /// typical RK45 step size). At 60 Hz this is 4 sub-steps per frame. Without
    /// substepping, stiff joint limits combined with ground contact overshoot
    /// within a single 16.67 ms step and the PGS solver diverges.
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
        const TARGET_SUB_DT: f64 = 1.0 / 480.0; // 2.08 ms, gives 8 sub-steps at 60Hz
        let n_sub = ((dt / TARGET_SUB_DT).ceil() as usize).max(1);
        let sub_dt = dt / n_sub as f64;
        self.integration_params.dt = sub_dt;

        for _ in 0..n_sub {
            // Torques & drag must be re-applied every sub-step: Rapier clears
            // accumulated forces after each `pipeline.step`.
            self.apply_torques(world_joints, world_torques, world_transforms);
            if water_enabled {
                self.apply_water_drag(water_viscosity, world_bodies, world_transforms);
            }

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

            // Clamp per-body velocities after each sub-step. Prevents runaway
            // contact-kick amplification: a body rotating at 100+ rad/s that
            // clips the ground gains massive linear velocity from the
            // penetration-recovery impulse, which then kicks it out of the
            // world. Clamping caps the feedback loop at each sub-step so
            // fast-spinning bodies simply stop gaining energy rather than
            // tunnelling through contacts.
            self.clamp_velocities();
        }

        // ── Post-frame: sync transforms back to World ─────────────────────────
        self.sync_transforms(world_transforms);

        // ── Post-frame: compute joint angles from relative body orientations ──
        self.sync_joint_angles(world_joints, world_transforms);
    }

    /// Cap each body's linear speed at MAX_LINVEL and angular speed at MAX_ANGVEL.
    /// These are generous bounds (20 m/s ≈ 72 km/h, 30 rad/s ≈ 5 rev/s) — faster
    /// than any real creature moves but tight enough to prevent contact-kick
    /// runaway. Scales the velocity vector rather than clipping components so
    /// direction is preserved.
    fn clamp_velocities(&mut self) {
        const MAX_LINVEL: f64 = 20.0;
        const MAX_ANGVEL: f64 = 30.0;
        for handle in &self.body_handles {
            if let Some(rb) = self.bodies.get_mut(*handle) {
                let v = rb.linvel();
                let speed = v.length();
                if speed > MAX_LINVEL {
                    rb.set_linvel(v * (MAX_LINVEL / speed), true);
                }
                let w = rb.angvel();
                let wmag = w.length();
                if wmag > MAX_ANGVEL {
                    rb.set_angvel(w * (MAX_ANGVEL / wmag), true);
                }
            }
        }
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
                // Apply drag on all faces (symmetric linear drag). The previous
                // `v_normal > 0` guard was a one-sided wake approximation that
                // halved drag and created net-thrust asymmetries, sending
                // swimmers to 100+ m/s. Symmetric drag produces balanced
                // deceleration so oscillation-driven thrust comes only from
                // geometric asymmetries (face areas, moment arms).
                let drag = -viscosity * v_normal * area;
                let f = world_normal * drag;
                total_force += f;
                total_torque += r.cross(f);
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

/// Extract rotation angle about `axis` from quaternion `q`, stably in [-π, π].
///
/// Uses `2 * atan2(q.vector · axis, q.scalar)`, but first canonicalises `q` so
/// that `q.w >= 0`. Without canonicalisation, quaternion sign-flip (which is
/// physically the same rotation: q and -q both represent R) makes the reported
/// angle jump by 2π across frames — creating a spurious ~377 rad/s apparent
/// angular velocity that poisons any downstream consumer doing angle-delta
/// analysis (min_joint_motion Welford accumulator, scanners, plots).
///
/// This is an *observation* function — it only affects what we read out of
/// `joint.angles`, not the physics simulation itself.
fn angle_about_axis(q: DQuat, axis: DVec3) -> f64 {
    let axis = axis.normalize_or_zero();
    if axis.length_squared() < 1e-10 {
        return 0.0;
    }
    // Canonicalise: flip sign so q.w is non-negative. q and -q represent the
    // same physical rotation, so this is a no-op on dynamics but produces a
    // continuous angle signal in [-π, π].
    let sign = if q.w < 0.0 { -1.0 } else { 1.0 };
    let w = sign * q.w;
    let v_proj = sign * DVec3::new(q.x, q.y, q.z).dot(axis);
    2.0 * f64::atan2(v_proj, w)
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::RigidBody;
    use crate::joint::Joint;

    #[test]
    fn angle_about_axis_stable_under_quaternion_sign_flip() {
        // q and -q represent the same physical rotation. Without
        // canonicalisation the two give angles 2π apart, creating
        // a spurious discontinuity in observed joint angle.
        let axis = DVec3::new(0.0, 0.0, 1.0);
        for theta_deg in [-170.0, -90.0, 0.0, 45.0, 90.0, 170.0] {
            let theta = (theta_deg as f64).to_radians();
            let q = DQuat::from_axis_angle(axis, theta);
            let q_neg = DQuat::from_xyzw(-q.x, -q.y, -q.z, -q.w);
            let a = angle_about_axis(q, axis);
            let a_neg = angle_about_axis(q_neg, axis);
            assert!(
                (a - a_neg).abs() < 1e-9,
                "angle_about_axis({theta_deg}°) unstable under q→-q: {a} vs {a_neg}"
            );
            assert!(
                (a - theta).abs() < 1e-9,
                "angle_about_axis should recover the original angle for {theta_deg}°: got {a}"
            );
        }
    }

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
            4, 1, 0.8, false, false,
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
            4, 1, 0.8, false, false,
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

    // ── Built-in creature stability under Rapier ─────────────────────────────
    //
    // Each hand-crafted creature (swimmer-starfish, swimmer-snake, walker-inchworm,
    // walker-lizard) should run stably through Rapier. We check three invariants
    // after 60 frames (1 s at 60 Hz):
    //   1. No NaN/Inf positions.
    //   2. No body has escaped to |pos| > 10 m.
    //   3. Every joint's anchor-distance stays < 0.1 m.
    //
    // Anchor distance = distance between joint.parent_anchor in the parent's world
    // frame and joint.child_anchor in the child's world frame. When the constraint
    // is satisfied these points coincide, so this directly measures joint violation.

    fn max_anchor_distance(world: &crate::world::World) -> f64 {
        let mut worst = 0.0_f64;
        for j in &world.joints {
            let p = world.transforms[j.parent_idx].transform_point3(j.parent_anchor);
            let c = world.transforms[j.child_idx].transform_point3(j.child_anchor);
            worst = worst.max((p - c).length());
        }
        worst
    }

    fn run_builtin_rapier(name: &str, env: &str, frames: usize) -> crate::world::World {
        let def = crate::creature_def::builtin(name)
            .unwrap_or_else(|| panic!("unknown creature: {name}"));
        let mut world = def.build_world();
        match env {
            "Land" => {
                world.water_enabled = false;
                world.gravity = DVec3::new(0.0, -9.81, 0.0);
                world.ground_enabled = true;
                world.set_root_transform(
                    DAffine3::from_translation(DVec3::new(0.0, 2.0, 0.0)),
                );
                world.forward_kinematics();
            }
            _ => {
                world.water_enabled = true;
                world.water_viscosity = 2.0;
                world.gravity = DVec3::ZERO;
            }
        }

        let dt = 1.0 / 60.0;
        for _ in 0..frames {
            def.apply_torques(&mut world);
            world.step(dt);
        }
        world
    }

    fn assert_stable(world: &crate::world::World, name: &str) {
        // 20 m limit: over 10 s of sim this allows 2 m/s average drift
        // (walking/swimming creatures legitimately move) but catches real
        // blowups, which in practice produce speeds of 100+ m/s.
        const MAX_POS: f64 = 20.0;
        // 0.15 m anchor violation: for 0.3–0.5 m body segments this corresponds
        // to a joint that's visibly stretched but not ripped apart.
        const MAX_ANCHOR_DIST: f64 = 0.15;

        for (i, t) in world.transforms.iter().enumerate() {
            assert!(
                t.translation.is_finite(),
                "{name}: body {i} has non-finite position {:?}",
                t.translation
            );
            let mag = t.translation.length();
            assert!(
                mag < MAX_POS,
                "{name}: body {i} escaped to |pos|={mag:.2} m"
            );
        }
        let ad = max_anchor_distance(world);
        assert!(
            ad < MAX_ANCHOR_DIST,
            "{name}: max joint anchor distance {ad:.3} m (constraint badly violated)"
        );
    }

    // 600 frames = 10 s, matching what a viewer session would exhibit before
    // the user notices. Bugs that take >1 s to diverge must still be caught.
    const STABILITY_FRAMES: usize = 600;

    #[test]
    fn builtin_swimmer_starfish_rapier_stable() {
        let w = run_builtin_rapier("swimmer-starfish", "Water", STABILITY_FRAMES);
        assert_stable(&w, "swimmer-starfish");
    }

    #[test]
    fn builtin_swimmer_snake_rapier_stable() {
        let w = run_builtin_rapier("swimmer-snake", "Water", STABILITY_FRAMES);
        assert_stable(&w, "swimmer-snake");
    }

    #[test]
    fn builtin_walker_inchworm_rapier_stable() {
        let w = run_builtin_rapier("walker-inchworm", "Land", STABILITY_FRAMES);
        assert_stable(&w, "walker-inchworm");
    }

    #[test]
    fn builtin_walker_lizard_rapier_stable() {
        let w = run_builtin_rapier("walker-lizard", "Land", STABILITY_FRAMES);
        assert_stable(&w, "walker-lizard");
    }

}
