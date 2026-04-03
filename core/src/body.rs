use glam::DVec3;
use crate::spatial::SMat6;

#[derive(Debug, Clone, Copy)]
pub struct BoxFace {
    pub center: DVec3,  // face center in local body frame
    pub normal: DVec3,  // outward normal in local body frame
    pub area: f64,      // face area
}

#[derive(Debug, Clone)]
pub struct RigidBody {
    pub half_extents: DVec3,
    pub mass: f64,
    pub inertia_diag: DVec3, // diagonal of inertia tensor (Ixx, Iyy, Izz)
}

impl RigidBody {
    pub fn new(half_extents: DVec3) -> Self {
        let w = half_extents.x * 2.0;
        let h = half_extents.y * 2.0;
        let d = half_extents.z * 2.0;
        let mass = w * h * d; // density = 1.0
        let ixx = mass / 12.0 * (h * h + d * d);
        let iyy = mass / 12.0 * (w * w + d * d);
        let izz = mass / 12.0 * (w * w + h * h);
        Self {
            half_extents,
            mass,
            inertia_diag: DVec3::new(ixx, iyy, izz),
        }
    }

    pub fn faces(&self) -> [BoxFace; 6] {
        let h = self.half_extents;
        [
            BoxFace { center: DVec3::new( h.x, 0.0, 0.0), normal: DVec3::X,     area: 4.0 * h.y * h.z },
            BoxFace { center: DVec3::new(-h.x, 0.0, 0.0), normal: DVec3::NEG_X, area: 4.0 * h.y * h.z },
            BoxFace { center: DVec3::new(0.0,  h.y, 0.0), normal: DVec3::Y,     area: 4.0 * h.x * h.z },
            BoxFace { center: DVec3::new(0.0, -h.y, 0.0), normal: DVec3::NEG_Y, area: 4.0 * h.x * h.z },
            BoxFace { center: DVec3::new(0.0, 0.0,  h.z), normal: DVec3::Z,     area: 4.0 * h.x * h.y },
            BoxFace { center: DVec3::new(0.0, 0.0, -h.z), normal: DVec3::NEG_Z, area: 4.0 * h.x * h.y },
        ]
    }

    pub fn spatial_inertia(&self) -> SMat6 {
        SMat6::from_body_inertia(self.inertia_diag, self.mass)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_cube_mass_and_inertia() {
        let body = RigidBody::new(DVec3::new(0.5, 0.5, 0.5));
        assert!((body.mass - 1.0).abs() < 1e-10);
        let expected_inertia = 1.0 / 6.0;
        assert!((body.inertia_diag.x - expected_inertia).abs() < 1e-10);
        assert!((body.inertia_diag.y - expected_inertia).abs() < 1e-10);
        assert!((body.inertia_diag.z - expected_inertia).abs() < 1e-10);
    }

    #[test]
    fn unit_cube_symmetric_inertia() {
        let body = RigidBody::new(DVec3::new(0.5, 0.5, 0.5));
        assert!((body.inertia_diag.x - body.inertia_diag.y).abs() < 1e-10);
        assert!((body.inertia_diag.y - body.inertia_diag.z).abs() < 1e-10);
    }

    #[test]
    fn rectangular_body() {
        let body = RigidBody::new(DVec3::new(1.0, 0.5, 0.5));
        assert!((body.mass - 2.0).abs() < 1e-10);
        // Ixx = 2/12 * (1^2 + 1^2) = 2/12 * 2 = 1/3
        assert!((body.inertia_diag.x - 1.0 / 3.0).abs() < 1e-10);
        // Iyy = 2/12 * (2^2 + 1^2) = 2/12 * 5 = 5/6
        assert!((body.inertia_diag.y - 5.0 / 6.0).abs() < 1e-10);
    }
}
