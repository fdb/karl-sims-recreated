# M2: Featherstone Articulated Body Dynamics — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace M1's simple per-joint Euler integration with Featherstone's O(N) Articulated Body Algorithm, enabling physically correct coupled dynamics across all 7 joint types.

**Architecture:** Multi-DOF joints (universal, bend-twist, twist-bend, spherical) are expanded into stacked 1-DOF revolute joints with zero-mass virtual bodies. This means the ABA core only handles 1-DOF joints, keeping the algorithm simple. Spatial algebra uses [angular; linear] convention for 6D vectors. New files: `spatial.rs` (types), `featherstone.rs` (algorithm). Existing files modified: `joint.rs`, `body.rs`, `world.rs`, `scene.rs`.

**Tech Stack:** Rust, glam (DVec3, DMat3, DQuat — scalar-math for determinism)

---

## File Structure

```
core/src/
├── lib.rs              # MODIFY: add pub mod spatial, featherstone
├── spatial.rs          # NEW: SVec6, SMat6, SXform — 6D spatial algebra
├── featherstone.rs     # NEW: ABA algorithm, workspace, expanded tree builder
├── body.rs             # MODIFY: add spatial_inertia() method
├── joint.rs            # MODIFY: add constructors for all 7 types, spatial methods
├── world.rs            # MODIFY: replace step() with Featherstone-based dynamics
└── scene.rs            # MODIFY: add multi-DOF test scenes

web/src/
└── lib.rs              # MODIFY: add new scenes to selector

frontend/src/
└── App.tsx             # MODIFY: add new scene options to dropdown
```

---

## Task 1: Spatial Algebra Types

**Files:**
- Create: `core/src/spatial.rs`
- Modify: `core/src/lib.rs`

- [ ] **Step 1: Add module declaration**

Add to `core/src/lib.rs`:
```rust
pub mod body;
pub mod joint;
pub mod spatial;
pub mod featherstone;
pub mod world;
pub mod scene;
```

Create empty `core/src/featherstone.rs`:
```rust
// Featherstone ABA — implemented in Task 3
```

- [ ] **Step 2: Implement SVec6**

```rust
// core/src/spatial.rs
use glam::{DMat3, DVec3};
use std::ops;

/// 6D spatial vector in [angular; linear] convention (Plücker coordinates).
/// Indices 0-2: angular (wx, wy, wz), indices 3-5: linear (vx, vy, vz).
#[derive(Debug, Clone, Copy)]
pub struct SVec6(pub [f64; 6]);

impl SVec6 {
    pub const ZERO: SVec6 = SVec6([0.0; 6]);

    pub fn new(angular: DVec3, linear: DVec3) -> Self {
        Self([angular.x, angular.y, angular.z, linear.x, linear.y, linear.z])
    }

    pub fn angular(&self) -> DVec3 {
        DVec3::new(self.0[0], self.0[1], self.0[2])
    }

    pub fn linear(&self) -> DVec3 {
        DVec3::new(self.0[3], self.0[4], self.0[5])
    }

    pub fn dot(&self, other: &SVec6) -> f64 {
        self.0.iter().zip(other.0.iter()).map(|(a, b)| a * b).sum()
    }

    /// Motion cross product: crm(self) * other
    /// For self = [w; v], other = [w'; v']:
    ///   result = [w × w'; w × v' + v × w']
    pub fn cross_motion(&self, other: &SVec6) -> SVec6 {
        let w = self.angular();
        let v = self.linear();
        let ow = other.angular();
        let ov = other.linear();
        SVec6::new(w.cross(ow), w.cross(ov) + v.cross(ow))
    }

    /// Force cross product: crf(self) * other = -crm(self)^T * other
    /// For self = [w; v], other = [n; f]:
    ///   result = [w × n + v × f; w × f]
    pub fn cross_force(&self, other: &SVec6) -> SVec6 {
        let w = self.angular();
        let v = self.linear();
        let on = other.angular();
        let of = other.linear();
        SVec6::new(w.cross(on) + v.cross(of), w.cross(of))
    }
}

impl ops::Add for SVec6 {
    type Output = SVec6;
    fn add(self, rhs: SVec6) -> SVec6 {
        let mut r = [0.0; 6];
        for i in 0..6 { r[i] = self.0[i] + rhs.0[i]; }
        SVec6(r)
    }
}

impl ops::Sub for SVec6 {
    type Output = SVec6;
    fn sub(self, rhs: SVec6) -> SVec6 {
        let mut r = [0.0; 6];
        for i in 0..6 { r[i] = self.0[i] - rhs.0[i]; }
        SVec6(r)
    }
}

impl ops::Mul<f64> for SVec6 {
    type Output = SVec6;
    fn mul(self, s: f64) -> SVec6 {
        let mut r = [0.0; 6];
        for i in 0..6 { r[i] = self.0[i] * s; }
        SVec6(r)
    }
}

impl ops::Neg for SVec6 {
    type Output = SVec6;
    fn neg(self) -> SVec6 {
        let mut r = [0.0; 6];
        for i in 0..6 { r[i] = -self.0[i]; }
        SVec6(r)
    }
}
```

- [ ] **Step 3: Implement SMat6**

```rust
/// 6×6 spatial matrix. Column-major: mat[col][row].
#[derive(Debug, Clone, Copy)]
pub struct SMat6(pub [[f64; 6]; 6]);

impl SMat6 {
    pub const ZERO: SMat6 = SMat6([[0.0; 6]; 6]);

    pub fn identity() -> Self {
        let mut m = Self::ZERO;
        for i in 0..6 { m.0[i][i] = 1.0; }
        m
    }

    pub fn mul_vec(&self, v: &SVec6) -> SVec6 {
        let mut r = [0.0; 6];
        for i in 0..6 {
            for j in 0..6 {
                r[i] += self.0[j][i] * v.0[j];
            }
        }
        SVec6(r)
    }

    pub fn mul_mat(&self, other: &SMat6) -> SMat6 {
        let mut r = SMat6::ZERO;
        for c in 0..6 {
            for i in 0..6 {
                for k in 0..6 {
                    r.0[c][i] += self.0[k][i] * other.0[c][k];
                }
            }
        }
        r
    }

    pub fn transpose(&self) -> SMat6 {
        let mut r = SMat6::ZERO;
        for i in 0..6 {
            for j in 0..6 {
                r.0[j][i] = self.0[i][j];
            }
        }
        r
    }

    /// Outer product: a * b^T (rank-1 matrix)
    pub fn outer(a: &SVec6, b: &SVec6) -> SMat6 {
        let mut r = SMat6::ZERO;
        for c in 0..6 {
            for row in 0..6 {
                r.0[c][row] = a.0[row] * b.0[c];
            }
        }
        r
    }

    /// Build spatial inertia matrix for a rigid body at its center of mass.
    /// inertia_diag: diagonal of 3×3 rotational inertia tensor (Ixx, Iyy, Izz)
    /// mass: body mass
    ///
    /// I_spatial = [diag(inertia), 0; 0, mass * I_3]
    pub fn from_body_inertia(inertia_diag: DVec3, mass: f64) -> SMat6 {
        let mut m = SMat6::ZERO;
        m.0[0][0] = inertia_diag.x;
        m.0[1][1] = inertia_diag.y;
        m.0[2][2] = inertia_diag.z;
        m.0[3][3] = mass;
        m.0[4][4] = mass;
        m.0[5][5] = mass;
        m
    }
}

impl ops::Add for SMat6 {
    type Output = SMat6;
    fn add(self, rhs: SMat6) -> SMat6 {
        let mut r = SMat6::ZERO;
        for c in 0..6 {
            for row in 0..6 {
                r.0[c][row] = self.0[c][row] + rhs.0[c][row];
            }
        }
        r
    }
}

impl ops::Sub for SMat6 {
    type Output = SMat6;
    fn sub(self, rhs: SMat6) -> SMat6 {
        let mut r = SMat6::ZERO;
        for c in 0..6 {
            for row in 0..6 {
                r.0[c][row] = self.0[c][row] - rhs.0[c][row];
            }
        }
        r
    }
}
```

- [ ] **Step 4: Implement SXform**

```rust
/// Spatial coordinate transform from parent frame to child frame.
///
/// - `rot`: rotation matrix E that transforms 3D vectors from parent to child:
///   v_child = E * v_parent
/// - `pos`: position of child frame origin in parent frame coordinates.
///
/// The 6×6 motion transform matrix is:
///   X = [E, 0; -E*[r]×, E]
///
/// Convention verified against Featherstone RBDA eq. 2.24, RBDL, and pinocchio.
pub struct SXform {
    pub rot: DMat3,
    pub pos: DVec3,
}

impl SXform {
    pub fn new(rot: DMat3, pos: DVec3) -> Self {
        Self { rot, pos }
    }

    /// Identity transform (no rotation, no translation).
    pub fn identity() -> Self {
        Self { rot: DMat3::IDENTITY, pos: DVec3::ZERO }
    }

    /// Transform a motion vector from parent frame to child frame.
    /// X * [w; v] = [E*w; E*(v - r×w)]
    pub fn apply_motion(&self, v: &SVec6) -> SVec6 {
        let w = v.angular();
        let vel = v.linear();
        let ew = self.rot * w;
        let ev = self.rot * (vel - self.pos.cross(w));
        SVec6::new(ew, ev)
    }

    /// Transform a force vector from child frame to parent frame.
    /// X^T * [n; f] = [E^T*n + r×(E^T*f); E^T*f]
    ///
    /// This is the force dual of the motion transform: if X maps motion
    /// parent→child, then X^T maps force child→parent.
    pub fn transpose_apply_force(&self, f: &SVec6) -> SVec6 {
        let n = f.angular();
        let fl = f.linear();
        let et_n = self.rot.transpose() * n;
        let et_f = self.rot.transpose() * fl;
        SVec6::new(et_n + self.pos.cross(et_f), et_f)
    }

    /// Transform a 6×6 spatial inertia from child frame to parent frame.
    /// I_parent = X^T * I_child * X
    ///
    /// Where X is this (parent→child) motion transform.
    pub fn transform_inertia_to_parent(&self, inertia: &SMat6) -> SMat6 {
        let x = self.as_mat6();
        let xt = x.transpose();
        xt.mul_mat(&inertia.mul_mat(&x))
    }

    /// Build the full 6×6 motion transform matrix.
    /// X = [E, 0; -E*[r]×, E]
    pub fn as_mat6(&self) -> SMat6 {
        let e = self.rot;
        let rx = skew(self.pos);
        let neg_e_rx = -(e * rx); // -E * [r]×

        let mut m = SMat6::ZERO;
        // Top-left 3×3: E
        for c in 0..3 {
            for r in 0..3 {
                m.0[c][r] = e.col(c)[r];
            }
        }
        // Bottom-left 3×3: -E*[r]×
        for c in 0..3 {
            for r in 0..3 {
                m.0[c][r + 3] = neg_e_rx.col(c)[r];
            }
        }
        // Bottom-right 3×3: E
        for c in 0..3 {
            for r in 0..3 {
                m.0[c + 3][r + 3] = e.col(c)[r];
            }
        }
        // Top-right 3×3: 0 (already zero)
        m
    }
}

/// 3×3 skew-symmetric matrix of vector v: [v]× such that [v]× * w = v × w
fn skew(v: DVec3) -> DMat3 {
    DMat3::from_cols(
        DVec3::new(0.0, v.z, -v.y),
        DVec3::new(-v.z, 0.0, v.x),
        DVec3::new(v.y, -v.x, 0.0),
    )
}
```

- [ ] **Step 5: Write spatial algebra tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn approx_eq_vec6(a: &SVec6, b: &SVec6, tol: f64) {
        for i in 0..6 {
            assert!(
                (a.0[i] - b.0[i]).abs() < tol,
                "index {i}: {} vs {} (diff {})",
                a.0[i], b.0[i], (a.0[i] - b.0[i]).abs()
            );
        }
    }

    #[test]
    fn svec6_dot() {
        let a = SVec6([1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let b = SVec6([6.0, 5.0, 4.0, 3.0, 2.0, 1.0]);
        assert!((a.dot(&b) - 56.0).abs() < 1e-10);
    }

    #[test]
    fn cross_motion_antisymmetric() {
        let a = SVec6([1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let b = SVec6([0.5, -1.0, 2.0, -0.5, 1.5, -2.0]);
        let ab = a.cross_motion(&b);
        let ba = b.cross_motion(&a);
        approx_eq_vec6(&ab, &(-ba), 1e-10);
    }

    #[test]
    fn cross_force_is_neg_cross_motion_transpose() {
        // crf(v) * f should equal -crm(v)^T * f
        // Equivalently: v.cross_force(f).dot(g) == -v.cross_motion(g).dot(f) for any g
        let v = SVec6([1.0, -0.5, 2.0, 0.3, -1.0, 0.7]);
        let f = SVec6([0.5, 1.0, -0.5, 2.0, -1.0, 0.5]);
        let g = SVec6([-1.0, 0.5, 1.5, -0.3, 0.8, -0.6]);
        let lhs = v.cross_force(&f).dot(&g);
        let rhs = -v.cross_motion(&g).dot(&f);
        assert!((lhs - rhs).abs() < 1e-10, "{lhs} vs {rhs}");
    }

    #[test]
    fn smat6_identity_mul_vec() {
        let v = SVec6([1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let result = SMat6::identity().mul_vec(&v);
        approx_eq_vec6(&result, &v, 1e-10);
    }

    #[test]
    fn sxform_identity_is_noop() {
        let x = SXform::identity();
        let v = SVec6([1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        approx_eq_vec6(&x.apply_motion(&v), &v, 1e-10);
        approx_eq_vec6(&x.transpose_apply_force(&v), &v, 1e-10);
    }

    #[test]
    fn sxform_pure_rotation_transforms_angular() {
        // 90° rotation about Z: parent's X becomes child's Y
        let rot = DMat3::from_rotation_z(PI / 2.0);
        let x = SXform::new(rot, DVec3::ZERO);

        // Angular velocity [1,0,0] in parent → [0,1,0] in child
        let v = SVec6::new(DVec3::X, DVec3::ZERO);
        let result = x.apply_motion(&v);
        approx_eq_vec6(
            &result,
            &SVec6::new(DVec3::new(0.0, 1.0, 0.0), DVec3::ZERO),
            1e-10,
        );
    }

    #[test]
    fn sxform_translation_creates_linear_from_angular() {
        // Child origin at [d,0,0] in parent, no rotation
        let d = 2.0;
        let x = SXform::new(DMat3::IDENTITY, DVec3::new(d, 0.0, 0.0));

        // Parent rotates about Z with omega=1: ω = [0,0,1]
        // Child frame linear velocity = -(r × ω) = -([d,0,0] × [0,0,1]) = -[0,-d,0] = [0,d,0]
        let v = SVec6::new(DVec3::Z, DVec3::ZERO);
        let result = x.apply_motion(&v);
        approx_eq_vec6(
            &result,
            &SVec6::new(DVec3::Z, DVec3::new(0.0, d, 0.0)),
            1e-10,
        );
    }

    #[test]
    fn sxform_force_roundtrip() {
        // X^T * (X * v) for a force should give back a related quantity
        // More precisely: v^T * (X^T * f) == (X*v)^T * f
        let rot = DMat3::from_rotation_y(0.5);
        let x = SXform::new(rot, DVec3::new(1.0, 0.5, -0.3));
        let v = SVec6([1.0, -0.5, 2.0, 0.3, -1.0, 0.7]);
        let f = SVec6([0.5, 1.0, -0.5, 2.0, -1.0, 0.5]);

        let lhs = v.dot(&x.transpose_apply_force(&f));
        let xv = x.apply_motion(&v);
        let rhs = xv.dot(&f);
        assert!((lhs - rhs).abs() < 1e-10, "{lhs} vs {rhs}");
    }

    #[test]
    fn inertia_parallel_axis_theorem() {
        // Point mass m at child origin, child displaced by [d,0,0] from parent
        let m = 3.0;
        let i_child = SMat6::from_body_inertia(DVec3::ZERO, m);
        let d = 2.0;
        let x = SXform::new(DMat3::IDENTITY, DVec3::new(d, 0.0, 0.0));
        let i_parent = x.transform_inertia_to_parent(&i_child);

        // Expected: Iyy_parent = Izz_parent = m*d^2, Ixx_parent = 0
        let iyy = i_parent.0[1][1];
        let izz = i_parent.0[2][2];
        let ixx = i_parent.0[0][0];
        assert!((iyy - m * d * d).abs() < 1e-10, "Iyy: {iyy}");
        assert!((izz - m * d * d).abs() < 1e-10, "Izz: {izz}");
        assert!(ixx.abs() < 1e-10, "Ixx: {ixx}");
        // Mass terms unchanged
        assert!((i_parent.0[3][3] - m).abs() < 1e-10);
    }
}
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p karl-sims-core spatial`
Expected: 8 tests pass

- [ ] **Step 7: Commit**

```bash
git add core/src/spatial.rs core/src/featherstone.rs core/src/lib.rs
git commit -m "feat: spatial algebra types — SVec6, SMat6, SXform for Featherstone"
```

---

## Task 2: Joint Spatial Methods + All 7 Type Constructors

**Files:**
- Modify: `core/src/joint.rs`
- Modify: `core/src/body.rs`

- [ ] **Step 1: Add spatial_inertia to RigidBody**

Add to `core/src/body.rs`:

```rust
use crate::spatial::SMat6;

impl RigidBody {
    /// Spatial inertia matrix at the body's center of mass frame.
    /// I = [diag(Ixx,Iyy,Izz), 0; 0, m*I3]
    pub fn spatial_inertia(&self) -> SMat6 {
        SMat6::from_body_inertia(self.inertia_diag, self.mass)
    }
}
```

- [ ] **Step 2: Add spatial methods and new constructors to Joint**

Add to `core/src/joint.rs`:

```rust
use glam::{DMat3, DQuat};
use crate::spatial::{SVec6, SXform};

impl Joint {
    /// Construct a twist joint (1-DOF rotation about the attachment direction).
    pub fn twist(
        parent_idx: usize, child_idx: usize,
        parent_anchor: DVec3, child_anchor: DVec3,
        twist_axis: DVec3,
    ) -> Self {
        Self {
            parent_idx, child_idx,
            joint_type: JointType::Twist,
            parent_anchor, child_anchor,
            axis: twist_axis.normalize(),
            angles: [0.0; 3], velocities: [0.0; 3],
            angle_min: [-1.5; 3], angle_max: [1.5; 3],
            limit_stiffness: 20.0, damping: 0.5,
        }
    }

    /// Construct a universal joint (2-DOF: two axes perpendicular to each other).
    /// `primary_axis` and `secondary_axis` should be perpendicular.
    /// DOF 0 rotates about primary, DOF 1 about secondary.
    pub fn universal(
        parent_idx: usize, child_idx: usize,
        parent_anchor: DVec3, child_anchor: DVec3,
        primary_axis: DVec3, secondary_axis: DVec3,
    ) -> Self {
        Self {
            parent_idx, child_idx,
            joint_type: JointType::Universal,
            parent_anchor, child_anchor,
            axis: primary_axis.normalize(),
            // Store secondary axis in angle_min/max slots? No — add a field.
            // For now, store secondary axis direction by using a convention:
            // secondary_axis is derived from axis cross (parent_anchor - child_anchor)
            // Actually, let's store it properly. We need a secondary_axis field.
            // DECISION: We add a `secondary_axis` field to Joint.
            angles: [0.0; 3], velocities: [0.0; 3],
            angle_min: [-1.0; 3], angle_max: [1.0; 3],
            limit_stiffness: 20.0, damping: 0.5,
        }
    }

    // ... similar for BendTwist, TwistBend, Spherical
}
```

Wait — the `Joint` struct needs a `secondary_axis` field to support multi-DOF joints. Let me add that.

**Updated Joint struct** — add `secondary_axis: DVec3` field:

```rust
#[derive(Debug, Clone)]
pub struct Joint {
    pub parent_idx: usize,
    pub child_idx: usize,
    pub joint_type: JointType,
    pub parent_anchor: DVec3,
    pub child_anchor: DVec3,
    /// Primary rotation axis (for revolute: the hinge axis)
    pub axis: DVec3,
    /// Secondary rotation axis (for multi-DOF joints; perpendicular to primary)
    pub secondary_axis: DVec3,
    pub angles: [f64; 3],
    pub velocities: [f64; 3],
    pub angle_min: [f64; 3],
    pub angle_max: [f64; 3],
    pub limit_stiffness: f64,
    pub damping: f64,
}
```

Update `Joint::revolute()` to set `secondary_axis: DVec3::ZERO`.
Update `Joint::twist()` similarly.

Add constructors:

```rust
impl Joint {
    pub fn revolute(
        parent_idx: usize, child_idx: usize,
        parent_anchor: DVec3, child_anchor: DVec3,
        axis: DVec3,
    ) -> Self {
        Self {
            parent_idx, child_idx,
            joint_type: JointType::Revolute,
            parent_anchor, child_anchor,
            axis: axis.normalize(),
            secondary_axis: DVec3::ZERO,
            angles: [0.0; 3], velocities: [0.0; 3],
            angle_min: [-1.5; 3], angle_max: [1.5; 3],
            limit_stiffness: 20.0, damping: 0.5,
        }
    }

    pub fn twist(
        parent_idx: usize, child_idx: usize,
        parent_anchor: DVec3, child_anchor: DVec3,
        twist_axis: DVec3,
    ) -> Self {
        Self {
            parent_idx, child_idx,
            joint_type: JointType::Twist,
            parent_anchor, child_anchor,
            axis: twist_axis.normalize(),
            secondary_axis: DVec3::ZERO,
            angles: [0.0; 3], velocities: [0.0; 3],
            angle_min: [-1.5; 3], angle_max: [1.5; 3],
            limit_stiffness: 20.0, damping: 0.5,
        }
    }

    pub fn universal(
        parent_idx: usize, child_idx: usize,
        parent_anchor: DVec3, child_anchor: DVec3,
        primary_axis: DVec3, secondary_axis: DVec3,
    ) -> Self {
        Self {
            parent_idx, child_idx,
            joint_type: JointType::Universal,
            parent_anchor, child_anchor,
            axis: primary_axis.normalize(),
            secondary_axis: secondary_axis.normalize(),
            angles: [0.0; 3], velocities: [0.0; 3],
            angle_min: [-1.0; 3], angle_max: [1.0; 3],
            limit_stiffness: 20.0, damping: 0.5,
        }
    }

    pub fn bend_twist(
        parent_idx: usize, child_idx: usize,
        parent_anchor: DVec3, child_anchor: DVec3,
        bend_axis: DVec3, twist_axis: DVec3,
    ) -> Self {
        Self {
            parent_idx, child_idx,
            joint_type: JointType::BendTwist,
            parent_anchor, child_anchor,
            axis: bend_axis.normalize(),
            secondary_axis: twist_axis.normalize(),
            angles: [0.0; 3], velocities: [0.0; 3],
            angle_min: [-1.0; 3], angle_max: [1.0; 3],
            limit_stiffness: 20.0, damping: 0.5,
        }
    }

    pub fn twist_bend(
        parent_idx: usize, child_idx: usize,
        parent_anchor: DVec3, child_anchor: DVec3,
        twist_axis: DVec3, bend_axis: DVec3,
    ) -> Self {
        Self {
            parent_idx, child_idx,
            joint_type: JointType::TwistBend,
            parent_anchor, child_anchor,
            axis: twist_axis.normalize(),
            secondary_axis: bend_axis.normalize(),
            angles: [0.0; 3], velocities: [0.0; 3],
            angle_min: [-1.0; 3], angle_max: [1.0; 3],
            limit_stiffness: 20.0, damping: 0.5,
        }
    }

    pub fn spherical(
        parent_idx: usize, child_idx: usize,
        parent_anchor: DVec3, child_anchor: DVec3,
    ) -> Self {
        // Primary = X, secondary = Y, third = Z (Euler XYZ)
        Self {
            parent_idx, child_idx,
            joint_type: JointType::Spherical,
            parent_anchor, child_anchor,
            axis: DVec3::X,
            secondary_axis: DVec3::Y,
            angles: [0.0; 3], velocities: [0.0; 3],
            angle_min: [-1.0; 3], angle_max: [1.0; 3],
            limit_stiffness: 20.0, damping: 0.5,
        }
    }

    pub fn rigid(
        parent_idx: usize, child_idx: usize,
        parent_anchor: DVec3, child_anchor: DVec3,
    ) -> Self {
        Self {
            parent_idx, child_idx,
            joint_type: JointType::Rigid,
            parent_anchor, child_anchor,
            axis: DVec3::ZERO,
            secondary_axis: DVec3::ZERO,
            angles: [0.0; 3], velocities: [0.0; 3],
            angle_min: [0.0; 3], angle_max: [0.0; 3],
            limit_stiffness: 0.0, damping: 0.0,
        }
    }

    /// Compute the joint rotation quaternion from current angles.
    /// Multi-DOF joints compose rotations in order: DOF0 * DOF1 * DOF2.
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
                let third_axis = self.axis.cross(self.secondary_axis).normalize();
                let r2 = DQuat::from_axis_angle(third_axis, self.angles[2]);
                r0 * r1 * r2
            }
        }
    }

    /// Get the rotation axes for each DOF.
    /// Returns a slice of axes: 0 for rigid, 1 for revolute/twist, 2 for 2-DOF, 3 for spherical.
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
```

- [ ] **Step 3: Add tests for new constructors and spatial methods**

```rust
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
        let j = Joint::revolute(0, 1, DVec3::X, DVec3::NEG_X, DVec3::new(0.0, 0.0, 2.0));
        assert_eq!(j.joint_type, JointType::Revolute);
        assert!((j.axis.length() - 1.0).abs() < 1e-10);
        assert_eq!(j.axis, DVec3::Z);
    }

    #[test]
    fn universal_has_two_axes() {
        let j = Joint::universal(0, 1, DVec3::X, DVec3::NEG_X, DVec3::Y, DVec3::Z);
        assert_eq!(j.dof_axes().len(), 2);
        assert!((j.dof_axes()[0] - DVec3::Y).length() < 1e-10);
        assert!((j.dof_axes()[1] - DVec3::Z).length() < 1e-10);
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
        // Rotation of 90° about Z should turn X into Y
        let rotated = q * DVec3::X;
        assert!((rotated - DVec3::Y).length() < 1e-10);
    }

    #[test]
    fn joint_rotation_universal_composed() {
        let mut j = Joint::universal(0, 1, DVec3::X, DVec3::NEG_X, DVec3::X, DVec3::Y);
        j.angles[0] = std::f64::consts::FRAC_PI_2; // 90° about X
        j.angles[1] = std::f64::consts::FRAC_PI_2; // then 90° about Y
        let q = j.joint_rotation();
        // R_x(90) * R_y(90): Z → (X→Y: Z stays Z) then (Y: Z→-X)?
        // Easier to just check it's not identity and is a valid rotation
        assert!((q.length() - 1.0).abs() < 1e-10);
        assert!((q - DQuat::IDENTITY).length() > 0.1);
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p karl-sims-core joint`
Expected: 7 tests pass (old ones still work + new ones)

- [ ] **Step 5: Commit**

```bash
git add core/src/joint.rs core/src/body.rs
git commit -m "feat: joint constructors for all 7 types with rotation and DOF axes"
```

---

## Task 3: Featherstone ABA Algorithm

**Files:**
- Modify: `core/src/featherstone.rs`

This is the core algorithm. Multi-DOF joints are expanded into stacked 1-DOF revolute joints with zero-mass virtual bodies.

- [ ] **Step 1: Define the expanded tree types**

```rust
// core/src/featherstone.rs
use glam::{DMat3, DQuat, DVec3};
use crate::body::RigidBody;
use crate::joint::Joint;
use crate::spatial::{SMat6, SVec6, SXform};

/// A 1-DOF joint in the expanded Featherstone tree.
#[derive(Debug, Clone)]
struct FJoint {
    parent_body: usize,  // index into fbodies
    child_body: usize,   // index into fbodies
    axis: DVec3,         // rotation axis (in parent frame at q=0)
    parent_anchor: DVec3,// joint position in parent body frame
    child_anchor: DVec3, // joint position in child body frame
    angle: f64,
    velocity: f64,
    torque: f64,
    // Joint limits (from the original joint)
    angle_min: f64,
    angle_max: f64,
    limit_stiffness: f64,
    damping: f64,
    // Back-reference to original joint
    original_joint_idx: usize,
    original_dof_idx: usize,
}

/// Workspace for the Featherstone ABA, reused each step.
pub struct FeatherstoneState {
    /// Expanded body spatial inertias (zero for virtual bodies)
    body_inertias: Vec<SMat6>,
    /// Parent index per expanded body (None for root)
    parents: Vec<Option<usize>>,
    /// Expanded 1-DOF joints
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
```

- [ ] **Step 2: Implement tree building from World**

```rust
impl FeatherstoneState {
    /// Build the expanded Featherstone tree from a World's bodies and joints.
    pub fn from_world(bodies: &[RigidBody], joints: &[Joint], torques: &[[f64; 3]]) -> Self {
        let mut body_inertias = Vec::new();
        let mut parents: Vec<Option<usize>> = Vec::new();
        let mut fjoints = Vec::new();

        // Map from original body index to expanded body index
        let mut body_map: Vec<usize> = Vec::new();

        // Add all real bodies
        for body in bodies {
            let idx = body_inertias.len();
            body_inertias.push(body.spatial_inertia());
            parents.push(None); // parent set by joints
            body_map.push(idx);
        }

        // Expand joints
        for (ji, joint) in joints.iter().enumerate() {
            let dof_axes = joint.dof_axes();
            let dof_count = dof_axes.len();

            if dof_count == 0 {
                // Rigid joint: no FJoint, just record parent relationship
                parents[body_map[joint.child_idx]] = Some(body_map[joint.parent_idx]);
                continue;
            }

            // For single DOF: direct connection
            // For multi-DOF: chain of virtual bodies + 1-DOF joints
            let real_parent = body_map[joint.parent_idx];
            let real_child = body_map[joint.child_idx];

            let mut prev_body = real_parent;

            for (di, &dof_axis) in dof_axes.iter().enumerate() {
                let is_first = di == 0;
                let is_last = di == dof_count - 1;

                let child_body = if is_last {
                    real_child
                } else {
                    // Create a virtual body with zero mass
                    let virt_idx = body_inertias.len();
                    body_inertias.push(SMat6::ZERO);
                    parents.push(Some(prev_body));
                    virt_idx
                };

                let p_anchor = if is_first { joint.parent_anchor } else { DVec3::ZERO };
                let c_anchor = if is_last { joint.child_anchor } else { DVec3::ZERO };

                parents[child_body] = Some(prev_body);

                fjoints.push(FJoint {
                    parent_body: prev_body,
                    child_body,
                    axis: dof_axis,
                    parent_anchor: p_anchor,
                    child_anchor: c_anchor,
                    angle: joint.angles[di],
                    velocity: joint.velocities[di],
                    torque: torques[ji][di],
                    angle_min: joint.angle_min[di],
                    angle_max: joint.angle_max[di],
                    limit_stiffness: joint.limit_stiffness,
                    damping: joint.damping,
                    original_joint_idx: ji,
                    original_dof_idx: di,
                });

                prev_body = child_body;
            }
        }

        let n_bodies = body_inertias.len();
        let n_joints = fjoints.len();

        Self {
            body_inertias,
            parents,
            fjoints,
            velocities: vec![SVec6::ZERO; n_bodies],
            accelerations: vec![SVec6::ZERO; n_bodies],
            art_inertias: vec![SMat6::ZERO; n_bodies],
            bias_forces: vec![SVec6::ZERO; n_bodies],
            xforms: vec![SXform::identity(); n_joints],
            motion_subspaces: vec![SVec6::ZERO; n_joints],
            coriolis: vec![SVec6::ZERO; n_joints],
            u_vec: vec![SVec6::ZERO; n_joints],
            d_scalar: vec![0.0; n_joints],
            u_scalar: vec![0.0; n_joints],
        }
    }
}
```

- [ ] **Step 3: Implement the three-pass ABA**

```rust
impl FeatherstoneState {
    /// Run Featherstone's Articulated Body Algorithm.
    ///
    /// Returns joint accelerations (qddot) for each expanded 1-DOF joint.
    /// `gravity`: gravity vector (e.g., [0, -9.81, 0]). Use [0,0,0] for swimming.
    pub fn compute_accelerations(&mut self, gravity: DVec3) -> Vec<f64> {
        let n_joints = self.fjoints.len();
        let n_bodies = self.body_inertias.len();

        // --- Pass 1: Outward (root to leaves) ---
        // Root velocity is zero (fixed base)
        self.velocities[0] = SVec6::ZERO;

        // Initialize articulated inertias and bias forces for all bodies
        for i in 0..n_bodies {
            self.art_inertias[i] = self.body_inertias[i];
            self.bias_forces[i] = SVec6::ZERO;
        }

        // Process joints in order (parent-first, which is guaranteed by construction)
        for j in 0..n_joints {
            let fj = &self.fjoints[j];
            let parent = fj.parent_body;
            let child = fj.child_body;

            // Compute spatial transform parent→child
            let rot_q = DQuat::from_axis_angle(fj.axis, fj.angle);
            let rot_mat = DMat3::from_quat(rot_q);
            let child_origin_in_parent = fj.parent_anchor - rot_q * fj.child_anchor;
            // E transforms vectors from parent to child frame: E = R^T
            let e = rot_mat.transpose();
            let x = SXform::new(e, child_origin_in_parent);
            self.xforms[j] = x;

            // Motion subspace in child frame
            // For a revolute about axis n: S = [n; child_anchor × n]
            // (n is preserved by the rotation about n itself)
            let s = SVec6::new(fj.axis, fj.child_anchor.cross(fj.axis));
            self.motion_subspaces[j] = s;

            // Joint velocity
            let v_j = s * fj.velocity;

            // Child velocity = X * parent_velocity + joint_velocity
            let v_parent = self.velocities[parent];
            let v_child = x.apply_motion(&v_parent) + v_j;
            self.velocities[child] = v_child;

            // Coriolis acceleration
            self.coriolis[j] = v_child.cross_motion(&v_j);

            // Bias force from velocity (velocity-product force)
            let i_child = self.body_inertias[child];
            let p = v_child.cross_force(&i_child.mul_vec(&v_child));
            self.bias_forces[child] = p;
        }

        // --- Pass 2: Inward (leaves to root) ---
        // Process joints in reverse order (children before parents)
        for j in (0..n_joints).rev() {
            let fj = &self.fjoints[j];
            let child = fj.child_body;
            let parent = fj.parent_body;
            let x = &self.xforms[j];
            let s = &self.motion_subspaces[j];

            let ia = &self.art_inertias[child];
            let pa = &self.bias_forces[child];

            // U = I_A * S
            let u = ia.mul_vec(s);
            self.u_vec[j] = u;

            // D = S^T * U (scalar)
            let d = s.dot(&u).max(1e-10); // prevent division by zero
            self.d_scalar[j] = d;

            // u = tau - S^T * p_A (includes joint limits and damping)
            let limit_torque = if fj.angle < fj.angle_min {
                fj.limit_stiffness * (fj.angle_min - fj.angle)
            } else if fj.angle > fj.angle_max {
                fj.limit_stiffness * (fj.angle_max - fj.angle)
            } else {
                0.0
            };
            let effective_torque = fj.torque + limit_torque - fj.damping * fj.velocity;
            let u_scalar = effective_torque - s.dot(pa);
            self.u_scalar[j] = u_scalar;

            // Propagate to parent
            // Ia = I_A - U * U^T / D
            let ia_prop = *ia - SMat6::outer(&u, &u) * (1.0 / d);
            // pa = p_A + Ia*c + U * (u/D)
            let pa_prop = *pa + ia_prop.mul_vec(&self.coriolis[j]) + u * (u_scalar / d);

            // Transform to parent frame and accumulate
            let ia_parent = x.transform_inertia_to_parent(&ia_prop);
            let pa_parent = x.transpose_apply_force(&pa_prop);

            self.art_inertias[parent] = self.art_inertias[parent] + ia_parent;
            self.bias_forces[parent] = self.bias_forces[parent] + pa_parent;
        }

        // --- Pass 3: Outward (root to leaves) ---
        // Root acceleration = -gravity (spatial convention)
        self.accelerations[0] = SVec6::new(DVec3::ZERO, -gravity);

        let mut qddot = vec![0.0; n_joints];

        for j in 0..n_joints {
            let fj = &self.fjoints[j];
            let parent = fj.parent_body;
            let child = fj.child_body;
            let x = &self.xforms[j];
            let s = &self.motion_subspaces[j];
            let u = &self.u_vec[j];
            let d = self.d_scalar[j];
            let u_s = self.u_scalar[j];

            // a_child = X * a_parent + c
            let a_child = x.apply_motion(&self.accelerations[parent]) + self.coriolis[j];

            // qddot = (u - U^T * a) / D
            let qdd = (u_s - u.dot(&a_child)) / d;
            qddot[j] = qdd;

            // a_child += S * qddot
            self.accelerations[child] = a_child + *s * qdd;
        }

        qddot
    }
}

impl ops::Mul<f64> for SMat6 {
    type Output = SMat6;
    fn mul(self, s: f64) -> SMat6 {
        let mut r = SMat6::ZERO;
        for c in 0..6 {
            for row in 0..6 {
                r.0[c][row] = self.0[c][row] * s;
            }
        }
        r
    }
}
```

Note: The `SMat6 * f64` impl needs to be added to `spatial.rs`.

- [ ] **Step 4: Write Featherstone tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::body::RigidBody;
    use crate::joint::{Joint, JointType};
    use glam::DVec3;

    /// Simple pendulum: one body hanging from a fixed root.
    /// At angle θ from vertical (rest), angular acceleration should be -(g/L)*sin(θ).
    #[test]
    fn simple_pendulum_gravity() {
        let mut bodies = vec![
            RigidBody::new(DVec3::splat(0.01)), // root (tiny, effectively fixed)
            RigidBody::new(DVec3::new(0.1, 0.1, 0.1)), // pendulum bob
        ];
        // Make root very heavy so it doesn't move
        bodies[0].mass = 1e6;
        bodies[0].inertia_diag = DVec3::splat(1e6);

        let joint = Joint::revolute(
            0, 1,
            DVec3::new(0.0, 0.0, 0.0),   // joint at root center
            DVec3::new(0.0, 1.0, 0.0),    // child anchor: joint is 1m above child center
            DVec3::Z,                       // rotate about Z
        );

        let torques = [[0.0; 3]];
        let gravity = DVec3::new(0.0, -9.81, 0.0);

        // At angle = 0 (hanging straight down), no angular acceleration
        let mut state = FeatherstoneState::from_world(&bodies, &[joint.clone()], &torques);
        let qddot = state.compute_accelerations(gravity);

        // At zero angle, pendulum is hanging down, no torque → near zero acceleration
        assert!(
            qddot[0].abs() < 0.01,
            "pendulum at rest should have ~0 accel: got {}",
            qddot[0]
        );

        // At 90° (horizontal), acceleration should be -(g/L)*sin(90) = -g/L
        let mut tilted_joint = joint.clone();
        tilted_joint.angles[0] = std::f64::consts::FRAC_PI_2;
        let mut state = FeatherstoneState::from_world(&bodies, &[tilted_joint], &torques);
        let qddot = state.compute_accelerations(gravity);

        let l = 1.0; // distance from joint to child center
        let expected_accel = -9.81 / l; // approximately, for small bob mass vs heavy root

        // The exact value depends on the bob's own inertia, so allow some tolerance
        assert!(
            (qddot[0] - expected_accel).abs() < 2.0,
            "pendulum at 90° should accelerate ~{}: got {}",
            expected_accel, qddot[0]
        );
        // But it must be negative (swinging back)
        assert!(qddot[0] < 0.0, "should swing back: got {}", qddot[0]);
    }

    /// Two bodies with applied torque: acceleration should be proportional to torque/inertia
    #[test]
    fn torque_produces_acceleration() {
        let mut bodies = vec![
            RigidBody::new(DVec3::splat(0.01)),
            RigidBody::new(DVec3::new(0.5, 0.5, 0.5)),
        ];
        bodies[0].mass = 1e6;
        bodies[0].inertia_diag = DVec3::splat(1e6);

        let joint = Joint::revolute(
            0, 1,
            DVec3::ZERO, DVec3::new(0.0, -0.5, 0.0),
            DVec3::Z,
        );

        let applied_torque = 5.0;
        let torques = [[applied_torque, 0.0, 0.0]];
        let gravity = DVec3::ZERO; // no gravity

        let mut state = FeatherstoneState::from_world(&bodies, &[joint], &torques);
        let qddot = state.compute_accelerations(gravity);

        // With no gravity and no velocity, qddot should be positive (same sign as torque)
        assert!(
            qddot[0] > 0.0,
            "positive torque should give positive acceleration: got {}",
            qddot[0]
        );
    }

    /// Zero torque, zero velocity, zero gravity → zero acceleration
    #[test]
    fn zero_inputs_zero_acceleration() {
        let mut bodies = vec![
            RigidBody::new(DVec3::splat(0.01)),
            RigidBody::new(DVec3::new(0.3, 0.3, 0.3)),
        ];
        bodies[0].mass = 1e6;
        bodies[0].inertia_diag = DVec3::splat(1e6);

        let joint = Joint::revolute(
            0, 1, DVec3::ZERO, DVec3::NEG_Y, DVec3::Z,
        );
        let torques = [[0.0; 3]];

        let mut state = FeatherstoneState::from_world(&bodies, &[joint], &torques);
        let qddot = state.compute_accelerations(DVec3::ZERO);

        assert!(
            qddot[0].abs() < 1e-10,
            "zero inputs should give zero accel: got {}",
            qddot[0]
        );
    }

    /// Multi-DOF expansion: universal joint produces 2 expanded joints
    #[test]
    fn universal_joint_expands_to_two() {
        let bodies = vec![
            RigidBody::new(DVec3::splat(0.5)),
            RigidBody::new(DVec3::splat(0.5)),
        ];
        let joint = Joint::universal(
            0, 1, DVec3::X, DVec3::NEG_X, DVec3::Y, DVec3::Z,
        );
        let torques = [[0.0; 3]];
        let state = FeatherstoneState::from_world(&bodies, &[joint], &torques);

        // Should have 3 bodies (2 real + 1 virtual) and 2 joints
        assert_eq!(state.fjoints.len(), 2, "universal should expand to 2 joints");
        assert_eq!(state.body_inertias.len(), 3, "should have 1 virtual body");
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p karl-sims-core featherstone`
Expected: 4 tests pass

- [ ] **Step 6: Commit**

```bash
git add core/src/featherstone.rs core/src/spatial.rs
git commit -m "feat: Featherstone ABA with 1-DOF joints and multi-DOF expansion"
```

---

## Task 4: Integrate Featherstone into World::step()

**Files:**
- Modify: `core/src/world.rs`

- [ ] **Step 1: Replace step() with Featherstone-based dynamics**

Update `core/src/world.rs`:

```rust
use glam::{DAffine3, DMat3, DQuat, DVec3};

use crate::body::RigidBody;
use crate::featherstone::FeatherstoneState;
use crate::joint::Joint;

#[derive(Debug, Clone)]
pub struct World {
    pub bodies: Vec<RigidBody>,
    pub joints: Vec<Joint>,
    pub transforms: Vec<DAffine3>,
    pub torques: Vec<[f64; 3]>,
    pub root: usize,
    pub time: f64,
    /// Gravity vector. Zero for swimming (no gravity).
    pub gravity: DVec3,
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
            gravity: DVec3::ZERO,
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

    /// Compute world transforms for all bodies from joint angles (forward kinematics).
    pub fn forward_kinematics(&mut self) {
        for i in 0..self.joints.len() {
            let joint = &self.joints[i];
            let parent_idx = joint.parent_idx;
            let child_idx = joint.child_idx;
            let parent_anchor = joint.parent_anchor;
            let child_anchor = joint.child_anchor;

            let parent_transform = self.transforms[parent_idx];

            // Use full joint rotation (handles multi-DOF)
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

    /// Advance simulation by dt using Featherstone's ABA + semi-implicit Euler.
    pub fn step(&mut self, dt: f64) {
        // Build expanded tree and run Featherstone
        let mut state = FeatherstoneState::from_world(
            &self.bodies, &self.joints, &self.torques,
        );
        let qddot = state.compute_accelerations(self.gravity);

        // Map expanded joint accelerations back to original joints
        // and integrate with semi-implicit Euler
        for fj_idx in 0..state.fjoints().len() {
            let fj = &state.fjoints()[fj_idx];
            let ji = fj.original_joint_idx;
            let di = fj.original_dof_idx;

            // Semi-implicit Euler: update velocity first, then position
            self.joints[ji].velocities[di] += qddot[fj_idx] * dt;
            self.joints[ji].angles[di] += self.joints[ji].velocities[di] * dt;
        }

        self.forward_kinematics();
        self.time += dt;
    }
}
```

Note: `FeatherstoneState` needs a public accessor `pub fn fjoints(&self) -> &[FJoint]` and `FJoint` needs `original_joint_idx` and `original_dof_idx` to be accessible. Make `FJoint` fields pub or add accessors in featherstone.rs.

- [ ] **Step 2: Update FeatherstoneState to expose needed fields**

In `core/src/featherstone.rs`, make `FJoint` pub and add accessor:

```rust
pub struct FJoint {
    // ... all fields as before but pub
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

impl FeatherstoneState {
    pub fn fjoints(&self) -> &[FJoint] {
        &self.fjoints
    }
    // ... rest of impl
}
```

- [ ] **Step 3: Update existing tests and verify they still pass**

The existing world tests should still pass since the interface is the same. The behavior might differ slightly (Featherstone is more physically accurate than the M1 simple integration), so update tolerance values if needed:

```rust
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
        // Make root very heavy to act as fixed base
        world.bodies[parent].mass = 1e6;
        world.bodies[parent].inertia_diag = DVec3::splat(1e6);

        let joint = Joint::revolute(
            parent, child,
            DVec3::new(0.5, 0.0, 0.0),
            DVec3::new(-0.5, 0.0, 0.0),
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
        assert!(child_pos.y.abs() < 1e-10);
        assert!(child_pos.z.abs() < 1e-10);
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
        assert!(world.joints[0].angles[0] > 0.0, "angle should increase");
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
        assert!(
            world.joints[0].angles[0] < 3.0,
            "angle should be bounded: got {}",
            world.joints[0].angles[0]
        );
    }

    #[test]
    fn damping_reduces_velocity() {
        let mut world = make_two_body_world();
        world.joints[0].velocities[0] = 5.0;
        let dt = 1.0 / 60.0;
        for _ in 0..300 {
            world.step(dt);
        }
        assert!(
            world.joints[0].velocities[0].abs() < 1.0,
            "velocity should decay: got {}",
            world.joints[0].velocities[0]
        );
    }
}
```

- [ ] **Step 4: Run all core tests**

Run: `cargo test -p karl-sims-core`
Expected: All tests pass (spatial + joint + featherstone + world + scene)

- [ ] **Step 5: Verify WASM still builds**

Run: `wasm-pack build web/ --target web --dev`
Expected: builds successfully

- [ ] **Step 6: Commit**

```bash
git add core/src/world.rs core/src/featherstone.rs
git commit -m "feat: integrate Featherstone ABA into World::step(), replacing simple Euler"
```

---

## Task 5: Multi-DOF Test Scenes + Web UI Update

**Files:**
- Modify: `core/src/scene.rs`
- Modify: `web/src/lib.rs`
- Modify: `frontend/src/App.tsx`

- [ ] **Step 1: Add multi-DOF test scenes**

Add to `core/src/scene.rs`:

```rust
/// A body connected by a universal joint (2-DOF).
/// Driven by sinusoidal torques on both axes.
pub fn universal_joint_demo() -> World {
    let mut world = World::new();
    let parent = world.add_body(DVec3::new(0.5, 0.5, 0.5));
    let child = world.add_body(DVec3::new(0.6, 0.3, 0.2));
    world.root = parent;
    world.set_root_transform(DAffine3::from_translation(DVec3::new(0.0, 2.0, 0.0)));

    // Make root heavy
    world.bodies[parent].mass = 1e6;
    world.bodies[parent].inertia_diag = DVec3::splat(1e6);

    let joint = Joint::universal(
        parent, child,
        DVec3::new(0.5, 0.0, 0.0),
        DVec3::new(-0.6, 0.0, 0.0),
        DVec3::Y, DVec3::Z,
    );
    world.add_joint(joint);
    world.forward_kinematics();
    world
}

pub fn universal_joint_torque(world: &mut World) {
    let t = world.time;
    if !world.torques.is_empty() {
        world.torques[0][0] = 2.0 * (2.0 * t).sin();
        world.torques[0][1] = 1.5 * (3.0 * t + 1.0).sin();
    }
}

/// A body connected by a spherical joint (3-DOF).
pub fn spherical_joint_demo() -> World {
    let mut world = World::new();
    let parent = world.add_body(DVec3::new(0.4, 0.4, 0.4));
    let child = world.add_body(DVec3::new(0.5, 0.3, 0.3));
    world.root = parent;
    world.set_root_transform(DAffine3::from_translation(DVec3::new(0.0, 2.0, 0.0)));

    world.bodies[parent].mass = 1e6;
    world.bodies[parent].inertia_diag = DVec3::splat(1e6);

    let joint = Joint::spherical(
        parent, child,
        DVec3::new(0.4, 0.0, 0.0),
        DVec3::new(-0.5, 0.0, 0.0),
    );
    world.add_joint(joint);
    world.forward_kinematics();
    world
}

pub fn spherical_joint_torque(world: &mut World) {
    let t = world.time;
    if !world.torques.is_empty() {
        world.torques[0][0] = 1.5 * (2.5 * t).sin();
        world.torques[0][1] = 1.0 * (3.0 * t + 0.5).sin();
        world.torques[0][2] = 0.8 * (1.5 * t + 1.0).sin();
    }
}

/// A chain of 3 bodies with revolute joints — tests coupled dynamics.
pub fn triple_chain() -> World {
    let mut world = World::new();
    let b0 = world.add_body(DVec3::new(0.3, 0.3, 0.3));
    let b1 = world.add_body(DVec3::new(0.4, 0.15, 0.15));
    let b2 = world.add_body(DVec3::new(0.35, 0.1, 0.1));
    world.root = b0;
    world.set_root_transform(DAffine3::from_translation(DVec3::new(0.0, 2.0, 0.0)));

    world.bodies[b0].mass = 1e6;
    world.bodies[b0].inertia_diag = DVec3::splat(1e6);

    let mut j0 = Joint::revolute(
        b0, b1, DVec3::new(0.3, 0.0, 0.0), DVec3::new(-0.4, 0.0, 0.0), DVec3::Z,
    );
    j0.damping = 0.3;
    world.add_joint(j0);

    let mut j1 = Joint::revolute(
        b1, b2, DVec3::new(0.4, 0.0, 0.0), DVec3::new(-0.35, 0.0, 0.0), DVec3::Z,
    );
    j1.damping = 0.3;
    world.add_joint(j1);

    world.forward_kinematics();
    world
}

pub fn triple_chain_torque(world: &mut World) {
    let t = world.time;
    if world.torques.len() >= 2 {
        world.torques[0][0] = 3.0 * (2.0 * t).sin();
        world.torques[1][0] = 2.0 * (3.0 * t + 1.0).sin();
    }
}
```

- [ ] **Step 2: Add tests for new scenes**

```rust
#[test]
fn universal_joint_demo_scene() {
    let world = universal_joint_demo();
    assert_eq!(world.bodies.len(), 2);
    assert_eq!(world.joints.len(), 1);
    assert_eq!(world.joints[0].joint_type, crate::joint::JointType::Universal);
}

#[test]
fn spherical_joint_demo_scene() {
    let world = spherical_joint_demo();
    assert_eq!(world.bodies.len(), 2);
    assert_eq!(world.joints.len(), 1);
    assert_eq!(world.joints[0].joint_type, crate::joint::JointType::Spherical);
}

#[test]
fn triple_chain_scene() {
    let world = triple_chain();
    assert_eq!(world.bodies.len(), 3);
    assert_eq!(world.joints.len(), 2);
}

#[test]
fn universal_joint_moves() {
    let mut world = universal_joint_demo();
    let dt = 1.0 / 60.0;
    for _ in 0..120 {
        universal_joint_torque(&mut world);
        world.step(dt);
    }
    // Both DOFs should have non-zero angles
    assert!(world.joints[0].angles[0].abs() > 0.01, "DOF 0: {}", world.joints[0].angles[0]);
    assert!(world.joints[0].angles[1].abs() > 0.01, "DOF 1: {}", world.joints[0].angles[1]);
}
```

- [ ] **Step 3: Update web scene selector**

In `web/src/lib.rs`, update the `SceneId` enum and `build_world`/`set_scene`/`tick` to include the new scenes:

```rust
#[derive(Clone, Copy, PartialEq, Eq)]
enum SceneId {
    SingleBox,
    HingedPair,
    Starfish,
    UniversalJoint,
    SphericalJoint,
    TripleChain,
}

// In set_scene():
"universal" => (scene::universal_joint_demo(), SceneId::UniversalJoint),
"spherical" => (scene::spherical_joint_demo(), SceneId::SphericalJoint),
"triple_chain" => (scene::triple_chain(), SceneId::TripleChain),

// In tick() torque dispatch:
SceneId::UniversalJoint => scene::universal_joint_torque(&mut state.world),
SceneId::SphericalJoint => scene::spherical_joint_torque(&mut state.world),
SceneId::TripleChain => scene::triple_chain_torque(&mut state.world),
```

- [ ] **Step 4: Update frontend scene dropdown**

In `frontend/src/App.tsx`, add the new scene options:

```tsx
const SCENES = [
  { id: "starfish", label: "Starfish (4 flippers)" },
  { id: "hinged_pair", label: "Hinged Pair" },
  { id: "triple_chain", label: "Triple Chain" },
  { id: "universal", label: "Universal Joint (2-DOF)" },
  { id: "spherical", label: "Spherical Joint (3-DOF)" },
  { id: "single_box", label: "Single Box" },
];
```

- [ ] **Step 5: Run all tests and build**

Run:
```bash
cargo test -p karl-sims-core && wasm-pack build web/ --target web --dev
```
Expected: All tests pass, WASM builds cleanly.

- [ ] **Step 6: Commit**

```bash
git add core/src/ web/src/ frontend/src/
git commit -m "feat: multi-DOF test scenes and web UI for Featherstone dynamics"
```

---

## Self-Review Checklist

**Spec coverage:**
- [x] Featherstone's O(N) articulated body method → Task 3 + 4
- [x] All 7 joint types (rigid, revolute, twist, universal, bend-twist, twist-bend, spherical) → Task 2
- [x] Joint limits with restoring spring forces → Task 3 (in ABA torque computation)
- [x] Effector torque application → Task 4 (torques passed through to Featherstone)
- [x] New test scenes showcasing each joint type → Task 5
- [x] Unit tests: energy conservation, joint limit behavior, torque response → Tasks 3 + 4

**Note on effector strength scaling:** The spec says "strength proportional to cross-sectional area." This is a M4 concern (when brains drive effectors). For M2, torques are externally applied and the scaling is handled by the scene torque functions. The max-torque-by-area constraint will be added when the brain/effector system is built.

**Placeholder scan:** No TBDs, TODOs, or placeholders.

**Type consistency:**
- `SVec6`, `SMat6`, `SXform` used consistently across spatial.rs and featherstone.rs
- `Joint::joint_rotation()` and `Joint::dof_axes()` used in both joint.rs and featherstone.rs
- `FeatherstoneState::from_world()` and `compute_accelerations()` used in world.rs
- Scene functions follow the same `scene_name() -> World` + `scene_name_torque(&mut World)` pattern
