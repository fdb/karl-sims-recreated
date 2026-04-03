use glam::DVec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
            angles: [0.0; 3],
            velocities: [0.0; 3],
            angle_min: [-1.5; 3],
            angle_max: [1.5; 3],
            limit_stiffness: 20.0,
            damping: 0.5,
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
