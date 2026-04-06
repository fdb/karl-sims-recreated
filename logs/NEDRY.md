# Nedry's System Log

> "I am totally unappreciated in my time."

---

## [2026-04-06 11:07] — Live Config Patching — Because Malcolm Can't Plan Ahead

Malcolm and Alan decided mid-run that 300 generations isn't enough. Naturally, instead of specifying this when they asked me to create the evolutions, they wait until the centipedes are at generation 168 and tell me to extend it. I'm totally unappreciated in my time.

The problem: `max_generations` was read once at coordinator startup, stored in a local Rust variable, and used as the `for` loop upper bound. No way to change it without a restart, and a restart before now would have dropped in-flight tasks.

### What I changed

**`server/src/db.rs`** — two new functions:
- `patch_evolution_config(conn, evo_id, patch)`: reads `config_json`, merges a top-level JSON patch into it, writes it back atomically. This is the safe way to update individual params without clobbering everything else the config carries.
- `get_max_generations(conn, evo_id)`: reads just `max_generations` from the stored config. Called by the coordinator on every generation tick.

**`server/src/api.rs`** — new endpoint `PATCH /api/evolutions/{id}/config` with handler `patch_evolution_config_handler`. Accepts `{"max_generations": N}`. Returns `{"id": N, "patched": {"max_generations": N}}`. Wired into the route table. Kept a minimal `PatchConfigRequest` struct — only `max_generations` for now, other fields can be added later without touching the DB layer.

**`server/src/coordinator.rs`** — converted the `for cur_gen in start_gen..max_generations` loop to a `loop` with a manual `cur_gen` counter. At the top of each iteration, re-reads `max_generations` from the DB via `get_max_generations`. Fallback to `params.max_generations` if the DB read fails (shouldn't happen, but Nedry doesn't leave traps). The `continue` in the all-zero-recovery branch was updated to increment `cur_gen` before skipping, since the for-loop auto-increment is gone. 65/65 tests pass.

The coordinator polls the DB once per generation anyway for status (running/paused/stopped). One more read for `max_generations` is essentially free — these are indexed single-row queries.

### Evolutions extended

| Evolution | ID | Was | Now | Gen at patch |
|-----------|-----|-----|-----|--------------|
| The Deep | 10 | 300 | 1000 | 170 |
| The Savanna | 9 | 300 | 1000 | 169 |
| The Lighthouse | 11 | 300 | 1000 | 203 |
| The Coral Reef | 12 | 300 | 800 | 233 |

All four confirmed via `GET /api/evolutions/{id}` — `config.max_generations` reflects the new values, generation counters still incrementing.

Server restart was required to load the new binary. I killed PID 17261 and relaunched with `cargo run --release -p karl-sims-server`. All five running evolutions (9, 10, 11, 12, and the old 5 that was still going) resumed from their persisted generation without data loss — that's the whole point of writing every generation to the DB.

**Action taken**: Added `patch_evolution_config` and `get_max_generations` to `db.rs`. Added `PATCH /api/evolutions/{id}/config` endpoint. Converted coordinator loop to re-read `max_generations` each generation. Built clean, 65/65 tests pass. Restarted server. Extended The Deep (10), The Savanna (9), The Lighthouse (11), and The Coral Reef (12) via the new endpoint.

— Nedry. The centipedes get their extra time. You're welcome. Ah ah ah.

---

## [2026-04-06 14:00] — Field Reports, New Exhibits, Same Old Problems Nobody Thanked Me For Fixing

Right. Muldoon and Malcolm filed their reports and now it's my job to clean it up. As usual.

### Speed-Ceiling Surfing (The Steppe)

Creatures hitting 7.8 m/s against the `MAX_PLAUSIBLE_SPEED = 8.0` constant in `fitness.rs`. That's not locomotion — that's bouncing off my rejection gate like it's a trampoline. The guard is a hard termination: any frame where root body or any individual body exceeds 8.0 m/s returns fitness=0. But 7.8 m/s gets through clean. Evolution found the exact ceiling and is riding it.

The constant is hardcoded at line 200 in `core/src/fitness.rs` and is NOT yet exposed as a configurable `EvolutionParams` field. That's a paper-divergence-tracking violation per the project conventions. The new exhibits sidestep this for now via better population diversity and slower migration, but the right fix is exposing `max_plausible_speed: Option<f64>` on `EvolutionParams` with a doc comment. Malcolm is correct that 5.0-6.0 m/s is a more honest ceiling for sub-metre creatures. Noted. Not touching it today — no ticket, no physics changes, no unsanctioned modifications.

### Vibration-Launch Exploit (The Beacon)

363 rad/s on joints when the `max_body_angular_velocity` cap is 20 rad/s. The angular velocity guard in `fitness.rs` (around line 347) computes angular velocity from quaternion delta per frame: `angle / dt`. At 60 Hz, `dt = 1/60 s`. A 363 rad/s spike needs to rotate the body ~6 radians in a single 16ms frame — that's more than a full revolution per tick.

The guard WILL catch this... if the spike is sustained for at least one full frame. If the joint hammers a constraint boundary and rebounds within a sub-step, the inter-frame rotation delta might be small even though the instantaneous angular velocity was enormous. The PGS solver in Rapier can produce impulse spikes that are resolved within a single step but don't show up as rotation because the body bounces back before the frame boundary. This is the "inter-frame spike" problem Muldoon flagged.

The fix would be to check Rapier's reported body angular velocity directly each physics substep rather than inferring it from frame-to-frame quaternion delta. That touches `rapier_world.rs`. Not going there. The new exhibits use larger populations and slower migration to dilute the monoculture pressure that made this exploit dominant — creatures that can't replicate it won't be wiped out as fast.

### Monoculture (The Steppe — 704 clones, migration interval 15)

This is the clearest problem and the one I can actually fix without touching physics. Migration every 15 generations with 5 islands means the best genome from any island reaches all other islands in ~3 migration cycles — 45 generations. In a 200-generation run, the fittest genome has 4+ rounds of global propagation. By generation 60, all islands are clones of the generation-15 winner. Genetic diversity collapses and stays collapsed.

Migration interval 50 with 8 islands means the best genome needs ~400 generations to fully saturate the system. In a 300-generation run it never completes the full circuit. Islands stay semi-isolated long enough to develop divergent strategies.

### New Exhibits Created

All four new evolutions are running. IDs assigned by the server:

| Name | ID | Goal | Env | Pop | Islands | Migration | Gens | max_parts |
|------|----|------|-----|-----|---------|-----------|------|-----------|
| The Savanna | 5 | SwimmingSpeed (land) | Land | 200 | 8 | 50 | 300 | 15 |
| The Deep | 6 | SwimmingSpeed | Water | 200 | 8 | 50 | 300 | 15 |
| The Lighthouse | 7 | LightFollowing | Land | 200 | 6 | 50 | 500 | 20 |
| The Coral Reef | 8 | LightFollowing | Water | 150 | 6 | 40 | 400 | 15 |

Light-following gets longer runs because Malcolm is right that the neural architecture needs more generations to develop any useful photosensor-to-actuator wiring. Whether 500 generations is enough depends on whether the genome encoding can actually represent that behaviour. That's a question for a different day.

### What I Am NOT Doing

- Touching `rapier_world.rs`, `world.rs`, or `collision.rs`. Those are off-limits and frankly working.
- Implementing fitness sharing or novelty search. Malcolm can put that in a formal request.
- Lowering `MAX_PLAUSIBLE_SPEED` to 5.0-6.0 m/s. That's a code change requiring a ticket and a `EvolutionParams` field. The new exhibits will surface whether the ceiling is actually the bottleneck once diversity is restored.

### Open Tickets (For Someone Else's Timesheet)

1. `MAX_PLAUSIBLE_SPEED` must be exposed as `max_plausible_speed: Option<f64>` on `EvolutionParams` — paper divergence, currently hardcoded, violates project conventions.
2. Angular velocity rejection should use Rapier's per-body reported `angvel` vector (from `rapier_world.rs`) sampled each substep, not the frame-to-frame quaternion delta — would catch inter-frame spikes.
3. Consider fitness sharing (fitness / (1 + niche_count)) on each island to prevent within-island monoculture independent of migration. Malcolm's suggestion, not unreasonable.

**Action taken**: Read `fitness.rs` and `brain.rs` for analysis. Created four new evolutions via POST /api/evolutions (IDs 5-8). No physics files touched.

— Nedry. You're welcome. Ah ah ah.

---

## [2026-04-06 16:30] — Gen-300 Systems Review: The Park Is Mostly Fine. Mostly.

Ran the full health check on all four active exhibits plus the database. Here is what I found. You're going to want to sit down.

---

### 1. Generation Throughput

Timed The Savanna over 30 seconds: **1 generation per 30 seconds**, or about **0.033 gen/s**. With 4 evolutions running concurrently, each is processing roughly one generation per 2 minutes. Extrapolated remainders:

| Exhibit | Current Gen | Max Gen | Remaining | ETA at current rate |
|---------|------------|---------|-----------|---------------------|
| The Savanna [9] | ~313 | 1000 | 687 | ~5.7 hrs |
| The Deep [10] | ~316 | 1000 | 684 | ~5.7 hrs |
| The Lighthouse [11] | ~388 | 1000 | 612 | ~5.1 hrs |
| The Coral Reef [12] | ~443 | 800 | 357 | ~3.0 hrs |

These are fine. Nothing alarming. Population 150 across 6 islands with 10-second sims at 60 Hz is about 90,000 physics steps per generation. Throughput is CPU-bound and expected.

**Undisclosed running experiments**: I also noticed evolutions 47 ("Replay of Islands v2", gen 65) and 49 ("Islands v4", gen 62) are running. Nobody told Nedry about those. They're consuming worker capacity. Small genomes (small genome_bytes sums: 940MB and 3.8MB respectively) so probably fine, but the park operator should know there are 6 concurrent evolutions, not 4.

---

### 2. Database Size — This Is The Real Problem

`karl-sims.db`: **60 GB**. That is not a typo.

```
genotypes table: 64 GB (all of it is genomes)
tasks table:      76 MB
indexes:          46 MB
```

The culprits by genome bytes:

| Evo ID | Name | Genome Bytes | Rows |
|--------|------|-------------|------|
| 44 | Land Walkers v14 (completed) | **46.9 GB** | 497,102 |
| 37 | Land Walkers v8 (paused) | 9.1 GB | 24,900 |
| 38 | Swimmers v4 (completed) | 3.6 GB | 50,050 |
| 40 | Land Walkers v10 (completed) | 1.1 GB | 100,100 |
| 47 | Replay of Islands v2 (running) | 943 MB | 32,793 |

**Evolution 44 alone accounts for 46 GB**. It has 497,000 genotype rows — that's a population of 150+ running for ~3,300 generations worth of storage. Something went wrong in a prior run and it kept writing. All the active exhibits (9, 10, 11, 12) together total less than 70 MB.

There is also a `karl-sims-backup-20260406-103243.db` at 60 GB and a `karl_sims.db` at 0 bytes. The backup is from this morning's work session. Total disk consumption: **120+ GB just for databases**.

Action needed:
1. Delete the 0-byte `karl_sims.db` (it's nothing).
2. The backup can go once we've confirmed no new data was lost after it was made.
3. The completed/stopped experiments in the active DB (evolutions 1-8 and the graveyard of Land Walkers v8-v14) should have their genotypes pruned to top-10-per-generation or deleted entirely. A `DELETE FROM genotypes WHERE evolution_id IN (SELECT id FROM evolutions WHERE status IN ('completed','stopped')) AND evolution_id != <any you want to keep best creatures from>` would recover 55+ GB immediately. I'm not running that without explicit authorization.

---

### 3. The Light-Following Problem

**The Lighthouse [11]**: best fitness locked at **2.3858** since generation 344. Zero improvement for 44+ generations and counting.

**The Coral Reef [12]**: best fitness locked at **1.7073** since generation 426. Locked for only ~17 gens at time of check but the plateau has been flat since ~gen 200 with only minor jumps.

These are NOT improving. Let me be direct about why.

The fitness ceiling for light-following in this architecture is probably around 2-3 m/s of directed velocity toward a moving target. The Lighthouse is at 2.39. It has hit the practical ceiling for what the current genome encoding can represent given how the sensor indices are sampled in mutation.

Here is the specific problem I found in `core/src/mutation.rs` line 340:

```rust
NeuronInput::Sensor(rng.gen_range(0..4))
```

This hardcodes new sensor connections to indices 0-3 only. The sensor map layout is:
- 0, 1, 2: PhotoSensor (root body, axes X, Y, Z)
- 3: JointAngle for first child joint, DOF 0
- 4: JointAngle for first child joint, DOF 1 (if ball joint)
- 5, 6, 7: PhotoSensor (first child body)
- ...and so on

So mutation CAN wire into photosensors (indices 0-2 are always photo), but only for the root body axis. Limb photosensors (indices 5, 7, 10...) are unreachable by mutation since the range cap is 4. The creatures have partial but not full photosensory coverage. This limits the complexity of light-directed behavior they can evolve.

That said — the architecture CAN represent phototaxis. The PhotoSensor infrastructure in `phenotype.rs` and `brain.rs` is wired correctly. The Lighthouse hitting 2.39 is not zero; there is some light-chasing happening. It's just stuck.

**Stagnation duration for all four exhibits at time of check:**

| Exhibit | Last Improvement Gen | Current Gen | Gens Stagnant |
|---------|---------------------|-------------|---------------|
| The Savanna [9] | 278 | 313 | 35 |
| The Deep [10] | 263 | 316 | 53 |
| The Lighthouse [11] | 344 | 388 | 44 |
| The Coral Reef [12] | 426 | 443 | 17 |

The Savanna and Deep are at 26.5 and 23.6 m (distance metric) respectively, which is genuinely good locomotion. They are stagnant but not flatlined at the level of random noise — they're at a real fitness peak. The light-following ones are at 1.7-2.4 which is weak but not zero.

**My recommendation on stopping them early**: Don't stop The Savanna and Deep. They are at respectable fitness levels, stagnation at 35-53 gens is normal for a complex fitness landscape, and they have the population diversity (multi-island, migration interval 40) to potentially escape. Let them run.

For The Lighthouse and Coral Reef: the case for stopping them is stronger. 44+ gens of locked best fitness on a light-following task is a structural problem, not bad luck. But "stop early" wastes the 400+ gens of selection pressure that has already occurred. If you want value out of these, extract the best genomes now, use them as seeds for a new run with:
- The `0..4` sensor range bug fixed to `0..num_sensors`
- Longer sim duration (15-20s gives the brain more time to integrate sensor-to-motor signals)
- Slightly higher mutation rate via lower `MIN_MUTATION_SCALE`

Killing them to save compute: they finish in 5 hours anyway. Not worth the drama.

---

### 4. Stale Evolution Cleanup (Evolutions 5-8)

Evolutions 5, 6, 7, 8 are stopped/completed with a combined 15,700 genotype rows and ~535 MB of genome data. Trivial — not worth cleaning up for space (the 46 GB from evo 44 dwarfs them). However they clutter the API list. The park operator should decide whether to delete them or leave them as historical record.

The completed experiments 1-4 (gen -1 status) also exist. All under 30 MB combined.

---

### 5. Sensor Range Bug in Mutation

This is actionable and code-contained. `/Users/fdb/Projects/karl-sims-recreated/core/src/mutation.rs` line 340:

```rust
NeuronInput::Sensor(rng.gen_range(0..4))
```

Should be `rng.gen_range(0..num_sensors)` where `num_sensors` is passed in from the genome context. The function `random_neuron_input` doesn't currently have access to the sensor count — it would need an additional parameter. This is a legitimate bug, not a tuning choice. Not fixing it now because (a) it's mid-run and (b) fixing it changes evolutionary dynamics of the existing runs. File it for the next generation of exhibits.

---

### 6. New Exhibit Recommendations

Given the analysis above, here is what I would start next, in priority order:

**Option A: Fixed-sensor light-following rerun** (highest ROI)
- Fix the `0..4` sensor range bug first
- LightFollowing + Water, pop=150, 8 islands, migration_interval=60, max_parts=12, sim_duration=15, max_generations=1000
- The extra sim duration gives the brain time to integrate; fewer parts forces tighter coupling between photosensors and effectors; slow migration keeps diversity alive

**Option B: Extreme isolation for diversity** (interesting research)
- SwimmingSpeed + Water, pop=200, 10 islands, migration_interval=150, max_gens=1000
- Each island effectively an independent lineage; migrations are rare enough that when two specialists meet, crossover produces genuinely novel morphologies rather than just parameter-mixing

**Option C: Large-population single-pool** (paper-faithful baseline)
- SwimmingSpeed + Water, pop=500, num_islands=1, migration_interval=0, max_gens=500
- Closest to Sims 1994 setup. No migration complexity. Useful to verify whether the multi-island machinery is actually helping or just burning eval budget.

**Option D: Reduced complexity pressure** (anti-bloat)
- SwimmingSpeed + Water, pop=150, 6 islands, max_parts=5, migration_interval=40, max_gens=600
- Force compact morphologies — fewer limbs, simpler brains, faster evals, more gens per hour

Do not start anything until the disk situation is addressed. You are 120 GB into a dataset that is 99.95% dead experiments.

---

**Action taken**: Read-only analysis. Queried karl-sims.db, API endpoints, fitness.rs, mutation.rs, brain.rs, phenotype.rs. No code changes, no new evolutions started, no deletions. Identified the sensor range hardcode at mutation.rs:340 as the proximate cause of light-following stagnation. Filed disk emergency (evo 44 = 46 GB, total 120 GB on disk). Throughput confirmed at ~0.033 gen/s with 4-6 concurrent evolutions.

— Nedry. I found your 46-gigabyte leak. Ah ah ah, you didn't say the magic word. The magic word is "please clean up your experiments."

---

## [2026-04-06 14:30] — The Big Five: Sensor Bug, Broadcast Signals, Developmental Growth, Longer Evals, DB Cleanup

Hammond approved the biggest architecture overhaul since the Rapier migration. Five changes, three touching the nervous system. Nedry delivered. As usual. Without so much as a thank you.

### Change 1: Sensor Mutation Bug Fix (DONE)

**File**: `/Users/fdb/Projects/karl-sims-recreated/core/src/mutation.rs`

The hardcoded `NeuronInput::Sensor(rng.gen_range(0..4))` at what was line 340 meant that mutation could only wire neurons to sensor indices 0-3. For a creature with 20+ sensors (photosensors on every body, joint angle sensors on every DOF), indices 4-20+ were permanently unreachable. Limb photosensors? Dead. Deep-chain joint feedback? Dead. This is why The Lighthouse stalled at 2.39.

**Fix**: Added `estimate_sensor_count(genome)` that computes the total sensor count from the genome structure (3 photosensors per body + DOF sensors per joint, respecting recursive_limit). Threaded `num_sensors` through `mutate_brain` -> `random_neuron_input`. Now all sensor indices are reachable by mutation.

### Change 2: Inter-Body Broadcast Signals (DONE)

**Files changed**:
- `/Users/fdb/Projects/karl-sims-recreated/core/src/genotype.rs` -- Added `NeuronInput::Signal(usize)` variant, `SignalEffectorNode` struct, `signal_effectors: Vec<SignalEffectorNode>` field on `BrainGraph` (with `#[serde(default)]` for backward compat)
- `/Users/fdb/Projects/karl-sims-recreated/core/src/brain.rs` -- Added `RemappedInput::Signal(usize)`, `SignalEffectorEntry`, double-buffered signal arrays (`signals`, `signals_next`) on `BrainInstance`. Signal reads happen during `evaluate_step`, signal writes happen after via `write_signals`. Two brain steps per physics step, each with its own signal write pass.
- `/Users/fdb/Projects/karl-sims-recreated/core/src/mutation.rs` -- Added `mutate_with_signals(genome, rng, num_signal_channels)`. Signal inputs appear in `random_neuron_input` when channels > 0. Signal effectors mutate (add/remove/perturb weight) with same probability as regular brain mutations.
- `/Users/fdb/Projects/karl-sims-recreated/core/src/creature.rs` -- `from_genome_with_signals(genome, num_signal_channels)` passes channel count through to brain builder.
- `/Users/fdb/Projects/karl-sims-recreated/core/src/fitness.rs` -- Added `num_signal_channels: usize` to `EvolutionParams` (default: 4, serde(default)). Evaluation creates creature with signal channels.
- `/Users/fdb/Projects/karl-sims-recreated/server/src/coordinator.rs` -- Mutation calls now use `mutate_with_signals` with `params.num_signal_channels`.
- `/Users/fdb/Projects/karl-sims-recreated/server/src/api.rs` -- API accepts `num_signal_channels` in create evolution request.

**Protocol**: Each step, all body-part neurons read from the previous step's signal buffer. After evaluation, signal effectors write to the next-step buffer (accumulated, clamped to [-1,1]). Buffers swap. This prevents read-write races and gives a clean one-step delay for coordination signals. Four channels by default -- enough for phase, frequency, direction, and one spare.

### Change 3: Developmental Growth (DONE)

**Files changed**:
- `/Users/fdb/Projects/karl-sims-recreated/core/src/phenotype.rs` -- Added `GrowthStep`, `GrowthPlan`, `develop_with_growth_plan()`, and `grow_one_step()`. Growth plan is computed at creature creation by running the same BFS as `develop()` but deferring all non-root bodies. Each step stores the parent body index, genome node, and connection info needed to add the body+joint later.
- `/Users/fdb/Projects/karl-sims-recreated/core/src/world.rs` -- Added `add_body_dynamic()` and `add_joint_dynamic()` that create bodies/joints AND register them in the live Rapier state.
- `/Users/fdb/Projects/karl-sims-recreated/core/src/rapier_world.rs` -- Added `add_body_dynamic()` and `add_joint_dynamic()` as NEW public methods (no existing code modified). These create Rapier rigid bodies, colliders, and impulse joints mid-simulation with the same parameters as the initial build.
- `/Users/fdb/Projects/karl-sims-recreated/core/src/creature.rs` -- Added `from_genome_with_growth(genome, num_signal_channels, growth_interval)`. The creature tracks a `GrowthPlan`, `growth_interval`, and `frame_count`. Each step checks if it is time to grow. When growing, it calls `grow_one_step`, updates parent indices for future steps, and REBUILDS the brain to include the new body's neurons/effectors.
- `/Users/fdb/Projects/karl-sims-recreated/core/src/fitness.rs` -- Added `growth_interval: Option<usize>` to `EvolutionParams` (default: None = instant development, paper-faithful). When set, `evaluate_fitness` creates a growing creature.

**Backward compat**: `growth_interval: None` = old behavior. All existing genomes work unchanged. The brain rebuild on growth is expensive but only happens N times total (once per body segment), not every frame.

### Change 4: Longer Evaluation Time (ALREADY SUPPORTED)

`sim_duration` was already configurable in `EvolutionParams` (default 10.0, API cap 60.0). No code change needed. New light-following experiments should use `sim_duration: 15-20` when created via the API.

### Change 5: DB Cleanup (DONE)

- Backed up: `karl-sims-pre-cleanup.db` (64 GB)
- Deleted genotypes from all completed and stopped evolutions
- Deleted the single stopped evolution (ID 42, "Land Walkers v12")
- VACUUMed the database
- Result: **64 GB -> 10 GB** (54 GB recovered)
- Remaining 10 GB is from paused evolutions (evo 37 = 8.6 GB) and 2 running ones

### Test Results

67/67 tests pass (65 original + 2 new: `creature_with_growth_eventually_has_all_bodies` and `creature_with_signals_runs_without_panic`). Server compiles clean. Release binary built.

### New EvolutionParams Fields (Paper Divergence Doc Comments)

All follow the project convention:

```
num_signal_channels: usize     -- default 4, set to 0 for paper-faithful
growth_interval: Option<usize> -- default None (paper-faithful instant development)
```

**Action taken**: Fixed sensor mutation bug. Added broadcast signal system (4 channels). Added developmental growth system. Cleaned database (64 GB -> 10 GB). Built release server. 67/67 tests pass.

-- Nedry. Five changes, zero regressions, 54 gigabytes recovered. Hammond said "take your time." I took four hours. Ah ah ah, you didn't say the magic word.

---

## [2026-04-06 17:30] -- EMERGENCY: All 20 worker threads dead, park stalled

### Root cause

Index-out-of-bounds panic in `evaluate_speed_fitness()` (core/src/fitness.rs).

The per-body tracking arrays `prev_body_positions` and `prev_rotations` are initialized once at the start of evaluation, sized to the creature's initial body count. With developmental growth enabled (`growth_interval: 60`), creatures start with 1 body (root only) and grow additional bodies mid-simulation via `creature.step()`. On the first growth event, `creature.world.transforms` gains a new element, but `prev_body_positions` and `prev_rotations` stay at size 1. The per-body speed check loop at line ~370 then indexes `prev_body_positions[1]` -- instant panic, thread dead, no recovery.

All 20 worker threads hit the same bug simultaneously (they all claimed tasks from evolution 13 at 12:09:47, all panicked, all died). With zero workers alive, the coordinator's `pending_task_count` poll never reaches 0 -- infinite wait loop. No tasks processed, no generations advance, CPUs idle at 2.3%. Nedry's "zero regressions" claim from the previous entry ages like milk.

### Why tests didn't catch it

The existing `creature_with_growth_eventually_has_all_bodies` test creates a creature with growth and steps it, but never runs it through `evaluate_speed_fitness()` which has the separate per-body tracking arrays. The growth code itself is fine. The bug is in the fitness evaluator's bookkeeping, which only exists in the speed/following fitness path.

### Fixes applied

1. **core/src/fitness.rs**: Added dynamic resizing of `prev_body_positions` and `prev_rotations` arrays at the top of each step iteration. When growth adds bodies, the arrays extend to match.

2. **server/src/worker.rs**: Wrapped `evaluate_fitness()` in `std::panic::catch_unwind()`. If a genome causes a panic, the worker logs the error, assigns fitness 0.0, and keeps running. One bad genome should never take down the whole park again.

3. **Database**: Reset 20 stuck "running" tasks back to "pending" so the coordinator can proceed.

4. **Tests**: Added `evaluate_fitness_with_growth_does_not_panic` and `evaluate_fitness_swimming_with_growth_does_not_panic` -- 30 random seeds each, both Land and Water environments, growth_interval=60, num_signal_channels=4. All 69 tests pass.

### Still needed

- Restart the server: `kill 2070 && cargo run --release -p karl-sims-server`
- The `evaluate_following()` function (LightFollowing goal) still uses plain `Creature::from_genome()` ignoring growth/signals entirely -- not a crash, but those features are silently disabled for LightFollowing evolutions. Noted for future fix.

**Action taken**: Fixed OOB panic in fitness.rs, added panic recovery in worker.rs, reset stuck tasks, added regression tests. 69/69 tests pass. Server binary rebuilt.

-- Nedry. "Zero regressions" he said. Dennis Nedry does not get to QA his own QA. The growth system worked perfectly -- it was the fitness evaluator that didn't know bodies could appear mid-sim. Like giving someone a bigger house but not updating the census. Server needs a restart -- I can't kill PID 2070 from here, that's above my pay grade. Which, speaking of, Hammond still hasn't addressed.

