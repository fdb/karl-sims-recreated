//! 6D spatial vector/matrix types and coordinate transforms for
//! Featherstone's articulated body dynamics.
//!
//! Convention: [angular; linear] (Plücker coordinates).

use glam::{DMat3, DVec3};
use std::ops::{Add, Mul, Neg, Sub};

// ── Helpers ──────────────────────────────────────────────────────────

/// Skew-symmetric matrix such that `skew(v) * w == v.cross(w)`.
pub fn skew(v: DVec3) -> DMat3 {
    // glam DMat3 is column-major: DMat3::from_cols(c0, c1, c2)
    DMat3::from_cols(
        DVec3::new(0.0, v.z, -v.y),
        DVec3::new(-v.z, 0.0, v.x),
        DVec3::new(v.y, -v.x, 0.0),
    )
}

// ── SVec6 ────────────────────────────────────────────────────────────

/// 6D spatial vector `[angular; linear]`.
#[derive(Debug, Clone, Copy)]
pub struct SVec6(pub [f64; 6]);

impl SVec6 {
    pub const ZERO: Self = Self([0.0; 6]);

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

    /// Motion cross product: `crm(self) * other`.
    ///
    /// If self = [w; v] and other = [w'; v'], then
    /// result = [w×w'; w×v' + v×w'].
    pub fn cross_motion(&self, other: &SVec6) -> SVec6 {
        let w = self.angular();
        let v = self.linear();
        let wp = other.angular();
        let vp = other.linear();
        SVec6::new(w.cross(wp), w.cross(vp) + v.cross(wp))
    }

    /// Force cross product: `crf(self) * other`.
    ///
    /// If self = [w; v] and other = [n; f], then
    /// result = [w×n + v×f; w×f].
    pub fn cross_force(&self, other: &SVec6) -> SVec6 {
        let w = self.angular();
        let v = self.linear();
        let n = other.angular();
        let f = other.linear();
        SVec6::new(w.cross(n) + v.cross(f), w.cross(f))
    }
}

impl Add for SVec6 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        let mut r = [0.0; 6];
        for i in 0..6 {
            r[i] = self.0[i] + rhs.0[i];
        }
        Self(r)
    }
}

impl Sub for SVec6 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        let mut r = [0.0; 6];
        for i in 0..6 {
            r[i] = self.0[i] - rhs.0[i];
        }
        Self(r)
    }
}

impl Mul<f64> for SVec6 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        let mut r = self.0;
        for v in &mut r {
            *v *= s;
        }
        Self(r)
    }
}

impl Neg for SVec6 {
    type Output = Self;
    fn neg(self) -> Self {
        self * -1.0
    }
}

// ── SMat6 ────────────────────────────────────────────────────────────

/// 6×6 spatial matrix, column-major: `mat[col][row]`.
#[derive(Debug, Clone, Copy)]
pub struct SMat6(pub [[f64; 6]; 6]);

impl SMat6 {
    pub const ZERO: Self = Self([[0.0; 6]; 6]);

    pub fn identity() -> Self {
        let mut m = Self::ZERO;
        for i in 0..6 {
            m.0[i][i] = 1.0;
        }
        m
    }

    pub fn mul_vec(&self, v: &SVec6) -> SVec6 {
        let mut r = [0.0; 6];
        for row in 0..6 {
            for col in 0..6 {
                r[row] += self.0[col][row] * v.0[col];
            }
        }
        SVec6(r)
    }

    pub fn mul_mat(&self, rhs: &SMat6) -> SMat6 {
        let mut r = SMat6::ZERO;
        for col in 0..6 {
            for row in 0..6 {
                let mut sum = 0.0;
                for k in 0..6 {
                    sum += self.0[k][row] * rhs.0[col][k];
                }
                r.0[col][row] = sum;
            }
        }
        r
    }

    pub fn transpose(&self) -> SMat6 {
        let mut r = SMat6::ZERO;
        for col in 0..6 {
            for row in 0..6 {
                r.0[col][row] = self.0[row][col];
            }
        }
        r
    }

    /// Rank-1 outer product: `a * b^T`.
    pub fn outer(a: &SVec6, b: &SVec6) -> SMat6 {
        let mut r = SMat6::ZERO;
        for col in 0..6 {
            for row in 0..6 {
                r.0[col][row] = a.0[row] * b.0[col];
            }
        }
        r
    }

    /// Spatial inertia for a rigid body at its center of mass.
    ///
    /// `I = [diag(Ixx,Iyy,Izz), 0; 0, m*I3]`
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

impl SMat6 {
    /// Solve Ax = b via Gaussian elimination with partial pivoting.
    /// Returns x. Returns SVec6::ZERO if matrix is near-singular.
    pub fn solve(&self, b: &SVec6) -> SVec6 {
        // Copy matrix and RHS for in-place modification
        let mut a = self.0;
        let mut x = b.0;

        // Forward elimination with partial pivoting
        for col in 0..6 {
            // Find pivot
            let mut max_row = col;
            let mut max_val = a[col][col].abs();
            for row in (col + 1)..6 {
                if a[col][row].abs() > max_val {
                    max_val = a[col][row].abs();
                    max_row = row;
                }
            }

            // Swap rows
            if max_row != col {
                for c in 0..6 {
                    a[c].swap(col, max_row);
                }
                x.swap(col, max_row);
            }

            let pivot = a[col][col];
            if pivot.abs() < 1e-20 {
                // Near-singular, return zero
                return SVec6::ZERO;
            }

            // Eliminate below
            for row in (col + 1)..6 {
                let factor = a[col][row] / pivot;
                for c in (col + 1)..6 {
                    a[c][row] -= factor * a[c][col];
                }
                x[row] -= factor * x[col];
                a[col][row] = 0.0;
            }
        }

        // Back substitution
        let mut result = [0.0; 6];
        for col in (0..6).rev() {
            let mut sum = x[col];
            for c in (col + 1)..6 {
                sum -= a[c][col] * result[c];
            }
            result[col] = sum / a[col][col];
        }

        SVec6(result)
    }
}

impl Add for SMat6 {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        let mut r = SMat6::ZERO;
        for col in 0..6 {
            for row in 0..6 {
                r.0[col][row] = self.0[col][row] + rhs.0[col][row];
            }
        }
        r
    }
}

impl Sub for SMat6 {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        let mut r = SMat6::ZERO;
        for col in 0..6 {
            for row in 0..6 {
                r.0[col][row] = self.0[col][row] - rhs.0[col][row];
            }
        }
        r
    }
}

impl Mul<f64> for SMat6 {
    type Output = Self;
    fn mul(self, s: f64) -> Self {
        let mut r = self;
        for col in 0..6 {
            for row in 0..6 {
                r.0[col][row] *= s;
            }
        }
        r
    }
}

// ── SXform ───────────────────────────────────────────────────────────

/// Spatial coordinate transform (parent → child).
///
/// `rot` (E): rotation matrix such that `v_child = E * v_parent`.
/// `pos` (r): child origin expressed in the parent frame.
#[derive(Debug, Clone, Copy)]
pub struct SXform {
    pub rot: DMat3,
    pub pos: DVec3,
}

impl SXform {
    pub fn new(rot: DMat3, pos: DVec3) -> Self {
        Self { rot, pos }
    }

    pub fn identity() -> Self {
        Self {
            rot: DMat3::IDENTITY,
            pos: DVec3::ZERO,
        }
    }

    /// Apply motion transform: `X * v`.
    ///
    /// ω' = E*ω, v' = E*(v − r×ω)
    pub fn apply_motion(&self, v: &SVec6) -> SVec6 {
        let omega = v.angular();
        let vel = v.linear();
        let new_ang = self.rot * omega;
        let new_lin = self.rot * (vel - self.pos.cross(omega));
        SVec6::new(new_ang, new_lin)
    }

    /// Apply force transform (child → parent): `X^T * f`.
    ///
    /// n' = E^T*n + r×(E^T*f), f' = E^T*f
    pub fn transpose_apply_force(&self, f: &SVec6) -> SVec6 {
        let n = f.angular();
        let fv = f.linear();
        let et_f = self.rot.transpose() * fv;
        let et_n = self.rot.transpose() * n;
        let new_ang = et_n + self.pos.cross(et_f);
        SVec6::new(new_ang, et_f)
    }

    /// Transform inertia to parent frame: `X^T * I * X`.
    pub fn transform_inertia_to_parent(&self, inertia: &SMat6) -> SMat6 {
        let x = self.as_mat6();
        let xt = x.transpose();
        xt.mul_mat(&inertia.mul_mat(&x))
    }

    /// Build the full 6×6 spatial transform matrix.
    ///
    /// `X = [E, 0; -E*skew(r), E]`
    pub fn as_mat6(&self) -> SMat6 {
        let e = self.rot;
        let neg_e_skew_r = -(e * skew(self.pos));
        let mut m = SMat6::ZERO;

        // Top-left 3×3: E
        set_block3(&mut m, 0, 0, &e);
        // Bottom-left 3×3: -E*skew(r)
        set_block3(&mut m, 3, 0, &neg_e_skew_r);
        // Bottom-right 3×3: E
        set_block3(&mut m, 3, 3, &e);
        // Top-right 3×3: 0 (already zero)

        m
    }
}

/// Write a glam DMat3 into a 3×3 block of an SMat6.
/// `row_off` / `col_off` are 0-based offsets into the 6×6 matrix.
fn set_block3(m: &mut SMat6, row_off: usize, col_off: usize, block: &DMat3) {
    // glam DMat3: col_major — block.col(c)[r]
    for c in 0..3 {
        let col = block.col(c);
        for r in 0..3 {
            m.0[col_off + c][row_off + r] = col[r];
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::FRAC_PI_2;

    const EPS: f64 = 1e-10;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < EPS
    }

    fn svec_approx_eq(a: &SVec6, b: &SVec6) -> bool {
        a.0.iter().zip(b.0.iter()).all(|(x, y)| approx_eq(*x, *y))
    }

    #[test]
    fn svec6_dot() {
        let a = SVec6([1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let b = SVec6([6.0, 5.0, 4.0, 3.0, 2.0, 1.0]);
        assert!(approx_eq(a.dot(&b), 56.0));
    }

    #[test]
    fn cross_motion_antisymmetric() {
        let a = SVec6([1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let b = SVec6([0.3, -0.7, 1.1, -0.5, 0.9, -1.3]);
        let ab = a.cross_motion(&b);
        let ba = b.cross_motion(&a);
        let sum = ab + ba;
        for i in 0..6 {
            assert!(
                approx_eq(sum.0[i], 0.0),
                "a×b + b×a should be zero, component {i}: {}",
                sum.0[i]
            );
        }
    }

    #[test]
    fn cross_force_is_neg_cross_motion_transpose() {
        // v.cross_force(f).dot(g) == -v.cross_motion(g).dot(f)
        let v = SVec6([1.0, 0.5, -0.3, 0.7, -1.2, 0.4]);
        let f = SVec6([0.2, -0.8, 1.5, -0.6, 0.3, 1.1]);
        let g = SVec6([-0.4, 0.9, -0.1, 1.3, -0.7, 0.5]);

        let lhs = v.cross_force(&f).dot(&g);
        let rhs = -v.cross_motion(&g).dot(&f);
        assert!(
            approx_eq(lhs, rhs),
            "crf relation failed: {lhs} vs {rhs}"
        );
    }

    #[test]
    fn smat6_identity_mul_vec() {
        let v = SVec6([1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let result = SMat6::identity().mul_vec(&v);
        assert!(svec_approx_eq(&result, &v));
    }

    #[test]
    fn sxform_identity_is_noop() {
        let v = SVec6([1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let x = SXform::identity();
        let result = x.apply_motion(&v);
        assert!(svec_approx_eq(&result, &v));
    }

    #[test]
    fn sxform_pure_rotation_transforms_angular() {
        // 90° about Z: x-axis angular → y-axis angular
        let rot = DMat3::from_rotation_z(FRAC_PI_2);
        let x = SXform::new(rot, DVec3::ZERO);
        let v = SVec6::new(DVec3::new(1.0, 0.0, 0.0), DVec3::ZERO);
        let result = x.apply_motion(&v);
        let expected = SVec6::new(DVec3::new(0.0, 1.0, 0.0), DVec3::ZERO);
        assert!(
            svec_approx_eq(&result, &expected),
            "got {:?}, expected {:?}",
            result,
            expected
        );
    }

    #[test]
    fn sxform_translation_creates_linear_from_angular() {
        // Child at [d,0,0] in parent frame, pure rotation about Z (ω=1).
        // v_child_linear = E*(v − r×ω) = I*(0 − [d,0,0]×[0,0,1]) = -[d,0,0]×[0,0,1]
        // [d,0,0]×[0,0,1] = [0*1−0*0, 0*0−d*1, d*0−0*0] = [0, −d, 0]
        // So linear = −[0,−d,0] = [0, d, 0]
        let d = 3.0;
        let x = SXform::new(DMat3::IDENTITY, DVec3::new(d, 0.0, 0.0));
        let v = SVec6::new(DVec3::new(0.0, 0.0, 1.0), DVec3::ZERO);
        let result = x.apply_motion(&v);
        assert!(approx_eq(result.0[3], 0.0), "vx = {}", result.0[3]);
        assert!(approx_eq(result.0[4], d), "vy = {} expected {d}", result.0[4]);
        assert!(approx_eq(result.0[5], 0.0), "vz = {}", result.0[5]);
    }

    #[test]
    fn sxform_force_roundtrip() {
        // Virtual work: v · (X^T * f) == (X * v) · f
        let rot = DMat3::from_rotation_y(0.7);
        let pos = DVec3::new(1.0, -0.5, 0.3);
        let x = SXform::new(rot, pos);

        let v = SVec6([1.0, 0.5, -0.3, 0.7, -1.2, 0.4]);
        let f = SVec6([0.2, -0.8, 1.5, -0.6, 0.3, 1.1]);

        let lhs = v.dot(&x.transpose_apply_force(&f));
        let rhs = x.apply_motion(&v).dot(&f);
        assert!(
            approx_eq(lhs, rhs),
            "virtual work failed: {lhs} vs {rhs}"
        );
    }

    #[test]
    fn smat6_solve() {
        let a = SMat6::from_body_inertia(DVec3::new(1.0, 2.0, 3.0), 5.0);
        let b = SVec6([1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let x = a.solve(&b);
        // Verify A*x = b
        let ax = a.mul_vec(&x);
        for i in 0..6 {
            assert!(
                (ax.0[i] - b.0[i]).abs() < 1e-8,
                "solve failed at index {i}: {} vs {}",
                ax.0[i],
                b.0[i]
            );
        }
    }

    #[test]
    fn inertia_parallel_axis_theorem() {
        // Point mass m at [d,0,0]. Body-frame inertia at CoM is zero (point mass).
        // After transforming to parent via X with r=[d,0,0], E=I:
        // Expected: Iyy = Izz = m*d², Ixx = 0
        let m = 2.5;
        let d = 3.0;
        let i_body = SMat6::from_body_inertia(DVec3::ZERO, m);

        // Transform puts child CoM at [d,0,0] in parent.
        let x = SXform::new(DMat3::IDENTITY, DVec3::new(d, 0.0, 0.0));
        let i_parent = x.transform_inertia_to_parent(&i_body);

        let md2 = m * d * d;
        // Ixx = 0 (rotation about the axis through which the mass is displaced)
        assert!(approx_eq(i_parent.0[0][0], 0.0), "Ixx = {}", i_parent.0[0][0]);
        // Iyy = m*d²
        assert!(
            approx_eq(i_parent.0[1][1], md2),
            "Iyy = {} expected {md2}",
            i_parent.0[1][1]
        );
        // Izz = m*d²
        assert!(
            approx_eq(i_parent.0[2][2], md2),
            "Izz = {} expected {md2}",
            i_parent.0[2][2]
        );
        // Mass block unchanged
        assert!(approx_eq(i_parent.0[3][3], m));
        assert!(approx_eq(i_parent.0[4][4], m));
        assert!(approx_eq(i_parent.0[5][5], m));
    }
}
