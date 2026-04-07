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
    /// Also zeroes multi-body creatures whose joints are *all* Rigid: they
    /// have no DOFs and no brain-drivable actuation, so any fitness they
    /// earn comes from the physics solver shoving the welded assembly
    /// around — the "rigid-skate" exploit. See `docs/shifty-movement.html`.
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
    /// Seconds of "settle" simulation before fitness measurement begins. On
    /// Land, creatures spawn at y=2.0 and fall; without a settle period the
    /// horizontal tumble during/after landing counts toward fitness, giving
    /// every creature a free ~1 m baseline regardless of whether its brain
    /// actually drove any motion. With a settle period, the position at the
    /// end of the settle phase is used as the reference for distance and
    /// max-displacement, so passive free-fall drift is excluded from score.
    ///
    /// Sims 1994: no settle period — creatures in water are neutrally
    /// buoyant and don't fall into the simulation.
    /// Our variant: ~1 second of settle on Land so free-fall drift doesn't
    /// pollute the locomotion score. Set to `None` to disable (paper-faithful
    /// for water, but Land evolutions will have a ~1-point passive baseline).
    /// Default: 1.0 s.
    #[serde(default = "default_settle_duration")]
    pub settle_duration: Option<f64>,
    /// Maximum plausible per-DOF joint angular velocity, in rad/s. A creature
    /// is rejected (fitness=0) if ANY joint DOF exceeds this rate at any frame
    /// during post-settle evaluation.
    ///
    /// Sims 1994: no joint-velocity rejection — joint speeds emerge from
    /// actuator torques, damping, and inertia.
    /// Our variant: Rapier's PGS solver + high-torque effectors allow joints
    /// to oscillate at 15-20+ rad/s, producing nearly invisible limb motion
    /// that exploits ground friction for "sliding" locomotion (creature
    /// 1827790 / evo 21 pattern). Capping joint angular velocity forces
    /// evolution to use visible, biologically-plausible gaits.
    /// Default: 12 rad/s (≈ 2 rev/s) — allows vigorous flapping and fast
    /// gaits while rejecting supersonic-flipper exploits.
    /// Set to `None` to disable (paper-faithful).
    #[serde(default = "default_max_joint_angular_velocity")]
    pub max_joint_angular_velocity: Option<f64>,
    /// Number of shared broadcast signal channels for inter-body neural
    /// communication. Each creature gets this many shared float channels
    /// that any body-part brain can read from (`NeuronInput::Signal`) or
    /// write to (`SignalEffectorNode`).
    ///
    /// Sims 1994: no inter-body signals — each body part's brain was local.
    /// Our variant: broadcast channels enable coordinated multi-segment gaits
    /// (e.g., centipede fin-beat synchronization). Set to 0 to disable
    /// (paper-faithful).
    /// Default: 4.
    #[serde(default = "default_num_signal_channels")]
    pub num_signal_channels: usize,
    /// Simulation frames between growth events during developmental growth.
    /// When set, creatures start with only the root body segment and grow
    /// one additional segment every `growth_interval` frames, following the
    /// BFS expansion order from the genome graph.
    ///
    /// Sims 1994: creatures instantiated fully-formed at t=0.
    /// Our variant: gradual growth gives complex body plans (7+ parts) time
    /// to develop coordination before all segments are active. Set to `None`
    /// to reproduce the paper behavior (instant full development).
    /// Default: `None` (paper-faithful, instant development).
    #[serde(default)]
    pub growth_interval: Option<usize>,
}

fn default_gravity() -> f64 { 9.81 }
fn default_viscosity() -> f64 { 2.0 }
fn default_max_body_angular_velocity() -> Option<f64> { Some(20.0) }
fn default_num_islands() -> usize { 1 }
fn default_migration_interval() -> usize { 20 }
fn default_min_joint_motion() -> Option<f64> { Some(0.15) }
fn default_max_joint_angular_velocity() -> Option<f64> { Some(12.0) }
fn default_settle_duration() -> Option<f64> { Some(1.0) }
fn default_num_signal_channels() -> usize { 4 }

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
            settle_duration: Some(1.0),
            num_signal_channels: 4,
            growth_interval: None,
            max_joint_angular_velocity: Some(12.0),
        }
    }
}

/// Evaluate fitness based on the configured goal and environment.
pub fn evaluate_fitness(genome: &GenomeGraph, params: &EvolutionParams) -> FitnessResult {
    let mut creature = match params.growth_interval {
        Some(interval) if interval > 0 => {
            Creature::from_genome_with_growth(
                genome.clone(),
                params.num_signal_channels,
                interval,
            )
        }
        _ => Creature::from_genome_with_signals(genome.clone(), params.num_signal_channels),
    };

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
    // Settle phase: run physics but don't accumulate fitness signals. The
    // `settle_duration` is the *minimum* settle time; we additionally extend
    // the settle dynamically until the root body's speed has been below
    // SETTLE_SPEED_THRESHOLD for SETTLE_STABLE_FRAMES consecutive frames, so
    // creatures that balance-then-topple don't leak their topple into eval.
    // Capped at `max_settle_steps` to preserve evaluation budget.
    // (See `settle_duration` doc on EvolutionParams.)
    const SETTLE_SPEED_THRESHOLD: f64 = 0.05;  // m/s
    const SETTLE_STABLE_FRAMES: usize = 15;     // 0.25 s at 60 Hz
    let min_settle_steps: usize = params
        .settle_duration
        .map(|s| (s / dt).round() as usize)
        .unwrap_or(0);
    // Cap settle at 50% of total sim so evaluation window is always ≥ 50%.
    let max_settle_steps: usize = (total_steps / 2).max(min_settle_steps);
    let spawn_pos = creature.world.transforms[creature.world.root].translation;
    // `initial_pos` is overwritten at the end of the settle phase to become
    // the reference point for distance + max_displacement. Post-settle we
    // measure the creature's locomotion, not its free-fall drift.
    let mut initial_pos = spawn_pos;
    let mut max_displacement: f64 = 0.0;
    let mut prev_pos = initial_pos;
    // Dynamic settle tracking.
    let mut stable_count: usize = 0;
    let mut actual_settle_steps: usize = 0;
    let settle_enabled = params.settle_duration.is_some();
    // early_check measured in post-settle frames; if settle disabled, use 2s from spawn.
    let mut early_check_step: usize = if settle_enabled { 0 } else { (2.0 / dt).round() as usize };

    // Capture initial body rotations for per-frame angular velocity tracking.
    // (See `max_body_angular_velocity` doc on EvolutionParams.)
    let mut prev_rotations: Vec<glam::DQuat> = creature
        .world
        .transforms
        .iter()
        .map(|t| glam::DQuat::from_mat3(&t.matrix3))
        .collect();
    // Per-body positions for per-body speed check. A physics blowup can
    // launch a single limb at thousands of m/s while the root body stays
    // bounded — without this check the creature scores whatever the root
    // happens to be doing, and evolution learns to trigger those blowups
    // because the explosion also yanks the root around to its advantage.
    let mut prev_body_positions: Vec<DVec3> = creature
        .world
        .transforms
        .iter()
        .map(|t| t.translation)
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
    // Unwrapped previous joint-angle sample (per DOF). The reported angle
    // lives in [-π, π] and wraps at the boundary, but physical joint
    // rotation is continuous. We unwrap the signal against the previous
    // sample before feeding Welford, so a joint that physically rotates
    // through ±π doesn't register a 2π variance spike that would pass a
    // truly pinned joint through the min_joint_motion check.
    let mut prev_joint_angle: Vec<f64> = vec![0.0; dof_index.len()];
    let mut have_prev: Vec<bool> = vec![false; dof_index.len()];

    for step in 0..total_steps {
        creature.step(dt);
        let pos = creature.world.transforms[creature.world.root].translation;

        // If bodies were added during growth, extend tracking arrays so
        // per-body speed and angular-velocity checks don't index OOB.
        while prev_body_positions.len() < creature.world.transforms.len() {
            let idx = prev_body_positions.len();
            prev_body_positions.push(creature.world.transforms[idx].translation);
            prev_rotations.push(glam::DQuat::from_mat3(
                &creature.world.transforms[idx].matrix3,
            ));
        }

        // Dynamic settle: if enabled, monitor root speed. When we've had
        // SETTLE_STABLE_FRAMES frames in a row with speed below threshold
        // (and we've passed min_settle_steps), end the settle phase and
        // rebase. Also end settle if we've hit max_settle_steps.
        if settle_enabled && actual_settle_steps == 0 {
            let frame_speed = (pos - prev_pos).length() / dt;
            if frame_speed < SETTLE_SPEED_THRESHOLD {
                stable_count += 1;
            } else {
                stable_count = 0;
            }
            let min_reached = step + 1 >= min_settle_steps;
            let settled = stable_count >= SETTLE_STABLE_FRAMES && min_reached;
            let capped = step + 1 >= max_settle_steps;
            if settled || capped {
                actual_settle_steps = step + 1;
                initial_pos = pos;
                max_displacement = 0.0;
                early_check_step = actual_settle_steps + (2.0 / dt).round() as usize;
                // Reset brain time so oscillators start their cycles at t=0
                // regardless of how long settle took. Without this, the same
                // creature produces different gait phases under different
                // settle durations, which breaks reproducibility and biases
                // evolution toward gaits that happen to land well at the
                // current settle timing.
                creature.brain.reset_time();
            }
        }

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

        // Per-body speed check: any body moving faster than the speed cap is
        // the signature of a constraint-solver explosion. Catches creatures
        // whose limb fires off at thousands of m/s while root stays tame.
        for (i, t) in creature.world.transforms.iter().enumerate() {
            let body_pos = t.translation;
            if !body_pos.is_finite() {
                return FitnessResult { score: 0.0, distance: 0.0, max_displacement: 0.0, terminated_early: true };
            }
            let body_speed = (body_pos - prev_body_positions[i]).length() / dt;
            if body_speed > MAX_PLAUSIBLE_SPEED {
                return FitnessResult { score: 0.0, distance: 0.0, max_displacement: 0.0, terminated_early: true };
            }
            prev_body_positions[i] = body_pos;
        }

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

        // Per-DOF joint angular velocity check (configurable, non-paper).
        // Reject creatures with joints spinning faster than the cap — the
        // creature 1827790 "invisible flipper" exploit pattern. We skip
        // this during settle because landing impacts can cause transient
        // high joint velocities that aren't representative of the gait.
        if settle_enabled && actual_settle_steps == 0 {
            // Still in settle — skip joint velocity AND Welford checks.
            continue;
        }
        if let Some(max_jvel) = params.max_joint_angular_velocity {
            for joint in creature.world.joints.iter() {
                for dof in 0..joint.joint_type.dof_count() {
                    let jvel = joint.velocities[dof].abs();
                    if jvel > max_jvel {
                        return FitnessResult {
                            score: 0.0,
                            distance: 0.0,
                            max_displacement: 0.0,
                            terminated_early: true,
                        };
                    }
                }
            }
        }

        // Also skip the very frame where settle ends (step+1 == actual_settle_steps)
        // because we just reset initial_pos above; joint sampling starts next frame.

        // Update in-window joint-angle Welford accumulators, unwrapping
        // against the previous sample so ±π boundary crossings don't
        // spoof huge variance.
        for (i, &(ji, d)) in dof_index.iter().enumerate() {
            let raw = creature.world.joints[ji].angles[d];
            let unwrapped = if have_prev[i] {
                let mut candidate = raw;
                let prev = prev_joint_angle[i];
                while candidate - prev >  std::f64::consts::PI { candidate -= 2.0 * std::f64::consts::PI; }
                while candidate - prev < -std::f64::consts::PI { candidate += 2.0 * std::f64::consts::PI; }
                candidate
            } else {
                have_prev[i] = true;
                raw
            };
            prev_joint_angle[i] = unwrapped;
            let (count, mean, m2) = &mut window_stats[i];
            *count += 1;
            let delta = unwrapped - *mean;
            *mean += delta / (*count as f64);
            let delta2 = unwrapped - *mean;
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
    // `initial_pos` is either the spawn (if no settle) or post-settle.
    let distance = (final_pos - initial_pos).length();
    let horizontal_distance = match params.environment {
        Environment::Land => {
            let diff = final_pos - initial_pos;
            DVec3::new(diff.x, 0.0, diff.z).length()
        }
        Environment::Water => distance,
    };

    // Joint-motion coefficient: mean of per-DOF min-window-stddevs, normalized.
    //
    // We distinguish three "no DOFs to measure" cases:
    //
    //   1. feature disabled (None)                → 1.0   (paper-faithful)
    //   2. single-body creature (num_bodies == 1) → 1.0   (truly cannot exploit
    //                                                      joints; legitimate)
    //   3. multi-body with ALL-Rigid joints       → 0.0   (rigid-welded assembly,
    //                                                      any motion comes from
    //                                                      the physics solver
    //                                                      shoving it around —
    //                                                      the creature 900423 /
    //                                                      evo 48 "shifty skate"
    //                                                      exploit)
    //
    // Previously cases 2 and 3 were conflated behind `dof_index.is_empty()`,
    // which let every rigid-welded assembly through with coefficient 1.0.
    let num_bodies = creature.world.bodies.len();
    let motion_coef = match (params.min_joint_motion, num_bodies == 1, dof_index.is_empty()) {
        (None, _, _) => 1.0,
        (_, true, _) => 1.0,
        (Some(_), false, true) => 0.0,
        (Some(threshold), false, false) => {
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
                signal_effectors: Vec::new(),
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
                signal_effectors: Vec::new(),
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
                    brain: BrainGraph { neurons: Vec::new(), effectors: Vec::new(), signal_effectors: Vec::new() },
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
                        signal_effectors: Vec::new(),
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
            global_brain: BrainGraph { neurons: Vec::new(), effectors: Vec::new(), signal_effectors: Vec::new() },
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
            settle_duration: Some(1.0),
            num_signal_channels: 0,
            growth_interval: None,
            max_joint_angular_velocity: None, // disable by default in existing tests
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
    fn settle_duration_kills_freefall_fitness() {
        // A single-body creature with no effectors spawned at y=2.0 on Land
        // falls, tumbles on landing, and ends up ~1 m from spawn — enough
        // to score ~1.27 fitness purely from passive physics. With a 1-second
        // settle period, the post-settle position becomes the reference, so
        // the fall-and-tumble phase is excluded from score.
        let genome = GenomeGraph {
            nodes: vec![MorphNode {
                // Asymmetric dimensions → tumbles on impact.
                dimensions: DVec3::new(0.5, 0.2, 0.3),
                joint_type: JointType::Rigid,
                joint_limit_min: [-1.0; 3],
                joint_limit_max: [1.0; 3],
                recursive_limit: 1,
                terminal_only: false,
                brain: BrainGraph { neurons: Vec::new(), effectors: Vec::new(), signal_effectors: Vec::new() },
            }],
            connections: Vec::new(),
            root: 0,
            global_brain: BrainGraph { neurons: Vec::new(), effectors: Vec::new(), signal_effectors: Vec::new() },
        };

        let mut p = land_params_with_motion(Some(0.3));
        p.sim_duration = 4.0;
        p.settle_duration = None;
        let r_no_settle = evaluate_fitness(&genome, &p);

        p.settle_duration = Some(1.0);
        let r_settled = evaluate_fitness(&genome, &p);

        assert!(
            r_no_settle.score > 0.2,
            "without settle, free-fall should give ≥0.2 baseline fitness; got {}",
            r_no_settle.score
        );
        assert!(
            r_settled.score < r_no_settle.score * 0.3 + 0.05,
            "settle_duration should kill free-fall fitness; \
             no_settle={:.3} settled={:.3}",
            r_no_settle.score, r_settled.score
        );
    }

    #[test]
    fn motion_coef_zeroes_rigid_only_multi_body() {
        // A multi-body creature whose joints are *all* Rigid has no DOFs —
        // the brain has nothing to actuate, so any fitness it earns comes
        // from Rapier's contact solver shoving a welded assembly around
        // (the "rigid skate" exploit on creature 900423 / evolution 48).
        // With min_joint_motion enabled, this MUST score zero — the guard
        // clause used to let it through because `dof_index.is_empty()` is
        // true for both single-body creatures (legitimate exempt) and
        // rigid-welded assemblies (exploit). Those two cases should
        // now diverge.
        let rigid_node = || MorphNode {
            dimensions: DVec3::new(0.4, 0.4, 0.1), // flat plate, skating-friendly
            joint_type: JointType::Rigid,
            joint_limit_min: [-1.0; 3],
            joint_limit_max: [1.0; 3],
            recursive_limit: 1,
            terminal_only: false,
            brain: BrainGraph { neurons: Vec::new(), effectors: Vec::new(), signal_effectors: Vec::new() },
        };
        let genome = GenomeGraph {
            nodes: vec![rigid_node(), rigid_node()],
            connections: vec![MorphConn {
                source: 0,
                target: 1,
                parent_face: AttachFace::PosX,
                child_face: AttachFace::NegX,
                scale: 1.0,
                reflection: false,
            }],
            root: 0,
            global_brain: BrainGraph { neurons: Vec::new(), effectors: Vec::new(), signal_effectors: Vec::new() },
        };
        let r_on = evaluate_fitness(&genome, &land_params_with_motion(Some(0.3)));
        assert_eq!(
            r_on.score, 0.0,
            "multi-body all-Rigid creature must score 0 when min_joint_motion \
             is enabled (this is the creature 900423 / evo 48 exploit pattern)"
        );
        // And as a sanity check — with the feature disabled, the same creature
        // still gets whatever fitness the physics gives it (not our concern).
        let r_off = evaluate_fitness(&genome, &land_params_with_motion(None));
        assert!(r_off.score.is_finite() && r_off.score >= 0.0);
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
                brain: BrainGraph { neurons: Vec::new(), effectors: Vec::new(), signal_effectors: Vec::new() },
            }],
            connections: Vec::new(),
            root: 0,
            global_brain: BrainGraph { neurons: Vec::new(), effectors: Vec::new(), signal_effectors: Vec::new() },
        };
        let p_with = land_params_with_motion(Some(0.15));
        let p_without = land_params_with_motion(None);
        let r_with = evaluate_fitness(&genome, &p_with);
        let r_without = evaluate_fitness(&genome, &p_without);
        assert!((r_with.score - r_without.score).abs() < 1e-9);
    }

    #[test]
    fn evaluate_fitness_with_growth_does_not_panic() {
        // Regression test: growth adds bodies mid-simulation, which used to
        // cause an index-out-of-bounds panic in the per-body speed/angular-
        // velocity tracking arrays (prev_body_positions, prev_rotations).
        use rand::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        let mut params = land_params_with_motion(Some(0.15));
        params.growth_interval = Some(60);
        params.num_signal_channels = 4;
        params.sim_duration = 4.0;

        // Try multiple seeds to hit multi-body creatures that actually grow.
        for seed in 0..30u64 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let genome = GenomeGraph::random(&mut rng);
            let result = evaluate_fitness(&genome, &params);
            assert!(result.score.is_finite(), "seed {seed}: fitness must be finite");
        }
    }

    #[test]
    fn evaluate_fitness_swimming_with_growth_does_not_panic() {
        // Same regression test but for water/swimming — the config that
        // the Wave 3 evolutions actually use.
        use rand::SeedableRng;
        use rand_chacha::ChaCha8Rng;

        let params = EvolutionParams {
            population_size: 200,
            max_generations: 10,
            goal: FitnessGoal::SwimmingSpeed,
            environment: Environment::Water,
            sim_duration: 4.0,
            max_parts: 20,
            gravity: 9.81,
            water_viscosity: 2.0,
            max_body_angular_velocity: Some(20.0),
            num_islands: 1,
            migration_interval: 20,
            min_joint_motion: Some(0.15),
            settle_duration: Some(1.0),
            num_signal_channels: 4,
            growth_interval: Some(60),
            max_joint_angular_velocity: Some(12.0),
        };

        for seed in 0..30u64 {
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let genome = GenomeGraph::random(&mut rng);
            let result = evaluate_fitness(&genome, &params);
            assert!(result.score.is_finite(), "seed {seed}: fitness must be finite");
        }
    }

    // ── max_joint_angular_velocity tests ──────────────────────────────────

    #[test]
    fn fast_joint_gets_zero_fitness_with_joint_velocity_cap() {
        // A high-frequency oscillator drives a revolute joint fast enough to
        // exceed the 12 rad/s cap — the creature 1827790 "invisible flipper"
        // exploit pattern. With the cap enabled, fitness must be zero.
        let neurons = vec![BrainNode {
            func: NeuronFunc::OscillateWave,
            inputs: vec![
                // High frequency → drives joint fast enough to exceed cap
                (NeuronInput::Constant(8.0), 1.0),  // freq
                (NeuronInput::Constant(0.0), 1.0),   // phase
            ],
        }];
        let genome = two_body_genome(NeuronInput::Neuron(0), 1.0, neurons);

        let mut params = land_params_with_motion(None); // disable min_joint_motion
        params.max_joint_angular_velocity = Some(12.0);
        params.sim_duration = 4.0;
        let r_capped = evaluate_fitness(&genome, &params);

        params.max_joint_angular_velocity = None;
        let r_uncapped = evaluate_fitness(&genome, &params);

        assert_eq!(
            r_capped.score, 0.0,
            "creature with fast-oscillating joint should score 0 when \
             max_joint_angular_velocity is enabled"
        );
        assert!(
            r_capped.terminated_early,
            "fast joint should trigger early termination"
        );
        // Sanity: without cap, the creature gets nonzero fitness
        assert!(
            r_uncapped.score >= 0.0,
            "uncapped creature should have valid fitness"
        );
    }

    #[test]
    fn slow_joint_passes_joint_velocity_cap() {
        // A slow oscillator (freq=1.0) should stay well under 12 rad/s
        // and should NOT be penalized by the joint velocity cap.
        let neurons = vec![BrainNode {
            func: NeuronFunc::OscillateWave,
            inputs: vec![
                (NeuronInput::Constant(1.0), 1.0),  // low freq
                (NeuronInput::Constant(0.0), 1.0),
            ],
        }];
        let genome = two_body_genome(NeuronInput::Neuron(0), 1.0, neurons);

        let mut params = land_params_with_motion(None);
        params.max_joint_angular_velocity = Some(12.0);
        params.sim_duration = 4.0;
        let r_capped = evaluate_fitness(&genome, &params);

        params.max_joint_angular_velocity = None;
        let r_uncapped = evaluate_fitness(&genome, &params);

        // Scores should be identical — slow joint never triggers the cap
        assert!(
            (r_capped.score - r_uncapped.score).abs() < 1e-9,
            "slow joint should not be affected by velocity cap; \
             capped={:.4} uncapped={:.4}",
            r_capped.score, r_uncapped.score
        );
    }

    #[test]
    fn joint_velocity_cap_disabled_when_none() {
        // With max_joint_angular_velocity=None, even fast joints should
        // not be rejected.
        let neurons = vec![BrainNode {
            func: NeuronFunc::OscillateWave,
            inputs: vec![
                (NeuronInput::Constant(8.0), 1.0),
                (NeuronInput::Constant(0.0), 1.0),
            ],
        }];
        let genome = two_body_genome(NeuronInput::Neuron(0), 1.0, neurons);

        let mut params = land_params_with_motion(None);
        params.max_joint_angular_velocity = None;
        params.sim_duration = 4.0;
        let result = evaluate_fitness(&genome, &params);

        assert!(result.score.is_finite());
        assert!(!result.terminated_early);
    }
}
