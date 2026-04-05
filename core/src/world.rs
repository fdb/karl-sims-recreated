use glam::{DAffine3, DMat3, DQuat, DVec3};

use crate::body::RigidBody;
use crate::featherstone::FeatherstoneState;
use crate::integrator::{IntegratorConfig, JointDeriv, JointState};
use crate::joint::Joint;
use crate::spatial::SVec6;

// ── RKF45 Butcher tableau (duplicated from integrator to avoid closure borrow issues) ──

const B21: f64 = 1.0 / 4.0;
const B31: f64 = 3.0 / 32.0;
const B32: f64 = 9.0 / 32.0;
const B41: f64 = 1932.0 / 2197.0;
const B42: f64 = -7200.0 / 2197.0;
const B43: f64 = 7296.0 / 2197.0;
const B51: f64 = 439.0 / 216.0;
const B52: f64 = -8.0;
const B53: f64 = 3680.0 / 513.0;
const B54: f64 = -845.0 / 4104.0;
const B61: f64 = -8.0 / 27.0;
const B62: f64 = 2.0;
const B63: f64 = -3544.0 / 2565.0;
const B64: f64 = 1859.0 / 4104.0;
const B65: f64 = -11.0 / 40.0;
const C4: [f64; 6] = [25.0 / 216.0, 0.0, 1408.0 / 2565.0, 2197.0 / 4104.0, -1.0 / 5.0, 0.0];
const C5: [f64; 6] = [
    16.0 / 135.0,
    0.0,
    6656.0 / 12825.0,
    28561.0 / 56430.0,
    -9.0 / 50.0,
    2.0 / 55.0,
];

/// Weighted sum of derivatives.
fn weighted_deriv(ks: &[JointDeriv], weights: &[f64]) -> JointDeriv {
    let n_angles = ks[0].d_angles.len();
    let n_vels = ks[0].d_velocities.len();
    let mut d_angles = vec![0.0; n_angles];
    let mut d_velocities = vec![0.0; n_vels];
    for (k, &w) in ks.iter().zip(weights.iter()) {
        if w == 0.0 {
            continue;
        }
        for (da, ka) in d_angles.iter_mut().zip(k.d_angles.iter()) {
            *da += w * ka;
        }
        for (dv, kv) in d_velocities.iter_mut().zip(k.d_velocities.iter()) {
            *dv += w * kv;
        }
    }
    JointDeriv {
        d_angles,
        d_velocities,
    }
}

/// Build trial state: base + dt * weighted_sum(ks, weights).
fn trial_state(base: &JointState, ks: &[JointDeriv], weights: &[f64], dt: f64) -> JointState {
    let wd = weighted_deriv(&ks[..weights.len()], weights);
    base.advance(&wd, dt)
}

pub struct World {
    pub bodies: Vec<RigidBody>,
    pub joints: Vec<Joint>,
    pub transforms: Vec<DAffine3>,
    pub torques: Vec<[f64; 3]>,
    pub root: usize,
    pub gravity: DVec3,
    pub time: f64,
    pub water_enabled: bool,
    pub water_viscosity: f64,
    pub collisions_enabled: bool,
    pub ground_enabled: bool,
    suggested_dt: f64,
    pub root_velocity: DVec3,
    pub root_angular_velocity: DVec3,
    last_root_accel: SVec6,
    pub light_position: DVec3,
    /// Route `step()` through the Rapier physics backend.
    /// Only meaningful when the `rapier-physics` feature is enabled.
    pub use_rapier: bool,
    #[cfg(feature = "rapier-physics")]
    rapier_state: Option<crate::rapier_world::RapierState>,
}

impl std::fmt::Debug for World {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("World")
            .field("bodies", &self.bodies.len())
            .field("joints", &self.joints.len())
            .field("root", &self.root)
            .field("time", &self.time)
            .field("gravity", &self.gravity)
            .finish_non_exhaustive()
    }
}

impl Clone for World {
    fn clone(&self) -> Self {
        Self {
            bodies: self.bodies.clone(),
            joints: self.joints.clone(),
            transforms: self.transforms.clone(),
            torques: self.torques.clone(),
            root: self.root,
            gravity: self.gravity,
            time: self.time,
            water_enabled: self.water_enabled,
            water_viscosity: self.water_viscosity,
            collisions_enabled: self.collisions_enabled,
            ground_enabled: self.ground_enabled,
            suggested_dt: self.suggested_dt,
            root_velocity: self.root_velocity,
            root_angular_velocity: self.root_angular_velocity,
            last_root_accel: self.last_root_accel,
            light_position: self.light_position,
            use_rapier: self.use_rapier,
            // Rapier state cannot be cloned — it will be lazily re-initialised
            // on the first step() call of the cloned World.
            #[cfg(feature = "rapier-physics")]
            rapier_state: None,
        }
    }
}

impl World {
    pub fn new() -> Self {
        Self {
            bodies: Vec::new(),
            joints: Vec::new(),
            transforms: Vec::new(),
            torques: Vec::new(),
            root: 0,
            gravity: DVec3::ZERO,
            time: 0.0,
            water_enabled: false,
            water_viscosity: crate::water::DEFAULT_VISCOSITY,
            collisions_enabled: false,
            ground_enabled: false,
            suggested_dt: 1.0 / 120.0,
            root_velocity: DVec3::ZERO,
            root_angular_velocity: DVec3::ZERO,
            last_root_accel: SVec6::ZERO,
            light_position: DVec3::new(5.0, 0.0, 0.0),
            use_rapier: false,
            #[cfg(feature = "rapier-physics")]
            rapier_state: None,
        }
    }

    pub fn add_body(&mut self, half_extents: DVec3) -> usize {
        let idx = self.bodies.len();
        self.bodies.push(RigidBody::new(half_extents));
        self.transforms.push(DAffine3::IDENTITY);
        idx
    }

    pub fn add_joint(&mut self, joint: Joint) -> usize {
        let idx = self.joints.len();
        self.joints.push(joint);
        self.torques.push([0.0; 3]);
        idx
    }

    pub fn set_root_transform(&mut self, transform: DAffine3) {
        self.transforms[self.root] = transform;
    }

    pub fn forward_kinematics(&mut self) {
        for i in 0..self.joints.len() {
            let joint = &self.joints[i];
            let parent_idx = joint.parent_idx;
            let child_idx = joint.child_idx;
            let parent_anchor = joint.parent_anchor;
            let child_anchor = joint.child_anchor;
            let parent_transform = self.transforms[parent_idx];

            // Joint rotation from angles (handles multi-DOF)
            let joint_rotation = joint.joint_rotation();

            // Joint position in world space
            let joint_pos = parent_transform.transform_point3(parent_anchor);

            // Parent rotation as quaternion
            let parent_rotation = DQuat::from_mat3(&parent_transform.matrix3);

            // World rotation = parent * joint
            let world_rotation = parent_rotation * joint_rotation;

            // Child offset in world space
            let child_offset = world_rotation * (-child_anchor);

            // Child transform
            self.transforms[child_idx] = DAffine3 {
                matrix3: DMat3::from_quat(world_rotation),
                translation: joint_pos + child_offset,
            };
        }
    }

    /// Map of (joint_index, dof_index) for all active DOFs.
    fn dof_map(&self) -> Vec<(usize, usize)> {
        let mut map = Vec::new();
        for (ji, joint) in self.joints.iter().enumerate() {
            for di in 0..joint.joint_type.dof_count() {
                map.push((ji, di));
            }
        }
        map
    }

    /// Extract current state as flat vectors.
    fn get_state(&self, dof_map: &[(usize, usize)]) -> JointState {
        JointState {
            angles: dof_map
                .iter()
                .map(|&(ji, di)| self.joints[ji].angles[di])
                .collect(),
            velocities: dof_map
                .iter()
                .map(|&(ji, di)| self.joints[ji].velocities[di])
                .collect(),
        }
    }

    /// Apply flat state back to joints.
    fn set_state(&mut self, state: &JointState, dof_map: &[(usize, usize)]) {
        for (idx, &(ji, di)) in dof_map.iter().enumerate() {
            self.joints[ji].angles[di] = state.angles[idx];
            self.joints[ji].velocities[di] = state.velocities[idx];
        }
    }

    /// Evaluate dynamics at a given state. Returns derivatives.
    fn evaluate(&mut self, state: &JointState, dof_map: &[(usize, usize)]) -> JointDeriv {
        self.set_state(state, dof_map);
        self.forward_kinematics();

        // Run Featherstone to get body velocities (with zero external forces first)
        let mut fstate =
            FeatherstoneState::from_world(&self.bodies, &self.joints, &self.torques);
        let mut ext_forces = vec![SVec6::ZERO; self.bodies.len()];
        let floating = true; // always floating-base so contact forces act on root for land too
        let root_vel = SVec6::new(self.root_angular_velocity, self.root_velocity);
        let _ = fstate.compute_accelerations(self.gravity, &ext_forces, root_vel, floating);
        let body_vels = fstate.body_velocities();

        // Compute external forces from water drag
        if self.water_enabled {
            let drag = crate::water::compute_water_drag(
                &self.bodies,
                &self.transforms,
                body_vels,
                self.water_viscosity,
            );
            for (i, f) in drag.into_iter().enumerate() {
                if i < ext_forces.len() {
                    ext_forces[i] = ext_forces[i] + f;
                }
            }
        }

        // Compute collision forces (body-body)
        if self.collisions_enabled {
            let contacts = crate::collision::detect_collisions(
                &self.bodies,
                &self.transforms,
                &self.joints,
            );
            if !contacts.is_empty() {
                let col_forces = crate::collision::compute_collision_forces(
                    &contacts,
                    &self.transforms,
                    body_vels,
                    self.bodies.len(),
                    crate::collision::COLLISION_STIFFNESS,
                    crate::collision::COLLISION_DAMPING,
                );
                for (i, f) in col_forces.into_iter().enumerate() {
                    if i < ext_forces.len() {
                        ext_forces[i] = ext_forces[i] + f;
                    }
                }
            }
        }

        // Ground plane collision (y=0)
        if self.ground_enabled {
            let ground_contacts = crate::collision::detect_ground_collisions(
                &self.bodies,
                &self.transforms,
            );
            if !ground_contacts.is_empty() {
                let ground_forces = crate::collision::compute_ground_forces(
                    &ground_contacts,
                    &self.transforms,
                    body_vels,
                    self.bodies.len(),
                    crate::collision::COLLISION_STIFFNESS * 4.0, // stiffer for ground
                    crate::collision::COLLISION_DAMPING * 8.0,  // high damping prevents bouncing
                );
                for (i, f) in ground_forces.into_iter().enumerate() {
                    if i < ext_forces.len() {
                        ext_forces[i] = ext_forces[i] + f;
                    }
                }
            }
        }

        // Re-run Featherstone with external forces
        let mut fstate2 =
            FeatherstoneState::from_world(&self.bodies, &self.joints, &self.torques);
        let (qddot, root_accel) = fstate2.compute_accelerations(self.gravity, &ext_forces, root_vel, floating);
        self.last_root_accel = root_accel;

        // Build derivative
        let n = state.angles.len();
        let mut d_velocities = vec![0.0; n];
        for (fj_idx, fj) in fstate2.fjoints().iter().enumerate() {
            let ji = fj.original_joint_idx;
            let di = fj.original_dof_idx;
            if let Some(dof_idx) = dof_map.iter().position(|&(j, d)| j == ji && d == di) {
                d_velocities[dof_idx] = qddot[fj_idx];
            }
        }

        JointDeriv {
            d_angles: state.velocities.clone(),
            d_velocities,
        }
    }

    /// Predict root position/velocity at a trial time offset using ballistic
    /// trajectory under gravity. This ensures ground collision detection sees
    /// an approximately correct root position during each RK45 evaluation.
    fn predict_root(&mut self, orig_pos: DVec3, orig_vel: DVec3, trial_t: f64) {
        self.transforms[self.root].translation =
            orig_pos + orig_vel * trial_t + self.gravity * 0.5 * trial_t * trial_t;
        self.root_velocity = orig_vel + self.gravity * trial_t;
    }

    /// Run one adaptive RK45 step, returning (new_state, dt_used, dt_next).
    /// Inlined here to avoid borrow-checker issues with closures over &mut self.
    fn rk45_adaptive_step(
        &mut self,
        base: &JointState,
        mut dt: f64,
        dof_map: &[(usize, usize)],
        config: &IntegratorConfig,
    ) -> (JointState, f64, f64) {
        dt = dt.clamp(config.min_dt, config.max_dt);
        let n = base.angles.len();

        // Save root state — predict root position at each trial time so ground
        // collision detection sees an approximately correct root position.
        let orig_root_pos = self.transforms[self.root].translation;
        let orig_root_vel = self.root_velocity;
        let orig_root_ang_vel = self.root_angular_velocity;

        // RK45 Butcher tableau c values (trial time fractions)
        const C_FRAC: [f64; 6] = [0.0, 0.25, 3.0/8.0, 12.0/13.0, 1.0, 0.5];

        for _attempt in 0..8 {
            self.predict_root(orig_root_pos, orig_root_vel, C_FRAC[0] * dt);
            let k1 = self.evaluate(base, dof_map);

            let s2 = trial_state(base, &[k1.clone()], &[B21], dt);
            self.predict_root(orig_root_pos, orig_root_vel, C_FRAC[1] * dt);
            let k2 = self.evaluate(&s2, dof_map);

            let s3 = trial_state(base, &[k1.clone(), k2.clone()], &[B31, B32], dt);
            self.predict_root(orig_root_pos, orig_root_vel, C_FRAC[2] * dt);
            let k3 = self.evaluate(&s3, dof_map);

            let s4 = trial_state(
                base,
                &[k1.clone(), k2.clone(), k3.clone()],
                &[B41, B42, B43],
                dt,
            );
            self.predict_root(orig_root_pos, orig_root_vel, C_FRAC[3] * dt);
            let k4 = self.evaluate(&s4, dof_map);

            let s5 = trial_state(
                base,
                &[k1.clone(), k2.clone(), k3.clone(), k4.clone()],
                &[B51, B52, B53, B54],
                dt,
            );
            self.predict_root(orig_root_pos, orig_root_vel, C_FRAC[4] * dt);
            let k5 = self.evaluate(&s5, dof_map);

            let s6 = trial_state(
                base,
                &[k1.clone(), k2.clone(), k3.clone(), k4.clone(), k5.clone()],
                &[B61, B62, B63, B64, B65],
                dt,
            );
            self.predict_root(orig_root_pos, orig_root_vel, C_FRAC[5] * dt);
            let k6 = self.evaluate(&s6, dof_map);

            let ks = [k1, k2, k3, k4, k5, k6];

            let wd4 = weighted_deriv(&ks, &C4);
            let y4 = base.advance(&wd4, dt);
            let wd5 = weighted_deriv(&ks, &C5);
            let y5 = base.advance(&wd5, dt);

            let mut error = 0.0_f64;
            for i in 0..n {
                error = error.max((y5.angles[i] - y4.angles[i]).abs());
                error = error.max((y5.velocities[i] - y4.velocities[i]).abs());
            }

            if error < 1e-15 {
                // Restore root to pre-prediction state; integrate_root() handles actual motion.
                // Re-evaluate at accepted state for correct root_accel.
                self.transforms[self.root].translation = orig_root_pos;
                self.root_velocity = orig_root_vel;
                self.root_angular_velocity = orig_root_ang_vel;
                self.set_state(&y4, dof_map);
                self.evaluate(&y4, dof_map);
                return (y4, dt, dt);
            }

            let dt_optimal = config.safety_factor * dt * (config.tolerance / error).powf(0.2);
            let dt_optimal = dt_optimal.clamp(config.min_dt, config.max_dt);

            if error <= config.tolerance {
                self.transforms[self.root].translation = orig_root_pos;
                self.root_velocity = orig_root_vel;
                self.root_angular_velocity = orig_root_ang_vel;
                self.set_state(&y4, dof_map);
                self.evaluate(&y4, dof_map);
                return (y4, dt, dt_optimal);
            }

            dt = dt_optimal;
        }

        // Fallback: Euler step with minimum dt
        self.transforms[self.root].translation = orig_root_pos;
        self.root_velocity = orig_root_vel;
        self.root_angular_velocity = orig_root_ang_vel;
        let dt_min = config.min_dt;
        let deriv = self.evaluate(base, dof_map);
        let fallback = base.advance(&deriv, dt_min);
        (fallback, dt_min, dt_min)
    }

    /// Integrate root body position/velocity using current `last_root_accel`.
    /// Called after each sub-step so the root tracks the adaptive integrator.
    fn integrate_root(&mut self, dt: f64) {
        let root_accel = self.last_root_accel;
        let gravity_spatial = SVec6::new(DVec3::ZERO, -self.gravity);
        let actual_accel = root_accel - gravity_spatial;

        self.root_velocity += (actual_accel.linear() + self.gravity) * dt;
        self.root_angular_velocity += actual_accel.angular() * dt;

        // Clamp root velocity to prevent runaway dynamics.
        // 5 m/s ≈ 18 km/h, well beyond any realistic creature speed.
        const MAX_ROOT_SPEED: f64 = 5.0;
        let speed = self.root_velocity.length();
        if speed > MAX_ROOT_SPEED {
            self.root_velocity *= MAX_ROOT_SPEED / speed;
        }
        let ang_speed = self.root_angular_velocity.length();
        if ang_speed > MAX_ROOT_SPEED {
            self.root_angular_velocity *= MAX_ROOT_SPEED / ang_speed;
        }

        self.transforms[self.root].translation += self.root_velocity * dt;

        let ang = self.root_angular_velocity;
        let ang_mag = ang.length();
        if ang_mag > 1e-10 {
            let drot = DQuat::from_axis_angle(ang / ang_mag, ang_mag * dt);
            let current = DQuat::from_mat3(&self.transforms[self.root].matrix3);
            self.transforms[self.root].matrix3 = DMat3::from_quat(drot * current);
        }

        self.forward_kinematics();
    }

    pub fn step(&mut self, frame_dt: f64) {
        // ── Rapier physics backend (opt-in, feature-gated) ────────────────────
        #[cfg(feature = "rapier-physics")]
        if self.use_rapier {
            // Lazy-init: build Rapier world from current body/joint/transform state.
            if self.rapier_state.is_none() {
                self.rapier_state = Some(crate::rapier_world::RapierState::build(
                    &self.bodies,
                    &self.joints,
                    &self.transforms,
                    self.root,
                    self.gravity,
                    self.ground_enabled,
                    self.water_enabled,
                ));
            }
            let rapier = self.rapier_state.as_mut().unwrap();
            rapier.step(
                frame_dt,
                self.gravity,
                &mut self.joints,
                &self.torques,
                &mut self.transforms,
                self.water_enabled,
                self.water_viscosity,
                &self.bodies,
            );
            self.time += frame_dt;
            return;
        }

        let dof_map = self.dof_map();

        if !dof_map.is_empty() {
            let config = IntegratorConfig::default();
            let mut remaining = frame_dt;
            // Don't carry over collapsed step sizes from previous frames.
            // Cap minimum suggested_dt to avoid >100 sub-steps per frame.
            let min_suggested = frame_dt / 100.0;
            let mut dt = self.suggested_dt.max(min_suggested).min(remaining);

            while remaining > 1e-10 {
                let step_dt = dt.min(remaining);
                let base_state = self.get_state(&dof_map);

                let (new_state, dt_used, dt_next) =
                    self.rk45_adaptive_step(&base_state, step_dt, &dof_map, &config);

                // If the integrator produced NaN, stop.
                let has_nan = new_state.angles.iter().chain(&new_state.velocities)
                    .any(|v| !v.is_finite());
                self.set_state(&new_state, &dof_map);

                // Integrate root body at this sub-step's dt, not the full frame_dt.
                self.integrate_root(dt_used);

                remaining -= dt_used;
                dt = dt_next;
                self.suggested_dt = dt;
                if has_nan { break; }
            }
        } else {
            // No joints — sub-step the root body for stable ground contact.
            let sub_dt: f64 = 1.0 / 240.0; // 4 sub-steps per frame at 60fps
            let mut remaining = frame_dt;
            while remaining > 1e-10 {
                let dt = sub_dt.min(remaining);
                let empty_state = self.get_state(&dof_map);
                self.evaluate(&empty_state, &dof_map);
                self.integrate_root(dt);
                remaining -= dt;
            }
        }

        self.time += frame_dt;
    }

    /// Fast single-step integration for browser preview rendering.
    /// Uses one Featherstone evaluation + simple Euler instead of adaptive RK45.
    /// Less accurate but ~6x faster — suitable for visual preview, not fitness evaluation.
    pub fn step_fast(&mut self, dt: f64) {
        let dof_map = self.dof_map();

        // Single evaluation: FK → forces → Featherstone → accelerations
        let state = self.get_state(&dof_map);
        let deriv = self.evaluate(&state, &dof_map);

        // Simple semi-implicit Euler for joint DOFs
        for (idx, &(ji, di)) in dof_map.iter().enumerate() {
            self.joints[ji].velocities[di] += deriv.d_velocities[idx] * dt;
            self.joints[ji].angles[di] += self.joints[ji].velocities[di] * dt;
        }

        self.integrate_root(dt);
        self.time += dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::joint::Joint;
    use std::f64::consts::PI;

    fn make_two_body_world() -> World {
        let mut world = World::new();
        let parent = world.add_body(DVec3::new(0.5, 0.5, 0.5));
        let child = world.add_body(DVec3::new(0.5, 0.25, 0.25));
        world.root = parent;
        world.set_root_transform(DAffine3::IDENTITY);
        // Make root heavy to act as fixed base for Featherstone
        world.bodies[parent].mass = 1e6;
        world.bodies[parent].inertia_diag = DVec3::splat(1e6);

        let joint = Joint::revolute(
            parent,
            child,
            DVec3::new(0.5, 0.0, 0.0),  // parent +X face
            DVec3::new(-0.5, 0.0, 0.0), // child -X face
            DVec3::Z,
        );
        world.add_joint(joint);
        world
    }

    #[test]
    fn fk_zero_angle_child_adjacent() {
        let mut world = make_two_body_world();
        world.forward_kinematics();
        let child_pos = world.transforms[1].translation;
        assert!((child_pos.x - 1.0).abs() < 1e-10, "child x: {}", child_pos.x);
        assert!(child_pos.y.abs() < 1e-10, "child y: {}", child_pos.y);
        assert!(child_pos.z.abs() < 1e-10, "child z: {}", child_pos.z);
    }

    #[test]
    fn fk_90_degree_rotates_child() {
        let mut world = make_two_body_world();
        world.joints[0].angles[0] = PI / 2.0;
        world.forward_kinematics();
        let child_pos = world.transforms[1].translation;
        assert!((child_pos.x - 0.5).abs() < 1e-6, "child x: {}", child_pos.x);
        assert!((child_pos.y - 0.5).abs() < 1e-6, "child y: {}", child_pos.y);
    }

    #[test]
    fn step_with_torque_changes_angle() {
        let mut world = make_two_body_world();
        world.torques[0][0] = 1.0;
        let dt = 1.0 / 60.0;
        for _ in 0..100 {
            world.step(dt);
        }
        assert!(world.joints[0].angles[0] > 0.0);
    }

    #[test]
    fn joint_limits_prevent_excessive_rotation() {
        let mut world = make_two_body_world();
        world.joints[0].angle_max[0] = 1.0;
        world.torques[0][0] = 10.0;
        let dt = 1.0 / 60.0;
        for _ in 0..1000 {
            world.step(dt);
        }
        assert!(world.joints[0].angles[0] < 3.0, "angle: {}", world.joints[0].angles[0]);
    }

    /// A two-body articulated creature at y=2 with gravity should fall.
    /// This is the minimal reproducer for the floating bug.
    #[test]
    fn two_body_falls_under_gravity() {
        let mut world = World::new();
        let root = world.add_body(DVec3::new(0.5, 0.5, 0.5));
        let child = world.add_body(DVec3::new(0.3, 0.2, 0.2));
        world.root = root;
        world.gravity = DVec3::new(0.0, -9.81, 0.0);
        world.ground_enabled = true;
        world.collisions_enabled = true;

        let joint = Joint::revolute(
            root, child,
            DVec3::new(0.5, 0.0, 0.0),
            DVec3::new(-0.3, 0.0, 0.0),
            DVec3::Z,
        );
        world.add_joint(joint);
        world.set_root_transform(DAffine3::from_translation(DVec3::new(0.0, 2.0, 0.0)));
        world.forward_kinematics();

        let dt = 1.0 / 60.0;
        let mut positions = Vec::new();
        for step in 0..120 {
            world.step(dt);
            if step % 10 == 0 {
                positions.push(world.transforms[0].translation.y);
            }
        }

        let final_y = world.transforms[0].translation.y;
        assert!(
            final_y < 1.5,
            "Two-body creature should fall from y=2, got final y={final_y:.4}. \
             Trajectory (every 10 frames): {positions:?}"
        );
    }

    /// A single body at y=2 with gravity should fall downward.
    /// This tests the root body integration in the RK45 step.
    #[test]
    fn free_fall_single_body() {
        let mut world = World::new();
        world.add_body(DVec3::new(0.5, 0.5, 0.5));
        world.root = 0;
        world.gravity = DVec3::new(0.0, -9.81, 0.0);
        world.set_root_transform(DAffine3::from_translation(DVec3::new(0.0, 2.0, 0.0)));

        let dt = 1.0 / 60.0;
        for _ in 0..60 {
            world.step(dt);
        }

        let root_y = world.transforms[0].translation.y;
        // After 1s of free fall: y = 2 - 0.5*9.81*1^2 = -2.905
        // Should be well below starting height
        assert!(
            root_y < 0.0,
            "After 1s free fall from y=2, root should be below y=0, got y={root_y:.4}"
        );
    }

    /// A body falling under gravity should come to rest on the ground plane (y=0).
    #[test]
    fn ground_contact_stops_fall() {
        let mut world = World::new();
        world.add_body(DVec3::new(0.5, 0.5, 0.5));
        world.root = 0;
        world.gravity = DVec3::new(0.0, -9.81, 0.0);
        world.ground_enabled = true;
        world.collisions_enabled = true;
        world.set_root_transform(DAffine3::from_translation(DVec3::new(0.0, 2.0, 0.0)));

        let dt = 1.0 / 60.0;
        // Run for 5 seconds — plenty of time to fall and settle
        for _ in 0..300 {
            world.step(dt);
        }

        let root_y = world.transforms[0].translation.y;
        // Body half-extents.y = 0.5, so center should rest near y=0.5
        // Allow some tolerance for penalty spring equilibrium
        assert!(
            root_y < 1.5,
            "Body should have fallen from y=2, got y={root_y:.4}"
        );
        assert!(
            root_y > -0.5,
            "Body should not have fallen through ground, got y={root_y:.4}"
        );
    }

    /// Internal joint torques should not create net linear force on a floating base.
    /// This is a fundamental conservation-of-momentum property.
    #[test]
    fn internal_torques_no_net_root_translation() {
        let mut world = World::new();
        let root = world.add_body(DVec3::new(0.5, 0.5, 0.5));
        let child = world.add_body(DVec3::new(0.3, 0.2, 0.2));
        world.root = root;
        world.set_root_transform(DAffine3::IDENTITY);
        // No gravity, no water, no ground — only internal torques
        world.gravity = DVec3::ZERO;

        let joint = Joint::revolute(
            root, child,
            DVec3::new(0.5, 0.0, 0.0),
            DVec3::new(-0.3, 0.0, 0.0),
            DVec3::Z,
        );
        world.add_joint(joint);
        world.torques[0][0] = 5.0; // Apply torque on the joint

        let dt = 1.0 / 60.0;
        for _ in 0..120 {
            world.step(dt);
        }

        let root_pos = world.transforms[0].translation;
        let displacement = root_pos.length();
        // Internal torques should not move the center of mass
        // Allow small numerical drift but not large displacement
        assert!(
            displacement < 0.1,
            "Internal torques moved root by {displacement:.4} — should be near zero"
        );
    }

    /// Simulate a full Creature under land conditions.
    /// The creature should fall under gravity, not float upward.
    #[test]
    fn creature_falls_under_gravity_with_brain() {
        use crate::creature::Creature;
        use crate::genotype::GenomeGraph;
        use rand::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        // Use seed=4 which barely floats (y=2.5) — easiest to diagnose
        let mut rng = ChaCha8Rng::seed_from_u64(4);
        let genome = GenomeGraph::random(&mut rng);
        let mut creature = Creature::from_genome(genome);

        creature.world.water_enabled = false;
        creature.world.gravity = DVec3::new(0.0, -9.81, 0.0);
        creature.world.collisions_enabled = true;
        creature.world.ground_enabled = true;
        creature.world.set_root_transform(
            DAffine3::from_translation(DVec3::new(0.0, 2.0, 0.0)),
        );
        creature.world.forward_kinematics();

        eprintln!("Bodies: {}, Joints: {}", creature.world.bodies.len(), creature.world.joints.len());

        let dt = 1.0 / 60.0;
        for step in 0..120 {
            creature.world.step(dt);
            let ry = creature.world.transforms[creature.world.root].translation.y;
            let rv = creature.world.root_velocity.y;
            if step < 20 || step % 20 == 0 {
                eprintln!(
                    "  step {:3}: root_y={:8.4} root_vy={:8.4}",
                    step, ry, rv
                );
            }
        }

        let root_y = creature.world.transforms[creature.world.root].translation.y;
        assert!(
            root_y < 2.0,
            "Creature should fall from y=2, got y={root_y:.4}"
        );
    }

    #[test]
    fn damping_reduces_velocity() {
        let mut world = make_two_body_world();
        world.joints[0].velocities[0] = 5.0;
        let dt = 1.0 / 60.0;
        for _ in 0..300 {
            world.step(dt);
        }
        assert!(world.joints[0].velocities[0].abs() < 1.0, "velocity: {}", world.joints[0].velocities[0]);
    }

    /// Rapier backend: two-body creature at y=2 should fall under gravity.
    #[cfg(feature = "rapier-physics")]
    #[test]
    fn rapier_creature_falls_under_gravity() {
        let mut world = make_two_body_world();
        world.root = 0;
        world.gravity = DVec3::new(0.0, -9.81, 0.0);
        world.ground_enabled = true;
        world.use_rapier = true;
        world.set_root_transform(DAffine3::from_translation(DVec3::new(0.0, 2.0, 0.0)));
        world.forward_kinematics();

        let dt = 1.0 / 60.0;
        for _ in 0..120 {
            world.step(dt);
        }

        let root_y = world.transforms[0].translation.y;
        assert!(root_y < 1.8, "Rapier: body should have fallen from y=2, got y={root_y:.3}");
        assert!(root_y > -0.5, "Rapier: body should not pass through ground, got y={root_y:.3}");
    }

    /// Rapier backend: water creature stays near surface (no gravity, has drag).
    #[cfg(feature = "rapier-physics")]
    #[test]
    fn rapier_water_creature_stable() {
        let mut world = World::new();
        let root = world.add_body(DVec3::new(0.5, 0.3, 0.3));
        let child = world.add_body(DVec3::new(0.3, 0.2, 0.2));
        world.root = root;
        world.gravity = DVec3::ZERO;
        world.water_enabled = true;
        world.water_viscosity = 2.0;
        world.use_rapier = true;

        let joint = Joint::revolute(root, child,
            DVec3::new(0.5, 0.0, 0.0), DVec3::new(-0.3, 0.0, 0.0), DVec3::Z);
        world.add_joint(joint);
        world.torques[0][0] = 1.0;
        world.set_root_transform(DAffine3::IDENTITY);
        world.forward_kinematics();

        let dt = 1.0 / 60.0;
        for _ in 0..120 {
            world.step(dt);
        }

        let root_pos = world.transforms[0].translation;
        assert!(root_pos.length() < 10.0, "Rapier water: creature exploded to {root_pos:?}");
        assert!(root_pos.length().is_finite(), "Rapier water: NaN/Inf position");
    }
}
