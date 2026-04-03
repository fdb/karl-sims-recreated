use glam::{DAffine3, DMat3, DQuat, DVec3};

use crate::body::RigidBody;
use crate::joint::Joint;

#[derive(Debug, Clone)]
pub struct World {
    pub bodies: Vec<RigidBody>,
    pub joints: Vec<Joint>,
    pub transforms: Vec<DAffine3>,
    pub torques: Vec<[f64; 3]>,
    pub root: usize,
    pub time: f64,
}

impl World {
    pub fn new() -> Self {
        Self {
            bodies: Vec::new(),
            joints: Vec::new(),
            transforms: Vec::new(),
            torques: Vec::new(),
            root: 0,
            time: 0.0,
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
            let axis = joint.axis;
            let angle = joint.angles[0];

            let parent_transform = self.transforms[parent_idx];

            // Joint rotation from angle
            let joint_rotation = DQuat::from_axis_angle(axis, angle);

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

    pub fn step(&mut self, dt: f64) {
        for i in 0..self.joints.len() {
            let dof = self.joints[i].joint_type.dof_count();
            if dof == 0 {
                continue;
            }

            let applied_torque = self.torques[i][0];
            let child_idx = self.joints[i].child_idx;
            let ax = self.joints[i].axis;
            let inertia = &self.bodies[child_idx].inertia_diag;
            let effective_inertia = (ax.x * ax.x * inertia.x
                + ax.y * ax.y * inertia.y
                + ax.z * ax.z * inertia.z)
                .max(0.001);

            let angle = self.joints[i].angles[0];
            let angle_min = self.joints[i].angle_min[0];
            let angle_max = self.joints[i].angle_max[0];
            let limit_stiffness = self.joints[i].limit_stiffness;
            let damping = self.joints[i].damping;
            let velocity = self.joints[i].velocities[0];

            // Limit spring torque
            let limit_torque = if angle < angle_min {
                limit_stiffness * (angle_min - angle)
            } else if angle > angle_max {
                limit_stiffness * (angle_max - angle)
            } else {
                0.0
            };

            let total_torque = applied_torque + limit_torque - damping * velocity;

            // Semi-implicit Euler
            let new_velocity = velocity + total_torque / effective_inertia * dt;
            let new_angle = angle + new_velocity * dt;

            self.joints[i].velocities[0] = new_velocity;
            self.joints[i].angles[0] = new_angle;
        }

        self.forward_kinematics();
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
        assert!(world.joints[0].angles[0] < 2.0, "angle: {}", world.joints[0].angles[0]);
    }

    #[test]
    fn damping_reduces_velocity() {
        let mut world = make_two_body_world();
        world.joints[0].velocities[0] = 5.0;
        let dt = 1.0 / 60.0;
        for _ in 0..300 {
            world.step(dt);
        }
        assert!(world.joints[0].velocities[0].abs() < 0.1, "velocity: {}", world.joints[0].velocities[0]);
    }
}
