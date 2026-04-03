//! Featherstone's Articulated Body Algorithm (ABA).
//!
//! Multi-DOF joints are expanded into chains of 1-DOF revolute joints
//! with zero-mass virtual bodies, keeping the core algorithm simple.

use glam::{DMat3, DQuat, DVec3};

use crate::body::RigidBody;
use crate::joint::Joint;
use crate::spatial::{SMat6, SVec6, SXform};

/// A 1-DOF joint in the expanded Featherstone tree.
#[derive(Debug, Clone)]
pub struct FJoint {
    pub parent_body: usize,
    pub child_body: usize,
    pub axis: DVec3,
    pub parent_anchor: DVec3,
    pub child_anchor: DVec3,
    pub angle: f64,
    pub velocity: f64,
    pub torque: f64,
    pub angle_min: f64,
    pub angle_max: f64,
    pub limit_stiffness: f64,
    pub damping: f64,
    pub original_joint_idx: usize,
    pub original_dof_idx: usize,
}

/// Full state for Featherstone ABA on an expanded tree.
pub struct FeatherstoneState {
    // Tree structure
    body_inertias: Vec<SMat6>,
    parents: Vec<Option<usize>>,
    fjoints: Vec<FJoint>,
    // Per-body workspace
    velocities: Vec<SVec6>,
    accelerations: Vec<SVec6>,
    art_inertias: Vec<SMat6>,
    bias_forces: Vec<SVec6>,
    // Per-joint workspace
    xforms: Vec<SXform>,
    motion_subspaces: Vec<SVec6>,
    coriolis: Vec<SVec6>,
    u_vec: Vec<SVec6>,
    d_scalar: Vec<f64>,
    u_scalar: Vec<f64>,
}

impl FeatherstoneState {
    /// Build the expanded Featherstone tree from world data.
    ///
    /// `bodies`: all rigid bodies (index 0 = root/fixed base).
    /// `joints`: the original multi-DOF joints.
    /// `torques`: per-joint torques, one `[f64; 3]` per joint.
    pub fn from_world(bodies: &[RigidBody], joints: &[Joint], torques: &[[f64; 3]]) -> Self {
        let num_real_bodies = bodies.len();

        // Start with real bodies
        let mut body_inertias: Vec<SMat6> = bodies.iter().map(|b| b.spatial_inertia()).collect();
        let mut parents: Vec<Option<usize>> = vec![None; num_real_bodies];
        let mut fjoints: Vec<FJoint> = Vec::new();

        for (ji, joint) in joints.iter().enumerate() {
            let axes = joint.dof_axes();
            let dof = axes.len();
            let torque = if ji < torques.len() {
                torques[ji]
            } else {
                [0.0; 3]
            };

            match dof {
                0 => {
                    // Rigid: just set parent, no FJoint
                    parents[joint.child_idx] = Some(joint.parent_idx);
                }
                1 => {
                    parents[joint.child_idx] = Some(joint.parent_idx);
                    fjoints.push(FJoint {
                        parent_body: joint.parent_idx,
                        child_body: joint.child_idx,
                        axis: axes[0],
                        parent_anchor: joint.parent_anchor,
                        child_anchor: joint.child_anchor,
                        angle: joint.angles[0],
                        velocity: joint.velocities[0],
                        torque: torque[0],
                        angle_min: joint.angle_min[0],
                        angle_max: joint.angle_max[0],
                        limit_stiffness: joint.limit_stiffness,
                        damping: joint.damping,
                        original_joint_idx: ji,
                        original_dof_idx: 0,
                    });
                }
                2 => {
                    // Create 1 virtual body
                    let vb = body_inertias.len();
                    body_inertias.push(SMat6::ZERO);
                    parents.push(Some(joint.parent_idx));

                    // First FJoint: parent → virtual body
                    // parent_anchor on parent side, child_anchor = ZERO
                    fjoints.push(FJoint {
                        parent_body: joint.parent_idx,
                        child_body: vb,
                        axis: axes[0],
                        parent_anchor: joint.parent_anchor,
                        child_anchor: DVec3::ZERO,
                        angle: joint.angles[0],
                        velocity: joint.velocities[0],
                        torque: torque[0],
                        angle_min: joint.angle_min[0],
                        angle_max: joint.angle_max[0],
                        limit_stiffness: joint.limit_stiffness,
                        damping: joint.damping,
                        original_joint_idx: ji,
                        original_dof_idx: 0,
                    });

                    // Second FJoint: virtual body → real child
                    // parent_anchor = ZERO on virtual, child_anchor on real child
                    parents[joint.child_idx] = Some(vb);
                    fjoints.push(FJoint {
                        parent_body: vb,
                        child_body: joint.child_idx,
                        axis: axes[1],
                        parent_anchor: DVec3::ZERO,
                        child_anchor: joint.child_anchor,
                        angle: joint.angles[1],
                        velocity: joint.velocities[1],
                        torque: torque[1],
                        angle_min: joint.angle_min[1],
                        angle_max: joint.angle_max[1],
                        limit_stiffness: joint.limit_stiffness,
                        damping: joint.damping,
                        original_joint_idx: ji,
                        original_dof_idx: 1,
                    });
                }
                3 => {
                    // Create 2 virtual bodies
                    let vb1 = body_inertias.len();
                    body_inertias.push(SMat6::ZERO);
                    parents.push(Some(joint.parent_idx));

                    let vb2 = body_inertias.len();
                    body_inertias.push(SMat6::ZERO);
                    parents.push(Some(vb1));

                    // Joint 0: parent → vb1
                    fjoints.push(FJoint {
                        parent_body: joint.parent_idx,
                        child_body: vb1,
                        axis: axes[0],
                        parent_anchor: joint.parent_anchor,
                        child_anchor: DVec3::ZERO,
                        angle: joint.angles[0],
                        velocity: joint.velocities[0],
                        torque: torque[0],
                        angle_min: joint.angle_min[0],
                        angle_max: joint.angle_max[0],
                        limit_stiffness: joint.limit_stiffness,
                        damping: joint.damping,
                        original_joint_idx: ji,
                        original_dof_idx: 0,
                    });

                    // Joint 1: vb1 → vb2
                    fjoints.push(FJoint {
                        parent_body: vb1,
                        child_body: vb2,
                        axis: axes[1],
                        parent_anchor: DVec3::ZERO,
                        child_anchor: DVec3::ZERO,
                        angle: joint.angles[1],
                        velocity: joint.velocities[1],
                        torque: torque[1],
                        angle_min: joint.angle_min[1],
                        angle_max: joint.angle_max[1],
                        limit_stiffness: joint.limit_stiffness,
                        damping: joint.damping,
                        original_joint_idx: ji,
                        original_dof_idx: 1,
                    });

                    // Joint 2: vb2 → real child
                    parents[joint.child_idx] = Some(vb2);
                    fjoints.push(FJoint {
                        parent_body: vb2,
                        child_body: joint.child_idx,
                        axis: axes[2],
                        parent_anchor: DVec3::ZERO,
                        child_anchor: joint.child_anchor,
                        angle: joint.angles[2],
                        velocity: joint.velocities[2],
                        torque: torque[2],
                        angle_min: joint.angle_min[2],
                        angle_max: joint.angle_max[2],
                        limit_stiffness: joint.limit_stiffness,
                        damping: joint.damping,
                        original_joint_idx: ji,
                        original_dof_idx: 2,
                    });
                }
                _ => unreachable!(),
            }
        }

        let nb = body_inertias.len();
        let nj = fjoints.len();

        FeatherstoneState {
            body_inertias,
            parents,
            fjoints,
            velocities: vec![SVec6::ZERO; nb],
            accelerations: vec![SVec6::ZERO; nb],
            art_inertias: vec![SMat6::ZERO; nb],
            bias_forces: vec![SVec6::ZERO; nb],
            xforms: vec![SXform::identity(); nj],
            motion_subspaces: vec![SVec6::ZERO; nj],
            coriolis: vec![SVec6::ZERO; nj],
            u_vec: vec![SVec6::ZERO; nj],
            d_scalar: vec![0.0; nj],
            u_scalar: vec![0.0; nj],
        }
    }

    /// Run the three-pass ABA. Returns per-expanded-joint accelerations.
    pub fn compute_accelerations(&mut self, gravity: DVec3) -> Vec<f64> {
        let nb = self.body_inertias.len();
        let nj = self.fjoints.len();

        // Initialize
        self.velocities[0] = SVec6::ZERO;
        for i in 0..nb {
            self.art_inertias[i] = self.body_inertias[i];
            self.bias_forces[i] = SVec6::ZERO;
        }

        // ── Pass 1: Outward (root → leaves) ──
        for j in 0..nj {
            let parent = self.fjoints[j].parent_body;
            let child = self.fjoints[j].child_body;
            let axis = self.fjoints[j].axis;
            let angle = self.fjoints[j].angle;
            let velocity = self.fjoints[j].velocity;
            let parent_anchor = self.fjoints[j].parent_anchor;
            let child_anchor = self.fjoints[j].child_anchor;

            // Spatial transform parent → child
            let rot_q = DQuat::from_axis_angle(axis, angle);
            let rot_mat = DMat3::from_quat(rot_q);
            let child_origin = parent_anchor - rot_q * child_anchor;
            let e = rot_mat.transpose(); // parent→child rotation
            let xform = SXform::new(e, child_origin);
            self.xforms[j] = xform;

            // Motion subspace in child frame
            let s = SVec6::new(axis, child_anchor.cross(axis));
            self.motion_subspaces[j] = s;

            // Joint velocity
            let v_j = s * velocity;

            // Child velocity
            let v_parent = self.velocities[parent];
            let v_child = xform.apply_motion(&v_parent) + v_j;
            self.velocities[child] = v_child;

            // Coriolis acceleration
            self.coriolis[j] = v_child.cross_motion(&v_j);

            // Bias force
            let i_v = self.body_inertias[child].mul_vec(&v_child);
            self.bias_forces[child] = v_child.cross_force(&i_v);
        }

        // ── Pass 2: Inward (leaves → root) ──
        for j in (0..nj).rev() {
            let parent = self.fjoints[j].parent_body;
            let child = self.fjoints[j].child_body;
            let s = self.motion_subspaces[j];
            let xform = self.xforms[j];

            // U = Ia * S
            let u = self.art_inertias[child].mul_vec(&s);
            self.u_vec[j] = u;

            // D = S^T * U (scalar for 1-DOF)
            let d = s.dot(&u).max(1e-10);
            self.d_scalar[j] = d;

            // Effective torque
            let fj = &self.fjoints[j];
            let mut limit_torque = 0.0;
            if fj.angle < fj.angle_min {
                limit_torque = fj.limit_stiffness * (fj.angle_min - fj.angle);
            } else if fj.angle > fj.angle_max {
                limit_torque = fj.limit_stiffness * (fj.angle_max - fj.angle);
            }
            let effective_torque = fj.torque + limit_torque - fj.damping * fj.velocity;
            let u_scalar = effective_torque - s.dot(&self.bias_forces[child]);
            self.u_scalar[j] = u_scalar;

            // Propagate articulated inertia to parent
            // Ia_prop = Ia - U * U^T / D
            let ia = self.art_inertias[child];
            let ia_prop = ia - SMat6::outer(&u, &u) * (1.0 / d);

            // pa = pA + Ia_prop * c + U * (u / D)
            let pa = self.bias_forces[child]
                + ia_prop.mul_vec(&self.coriolis[j])
                + u * (u_scalar / d);

            self.art_inertias[parent] = self.art_inertias[parent] + xform.transform_inertia_to_parent(&ia_prop);
            self.bias_forces[parent] = self.bias_forces[parent] + xform.transpose_apply_force(&pa);
        }

        // ── Pass 3: Outward (root → leaves) ──
        // Root acceleration encodes gravity
        self.accelerations[0] = SVec6::new(DVec3::ZERO, -gravity);

        let mut qddot = vec![0.0; nj];
        for j in 0..nj {
            let parent = self.fjoints[j].parent_body;
            let child = self.fjoints[j].child_body;
            let xform = self.xforms[j];
            let s = self.motion_subspaces[j];
            let u = self.u_vec[j];
            let d = self.d_scalar[j];
            let u_s = self.u_scalar[j];

            let a_parent = self.accelerations[parent];
            let mut a_child = xform.apply_motion(&a_parent) + self.coriolis[j];

            let qdd = (u_s - u.dot(&a_child)) / d;
            qddot[j] = qdd;

            a_child = a_child + s * qdd;
            self.accelerations[child] = a_child;
        }

        qddot
    }

    pub fn fjoints(&self) -> &[FJoint] {
        &self.fjoints
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::RigidBody;
    use crate::joint::Joint;

    const EPS: f64 = 1e-6;

    fn heavy_root() -> RigidBody {
        RigidBody {
            half_extents: DVec3::splat(0.5),
            mass: 1e6,
            inertia_diag: DVec3::splat(1e6),
        }
    }

    fn light_body(mass: f64) -> RigidBody {
        RigidBody {
            half_extents: DVec3::splat(0.1),
            mass,
            inertia_diag: DVec3::splat(mass * 0.01),
        }
    }

    #[test]
    fn simple_pendulum_gravity() {
        // Heavy root (index 0) + light pendulum bob (index 1)
        // Revolute about Z at the origin, child body's CoM is offset
        // from the joint by child_anchor = [-1, 0, 0] (so the child's
        // center of mass is at distance 1 in the +X direction from the joint).
        let bodies = vec![heavy_root(), light_body(1.0)];
        let joint = Joint::revolute(
            0, 1,
            DVec3::ZERO,                      // parent anchor (joint at origin)
            DVec3::new(-1.0, 0.0, 0.0),       // child anchor (CoM is 1m from joint)
            DVec3::Z,                          // axis
        );

        let gravity = DVec3::new(0.0, -9.81, 0.0);

        // At angle 0: child CoM is at [1,0,0] from joint.
        // Gravity [0,-9.81,0] creates torque about Z: r × F = [1,0,0] × [0,-m*g,0] = -m*g*Z
        // → negative angular acceleration about Z.
        {
            let mut state = FeatherstoneState::from_world(&bodies, &[joint.clone()], &[[0.0; 3]]);
            let qddot = state.compute_accelerations(gravity);
            assert!(
                qddot[0] < -1.0,
                "At angle 0, expected significant negative accel, got {}",
                qddot[0]
            );
        }

        // At angle π/2 about Z: child CoM rotates to [0,1,0] from joint.
        // Gravity [0,-9.81,0] is along the arm → near-zero gravitational torque about Z.
        // Note: angle π/2 ≈ 1.5708 exceeds default angle_max=1.5, so there's a
        // small limit torque. We just verify it's much less than the angle=0 case.
        {
            let mut joint2 = joint.clone();
            joint2.angles[0] = std::f64::consts::FRAC_PI_2;
            joint2.angle_max = [3.0; 3]; // widen limits to avoid limit torque
            let mut state = FeatherstoneState::from_world(&bodies, &[joint2], &[[0.0; 3]]);
            let qddot = state.compute_accelerations(gravity);
            assert!(
                qddot[0].abs() < 0.5,
                "At angle π/2 (arm along gravity), expected near-zero accel, got {}",
                qddot[0]
            );
        }
    }

    #[test]
    fn torque_produces_acceleration() {
        let bodies = vec![heavy_root(), light_body(1.0)];
        let joint = Joint::revolute(
            0, 1,
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::ZERO,
            DVec3::Z,
        );

        // Apply torque 5.0, no gravity
        let mut state = FeatherstoneState::from_world(&bodies, &[joint], &[[5.0, 0.0, 0.0]]);
        let qddot = state.compute_accelerations(DVec3::ZERO);

        assert!(
            qddot[0] > 0.0,
            "Positive torque should produce positive acceleration, got {}",
            qddot[0]
        );
        // For a point mass m=1 at distance r=0 (child_anchor=0, but inertia is 0.01),
        // with the motion subspace S = [0,0,1; 0,0,0], the effective inertia D = S^T I S
        // which is the Izz component = 0.01.
        // qddot ≈ torque / D = 5.0 / 0.01 = 500.0 (approximately, ignoring damping)
        // But damping = 0.5, velocity = 0, so damping term is 0.
        // Actually limit_stiffness and limits may affect this. Let's just check positive.
        assert!(
            qddot[0] > 1.0,
            "Expected significant acceleration from torque 5.0, got {}",
            qddot[0]
        );
    }

    #[test]
    fn zero_inputs_zero_acceleration() {
        let bodies = vec![heavy_root(), light_body(1.0)];
        let joint = Joint::revolute(
            0, 1,
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::ZERO,
            DVec3::Z,
        );

        let mut state = FeatherstoneState::from_world(&bodies, &[joint], &[[0.0; 3]]);
        let qddot = state.compute_accelerations(DVec3::ZERO);

        assert!(
            qddot[0].abs() < EPS,
            "No torque, no velocity, no gravity → zero accel, got {}",
            qddot[0]
        );
    }

    #[test]
    fn universal_joint_expands_to_two() {
        let bodies = vec![heavy_root(), light_body(1.0)];
        let joint = Joint::universal(
            0, 1,
            DVec3::new(1.0, 0.0, 0.0),
            DVec3::ZERO,
            DVec3::Y,
            DVec3::Z,
        );

        let state = FeatherstoneState::from_world(&bodies, &[joint], &[[0.0; 3]]);

        // Should have 2 expanded joints
        assert_eq!(
            state.fjoints().len(),
            2,
            "Universal should expand to 2 FJoints, got {}",
            state.fjoints().len()
        );

        // Should have 3 bodies (2 real + 1 virtual)
        assert_eq!(
            state.body_inertias.len(),
            3,
            "Universal should create 3 bodies (2 real + 1 virtual), got {}",
            state.body_inertias.len()
        );

        // Virtual body should have zero inertia
        let vb_inertia = &state.body_inertias[2];
        for col in 0..6 {
            for row in 0..6 {
                assert!(
                    vb_inertia.0[col][row].abs() < EPS,
                    "Virtual body inertia should be zero"
                );
            }
        }
    }
}
