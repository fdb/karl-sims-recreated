//! Physics world — owns bodies, joints, transforms, and the Rapier backend.
//!
//! Every `World::step()` delegates to [`RapierState`](crate::rapier_world::RapierState).
//! The state is lazily initialised on the first step from whatever poses
//! `forward_kinematics()` produced.

use glam::{DAffine3, DMat3, DQuat, DVec3};

use crate::body::RigidBody;
use crate::joint::Joint;
use crate::rapier_world::RapierState;

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
    pub ground_enabled: bool,
    pub light_position: DVec3,
    rapier_state: Option<RapierState>,
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
            ground_enabled: self.ground_enabled,
            light_position: self.light_position,
            // Rapier state cannot be cloned — it will be lazily rebuilt on
            // the first `step()` call of the cloned World.
            rapier_state: None,
        }
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
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
            water_viscosity: 2.0,
            ground_enabled: false,
            // Default light at the origin. For SwimmingSpeed goal this is
            // inert (photosensor reads the direction *toward* origin, which
            // has no fitness relevance). For LightFollowing, the fitness
            // evaluator overrides this per trial. The previous default of
            // (5,0,0) gave SwimmingSpeed creatures an accidental compass
            // pointing off-axis, which evolution could exploit to maintain
            // a consistent heading without any real "navigation" ability.
            light_position: DVec3::ZERO,
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

    /// Recompute child body transforms from root + joint angles.
    ///
    /// Must be called after mutating `set_root_transform` or joint angles but
    /// **before** the first `step()`, so Rapier initialises from consistent
    /// poses. `RapierState::build` snapshots these transforms as body poses.
    pub fn forward_kinematics(&mut self) {
        for i in 0..self.joints.len() {
            let joint = &self.joints[i];
            let parent_idx = joint.parent_idx;
            let child_idx = joint.child_idx;
            let parent_anchor = joint.parent_anchor;
            let child_anchor = joint.child_anchor;
            let parent_transform = self.transforms[parent_idx];

            let joint_rotation = joint.joint_rotation();
            let joint_pos = parent_transform.transform_point3(parent_anchor);
            let parent_rotation = DQuat::from_mat3(&parent_transform.matrix3);
            let world_rotation = parent_rotation * joint_rotation;
            let child_offset = world_rotation * (-child_anchor);

            self.transforms[child_idx] = DAffine3 {
                matrix3: DMat3::from_quat(world_rotation),
                translation: joint_pos + child_offset,
            };
        }
    }

    /// Step the simulation forward by `frame_dt` seconds.
    ///
    /// On the first call, builds Rapier state from current transforms. On
    /// subsequent calls, applies `self.torques` as joint drive torques, runs
    /// Rapier (sub-stepped internally for stiffness), and syncs body poses
    /// and joint angles back into `self.transforms` / `self.joints`.
    pub fn step(&mut self, frame_dt: f64) {
        if self.rapier_state.is_none() {
            self.rapier_state = Some(RapierState::build(
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
        let _child = world.add_body(DVec3::new(0.5, 0.25, 0.25));
        world.root = parent;
        world.set_root_transform(DAffine3::IDENTITY);

        let joint = Joint::revolute(
            parent,
            1,
            DVec3::new(0.5, 0.0, 0.0),
            DVec3::new(-0.5, 0.0, 0.0),
            DVec3::Z,
        );
        world.add_joint(joint);
        world.forward_kinematics();
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

    /// Body at y=2 with gravity should fall under Rapier.
    #[test]
    fn two_body_falls_under_gravity() {
        let mut world = World::new();
        let root = world.add_body(DVec3::new(0.5, 0.5, 0.5));
        let child = world.add_body(DVec3::new(0.3, 0.2, 0.2));
        world.root = root;
        world.gravity = DVec3::new(0.0, -9.81, 0.0);
        world.ground_enabled = true;

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
        for _ in 0..120 {
            world.step(dt);
        }
        let final_y = world.transforms[0].translation.y;
        assert!(final_y < 1.8, "Body should have fallen from y=2, got {final_y:.4}");
        assert!(final_y > -0.5, "Body should not pass through ground, got {final_y:.4}");
    }

    /// Single body at y=2 should fall (Rapier's internal free-fall integration).
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
        assert!(root_y < 0.0, "After 1s free-fall from y=2, expected y<0, got y={root_y:.4}");
    }

    /// Body falling under gravity should come to rest on the ground plane (y≈0.5).
    #[test]
    fn ground_contact_stops_fall() {
        let mut world = World::new();
        world.add_body(DVec3::new(0.5, 0.5, 0.5));
        world.root = 0;
        world.gravity = DVec3::new(0.0, -9.81, 0.0);
        world.ground_enabled = true;
        world.set_root_transform(DAffine3::from_translation(DVec3::new(0.0, 2.0, 0.0)));

        let dt = 1.0 / 60.0;
        for _ in 0..300 {
            world.step(dt);
        }
        let root_y = world.transforms[0].translation.y;
        assert!(root_y < 1.5, "Body should have fallen, got y={root_y:.4}");
        assert!(root_y > -0.5, "Body should not pass through ground, got y={root_y:.4}");
    }

    /// Water creature with internal torque stays bounded (no explosion).
    #[test]
    fn water_creature_stable() {
        let mut world = World::new();
        let root = world.add_body(DVec3::new(0.5, 0.3, 0.3));
        let child = world.add_body(DVec3::new(0.3, 0.2, 0.2));
        world.root = root;
        world.gravity = DVec3::ZERO;
        world.water_enabled = true;
        world.water_viscosity = 2.0;

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
        assert!(root_pos.length() < 10.0, "Water creature exploded to {root_pos:?}");
        assert!(root_pos.length().is_finite(), "Water creature produced NaN/Inf");
    }
}
