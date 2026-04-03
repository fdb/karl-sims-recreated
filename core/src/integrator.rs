/// Joint state: flat vectors of angles and velocities for all DOFs.
#[derive(Clone)]
pub struct JointState {
    pub angles: Vec<f64>,
    pub velocities: Vec<f64>,
}

/// Derivative: d(angles)/dt = velocities, d(velocities)/dt = accelerations.
#[derive(Clone)]
pub struct JointDeriv {
    pub d_angles: Vec<f64>,
    pub d_velocities: Vec<f64>,
}

/// Configuration for the adaptive integrator.
pub struct IntegratorConfig {
    pub tolerance: f64,
    pub min_dt: f64,
    pub max_dt: f64,
    pub safety_factor: f64,
}

impl Default for IntegratorConfig {
    fn default() -> Self {
        Self {
            tolerance: 1e-4,
            min_dt: 1e-5,
            max_dt: 1.0 / 30.0,
            safety_factor: 0.84,
        }
    }
}

pub struct StepResult {
    pub state: JointState,
    pub dt_used: f64,
    pub dt_next: f64,
}

impl JointState {
    /// Advance state by deriv * dt: s + d*dt
    pub fn advance(&self, deriv: &JointDeriv, dt: f64) -> JointState {
        let angles = self
            .angles
            .iter()
            .zip(deriv.d_angles.iter())
            .map(|(s, d)| s + d * dt)
            .collect();
        let velocities = self
            .velocities
            .iter()
            .zip(deriv.d_velocities.iter())
            .map(|(s, d)| s + d * dt)
            .collect();
        JointState {
            angles,
            velocities,
        }
    }
}

/// Weighted sum of derivatives: sum_i(weights[i] * k[i]).
fn weighted_deriv(ks: &[JointDeriv], weights: &[f64]) -> JointDeriv {
    let n_angles = ks[0].d_angles.len();
    let n_vels = ks[0].d_velocities.len();
    let mut d_angles = vec![0.0; n_angles];
    let mut d_velocities = vec![0.0; n_vels];
    for (k, &w) in ks.iter().zip(weights.iter()) {
        if w == 0.0 {
            continue;
        }
        for (da, ka) in d_angles.iter_mut().zip(k.d_angles.iter()) {
            *da += w * ka;
        }
        for (dv, kv) in d_velocities.iter_mut().zip(k.d_velocities.iter()) {
            *dv += w * kv;
        }
    }
    JointDeriv {
        d_angles,
        d_velocities,
    }
}

/// Build a trial state: base + dt * weighted_sum(ks, weights).
/// `weights` may be shorter than `ks`; trailing stages are ignored.
fn trial_state(base: &JointState, ks: &[JointDeriv], weights: &[f64], dt: f64) -> JointState {
    let wd = weighted_deriv(&ks[..weights.len()], weights);
    base.advance(&wd, dt)
}

// ── RKF45 Butcher tableau ──────────────────────────────────────────────────

// b coefficients (weights for previous stages in each row)
const B21: f64 = 1.0 / 4.0;

const B31: f64 = 3.0 / 32.0;
const B32: f64 = 9.0 / 32.0;

const B41: f64 = 1932.0 / 2197.0;
const B42: f64 = -7200.0 / 2197.0;
const B43: f64 = 7296.0 / 2197.0;

const B51: f64 = 439.0 / 216.0;
const B52: f64 = -8.0;
const B53: f64 = 3680.0 / 513.0;
const B54: f64 = -845.0 / 4104.0;

const B61: f64 = -8.0 / 27.0;
const B62: f64 = 2.0;
const B63: f64 = -3544.0 / 2565.0;
const B64: f64 = 1859.0 / 4104.0;
const B65: f64 = -11.0 / 40.0;

// 4th order weights
const C4: [f64; 6] = [25.0 / 216.0, 0.0, 1408.0 / 2565.0, 2197.0 / 4104.0, -1.0 / 5.0, 0.0];

// 5th order weights
const C5: [f64; 6] = [
    16.0 / 135.0,
    0.0,
    6656.0 / 12825.0,
    28561.0 / 56430.0,
    -9.0 / 50.0,
    2.0 / 55.0,
];

pub fn rk45_step<F>(
    state: &JointState,
    mut dt: f64,
    eval: &mut F,
    config: &IntegratorConfig,
) -> StepResult
where
    F: FnMut(&JointState) -> JointDeriv,
{
    dt = dt.clamp(config.min_dt, config.max_dt);
    let n = state.angles.len();

    for _attempt in 0..8 {
        // Stage 1
        let k1 = eval(state);

        // Stage 2
        let s2 = trial_state(state, &[k1.clone()], &[B21], dt);
        let k2 = eval(&s2);

        // Stage 3
        let s3 = trial_state(state, &[k1.clone(), k2.clone()], &[B31, B32], dt);
        let k3 = eval(&s3);

        // Stage 4
        let s4 = trial_state(
            state,
            &[k1.clone(), k2.clone(), k3.clone()],
            &[B41, B42, B43],
            dt,
        );
        let k4 = eval(&s4);

        // Stage 5
        let s5 = trial_state(
            state,
            &[k1.clone(), k2.clone(), k3.clone(), k4.clone()],
            &[B51, B52, B53, B54],
            dt,
        );
        let k5 = eval(&s5);

        // Stage 6
        let s6 = trial_state(
            state,
            &[k1.clone(), k2.clone(), k3.clone(), k4.clone(), k5.clone()],
            &[B61, B62, B63, B64, B65],
            dt,
        );
        let k6 = eval(&s6);

        let ks = [k1, k2, k3, k4, k5, k6];

        // Compute 4th and 5th order solutions
        let wd4 = weighted_deriv(&ks, &C4);
        let y4 = state.advance(&wd4, dt);
        let wd5 = weighted_deriv(&ks, &C5);
        let y5 = state.advance(&wd5, dt);

        // Error = max |y5 - y4| across all DOFs
        let mut error = 0.0_f64;
        for i in 0..n {
            error = error.max((y5.angles[i] - y4.angles[i]).abs());
            error = error.max((y5.velocities[i] - y4.velocities[i]).abs());
        }

        // If error is essentially zero, accept immediately
        if error < 1e-15 {
            return StepResult {
                state: y4,
                dt_used: dt,
                dt_next: dt,
            };
        }

        // Optimal step size
        let dt_optimal = config.safety_factor * dt * (config.tolerance / error).powf(0.2);
        let dt_optimal = dt_optimal.clamp(config.min_dt, config.max_dt);

        if error <= config.tolerance {
            return StepResult {
                state: y4,
                dt_used: dt,
                dt_next: dt_optimal,
            };
        }

        // Error too large — reduce dt and retry
        dt = dt_optimal;
    }

    // Fallback: Euler step with minimum dt
    let dt_min = config.min_dt;
    let deriv = eval(state);
    let fallback = state.advance(&deriv, dt_min);
    StepResult {
        state: fallback,
        dt_used: dt_min,
        dt_next: dt_min,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// x'' = -x  (harmonic oscillator)
    /// x(0) = 1, v(0) = 0  =>  x(t) = cos(t), v(t) = -sin(t)
    /// At t = pi: x = -1, v ~ 0
    #[test]
    fn harmonic_oscillator_accuracy() {
        let mut state = JointState {
            angles: vec![1.0],
            velocities: vec![0.0],
        };
        let config = IntegratorConfig::default();
        let target = std::f64::consts::PI;
        let mut t = 0.0;
        let mut dt = config.max_dt;

        while t < target {
            let remaining = target - t;
            let step_dt = dt.min(remaining);
            let result = rk45_step(
                &state,
                step_dt,
                &mut |s: &JointState| JointDeriv {
                    d_angles: s.velocities.clone(),
                    d_velocities: s.angles.iter().map(|&a| -a).collect(),
                },
                &config,
            );
            t += result.dt_used;
            dt = result.dt_next;
            state = result.state;
        }

        let x_err = (state.angles[0] - (-1.0)).abs();
        let v_err = state.velocities[0].abs();
        assert!(
            x_err < 0.01,
            "x error {x_err} should be < 0.01"
        );
        assert!(
            v_err < 0.01,
            "v error {v_err} should be < 0.01"
        );
    }

    /// x'' = -1000x is stiff. Starting with dt=0.1 should force adaptation.
    #[test]
    fn adaptive_step_reduces_dt_for_stiff_system() {
        let state = JointState {
            angles: vec![1.0],
            velocities: vec![0.0],
        };
        let config = IntegratorConfig {
            max_dt: 0.5, // allow larger max so 0.1 is accepted as initial
            ..IntegratorConfig::default()
        };
        let result = rk45_step(
            &state,
            0.1,
            &mut |s: &JointState| JointDeriv {
                d_angles: s.velocities.clone(),
                d_velocities: s.angles.iter().map(|&a| -1000.0 * a).collect(),
            },
            &config,
        );
        assert!(
            result.dt_next < 0.05,
            "dt_next {} should be much smaller than 0.1",
            result.dt_next
        );
    }
}
