use glam::{DAffine3, DVec3};
use crate::body::RigidBody;
use crate::spatial::SVec6;

pub const DEFAULT_VISCOSITY: f64 = 2.0;

/// Compute viscous water drag forces on all bodies.
/// Returns one SVec6 per body (spatial force in body-local frame).
///
/// For each face: compute velocity at face center, project onto face normal,
/// apply drag force = -viscosity * area * v_normal * normal.
/// The force opposes the normal component of velocity.
pub fn compute_water_drag(
    bodies: &[RigidBody],
    transforms: &[DAffine3],
    body_velocities: &[SVec6],
    viscosity: f64,
) -> Vec<SVec6> {
    bodies
        .iter()
        .enumerate()
        .map(|(i, body)| {
            let xform = &transforms[i];
            let vel = &body_velocities[i];
            let rot = xform.matrix3;
            let rot_t = rot.transpose();

            let omega = vel.angular();
            let v_center = vel.linear();

            // Transform to world frame
            let omega_world = rot * omega;
            let v_center_world = rot * v_center;

            let mut total_force_world = DVec3::ZERO;
            let mut total_torque_world = DVec3::ZERO;

            for face in body.faces() {
                let face_center_world = rot * face.center;
                let normal_world = rot * face.normal;

                // Velocity at face center in world frame
                let v_face = v_center_world + omega_world.cross(face_center_world);

                // Normal component of velocity
                let v_normal = v_face.dot(normal_world);

                // Drag force in world frame
                let drag_force = -viscosity * face.area * v_normal * normal_world;

                total_force_world += drag_force;
                total_torque_world += face_center_world.cross(drag_force);
            }

            // Convert back to body-local frame
            let local_torque = rot_t * total_torque_world;
            let local_force = rot_t * total_force_world;

            SVec6::new(local_torque, local_force)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    const EPS: f64 = 1e-10;

    fn identity_transform() -> DAffine3 {
        DAffine3::IDENTITY
    }

    #[test]
    fn stationary_body_no_drag() {
        let body = RigidBody::new(DVec3::new(0.5, 0.5, 0.5));
        let result = compute_water_drag(
            &[body],
            &[identity_transform()],
            &[SVec6::ZERO],
            DEFAULT_VISCOSITY,
        );
        assert_eq!(result.len(), 1);
        for j in 0..6 {
            assert!(
                result[0].0[j].abs() < EPS,
                "component {j} should be zero, got {}",
                result[0].0[j]
            );
        }
    }

    #[test]
    fn moving_body_experiences_drag() {
        let body = RigidBody::new(DVec3::new(0.5, 0.5, 0.5));
        // Moving in +X direction (pure linear velocity, no rotation)
        let vel = SVec6::new(DVec3::ZERO, DVec3::new(1.0, 0.0, 0.0));
        let result = compute_water_drag(
            &[body],
            &[identity_transform()],
            &[vel],
            DEFAULT_VISCOSITY,
        );
        // Drag force should be in -X direction
        let fx = result[0].linear().x;
        let fy = result[0].linear().y;
        let fz = result[0].linear().z;
        assert!(fx < -EPS, "drag force X should be negative, got {fx}");
        assert!(fy.abs() < EPS, "drag force Y should be zero, got {fy}");
        assert!(fz.abs() < EPS, "drag force Z should be zero, got {fz}");
        // Torque should be zero (symmetric body, linear motion)
        for j in 0..3 {
            assert!(
                result[0].angular()[j].abs() < EPS,
                "torque component {j} should be zero, got {}",
                result[0].angular()[j]
            );
        }
    }

    #[test]
    fn drag_proportional_to_velocity() {
        let body = RigidBody::new(DVec3::new(0.5, 0.5, 0.5));
        let vel1 = SVec6::new(DVec3::ZERO, DVec3::new(1.0, 0.0, 0.0));
        let vel2 = SVec6::new(DVec3::ZERO, DVec3::new(2.0, 0.0, 0.0));
        let result1 = compute_water_drag(
            &[body.clone()],
            &[identity_transform()],
            &[vel1],
            DEFAULT_VISCOSITY,
        );
        let result2 = compute_water_drag(
            &[body],
            &[identity_transform()],
            &[vel2],
            DEFAULT_VISCOSITY,
        );
        let fx1 = result1[0].linear().x;
        let fx2 = result2[0].linear().x;
        assert!(
            (fx2 - 2.0 * fx1).abs() < EPS,
            "2x velocity should produce 2x drag: fx1={fx1}, fx2={fx2}"
        );
    }

    #[test]
    fn flat_body_more_drag_in_broad_direction() {
        // Flat pancake: wide in X and Z, thin in Y
        let body = RigidBody::new(DVec3::new(2.0, 0.1, 2.0));

        // Moving in +Y (against the broad XZ faces)
        let vel_y = SVec6::new(DVec3::ZERO, DVec3::new(0.0, 1.0, 0.0));
        let drag_y = compute_water_drag(
            &[body.clone()],
            &[identity_transform()],
            &[vel_y],
            DEFAULT_VISCOSITY,
        );

        // Moving in +X (against the narrow YZ faces)
        let vel_x = SVec6::new(DVec3::ZERO, DVec3::new(1.0, 0.0, 0.0));
        let drag_x = compute_water_drag(
            &[body],
            &[identity_transform()],
            &[vel_x],
            DEFAULT_VISCOSITY,
        );

        let drag_y_mag = drag_y[0].linear().y.abs();
        let drag_x_mag = drag_x[0].linear().x.abs();
        assert!(
            drag_y_mag > drag_x_mag,
            "drag in Y (broad direction) should exceed drag in X (narrow direction): \
             drag_y={drag_y_mag}, drag_x={drag_x_mag}"
        );
    }
}
