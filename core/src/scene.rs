use glam::{DAffine3, DVec3};

use crate::joint::Joint;
use crate::world::World;

/// Single box at (0, 1, 0).
pub fn single_box() -> World {
    let mut world = World::new();
    let body = world.add_body(DVec3::new(0.6, 0.4, 0.5));
    world.root = body;
    world.set_root_transform(DAffine3::from_translation(DVec3::new(0.0, 1.0, 0.0)));
    world
}

/// Parent box + child box connected by a revolute joint.
pub fn hinged_pair() -> World {
    let mut world = World::new();
    let parent = world.add_body(DVec3::new(0.5, 0.5, 0.5));
    let child = world.add_body(DVec3::new(0.6, 0.2, 0.3));
    world.root = parent;
    world.set_root_transform(DAffine3::from_translation(DVec3::new(0.0, 2.0, 0.0)));

    let joint = Joint::revolute(
        parent,
        child,
        DVec3::new(0.5, 0.0, 0.0),  // parent +X face
        DVec3::new(-0.6, 0.0, 0.0), // child -X face
        DVec3::Z,
    );
    world.add_joint(joint);
    world.forward_kinematics();
    world
}

/// Central body with 4 flippers attached to ±X and ±Z faces.
pub fn starfish() -> World {
    let mut world = World::new();
    let center = world.add_body(DVec3::new(0.5, 0.3, 0.5));
    world.root = center;
    world.set_root_transform(DAffine3::from_translation(DVec3::new(0.0, 1.5, 0.0)));

    let flipper_half = DVec3::new(0.5, 0.08, 0.25);

    // +X flipper
    let f0 = world.add_body(flipper_half);
    let mut j0 = Joint::revolute(
        center, f0,
        DVec3::new(0.5, 0.0, 0.0),
        DVec3::new(-0.5, 0.0, 0.0),
        DVec3::Z,
    );
    j0.angle_min = [-0.8; 3];
    j0.angle_max = [0.8; 3];
    j0.damping = 0.3;
    world.add_joint(j0);

    // -X flipper
    let f1 = world.add_body(flipper_half);
    let mut j1 = Joint::revolute(
        center, f1,
        DVec3::new(-0.5, 0.0, 0.0),
        DVec3::new(0.5, 0.0, 0.0),
        DVec3::Z,
    );
    j1.angle_min = [-0.8; 3];
    j1.angle_max = [0.8; 3];
    j1.damping = 0.3;
    world.add_joint(j1);

    // +Z flipper
    let f2 = world.add_body(flipper_half);
    let mut j2 = Joint::revolute(
        center, f2,
        DVec3::new(0.0, 0.0, 0.5),
        DVec3::new(0.0, 0.0, -0.25),
        DVec3::X,
    );
    j2.angle_min = [-0.8; 3];
    j2.angle_max = [0.8; 3];
    j2.damping = 0.3;
    world.add_joint(j2);

    // -Z flipper
    let f3 = world.add_body(flipper_half);
    let mut j3 = Joint::revolute(
        center, f3,
        DVec3::new(0.0, 0.0, -0.5),
        DVec3::new(0.0, 0.0, 0.25),
        DVec3::X,
    );
    j3.angle_min = [-0.8; 3];
    j3.angle_max = [0.8; 3];
    j3.damping = 0.3;
    world.add_joint(j3);

    world.forward_kinematics();
    world
}

/// Apply sinusoidal torques to starfish flippers.
pub fn starfish_torques(world: &mut World) {
    use std::f64::consts::PI;
    let t = world.time;
    let amplitude = 2.0;
    let frequency = 3.0;
    let phases = [0.0, PI, PI / 2.0, 3.0 * PI / 2.0];
    for (i, &phase) in phases.iter().enumerate() {
        if i < world.torques.len() {
            world.torques[i][0] = amplitude * (frequency * t + phase).sin();
        }
    }
}

/// Apply sinusoidal torque to hinged pair joint.
pub fn hinged_pair_torque(world: &mut World) {
    let t = world.time;
    world.torques[0][0] = 3.0 * (2.0 * t).sin();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_box_scene() {
        let world = single_box();
        assert_eq!(world.bodies.len(), 1);
        assert_eq!(world.joints.len(), 0);
    }

    #[test]
    fn hinged_pair_scene() {
        let world = hinged_pair();
        assert_eq!(world.bodies.len(), 2);
        assert_eq!(world.joints.len(), 1);
    }

    #[test]
    fn starfish_scene() {
        let world = starfish();
        assert_eq!(world.bodies.len(), 5);
        assert_eq!(world.joints.len(), 4);
    }

    #[test]
    fn starfish_paddling_motion() {
        let mut world = starfish();
        let dt = 1.0 / 60.0;
        for _ in 0..120 {
            starfish_torques(&mut world);
            world.step(dt);
        }
        for j in &world.joints {
            assert!(
                j.angles[0].abs() > 0.01,
                "flipper angle too small: {}",
                j.angles[0]
            );
        }
    }
}
