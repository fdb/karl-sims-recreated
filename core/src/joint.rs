use glam::{DVec3, DQuat};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JointType {
    Rigid,     // 0 DOF
    Revolute,  // 1 DOF
    Twist,     // 1 DOF
    Universal, // 2 DOF
    BendTwist, // 2 DOF
    TwistBend, // 2 DOF
    Spherical, // 3 DOF
}

impl JointType {
    pub fn dof_count(&self) -> usize {
        match self {
            JointType::Rigid => 0,
            JointType::Revolute | JointType::Twist => 1,
            JointType::Universal | JointType::BendTwist | JointType::TwistBend => 2,
            JointType::Spherical => 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Joint {
    pub parent_idx: usize,
    pub child_idx: usize,
    pub joint_type: JointType,
    pub parent_anchor: DVec3,
    pub child_anchor: DVec3,
    pub axis: DVec3,
    pub secondary_axis: DVec3,
    pub angles: [f64; 3],
    pub velocities: [f64; 3],
    pub angle_min: [f64; 3],
    pub angle_max: [f64; 3],
    pub limit_stiffness: f64,
    pub damping: f64,
}

impl Joint {
    pub fn revolute(
        parent_idx: usize,
        child_idx: usize,
        parent_anchor: DVec3,
        child_anchor: DVec3,
        axis: DVec3,
    ) -> Self {
        Self {
            parent_idx,
            child_idx,
            joint_type: JointType::Revolute,
            parent_anchor,
            child_anchor,
            axis: axis.normalize(),
            secondary_axis: DVec3::ZERO,
            angles: [0.0; 3],
            velocities: [0.0; 3],
            angle_min: [-1.5; 3],
            angle_max: [1.5; 3],
            limit_stiffness: 20.0,
            damping: 0.5,
        }
    }

    pub fn twist(
        parent_idx: usize,
        child_idx: usize,
        parent_anchor: DVec3,
        child_anchor: DVec3,
        twist_axis: DVec3,
    ) -> Self {
        Self {
            parent_idx,
            child_idx,
            joint_type: JointType::Twist,
            parent_anchor,
            child_anchor,
            axis: twist_axis.normalize(),
            secondary_axis: DVec3::ZERO,
            angles: [0.0; 3],
            velocities: [0.0; 3],
            angle_min: [-1.5; 3],
            angle_max: [1.5; 3],
            limit_stiffness: 20.0,
            damping: 0.5,
        }
    }

    pub fn universal(
        parent_idx: usize,
        child_idx: usize,
        parent_anchor: DVec3,
        child_anchor: DVec3,
        primary_axis: DVec3,
        secondary_axis: DVec3,
    ) -> Self {
        Self {
            parent_idx,
            child_idx,
            joint_type: JointType::Universal,
            parent_anchor,
            child_anchor,
            axis: primary_axis.normalize(),
            secondary_axis: secondary_axis.normalize(),
            angles: [0.0; 3],
            velocities: [0.0; 3],
            angle_min: [-1.0; 3],
            angle_max: [1.0; 3],
            limit_stiffness: 20.0,
            damping: 0.5,
        }
    }

    pub fn bend_twist(
        parent_idx: usize,
        child_idx: usize,
        parent_anchor: DVec3,
        child_anchor: DVec3,
        bend_axis: DVec3,
        twist_axis: DVec3,
    ) -> Self {
        Self {
            parent_idx,
            child_idx,
            joint_type: JointType::BendTwist,
            parent_anchor,
            child_anchor,
            axis: bend_axis.normalize(),
            secondary_axis: twist_axis.normalize(),
            angles: [0.0; 3],
            velocities: [0.0; 3],
            angle_min: [-1.0; 3],
            angle_max: [1.0; 3],
            limit_stiffness: 20.0,
            damping: 0.5,
        }
    }

    pub fn twist_bend(
        parent_idx: usize,
        child_idx: usize,
        parent_anchor: DVec3,
        child_anchor: DVec3,
        twist_axis: DVec3,
        bend_axis: DVec3,
    ) -> Self {
        Self {
            parent_idx,
            child_idx,
            joint_type: JointType::TwistBend,
            parent_anchor,
            child_anchor,
            axis: twist_axis.normalize(),
            secondary_axis: bend_axis.normalize(),
            angles: [0.0; 3],
            velocities: [0.0; 3],
            angle_min: [-1.0; 3],
            angle_max: [1.0; 3],
            limit_stiffness: 20.0,
            damping: 0.5,
        }
    }

    pub fn spherical(
        parent_idx: usize,
        child_idx: usize,
        parent_anchor: DVec3,
        child_anchor: DVec3,
    ) -> Self {
        Self {
            parent_idx,
            child_idx,
            joint_type: JointType::Spherical,
            parent_anchor,
            child_anchor,
            axis: DVec3::X,
            secondary_axis: DVec3::Y,
            angles: [0.0; 3],
            velocities: [0.0; 3],
            angle_min: [-1.0; 3],
            angle_max: [1.0; 3],
            limit_stiffness: 20.0,
            damping: 0.5,
        }
    }

    pub fn rigid(
        parent_idx: usize,
        child_idx: usize,
        parent_anchor: DVec3,
        child_anchor: DVec3,
    ) -> Self {
        Self {
            parent_idx,
            child_idx,
            joint_type: JointType::Rigid,
            parent_anchor,
            child_anchor,
            axis: DVec3::ZERO,
            secondary_axis: DVec3::ZERO,
            angles: [0.0; 3],
            velocities: [0.0; 3],
            angle_min: [0.0; 3],
            angle_max: [0.0; 3],
            limit_stiffness: 20.0,
            damping: 0.5,
        }
    }

    /// Compute the joint rotation quaternion from current angles.
    pub fn joint_rotation(&self) -> DQuat {
        match self.joint_type {
            JointType::Rigid => DQuat::IDENTITY,
            JointType::Revolute | JointType::Twist => {
                DQuat::from_axis_angle(self.axis, self.angles[0])
            }
            JointType::Universal | JointType::BendTwist | JointType::TwistBend => {
                let r0 = DQuat::from_axis_angle(self.axis, self.angles[0]);
                let r1 = DQuat::from_axis_angle(self.secondary_axis, self.angles[1]);
                r0 * r1
            }
            JointType::Spherical => {
                let r0 = DQuat::from_axis_angle(self.axis, self.angles[0]);
                let r1 = DQuat::from_axis_angle(self.secondary_axis, self.angles[1]);
                let third = self.axis.cross(self.secondary_axis).normalize();
                let r2 = DQuat::from_axis_angle(third, self.angles[2]);
                r0 * r1 * r2
            }
        }
    }

    /// Get the rotation axes for each DOF.
    pub fn dof_axes(&self) -> Vec<DVec3> {
        match self.joint_type {
            JointType::Rigid => vec![],
            JointType::Revolute | JointType::Twist => vec![self.axis],
            JointType::Universal | JointType::BendTwist | JointType::TwistBend => {
                vec![self.axis, self.secondary_axis]
            }
            JointType::Spherical => {
                let third = self.axis.cross(self.secondary_axis).normalize();
                vec![self.axis, self.secondary_axis, third]
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dof_counts() {
        assert_eq!(JointType::Revolute.dof_count(), 1);
        assert_eq!(JointType::Spherical.dof_count(), 3);
        assert_eq!(JointType::Rigid.dof_count(), 0);
        assert_eq!(JointType::Universal.dof_count(), 2);
    }

    #[test]
    fn universal_has_two_axes() {
        let j = Joint::universal(0, 1, DVec3::X, DVec3::NEG_X, DVec3::Y, DVec3::Z);
        assert_eq!(j.dof_axes().len(), 2);
    }

    #[test]
    fn spherical_has_three_axes() {
        let j = Joint::spherical(0, 1, DVec3::X, DVec3::NEG_X);
        assert_eq!(j.dof_axes().len(), 3);
    }

    #[test]
    fn joint_rotation_identity_at_zero() {
        let j = Joint::revolute(0, 1, DVec3::X, DVec3::NEG_X, DVec3::Z);
        let q = j.joint_rotation();
        assert!((q - DQuat::IDENTITY).length() < 1e-10);
    }

    #[test]
    fn joint_rotation_revolute_90deg() {
        let mut j = Joint::revolute(0, 1, DVec3::X, DVec3::NEG_X, DVec3::Z);
        j.angles[0] = std::f64::consts::FRAC_PI_2;
        let q = j.joint_rotation();
        let rotated = q * DVec3::X;
        assert!((rotated - DVec3::Y).length() < 1e-10);
    }

    #[test]
    fn joint_rotation_universal_composed() {
        let mut j = Joint::universal(0, 1, DVec3::X, DVec3::NEG_X, DVec3::X, DVec3::Y);
        j.angles[0] = std::f64::consts::FRAC_PI_2;
        j.angles[1] = std::f64::consts::FRAC_PI_2;
        let q = j.joint_rotation();
        assert!((q.length() - 1.0).abs() < 1e-10); // valid rotation
        assert!((q - DQuat::IDENTITY).length() > 0.1); // not identity
    }

    #[test]
    fn revolute_defaults() {
        let j = Joint::revolute(
            0,
            1,
            DVec3::X,
            DVec3::NEG_X,
            DVec3::new(0.0, 0.0, 2.0),
        );
        assert_eq!(j.joint_type, JointType::Revolute);
        // axis should be normalized
        assert!((j.axis.length() - 1.0).abs() < 1e-10);
        assert_eq!(j.axis, DVec3::Z);
        assert_eq!(j.angles, [0.0; 3]);
    }
}
