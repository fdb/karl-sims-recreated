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

#[derive(Debug, Clone)]
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
    suggested_dt: f64,
    pub root_velocity: DVec3,
    pub root_angular_velocity: DVec3,
    last_root_accel: SVec6,
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
            suggested_dt: 1.0 / 120.0,
            root_velocity: DVec3::ZERO,
            root_angular_velocity: DVec3::ZERO,
            last_root_accel: SVec6::ZERO,
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
        let floating = self.water_enabled;
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

        // Compute collision forces
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

        for _attempt in 0..8 {
            let k1 = self.evaluate(base, dof_map);

            let s2 = trial_state(base, &[k1.clone()], &[B21], dt);
            let k2 = self.evaluate(&s2, dof_map);

            let s3 = trial_state(base, &[k1.clone(), k2.clone()], &[B31, B32], dt);
            let k3 = self.evaluate(&s3, dof_map);

            let s4 = trial_state(
                base,
                &[k1.clone(), k2.clone(), k3.clone()],
                &[B41, B42, B43],
                dt,
            );
            let k4 = self.evaluate(&s4, dof_map);

            let s5 = trial_state(
                base,
                &[k1.clone(), k2.clone(), k3.clone(), k4.clone()],
                &[B51, B52, B53, B54],
                dt,
            );
            let k5 = self.evaluate(&s5, dof_map);

            let s6 = trial_state(
                base,
                &[k1.clone(), k2.clone(), k3.clone(), k4.clone(), k5.clone()],
                &[B61, B62, B63, B64, B65],
                dt,
            );
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
                return (y4, dt, dt);
            }

            let dt_optimal = config.safety_factor * dt * (config.tolerance / error).powf(0.2);
            let dt_optimal = dt_optimal.clamp(config.min_dt, config.max_dt);

            if error <= config.tolerance {
                return (y4, dt, dt_optimal);
            }

            dt = dt_optimal;
        }

        // Fallback: Euler step with minimum dt
        let dt_min = config.min_dt;
        let deriv = self.evaluate(base, dof_map);
        let fallback = base.advance(&deriv, dt_min);
        (fallback, dt_min, dt_min)
    }

    pub fn step(&mut self, frame_dt: f64) {
        let dof_map = self.dof_map();
        if dof_map.is_empty() {
            self.time += frame_dt;
            self.forward_kinematics();
            return;
        }

        let config = IntegratorConfig::default();
        let mut remaining = frame_dt;
        let mut dt = self.suggested_dt.min(remaining);

        while remaining > 1e-10 {
            let step_dt = dt.min(remaining);
            let base_state = self.get_state(&dof_map);

            let (new_state, dt_used, dt_next) =
                self.rk45_adaptive_step(&base_state, step_dt, &dof_map, &config);

            self.set_state(&new_state, &dof_map);
            remaining -= dt_used;
            dt = dt_next;
            self.suggested_dt = dt;
        }

        // Integrate root body velocity and position for floating base (swimming)
        if self.water_enabled {
            let root_accel = self.last_root_accel;
            // Subtract gravity spatial from root_accel to get actual acceleration
            // (root_accel includes the gravity trick, so actual = root_accel - gravity_spatial)
            let gravity_spatial = SVec6::new(DVec3::ZERO, -self.gravity);
            let actual_accel = root_accel - gravity_spatial;

            self.root_velocity += actual_accel.linear() * frame_dt;
            self.root_angular_velocity += actual_accel.angular() * frame_dt;

            let displacement = self.root_velocity * frame_dt;
            self.transforms[self.root].translation += displacement;

            // Apply angular velocity to root rotation
            let ang = self.root_angular_velocity;
            let ang_mag = ang.length();
            if ang_mag > 1e-10 {
                let drot = DQuat::from_axis_angle(ang / ang_mag, ang_mag * frame_dt);
                let current = DQuat::from_mat3(&self.transforms[self.root].matrix3);
                self.transforms[self.root].matrix3 = DMat3::from_quat(drot * current);
            }
        }

        self.forward_kinematics();
        self.time += frame_dt;
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
}
