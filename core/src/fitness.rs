use glam::DVec3;
use serde::{Deserialize, Serialize};

use crate::creature::Creature;
use crate::genotype::GenomeGraph;

// ---------------------------------------------------------------------------
// Unified evolution params (serialized to JSON in the DB)
// ---------------------------------------------------------------------------

/// The fitness goal to optimize for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FitnessGoal {
    SwimmingSpeed,
    LightFollowing,
}

/// Environment type — affects gravity, water drag.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Environment {
    Water,
    Land,
}

/// Complete evolution parameters — serialized to JSON in the DB.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionParams {
    pub population_size: usize,
    pub max_generations: usize,
    pub goal: FitnessGoal,
    pub environment: Environment,
    pub sim_duration: f64,
    pub max_parts: usize,
    /// Gravity strength in m/s² (only used for Land environment). Default: 9.81
    #[serde(default = "default_gravity")]
    pub gravity: f64,
    /// Water viscosity coefficient (only used for Water environment). Default: 2.0
    #[serde(default = "default_viscosity")]
    pub water_viscosity: f64,
    /// Number of islands for the islands-model genetic algorithm. Each
    /// island is an isolated sub-population with its own selection &
    /// reproduction; best individuals migrate between islands every
    /// `migration_interval` generations to keep gene flow alive.
    ///
    /// Sims 1994: single population.
    /// Our variant: multi-island model to maintain species diversity over
    /// long runs (see Whitley 1989, Cantú-Paz 2000).
    /// `population_size` is split evenly across islands — so `num_islands=3`
    /// with `population_size=150` gives 50 creatures per island.
    /// Default: 1 (single pool, paper-faithful).
    #[serde(default = "default_num_islands")]
    pub num_islands: usize,
    /// Generation interval at which the best creature of each island is
    /// migrated to the next island in a ring topology. Set to 0 to disable
    /// migration entirely (each island evolves in complete isolation).
    ///
    /// Sims 1994: n/a.
    /// Default: 20 generations.
    #[serde(default = "default_migration_interval")]
    pub migration_interval: usize,
    /// Maximum plausible per-body angular velocity, in rad/s. A creature is
    /// rejected (fitness=0) if ANY body exceeds this rate at any frame.
    ///
    /// Sims 1994: no angular-velocity rejection. Real physics has no such
    /// direct cap either — spin rates emerge from joint damping + actuator
    /// forces + external contacts.
    /// Our variant: the Rapier PGS solver allows contact-impulse exploits
    /// that spin small bodies at 50+ rad/s (8+ rev/s), which evolution
    /// latches onto for unphysical "wing-spinner" gaits. This cap rejects
    /// such creatures as a selection-pressure signal.
    /// Default: 20 rad/s (≈ 3.2 rev/s) — allows vigorous tumbling, rejects
    /// spinning. Set to `None` to disable (paper-faithful).
    #[serde(default = "default_max_body_angular_velocity")]
    pub max_body_angular_velocity: Option<f64>,
    /// Minimum joint-motion threshold (radian stddev) required for full fitness.
    /// The score is multiplied by `clamp(min_window_stddev / threshold, 0, 1)`, where
    /// `min_window_stddev` is the minimum over 2-second non-overlapping windows of
    /// the per-DOF mean angle stddev. A creature whose joints stay frozen for any
    /// 2-second stretch gets coefficient 0 and therefore zero fitness. We use the
    /// windowed minimum (not global stddev) because the exploit pattern — joint
    /// pinned at its limit for most of the sim, with brief transients at the start
    /// and end — produces a deceptively-healthy global stddev but a zero-stddev
    /// window in the middle. Closes the "ground-torque-against-a-joint-limit"
    /// exploit (see `docs/debugging-creature-physics.html`).
    ///
    /// Sims 1994: no such requirement — creatures' cyclic gaits were the only
    /// locomotion strategy available in his setup, so static-pose drift didn't emerge.
    /// Our variant: Rapier's PGS contact solver + joint-limit constraints can produce
    /// non-zero root drift from a DC effector bias, which evolution latches onto.
    /// Multiplying by joint-motion coefficient makes this strategy score zero.
    /// Set to `None` to disable (paper-faithful).
    /// Default: 0.15 rad — a sine-wave gait of amplitude ≥ 0.21 rad comfortably passes
    /// in every 2-second window (sine stddev = amplitude/√2 = 0.15), while a joint
    /// pinned at a limit for the whole window fails.
    #[serde(default = "default_min_joint_motion")]
    pub min_joint_motion: Option<f64>,
}

fn default_gravity() -> f64 { 9.81 }
fn default_viscosity() -> f64 { 2.0 }
fn default_max_body_angular_velocity() -> Option<f64> { Some(20.0) }
fn default_num_islands() -> usize { 1 }
fn default_migration_interval() -> usize { 20 }
fn default_min_joint_motion() -> Option<f64> { Some(0.15) }

impl Default for EvolutionParams {
    fn default() -> Self {
        Self {
            // Larger population (150) trades ~3× eval cost for substantially
            // more genetic diversity. With parallel workers and early
            // termination of non-moving creatures, wall-time per generation
            // stays low (~1 s on 8-core). Sims 1994 used 300+.
            population_size: 150,
            max_generations: 100,
            goal: FitnessGoal::SwimmingSpeed,
            environment: Environment::Water,
            sim_duration: 10.0,
            max_parts: 20,
            gravity: 9.81,
            water_viscosity: 2.0,
            max_body_angular_velocity: Some(20.0),
            num_islands: 1,
            migration_interval: 20,
            min_joint_motion: Some(0.3),
        }
    }
}

/// Evaluate fitness based on the configured goal and environment.
pub fn evaluate_fitness(genome: &GenomeGraph, params: &EvolutionParams) -> FitnessResult {
    let mut creature = Creature::from_genome(genome.clone());

    // Apply environment settings.
    match params.environment {
        Environment::Water => {
            creature.world.water_enabled = true;
            creature.world.water_viscosity = params.water_viscosity;
            creature.world.gravity = DVec3::ZERO;
        }
        Environment::Land => {
            creature.world.water_enabled = false;
            creature.world.gravity = DVec3::new(0.0, -params.gravity, 0.0);
            creature.world.ground_enabled = true;
            // Position root above ground
            creature.world.set_root_transform(
                glam::DAffine3::from_translation(DVec3::new(0.0, 2.0, 0.0)),
            );
            creature.world.forward_kinematics();
        }
    }

    // Viability check.
    if creature.world.bodies.len() > params.max_parts {
        return FitnessResult {
            score: 0.0,
            distance: 0.0,
            max_displacement: 0.0,
            terminated_early: true,
        };
    }

    match params.goal {
        FitnessGoal::SwimmingSpeed => evaluate_speed_fitness(&mut creature, params),
        FitnessGoal::LightFollowing => evaluate_following(genome, params),
    }
}

/// Maximum plausible displacement in one simulation — anything beyond this is a physics blowup.
const MAX_PLAUSIBLE_DISTANCE: f64 = 1000.0;

/// Maximum plausible root speed, m/s. Real small creatures max out around
/// 2–5 m/s (cockroach 1.5, mouse 4, lizard 6). We allow 8 m/s headroom for
/// initial falls (free-fall from spawn height 2m gives ~6 m/s terminal
/// before ground contact) but reject sustained faster-than-cheetah motion
/// from a sub-metre creature — that's always a solver exploit riding the
/// 20 m/s Rapier body-velocity clamp.
const MAX_PLAUSIBLE_SPEED: f64 = 8.0;

fn evaluate_speed_fitness(creature: &mut Creature, params: &EvolutionParams) -> FitnessResult {
    let dt = 1.0 / 60.0;
    let total_steps = (params.sim_duration / dt).round() as usize;
    let early_check_step = (2.0 / dt).round() as usize;
    let initial_pos = creature.world.transforms[creature.world.root].translation;
    let mut max_displacement: f64 = 0.0;
    let mut prev_pos = initial_pos;

    // Capture initial body rotations for per-frame angular velocity tracking.
    // (See `max_body_angular_velocity` doc on EvolutionParams.)
    let mut prev_rotations: Vec<glam::DQuat> = creature
        .world
        .transforms
        .iter()
        .map(|t| glam::DQuat::from_mat3(&t.matrix3))
        .collect();

    // Per-DOF windowed Welford accumulators for joint-angle stddev tracking.
    // (See `min_joint_motion` doc on EvolutionParams.) We split the sim into
    // fixed 2-second windows and track each window's stddev per DOF, then take
    // the minimum window's cross-DOF mean. Catches creatures that pin a joint
    // for any 2-second stretch — the 794863 exploit pattern.
    let dof_index: Vec<(usize, usize)> = creature
        .world
        .joints
        .iter()
        .enumerate()
        .flat_map(|(ji, j)| (0..j.joint_type.dof_count()).map(move |d| (ji, d)))
        .collect();
    const WINDOW_SECONDS: f64 = 2.0;
    let window_frames: usize = (WINDOW_SECONDS / dt).round() as usize;
    // min_window_mean_stddev[dof] = smallest per-window stddev observed so far
    let mut min_window_stddev: Vec<f64> = vec![f64::INFINITY; dof_index.len()];
    // Current in-progress window Welford state (count, mean, m2) per DOF.
    let mut window_stats: Vec<(u64, f64, f64)> = vec![(0, 0.0, 0.0); dof_index.len()];
    let mut frames_in_window: usize = 0;

    for step in 0..total_steps {
        creature.step(dt);
        let pos = creature.world.transforms[creature.world.root].translation;
        let disp = (pos - initial_pos).length();

        // Physics divergence check: NaN, implausible distance, or speed.
        let frame_speed = (pos - prev_pos).length() / dt;
        if !disp.is_finite() || disp > MAX_PLAUSIBLE_DISTANCE
            || !frame_speed.is_finite() || frame_speed > MAX_PLAUSIBLE_SPEED
        {
            return FitnessResult {
                score: 0.0,
                distance: 0.0,
                max_displacement: 0.0,
                terminated_early: true,
            };
        }
        prev_pos = pos;

        // Per-body angular velocity check (configurable, non-paper).
        if let Some(max_angvel) = params.max_body_angular_velocity {
            for (i, t) in creature.world.transforms.iter().enumerate() {
                let cur_q = glam::DQuat::from_mat3(&t.matrix3);
                // Relative rotation from prev → cur: q_rel = cur * prev^{-1}.
                let q_rel = cur_q * prev_rotations[i].inverse();
                // Extract rotation angle: θ = 2·acos(|w|), clamped for
                // numerical safety against slightly-out-of-range values.
                let w = q_rel.w.abs().clamp(-1.0, 1.0);
                let angle = 2.0 * w.acos();
                let angvel = angle / dt;
                if angvel > max_angvel {
                    return FitnessResult {
                        score: 0.0,
                        distance: 0.0,
                        max_displacement: 0.0,
                        terminated_early: true,
                    };
                }
                prev_rotations[i] = cur_q;
            }
        }

        // Update in-window joint-angle Welford accumulators.
        for (i, &(ji, d)) in dof_index.iter().enumerate() {
            let x = creature.world.joints[ji].angles[d];
            let (count, mean, m2) = &mut window_stats[i];
            *count += 1;
            let delta = x - *mean;
            *mean += delta / (*count as f64);
            let delta2 = x - *mean;
            *m2 += delta * delta2;
        }
        frames_in_window += 1;
        // At each window boundary: finalize this window's stddev and reset.
        if frames_in_window >= window_frames {
            for (i, stats) in window_stats.iter_mut().enumerate() {
                let (count, _mean, m2) = *stats;
                let sd = if count < 2 {
                    0.0
                } else {
                    (m2 / (count as f64 - 1.0)).sqrt()
                };
                if sd < min_window_stddev[i] {
                    min_window_stddev[i] = sd;
                }
                *stats = (0, 0.0, 0.0);
            }
            frames_in_window = 0;
        }

        max_displacement = max_displacement.max(disp);
        // Early-termination: if a creature has not reached 5 cm of peak
        // displacement from origin in the first 2 s, kill it. We test
        // max_displacement (not instantaneous disp) so a creature that
        // tumbles 5 cm away and returns still survives the check — the
        // point is to cull creatures that never move, not those whose
        // gait passes through the origin.
        if step + 1 == early_check_step && max_displacement < 0.05 {
            return FitnessResult {
                score: 0.0,
                distance: 0.0,
                max_displacement,
                terminated_early: true,
            };
        }
    }

    let final_pos = creature.world.transforms[creature.world.root].translation;
    let distance = (final_pos - initial_pos).length();
    let horizontal_distance = match params.environment {
        Environment::Land => {
            let diff = final_pos - initial_pos;
            DVec3::new(diff.x, 0.0, diff.z).length()
        }
        Environment::Water => distance,
    };

    // Joint-motion coefficient: mean of per-DOF min-window-stddevs, normalized.
    // A creature with no joints (one body) gets coefficient 1.0 — the penalty
    // only applies when there IS a brain→joint control path that could exploit.
    let motion_coef = match (params.min_joint_motion, dof_index.is_empty()) {
        (None, _) | (_, true) => 1.0,
        (Some(threshold), false) => {
            // If no window ever completed (sim shorter than WINDOW_SECONDS)
            // fall back to coefficient 1.0 — not enough data to penalize.
            if !min_window_stddev.iter().any(|sd| sd.is_finite()) {
                1.0
            } else {
                let mean_min_sd: f64 = min_window_stddev
                    .iter()
                    .map(|&sd| if sd.is_finite() { sd } else { 0.0 })
                    .sum::<f64>()
                    / min_window_stddev.len() as f64;
                (mean_min_sd / threshold).clamp(0.0, 1.0)
            }
        }
    };

    let base_score = horizontal_distance * 0.7 + max_displacement * 0.3;
    FitnessResult {
        score: base_score * motion_coef,
        distance: horizontal_distance,
        max_displacement,
        terminated_early: false,
    }
}

fn evaluate_following(genome: &GenomeGraph, params: &EvolutionParams) -> FitnessResult {
    let dt = 1.0 / 60.0;
    let light_positions = [
        DVec3::new(5.0, 0.0, 0.0),
        DVec3::new(-5.0, 0.0, 0.0),
        DVec3::new(0.0, 0.0, 5.0),
        DVec3::new(0.0, 0.0, -5.0),
    ];
    let num_trials = 4;
    let steps_per_trial = (params.sim_duration / dt) as usize;
    let reposition_steps = (5.0 / dt) as usize;

    let mut total_score = 0.0;

    for trial in 0..num_trials {
        let mut creature = Creature::from_genome(genome.clone());
        match params.environment {
            Environment::Water => {
                creature.world.water_enabled = true;
                creature.world.gravity = DVec3::ZERO;
            }
            Environment::Land => {
                creature.world.water_enabled = false;
                creature.world.gravity = DVec3::new(0.0, -params.gravity, 0.0);
                creature.world.ground_enabled = true;
                creature.world.set_root_transform(
                    glam::DAffine3::from_translation(DVec3::new(0.0, 2.0, 0.0)),
                );
                creature.world.forward_kinematics();
            }
        }

        creature.world.light_position = light_positions[trial];
        let mut prev_pos = creature.world.transforms[creature.world.root].translation;
        let mut speed_sum = 0.0;
        let mut samples = 0;

        for step in 0..steps_per_trial {
            if step > 0 && step % reposition_steps == 0 {
                let angle = (trial as f64 + step as f64 * 0.01) * 2.0;
                creature.world.light_position =
                    DVec3::new(5.0 * angle.cos(), 0.0, 5.0 * angle.sin());
            }
            creature.step(dt);
            let pos = creature.world.transforms[creature.world.root].translation;

            // Physics divergence check (position + speed).
            let disp = (pos - DVec3::ZERO).length();
            let movement = pos - prev_pos;
            let frame_speed = movement.length() / dt;
            if !disp.is_finite() || disp > MAX_PLAUSIBLE_DISTANCE
                || !frame_speed.is_finite() || frame_speed > MAX_PLAUSIBLE_SPEED
            {
                return FitnessResult {
                    score: 0.0,
                    distance: 0.0,
                    max_displacement: 0.0,
                    terminated_early: true,
                };
            }

            let to_light = (creature.world.light_position - pos).normalize_or_zero();
            let speed = movement.dot(to_light) / dt;
            if speed > 0.0 {
                speed_sum += speed;
            }
            samples += 1;
            prev_pos = pos;
        }

        total_score += if samples > 0 {
            speed_sum / samples as f64
        } else {
            0.0
        };
    }

    FitnessResult {
        score: total_score / num_trials as f64,
        distance: total_score / num_trials as f64,
        max_displacement: 0.0,
        terminated_early: false,
    }
}

// ---------------------------------------------------------------------------
// Legacy config (kept for backward compatibility with existing tests)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FitnessConfig {
    pub sim_duration: f64,
    pub dt: f64,
    pub max_parts: usize,
    pub early_termination_time: f64,
    pub min_movement: f64,
}

impl Default for FitnessConfig {
    fn default() -> Self {
        Self {
            sim_duration: 10.0,
            dt: 1.0 / 60.0,
            max_parts: 20,
            early_termination_time: 2.0,
            min_movement: 0.01,
        }
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

pub struct FitnessResult {
    pub score: f64,
    pub distance: f64,
    pub max_displacement: f64,
    pub terminated_early: bool,
}

// ---------------------------------------------------------------------------
// Evaluation
// ---------------------------------------------------------------------------

pub fn evaluate_swimming_fitness(
    genome: &GenomeGraph,
    config: &FitnessConfig,
) -> FitnessResult {
    let mut creature = Creature::from_genome(genome.clone());

    // Viability check: too many parts → zero fitness.
    if creature.world.bodies.len() > config.max_parts {
        return FitnessResult {
            score: 0.0,
            distance: 0.0,
            max_displacement: 0.0,
            terminated_early: false,
        };
    }

    let initial_pos = creature.world.transforms[creature.world.root].translation;
    let total_steps = (config.sim_duration / config.dt).round() as usize;
    let early_check_step = (config.early_termination_time / config.dt).round() as usize;

    let mut max_displacement: f64 = 0.0;
    let mut terminated_early = false;
    let mut prev_pos = initial_pos;

    for step in 0..total_steps {
        creature.step(config.dt);

        let current_pos = creature.world.transforms[creature.world.root].translation;
        let disp = (current_pos - initial_pos).length();
        let frame_speed = (current_pos - prev_pos).length() / config.dt;

        // Physics divergence check (position + speed).
        if !disp.is_finite() || disp > MAX_PLAUSIBLE_DISTANCE
            || !frame_speed.is_finite() || frame_speed > MAX_PLAUSIBLE_SPEED
        {
            return FitnessResult {
                score: 0.0,
                distance: 0.0,
                max_displacement: 0.0,
                terminated_early: true,
            };
        }
        prev_pos = current_pos;

        if disp > max_displacement {
            max_displacement = disp;
        }

        // Early termination check.
        if step + 1 == early_check_step && disp < config.min_movement {
            terminated_early = true;
            break;
        }
    }

    let final_pos = creature.world.transforms[creature.world.root].translation;
    let distance = (final_pos - initial_pos).length();

    let score = if terminated_early {
        0.0
    } else {
        distance * 0.7 + max_displacement * 0.3
    };

    FitnessResult {
        score,
        distance,
        max_displacement,
        terminated_early,
    }
}

// ---------------------------------------------------------------------------
// Following fitness
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FollowingFitnessConfig {
    pub trial_duration: f64,
    pub dt: f64,
    pub max_parts: usize,
    pub light_reposition_interval: f64,
    pub num_trials: usize,
}

impl Default for FollowingFitnessConfig {
    fn default() -> Self {
        Self {
            trial_duration: 10.0,
            dt: 1.0 / 60.0,
            max_parts: 20,
            light_reposition_interval: 5.0,
            num_trials: 4,
        }
    }
}

pub fn evaluate_following_fitness(
    genome: &GenomeGraph,
    config: &FollowingFitnessConfig,
) -> FitnessResult {
    let creature = Creature::from_genome(genome.clone());

    // Viability check: too many parts -> zero fitness.
    if creature.world.bodies.len() > config.max_parts {
        return FitnessResult {
            score: 0.0,
            distance: 0.0,
            max_displacement: 0.0,
            terminated_early: true,
        };
    }

    let light_positions = [
        DVec3::new(5.0, 0.0, 0.0),
        DVec3::new(-5.0, 0.0, 0.0),
        DVec3::new(0.0, 0.0, 5.0),
        DVec3::new(0.0, 0.0, -5.0),
    ];

    let mut total_score = 0.0;

    for trial in 0..config.num_trials.min(light_positions.len()) {
        let mut creature = Creature::from_genome(genome.clone());
        let steps_per_trial = (config.trial_duration / config.dt) as usize;
        let reposition_steps = (config.light_reposition_interval / config.dt) as usize;

        let mut speed_toward_light_sum = 0.0;
        let mut speed_samples = 0;

        creature.world.light_position = light_positions[trial];

        let mut prev_pos = creature.world.transforms[creature.world.root].translation;

        for step in 0..steps_per_trial {
            // Reposition light every N steps.
            if step > 0 && step % reposition_steps == 0 {
                let angle = (trial as f64 + step as f64 * 0.01) * 2.0;
                creature.world.light_position = DVec3::new(
                    5.0 * angle.cos(),
                    0.0,
                    5.0 * angle.sin(),
                );
            }

            creature.step(config.dt);

            let current_pos = creature.world.transforms[creature.world.root].translation;

            // Physics divergence check.
            let disp = current_pos.length();
            if !disp.is_finite() || disp > MAX_PLAUSIBLE_DISTANCE {
                return FitnessResult {
                    score: 0.0,
                    distance: 0.0,
                    max_displacement: 0.0,
                    terminated_early: true,
                };
            }

            let movement = current_pos - prev_pos;

            // Speed toward the light = projection of movement onto light direction.
            let to_light = (creature.world.light_position - current_pos).normalize_or_zero();
            let speed_toward = movement.dot(to_light) / config.dt;

            if speed_toward > 0.0 {
                speed_toward_light_sum += speed_toward;
            }
            speed_samples += 1;

            prev_pos = current_pos;
        }

        let avg_speed = if speed_samples > 0 {
            speed_toward_light_sum / speed_samples as f64
        } else {
            0.0
        };
        total_score += avg_speed;
    }

    let score = total_score / config.num_trials as f64;
    FitnessResult {
        score,
        distance: score,
        max_displacement: 0.0,
        terminated_early: false,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::genotype::{
        AttachFace, BrainGraph, GenomeGraph, MorphConn, MorphNode,
    };
    use crate::joint::JointType;
    use glam::DVec3;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    /// Build a genome that produces many parts via high recursive limits.
    fn large_genome() -> GenomeGraph {
        let mut nodes = Vec::new();
        // Root node
        nodes.push(MorphNode {
            dimensions: DVec3::new(0.3, 0.3, 0.3),
            joint_type: JointType::Rigid,
            joint_limit_min: [-1.0; 3],
            joint_limit_max: [1.0; 3],
            recursive_limit: 8,
            terminal_only: false,
            brain: BrainGraph {
                neurons: Vec::new(),
                effectors: Vec::new(),
            },
        });
        // Child node with high recursion
        nodes.push(MorphNode {
            dimensions: DVec3::new(0.2, 0.2, 0.2),
            joint_type: JointType::Revolute,
            joint_limit_min: [-1.0; 3],
            joint_limit_max: [1.0; 3],
            recursive_limit: 8,
            terminal_only: false,
            brain: BrainGraph::random_for_joint(
                &mut ChaCha8Rng::seed_from_u64(0),
                JointType::Revolute,
            ),
        });

        let connections = vec![
            MorphConn {
                source: 0,
                target: 1,
                parent_face: AttachFace::PosX,
                child_face: AttachFace::NegX,
                scale: 1.0,
                reflection: true,
            },
            // Self-referencing connection to drive recursion
            MorphConn {
                source: 1,
                target: 1,
                parent_face: AttachFace::PosX,
                child_face: AttachFace::NegX,
                scale: 0.9,
                reflection: false,
            },
        ];

        GenomeGraph {
            nodes,
            connections,
            root: 0,
            global_brain: BrainGraph {
                neurons: Vec::new(),
                effectors: Vec::new(),
            },
        }
    }

    #[test]
    fn large_creature_gets_zero_fitness() {
        let genome = large_genome();
        let config = FitnessConfig {
            max_parts: 5,
            ..Default::default()
        };
        let result = evaluate_swimming_fitness(&genome, &config);
        assert_eq!(result.score, 0.0, "creature with too many parts should get zero fitness");
    }

    #[test]
    fn fitness_evaluation_completes() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let genome = GenomeGraph::random(&mut rng);
        let config = FitnessConfig {
            sim_duration: 1.0, // short for test speed
            ..Default::default()
        };
        let result = evaluate_swimming_fitness(&genome, &config);
        // Should complete without panic; score is non-negative.
        assert!(result.score >= 0.0);
        assert!(result.distance >= 0.0);
        assert!(result.max_displacement >= 0.0);
    }

    #[test]
    fn fitness_is_deterministic() {
        let mut rng = ChaCha8Rng::seed_from_u64(77);
        let genome = GenomeGraph::random(&mut rng);
        let config = FitnessConfig {
            sim_duration: 1.0,
            ..Default::default()
        };

        let r1 = evaluate_swimming_fitness(&genome, &config);
        let r2 = evaluate_swimming_fitness(&genome, &config);

        assert_eq!(
            r1.score, r2.score,
            "same genome should produce identical fitness"
        );
        assert_eq!(r1.distance, r2.distance);
        assert_eq!(r1.max_displacement, r2.max_displacement);
        assert_eq!(r1.terminated_early, r2.terminated_early);
    }

    #[test]
    fn following_fitness_completes() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let genome = GenomeGraph::random(&mut rng);
        let config = FollowingFitnessConfig {
            trial_duration: 1.0,
            num_trials: 2,
            ..Default::default()
        };
        let result = evaluate_following_fitness(&genome, &config);
        assert!(result.score >= 0.0);
    }

    #[test]
    fn following_fitness_deterministic() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let genome = GenomeGraph::random(&mut rng);
        let config = FollowingFitnessConfig {
            trial_duration: 1.0,
            num_trials: 2,
            ..Default::default()
        };
        let r1 = evaluate_following_fitness(&genome, &config);
        let r2 = evaluate_following_fitness(&genome, &config);
        assert!((r1.score - r2.score).abs() < 1e-10);
    }

    #[test]
    fn photosensors_created_for_all_bodies() {
        use crate::phenotype::{develop, SensorType};

        for seed in 0..10u64 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let genome = GenomeGraph::random(&mut rng);
            let pheno = develop(&genome);
            let num_bodies = pheno.world.bodies.len();
            let num_photo = pheno
                .sensor_map
                .iter()
                .filter(|s| matches!(s.sensor_type, SensorType::PhotoSensor { .. }))
                .count();
            assert_eq!(
                num_photo,
                num_bodies * 3,
                "seed {seed}: {num_photo} photosensors for {num_bodies} bodies"
            );
        }
    }

    // ── joint-motion coefficient tests ──────────────────────────────────────
    //
    // These exercise the `min_joint_motion` exploit mitigation. We build a
    // minimal two-body, single-revolute-joint genome where the effector is
    // driven by either a constant signal (static exploit) or an OscillateWave
    // (real gait). The fitness score should reflect the motion coefficient.

    use crate::genotype::{
        BrainNode, EffectorNode, NeuronFunc, NeuronInput,
    };

    fn two_body_genome(
        effector_input: NeuronInput,
        effector_weight: f64,
        neurons: Vec<BrainNode>,
    ) -> GenomeGraph {
        GenomeGraph {
            nodes: vec![
                MorphNode {
                    dimensions: DVec3::new(0.3, 0.3, 0.3),
                    joint_type: JointType::Rigid,
                    joint_limit_min: [-1.0; 3],
                    joint_limit_max: [1.0; 3],
                    recursive_limit: 1,
                    terminal_only: false,
                    brain: BrainGraph { neurons: Vec::new(), effectors: Vec::new() },
                },
                MorphNode {
                    dimensions: DVec3::new(0.4, 0.3, 0.3),
                    joint_type: JointType::Revolute,
                    joint_limit_min: [-1.0; 3],
                    joint_limit_max: [1.0; 3],
                    recursive_limit: 1,
                    terminal_only: false,
                    brain: BrainGraph {
                        neurons,
                        effectors: vec![EffectorNode {
                            input: effector_input,
                            weight: effector_weight,
                        }],
                    },
                },
            ],
            connections: vec![MorphConn {
                source: 0,
                target: 1,
                parent_face: AttachFace::PosX,
                child_face: AttachFace::NegX,
                scale: 1.0,
                reflection: false,
            }],
            root: 0,
            global_brain: BrainGraph { neurons: Vec::new(), effectors: Vec::new() },
        }
    }

    fn land_params_with_motion(min_joint_motion: Option<f64>) -> EvolutionParams {
        EvolutionParams {
            population_size: 10,
            max_generations: 1,
            goal: FitnessGoal::SwimmingSpeed,
            environment: Environment::Land,
            sim_duration: 4.0,
            max_parts: 20,
            gravity: 9.81,
            water_viscosity: 2.0,
            max_body_angular_velocity: Some(20.0),
            num_islands: 1,
            migration_interval: 20,
            min_joint_motion,
        }
    }

    #[test]
    fn motion_coef_zeroes_out_frozen_joint_creature() {
        // A very slow OscillateWave (≈ 0.1 rad/s) with a small weight drives
        // the joint to its limit early and keeps it pinned for the whole sim
        // window — this is the 794863 exploit pattern exactly.
        let neurons = vec![BrainNode {
            func: NeuronFunc::OscillateWave,
            inputs: vec![
                (NeuronInput::Constant(0.1), 1.0), // freq (period ≈ 63 s)
                (NeuronInput::Constant(1.0), 1.0), // phase (non-zero → always pushing)
            ],
        }];
        let genome = two_body_genome(NeuronInput::Neuron(0), 0.2, neurons);

        // Compare enabled vs disabled. Even if the specific physics gives a
        // small intrinsic score, enabling the coefficient must shrink it.
        let r_on = evaluate_fitness(&genome, &land_params_with_motion(Some(0.15)));
        let r_off = evaluate_fitness(&genome, &land_params_with_motion(None));
        assert!(
            r_on.score <= r_off.score * 0.5 + 1e-6,
            "motion coefficient should penalize frozen-joint creature; \
             on={:.4} off={:.4}",
            r_on.score, r_off.score
        );
    }

    #[test]
    fn motion_coef_preserves_oscillating_creature_score() {
        // OscillateWave with freq 4.0 rad/s, phase 0 — a real ~0.64 Hz gait.
        // Effector weight 1.0 so it drives the joint through its full range.
        let neurons = vec![BrainNode {
            func: NeuronFunc::OscillateWave,
            inputs: vec![
                (NeuronInput::Constant(4.0), 1.0),
                (NeuronInput::Constant(0.0), 1.0),
            ],
        }];
        let genome = two_body_genome(NeuronInput::Neuron(0), 1.0, neurons);

        let params_with_coef = land_params_with_motion(Some(0.15));
        let params_without_coef = land_params_with_motion(None);

        let r_with = evaluate_fitness(&genome, &params_with_coef);
        let r_without = evaluate_fitness(&genome, &params_without_coef);

        // With the coefficient enabled, score should be ≥ ~80% of the unpenalized
        // score — a 4 rad/s sine with amplitude ~0.5 rad has stddev ≈ 0.35 ≥ 0.3.
        if r_without.score > 0.01 {
            let ratio = r_with.score / r_without.score;
            assert!(
                ratio > 0.8,
                "oscillating creature should keep most of its score; \
                 with_coef={:.4} without_coef={:.4} ratio={:.3}",
                r_with.score, r_without.score, ratio
            );
        }
    }

    #[test]
    fn motion_coef_disabled_when_none() {
        // With min_joint_motion=None we should get the same score as before
        // the feature existed, even for a frozen-joint creature.
        let genome = two_body_genome(NeuronInput::Constant(1.0), 1.0, Vec::new());
        let params = land_params_with_motion(None);
        let result = evaluate_fitness(&genome, &params);
        // Don't assert a specific value — just that it's a valid (possibly non-zero)
        // fitness. The point is the multiplier is bypassed.
        assert!(result.score.is_finite());
        assert!(result.score >= 0.0);
    }

    #[test]
    fn motion_coef_ignores_single_body_creature() {
        // A creature with one body and no joints should get coefficient 1 —
        // it has no brain→joint control path to exploit, so we shouldn't
        // penalize it. Its score should match the None case.
        let genome = GenomeGraph {
            nodes: vec![MorphNode {
                dimensions: DVec3::new(0.3, 0.3, 0.3),
                joint_type: JointType::Rigid,
                joint_limit_min: [-1.0; 3],
                joint_limit_max: [1.0; 3],
                recursive_limit: 1,
                terminal_only: false,
                brain: BrainGraph { neurons: Vec::new(), effectors: Vec::new() },
            }],
            connections: Vec::new(),
            root: 0,
            global_brain: BrainGraph { neurons: Vec::new(), effectors: Vec::new() },
        };
        let p_with = land_params_with_motion(Some(0.15));
        let p_without = land_params_with_motion(None);
        let r_with = evaluate_fitness(&genome, &p_with);
        let r_without = evaluate_fitness(&genome, &p_without);
        assert!((r_with.score - r_without.score).abs() < 1e-9);
    }
}
