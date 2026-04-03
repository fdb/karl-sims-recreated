use glam::{DAffine3, DVec3};

use crate::body::RigidBody;
use crate::joint::Joint;
use crate::spatial::SVec6;

pub struct Contact {
    pub body_a: usize,
    pub body_b: usize,
    pub normal: DVec3,  // from A toward B (world space)
    pub depth: f64,     // penetration depth (positive = overlapping)
    pub point: DVec3,   // contact point (world space)
}

pub const COLLISION_STIFFNESS: f64 = 500.0;
pub const COLLISION_DAMPING: f64 = 5.0;

/// Compute world-space AABB min/max for an oriented box.
pub fn compute_aabb(half_extents: DVec3, transform: &DAffine3) -> (DVec3, DVec3) {
    let rot = transform.matrix3;
    let center = transform.translation;
    // For each world axis, AABB half-size = sum of |rot_row[j]| * half_extents[j]
    // Since glam DMat3 is column-major, row i component j = rot.col(j)[i]
    let mut aabb_half = DVec3::ZERO;
    for axis in 0..3 {
        let mut extent = 0.0;
        for j in 0..3 {
            extent += rot.col(j)[axis].abs() * half_extents[j];
        }
        aabb_half[axis] = extent;
    }
    (center - aabb_half, center + aabb_half)
}

/// Simple 3-axis overlap test.
pub fn aabb_overlap(min_a: DVec3, max_a: DVec3, min_b: DVec3, max_b: DVec3) -> bool {
    for i in 0..3 {
        if max_a[i] < min_b[i] || max_b[i] < min_a[i] {
            return false;
        }
    }
    true
}

/// Separating Axis Test for two oriented boxes.
/// Returns a Contact if the boxes overlap, None if separated.
pub fn obb_sat(
    he_a: DVec3,
    tf_a: &DAffine3,
    he_b: DVec3,
    tf_b: &DAffine3,
    body_a: usize,
    body_b: usize,
) -> Option<Contact> {
    let rot_a = tf_a.matrix3;
    let rot_b = tf_b.matrix3;
    let center_a = tf_a.translation;
    let center_b = tf_b.translation;
    let d = center_b - center_a; // from A to B

    let mut min_overlap = f64::MAX;
    let mut min_axis = DVec3::ZERO;

    let axes_a = [rot_a.col(0), rot_a.col(1), rot_a.col(2)];
    let axes_b = [rot_b.col(0), rot_b.col(1), rot_b.col(2)];
    let he_a_arr = [he_a.x, he_a.y, he_a.z];
    let he_b_arr = [he_b.x, he_b.y, he_b.z];

    let mut test_axis = |axis: DVec3| -> bool {
        let len = axis.length();
        if len < 1e-10 {
            return true; // degenerate axis, skip
        }
        let axis = axis / len;

        // Project half-extents of A onto axis
        let proj_a: f64 = (0..3).map(|j| (axes_a[j].dot(axis)).abs() * he_a_arr[j]).sum();
        // Project half-extents of B onto axis
        let proj_b: f64 = (0..3).map(|j| (axes_b[j].dot(axis)).abs() * he_b_arr[j]).sum();
        // Project center distance onto axis
        let dist = d.dot(axis).abs();

        let overlap = proj_a + proj_b - dist;
        if overlap < 0.0 {
            return false; // separating axis found
        }
        if overlap < min_overlap {
            min_overlap = overlap;
            min_axis = axis;
        }
        true
    };

    // 3 face normals of A
    for i in 0..3 {
        if !test_axis(axes_a[i]) {
            return None;
        }
    }
    // 3 face normals of B
    for i in 0..3 {
        if !test_axis(axes_b[i]) {
            return None;
        }
    }
    // 9 edge-edge cross products
    for i in 0..3 {
        for j in 0..3 {
            if !test_axis(axes_a[i].cross(axes_b[j])) {
                return None;
            }
        }
    }

    // Ensure normal points from A toward B
    if d.dot(min_axis) < 0.0 {
        min_axis = -min_axis;
    }

    // Contact point: midpoint between the two centers projected along normal
    let point = center_a + d * 0.5;

    Some(Contact {
        body_a,
        body_b,
        normal: min_axis,
        depth: min_overlap,
        point,
    })
}

/// All-pairs collision detection with AABB broad phase and OBB-SAT narrow phase.
/// Skips pairs connected by joints (parent-child).
pub fn detect_collisions(
    bodies: &[RigidBody],
    transforms: &[DAffine3],
    joints: &[Joint],
) -> Vec<Contact> {
    let n = bodies.len();
    let mut contacts = Vec::new();

    // Precompute AABBs
    let aabbs: Vec<(DVec3, DVec3)> = (0..n)
        .map(|i| compute_aabb(bodies[i].half_extents, &transforms[i]))
        .collect();

    // Build set of connected pairs for fast lookup
    let mut connected = std::collections::HashSet::new();
    for j in joints {
        let (a, b) = if j.parent_idx < j.child_idx {
            (j.parent_idx, j.child_idx)
        } else {
            (j.child_idx, j.parent_idx)
        };
        connected.insert((a, b));
    }

    for i in 0..n {
        for j in (i + 1)..n {
            // Skip connected pairs
            if connected.contains(&(i, j)) {
                continue;
            }

            // Broad phase
            if !aabb_overlap(aabbs[i].0, aabbs[i].1, aabbs[j].0, aabbs[j].1) {
                continue;
            }

            // Narrow phase
            if let Some(contact) = obb_sat(
                bodies[i].half_extents,
                &transforms[i],
                bodies[j].half_extents,
                &transforms[j],
                i,
                j,
            ) {
                contacts.push(contact);
            }
        }
    }

    contacts
}

/// Penalty spring collision response forces.
pub fn compute_collision_forces(
    contacts: &[Contact],
    transforms: &[DAffine3],
    body_velocities: &[SVec6],
    num_bodies: usize,
    stiffness: f64,
    damping: f64,
) -> Vec<SVec6> {
    let mut forces = vec![SVec6::ZERO; num_bodies];

    for contact in contacts {
        let a = contact.body_a;
        let b = contact.body_b;
        let n = contact.normal;

        // Relative velocity at contact point (B - A)
        let vel_a = body_velocities[a].linear();
        let vel_b = body_velocities[b].linear();
        let rel_vel = vel_b - vel_a;
        let rel_vel_normal = rel_vel.dot(n);

        // Penalty force magnitude: spring + damping
        let force_mag = stiffness * contact.depth - damping * rel_vel_normal;
        let force_mag = force_mag.max(0.0); // only push apart

        let force_world = n * force_mag;

        // Convert to body-local spatial forces
        // For body A: force is -force_world
        // For body B: force is +force_world
        let center_a = transforms[a].translation;
        let center_b = transforms[b].translation;
        let rot_a = transforms[a].matrix3;
        let rot_b = transforms[b].matrix3;

        // r vectors from body center to contact point (world space)
        let r_a = contact.point - center_a;
        let r_b = contact.point - center_b;

        // Force on A (in world space): -force_world
        let f_a_world = -force_world;
        let torque_a_world = r_a.cross(f_a_world);
        // Transform to body-local frame
        let rot_a_inv = rot_a.transpose();
        let f_a_local = rot_a_inv * f_a_world;
        let torque_a_local = rot_a_inv * torque_a_world;
        forces[a] = forces[a] + SVec6::new(torque_a_local, f_a_local);

        // Force on B (in world space): +force_world
        let f_b_world = force_world;
        let torque_b_world = r_b.cross(f_b_world);
        let rot_b_inv = rot_b.transpose();
        let f_b_local = rot_b_inv * f_b_world;
        let torque_b_local = rot_b_inv * torque_b_world;
        forces[b] = forces[b] + SVec6::new(torque_b_local, f_b_local);
    }

    forces
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DMat3;

    fn identity_transform_at(pos: DVec3) -> DAffine3 {
        DAffine3 {
            matrix3: DMat3::IDENTITY,
            translation: pos,
        }
    }

    #[test]
    fn no_collision_when_apart() {
        let bodies = vec![
            RigidBody::new(DVec3::splat(0.5)),
            RigidBody::new(DVec3::splat(0.5)),
        ];
        let transforms = vec![
            identity_transform_at(DVec3::new(0.0, 0.0, 0.0)),
            identity_transform_at(DVec3::new(3.0, 0.0, 0.0)),
        ];
        let joints: Vec<Joint> = vec![];
        let contacts = detect_collisions(&bodies, &transforms, &joints);
        assert!(contacts.is_empty(), "expected no contacts for separated boxes");
    }

    #[test]
    fn collision_when_overlapping() {
        let bodies = vec![
            RigidBody::new(DVec3::splat(0.5)),
            RigidBody::new(DVec3::splat(0.5)),
        ];
        // Boxes overlap by 0.2 along X
        let transforms = vec![
            identity_transform_at(DVec3::new(0.0, 0.0, 0.0)),
            identity_transform_at(DVec3::new(0.8, 0.0, 0.0)),
        ];
        let joints: Vec<Joint> = vec![];
        let contacts = detect_collisions(&bodies, &transforms, &joints);
        assert_eq!(contacts.len(), 1);
        assert!(contacts[0].depth > 0.0, "depth should be positive");
        assert!(
            contacts[0].depth.abs() - 0.2 < 0.01,
            "depth should be ~0.2, got {}",
            contacts[0].depth
        );
        // Normal should be roughly in X direction (from A toward B)
        assert!(
            contacts[0].normal.dot(DVec3::X) > 0.9,
            "normal should point along +X, got {:?}",
            contacts[0].normal
        );
    }

    #[test]
    fn connected_bodies_skip_collision() {
        let bodies = vec![
            RigidBody::new(DVec3::splat(0.5)),
            RigidBody::new(DVec3::splat(0.5)),
        ];
        // Overlapping boxes
        let transforms = vec![
            identity_transform_at(DVec3::new(0.0, 0.0, 0.0)),
            identity_transform_at(DVec3::new(0.8, 0.0, 0.0)),
        ];
        let joints = vec![Joint::revolute(
            0,
            1,
            DVec3::new(0.5, 0.0, 0.0),
            DVec3::new(-0.5, 0.0, 0.0),
            DVec3::Y,
        )];
        let contacts = detect_collisions(&bodies, &transforms, &joints);
        assert!(
            contacts.is_empty(),
            "connected bodies should not collide"
        );
    }

    #[test]
    fn rotated_boxes_no_crash() {
        let bodies = vec![
            RigidBody::new(DVec3::splat(0.5)),
            RigidBody::new(DVec3::splat(0.5)),
        ];
        let rot45 = DMat3::from_rotation_z(std::f64::consts::FRAC_PI_4);
        let transforms = vec![
            identity_transform_at(DVec3::ZERO),
            DAffine3 {
                matrix3: rot45,
                translation: DVec3::new(1.0, 0.0, 0.0),
            },
        ];
        let joints: Vec<Joint> = vec![];
        // Just verify it doesn't panic
        let _contacts = detect_collisions(&bodies, &transforms, &joints);
    }

    #[test]
    fn penalty_forces_push_apart() {
        let contact = Contact {
            body_a: 0,
            body_b: 1,
            normal: DVec3::X,
            depth: 0.2,
            point: DVec3::new(0.5, 0.0, 0.0),
        };
        let transforms = vec![
            identity_transform_at(DVec3::ZERO),
            identity_transform_at(DVec3::new(1.0, 0.0, 0.0)),
        ];
        let velocities = vec![SVec6::ZERO, SVec6::ZERO];
        let forces =
            compute_collision_forces(&[contact], &transforms, &velocities, 2, 500.0, 5.0);

        // Force on A should be in -X (pushed away from B)
        assert!(
            forces[0].linear().x < 0.0,
            "force on A should be in -X, got {:?}",
            forces[0].linear()
        );
        // Force on B should be in +X (pushed away from A)
        assert!(
            forces[1].linear().x > 0.0,
            "force on B should be in +X, got {:?}",
            forces[1].linear()
        );
        // Forces should be equal and opposite (linear part)
        let sum = forces[0].linear() + forces[1].linear();
        assert!(
            sum.length() < 1e-10,
            "forces should be equal and opposite, sum = {:?}",
            sum
        );
    }
}
