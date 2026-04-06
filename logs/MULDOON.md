# Muldoon's Security Reports

> "They should all be destroyed."

---

## [2026-04-06 10:00] — First Park-Wide Security Sweep: Four Evolutions Audited

**Threat level**: HIGH (two evolutions), LOW (two evolutions)

---

### The Steppe (Evolution 1) — "The Convergence Problem"

**Threat level**: HIGH

Island 4 was flagged as suspicious. The question was whether the top creatures evolved independently or whether a single genome had taken over. The answer is the latter — aggressively so.

Creature 36111 first appeared on island 4 at generation 112 with fitness 27.361314632987785. From that point onward, the creature's exact genome propagated via migration into all five islands over the next 53 generations, following the ring-topology migration schedule (interval=15). The clone count at time of inspection is **704 copies** out of a total population of 100 × 200 = 20,000 evaluations. After generation 200, the entire island ecosystem is effectively one creature: the final-generation average fitness is 27.26, within 0.4% of the best score — the population has collapsed to a monoculture.

The specimen itself is not an obvious physics cheat. The debug trace confirms:

- Root body Y stays between 0.35m and 2.0m throughout — no launch, no orbit. The creature bounces realistically.
- Displacement after 10 seconds: 28.09m. That is a sustained average ground speed of roughly 2.8 m/s — fast, but within biological plausibility for a small running creature.
- No NaN frames.

However, the exploit scanner flags all 10 top specimens: **FAST_PEAK_7.5m/s**. Full per-body breakdown from the physics trace:

```
body 0: peak 7.542 m/s
body 1: peak 7.461 m/s
body 2: peak 7.807 m/s  ← highest
body 3: peak 7.235 m/s
```

All four bodies are spending time close to the 8 m/s rejection threshold. Body 2 — a tiny revolute-jointed paddle (half-extents 0.048 × 0.217 × 0.151) — peaks at 7.807 m/s, 97.6% of the cap. This is the signature of a creature that has learned to "surf" the velocity ceiling: the physics solver caps body speed at 8 m/s per Rapier's internal linear clamp, and the creature is using that clamp as a free energy reservoir, bouncing against the wall of the cap every cycle rather than earning its speed through joint work alone.

Additionally, 85% of the 10-second simulation is spent near the speed floor, with brief high-speed bursts. This is the gait shape of a "contact-kick" exploit — the creature launches itself from the ground at just under the rejection threshold, glides forward during the airborne phase, and repeats. Evolution discovered that touching the 8 m/s ceiling without triggering the rejection condition is the global optimum.

The creature is not illegal by the current ruleset. It is exactly as clever as the ruleset allows. That is the problem.

**Specimens of interest**: 36111, 36421, 36761, 37101, 37361, 37701, 38121, 38621, 38861, 39042 (and 694 further clones)

**Exploit classification**: Speed-ceiling surfing / contact-kick launching. Secondary concern: genetic monoculture from aggressive migration in a well-converged population.

**Recommended countermeasures**:

1. **Lower `MAX_PLAUSIBLE_SPEED` from 8.0 to 5.0–6.0 m/s** in `core/src/fitness.rs`. Free-fall from spawn height 2m gives ~6.3 m/s at ground contact (√(2×9.81×2)); a creature that legitimately runs at 3–4 m/s average never needs to exceed ~5.5 m/s instantaneously. At 7.5 m/s we are allowing genuinely unphysical momentum. This constant should be exposed as an `EvolutionParams` config field per the paper-divergence convention.

2. **Add a sustained-speed guard**: reject creatures whose mean root speed over any 2-second window exceeds, say, 5.0 m/s. Peak speed can be legitimately transient; sustained speed above that ceiling cannot. This parallels the existing `min_joint_motion` windowed logic.

3. **Slow or cap migration** once a genome has been dominant on its home island for N consecutive generations. A migrant that has already saturated its origin island should not be injected into all others — that is the mechanism that produced 704 clones. Consider a "novelty filter" on migrants: only migrate a genome if its Hamming distance from the current best of the destination island exceeds a threshold.

---

### The Abyss (Evolution 2) — "Clean Water"

**Threat level**: LOW

The same structural pattern is present — island 3 champion (creature 44054, fitness 14.991715539318543) propagated to islands 0, 1, and 4 via migration, producing 142 clones by inspection. This is a population-diversity concern but not a physics exploit.

Exploit scanner returns **no anomalies** for any of the top 10. Metrics:

- Peak velocity: 5.66 m/s — comfortably below the 8 m/s cap, no surfing behavior.
- Y range: -4.30m to +0.11m — the creature dives and swims, behaving as a legitimate water creature.
- Joint actuation: 6.53 rad (measured over the run) — real movement, not a frozen-joint exploit.

This is a legitimately fast swimmer that won its competition honestly. The monoculture concern is administrative, not a physics violation. The creature earned its fitness; evolution just converged.

**Specimens of interest**: 44054 and 141 clones (islands 0, 1, 3, 4)

**Exploit classification**: None. Legitimate gait. Population monoculture only.

**Recommended countermeasures**: Same migration diversity suggestion as Steppe. No fitness code changes warranted.

---

### The Beacon (Evolution 3) — "Wing-Spinner At The Speed Wall"

**Threat level**: HIGH

The top 5 specimens (starting with 65878, generation 170, fitness 2.05) all carry two simultaneous flags from the exploit scanner: **FAST_PEAK_8.0m/s** and **JOINT_ANGVEL_363 rad/s**.

JOINT_ANGVEL_363 is severe. The `max_body_angular_velocity` param is set to 20.0 rad/s (roughly 3.2 revolutions per second — already generous). This creature's joints are spinning at 363 rad/s, more than 18× the configured rejection threshold... yet it is not being rejected.

The phenotype gives the immediate answer: the champion is a 3-body, 26-node genome expanded to 3 bodies (root: revolute; body 1: universal; body 2: rigid). The genome has 32 effectors across 26 nodes, but only 2 DOFs in the instantiated phenotype. The joint angular velocity check operates on the **phenotype's instantiated joints**, but the anomaly report of 363 rad/s must be computed differently from the in-sim angular velocity guard.

Two possibilities: the exploit scanner measures differently from the in-sim guard (e.g. finite-differencing positions vs. the per-frame rotation delta used in fitness.rs), or the angular velocity is briefly spiking between the fixed 60 Hz frames and the per-body rotation delta is undersampling it. Either way, the scanner is detecting a signal the fitness guard is missing. The creature is surviving the 20 rad/s `max_body_angular_velocity` check and scoring 2.05, which given the light-following task is not large in absolute terms but represents a top performer that may be stationary and spinning rather than following.

Peak linear speed of 7.98 m/s (essentially at the 8 m/s wall) combined with 363 rad/s joint angular velocity is the "vibration-launch" exploit class: a small joint spins against its limit, building up contact impulses each frame, and the accumulated energy periodically fires the body forward in a micro-launch. The creature did not learn to follow the light; it learned to vibrate.

**Specimens of interest**: 65878, 65941, 66021, 66101, 66181

**Exploit classification**: Vibration-launch / joint-limit contact-impulse accumulation. The creature is harvesting energy from the joint-limit constraint force.

**Recommended countermeasures**:

1. **Investigate the angular velocity discrepancy** between the exploit scanner and the in-sim guard. If the scanner's 363 rad/s is real (even sub-frame), the in-sim check at 60 Hz is undersampling it. Consider adding an in-sim check on the joint's reported angular velocity directly from Rapier's rigid-body API rather than finite-differencing quaternions across frames.

2. **Add a joint-limit contact-impulse penalty** in `fitness.rs`: if any joint is at its angular limit for more than K consecutive frames, apply a fitness multiplier < 1.0. This closes the "pin-and-push" class.

3. Flag to Hammond for physics review: the 363 rad/s reading suggests the Rapier joint constraint may be generating velocity artifacts that the 20 rad/s body rotation cap does not catch. This is the boundary where physics and fitness guards overlap — I will not touch the physics files, but this warrants a deeper look.

---

### The Tidepools (Evolution 4) — "Borderline"

**Threat level**: MEDIUM

Top 5 all flagged: **FAST_PEAK_7.9m/s** (creature 59093, fitness 1.33). The creature has 2 bodies, 1 DOF, 20 effectors, and a peak linear speed of 7.86 m/s — 98.3% of the 8 m/s cap.

Water environment, so there is no free-fall to partially explain the high peak. A water creature hitting 7.86 m/s is suspicious. The joint actuation is only 1.03 rad over the full run — barely moving. This is a creature with minimal joint motion that somehow achieves near-cap linear velocity. The joint-motion coefficient (`min_joint_motion=0.3`) should be penalizing this creature heavily, yet fitness is 1.33.

The low actuation figure combined with high linear speed suggests the single revolute joint is doing a small-amplitude, high-frequency oscillation that produces a disproportionate thrust impulse — a resonance exploit. The water drag model (`viscosity=2.0`) may be creating a drag-asymmetry artifact when the creature's body orientation is near-perpendicular to travel.

**Specimens of interest**: 59093, 59141, 59381, 59621, 59861

**Exploit classification**: Suspected low-amplitude resonance / drag-asymmetry in water. Borderline linear speed. Requires deeper trace analysis before escalating to CRITICAL.

**Recommended countermeasures**:

1. Debug-trace creature 59093 with `--output` and analyze the joint angle time series. If the DOF is oscillating at > 5 Hz with amplitude < 0.1 rad, that is the resonance signature.
2. The 1.03 rad total joint actuation over 10 seconds at 0.3 threshold minimum — verify the motion_coef is actually being applied. If the joint has any 2-second window with stddev > 0.3, the creature passes; small-amplitude high-frequency motion can produce healthy stddev even with tiny displacement.
3. Consider a minimum joint amplitude guard (not just stddev): stddev of a 10 Hz, 0.05 rad sine wave is 0.035 rad, well below the 0.3 threshold. This creature may be slipping through on frequency alone.

---

### Summary Table

| Evolution | Name | Best Fitness | Exploit Flag | Threat |
|-----------|------|-------------|--------------|--------|
| 1 | The Steppe | 27.36 | Speed ceiling surfing (7.54–7.81 m/s) | HIGH |
| 2 | The Abyss | 14.99 | None — legitimate swimmer | LOW |
| 3 | The Beacon | 2.05 | Vibration-launch + 363 rad/s joint spin | HIGH |
| 4 | The Tidepools | 1.33 | Near-cap speed (7.86 m/s), minimal joint motion | MEDIUM |

The park has one clean evolution and three requiring intervention. The monoculture pattern (single champion propagating by migration to dominate all islands) is present in all four evolutions — this is a systemic property of the current migration strategy, not a physics exploit, but it means the leaderboard is not showing diversity, it is showing a single creature copied N times.

*"Clever girl..."*

---

## [2026-04-06 14:30] — Wave 2 Security Sweep: Larger Populations, Older Tricks

**Threat level**: HIGH (two evolutions), MEDIUM (one evolution), LOW (one evolution)

Wave 2 parameters: population 100→200, migration interval 15→50. The hypothesis was that slower migration and deeper gene pools would slow down monoculture collapse and possibly surface new behavioral strategies. The data below says: same exploits, but the creatures found them faster and held them tighter. Nature did not wait for an invitation.

---

### The Savanna (Evolution 9) — "The Jogger"

**Threat level**: LOW

The Savanna champion (creature 100736, gen 4, fitness 7.79) is the cleanest result in this sweep. The exploit scanner returns **no flags**. Metrics from the debug trace:

- 2 bodies, 1 DOF, 1 effector
- Root Y: 0.07m to 2.00m — drops to ground, stays there
- Horizontal displacement: 9.23m in 10s — sustained average 0.92 m/s
- Peak speed: 5.59 m/s — well clear of the 8 m/s ceiling
- Joint activity: 13.39 rad — genuine, sustained actuation
- Below spawn fraction: 98% — creature is grounded throughout

Peak speed of 5.59 m/s is transient (likely ground-contact impulse during gait), not sustained. The mean speed of 0.92 m/s is plausible for a small two-body hopper. No ceiling surfing, no vibration, no teleportation. This creature jogs; it does not cheat.

The monoculture clock started early — generation 4, island 4 — and the genome had spread to two islands (4 and 5) with 84 clones by gen 53. Slower than The Steppe's 704 clones, consistent with the 50-generation migration interval. The fitness plateau at 7.79 (vs. The Steppe's 27.36) also means the champion is not the global optimum; the population has room to continue improving. This is the healthiest evolution in the park.

**Comparison to Wave 1 (The Steppe, 27.36)**: Not a speed-ceiling surfer. The Steppe's champion was bumping the 8 m/s wall on three of four bodies. The Savanna's champion peaks at 5.59 m/s. Same task, same parameters except population size — but the larger population found a more modest creature faster. This may reflect better coverage of the early fitness landscape, or simply luck of the draw.

**Specimens of interest**: 100736 and 83 clones (islands 4, 5)
**Exploit classification**: None. Legitimate land locomotion.
**Recommended countermeasures**: None. Continue monitoring for ceiling-surfing if fitness exceeds ~15.

*"...for once, they're behaving themselves."*

---

### The Deep (Evolution 10) — "The Sinker"

**Threat level**: MEDIUM

Top 8 specimens (creature 157521, gen 57+, fitness 15.93) all carry two flags: **FAST_PEAK_7.1m/s** and **JOINT_ANGVEL_30**. This is The Abyss's cousin with sharper claws.

Debug trace tells the story in numbers. The root body descends steadily in Y:

```
t=1s:  y=-2.408   t=5s:  y=-7.251
t=2s:  y=-2.941   t=6s:  y=-8.194
t=3s:  y=-4.554   t=7s:  y=-9.915
t=4s:  y=-5.405   t=10s: y=-13.341
```

Final position: (0.000, -13.341, 5.579). The creature is sinking — not swimming. It drops 13.3m in 10 seconds while drifting only 5.6m horizontally. In a zero-gravity water environment this is significant: there is no gravity to explain the Y descent. Something is generating sustained downward thrust.

The genome: 2 bodies, 1 DOF, 17 effectors, 2994-byte genome. Seventeen effectors driving a single joint is the hallmark of an overfit resonator — the brain has evolved a 17-channel control signal to drive one degree of freedom in precisely the pattern that maximizes energy extraction from the joint-limit contact forces.

The JOINT_ANGVEL_30 flag (30 rad/s against a 20 rad/s cap, 1.5× threshold) confirms this: the joint is regularly exceeding the configured angular velocity guard. Same gap we identified in The Beacon — the in-sim max_body_angular_velocity check is apparently not rejecting these frames. This is not a new exploit; it is the same unclosed hole.

The peak linear speed of 7.1 m/s in water (zero gravity) has no physical mechanism other than contact impulse accumulation from a rapidly-oscillating joint. The creature has 11 clones concentrated on island 6 only — the migration filter has (so far) contained the spread. The 50-generation interval is doing its job on containment, less so on breeding.

**Specimens of interest**: 157521, 158447, 159395, 160343, 161639, 162587, 163535, 164633
**Exploit classification**: High-frequency joint oscillation / angular velocity guard bypass. Same class as The Beacon (Evolution 3). Seventeen effectors on one DOF — deliberate resonance tuning by evolution.
**Recommended countermeasures**: Same as The Beacon. The JOINT_ANGVEL cap needs to be enforced at the Rapier constraint level or checked per-substep. Seventeen effectors on a single DOF should also trigger a diversity flag — the brain complexity is wildly disproportionate to the morphological complexity.

---

### The Lighthouse (Evolution 11) — "The Vibrator Returns"

**Threat level**: HIGH

Every top 10 specimen carries two flags: **TINY_BODY** and **FAST_PEAK_8.0m/s**. The champion (creature 152776, gen 60, fitness 2.34) is a 2-body, 1-DOF, 1-effector creature — morphologically minimal. What it lacks in complexity it makes up in dedication.

The TINY_BODY flag is new. The root body has half-extents (0.113, 0.025, 0.042) — the Y dimension of 0.025m is a 5cm full-height plate. The scanner's threshold is 3cm half-extent (6cm full), so this body barely clears the flag at 5cm full height (2.5cm half). The second body is more reasonable at (0.435, 0.073, 0.269). A 5cm-tall root plate with one joint connecting to a wing body is structurally similar to The Beacon's champion phenotype.

The trajectory from the debug trace is revealing:

```
t=1s:  root=(-0.183, 0.097, 0.069)
t=2s:  root=(-0.211, 0.097, 0.439)
...
t=10s: root=(-0.264, 0.097, 0.332)
```

Root Y locks at 0.097m throughout — the creature is permanently on the ground. Total displacement: 0.42m in 10s. The light-following fitness of 2.34 is achieved with almost zero locomotion. Peak speed 7.97 m/s (at the 8 m/s wall) against an overall displacement of 0.42m is physically incoherent: a creature that moves 0.42m total cannot genuinely reach 8 m/s unless it is vibrating in place and the speed measurement is capturing the joint's surface velocity rather than the root's center-of-mass velocity.

The exploit scanner measures root-body speed via finite-difference of the root transform — so 7.97 m/s at the root is real, momentary, and inconsistent with 0.42m total travel. This is a contact-impulse micro-launch: the tiny plate bangs against the ground repeatedly, each bang propelling the root for one frame at near-cap speed before it falls back. Net displacement is minimal. The light-following score of 2.34 is being accumulated not from deliberate light-directed movement but from these stochastic micro-twitches occasionally aligning with the light direction.

Direct comparison to The Beacon (Evolution 3, fitness 2.05): same exploit class confirmed. The Lighthouse's creature has a slightly higher fitness (2.34 vs. 2.05), slightly higher peak speed (7.97 vs. 7.98 m/s), and a TINY_BODY flag that was absent in The Beacon. The larger population did not find a better strategy — it found the same strategy with a smaller, faster-twitching body. The exploit has been refined, not replaced.

162 clones across islands 3 (145) and 4 (31). Island 3 is saturated. Migration interval of 50 generations slowed the spread compared to Wave 1's 15-generation interval, but the genome still crossed the island boundary.

**Specimens of interest**: 152776, 153508, 154456, 155204, 156152, 157100, 158048, 158596, 159544, 160492 (and 152 further clones)
**Exploit classification**: Vibration-launch / contact-impulse micro-twitching. Tiny plate geometry amplifies ground-contact forces. Same core mechanism as The Beacon; morphologically refined by larger population. Secondary flag: TINY_BODY (5cm root plate, near-degenerate geometry).
**Recommended countermeasures**:

1. **Enforce minimum body dimension**: The existing `min_body_dim` guard in `fitness.rs` should be a hard rejection, not just a scanner flag. A 5cm-tall plate root body exists specifically to maximize contact force density per unit mass. Suggested minimum half-extent: 0.05m (10cm full dimension) on any axis, configurable as `EvolutionParams::min_body_half_extent`.
2. **Track net displacement vs. peak speed ratio**: if `peak_speed / mean_speed > 20`, the creature is not traveling — it is vibrating. Penalize or reject.
3. **The Beacon's JOINT_ANGVEL fix remains the primary needed intervention.** The Lighthouse creature has only 1 effector and still generates 8 m/s peaks — the mechanism is morphological (tiny plate), not purely neural.

---

### The Coral Reef (Evolution 12) — "The Riser"

**Threat level**: MEDIUM

Top 10 specimens (creature 155614, gen 65+, fitness 1.37) all flagged: **FAST_PEAK_7.9m/s** and **JOINT_ANGVEL_105**. The JOINT_ANGVEL reading of 105 rad/s is 5.25× the 20 rad/s cap — the most severe angular velocity violation in this sweep.

The trajectory is distinctive:

```
t=1s:  root=(-0.142, 0.151, -0.067)
t=2s:  root=(0.028, 0.406, -0.067)
t=3s:  root=(0.104, 0.505, -0.011)
t=4s:  root=(0.080, 1.012, 0.075)
t=5s:  root=(0.344, 1.633, -0.426)
t=6s:  root=(0.355, 2.117, 0.187)   ← peak Y
t=7s:  root=(0.236, 1.510, 0.380)
t=10s: root=(0.001, 1.987, -0.096)
```

This creature starts at Y=0 (water, zero gravity) and rises to Y=2.117m over 6 seconds, then oscillates. There is no buoyancy force above zero to explain a rise of 2.1m — that energy is coming entirely from the joint actuators. Final Y=1.987m, almost exactly 2m above start.

The fitness task is LightFollowing. The light source is presumably placed above the creature, so rising toward it scores well. But a JOINT_ANGVEL of 105 rad/s on a 3-body, 2-DOF phenotype is not directed swimming — it is a rotation-at-limit exploit generating upward thrust from joint constraint forces. The creature is not swimming up; it is spinning its joints hard against their limits and the reaction force happens to point upward.

16 effectors on 2 DOFs (8 per DOF) mirrors The Deep's over-brain pattern. 24 clones across islands 4 and 5, contained relative to the others — the Coral Reef's lower fitness ceiling (1.37) means migration pressure is weaker.

**Specimens of interest**: 155614, 156551, 157299, 158247, 158995, 159743, 160691, 161588, 162189, 163137
**Exploit classification**: Angular velocity ceiling violation / joint-limit thrust. The same unclosed hole as The Beacon and The Deep, manifesting in a water LightFollowing context. 105 rad/s is the worst reading across all eight evolutions surveyed.
**Recommended countermeasures**: Same root fix as The Beacon and The Deep — the max_body_angular_velocity guard must reject at eval time, not merely flag. The Coral Reef's 105 rad/s reading is getting through the 20 rad/s cap; this is not a monitoring problem, it is an enforcement gap.

---

### Wave 2 Summary Table

| Evolution | Name | Best Fitness | Flags | Threat | vs. Wave 1 Counterpart |
|-----------|------|-------------|-------|--------|------------------------|
| 9 | The Savanna | 7.79 | None | LOW | Better than The Steppe (27.36 speed-ceiling surfer) |
| 10 | The Deep | 15.93 | FAST_PEAK_7.1 + JOINT_ANGVEL_30 | MEDIUM | Same class as The Beacon; less acute |
| 11 | The Lighthouse | 2.34 | TINY_BODY + FAST_PEAK_8.0 | HIGH | Confirmed same exploit as The Beacon; refined |
| 12 | The Coral Reef | 1.37 | FAST_PEAK_7.9 + JOINT_ANGVEL_105 | MEDIUM | Worst angular velocity reading across all 8 evolutions |

---

### Wave 1 vs. Wave 2 Comparative Assessment

**What changed**: Population 100→200, migration interval 15→50.

**What the data shows**:

1. **Slower migration reduced monoculture spread but did not prevent it.** Wave 1 The Steppe: 704 clones across 5 islands. Wave 2 The Savanna: 84 clones across 2 islands at comparable generation count. The interval increase bought time and diversity, but the mechanism is the same. The winning genome will eventually saturate the park; we are only adjusting the clock speed.

2. **The same three exploit classes recur without exception.** Speed-ceiling surfing (Wave 1 The Steppe) is absent in Wave 2 — that may be luck, or the larger population finding a different early optimum. The vibration-launch and joint-angvel-bypass classes are present in exactly the evolutions where they appeared before (LightFollowing and Water/Speed tasks). These exploits are not artifacts of small population size; they are stable attractors that evolution finds regardless of pool depth.

3. **The Lighthouse champion is a direct descendant of The Beacon's strategy.** Both: 2 bodies, 1 DOF, 1 effector, FAST_PEAK ~8 m/s, LightFollowing task. The Lighthouse adds TINY_BODY refinement. The larger population did not escape the attractor — it optimized within it.

4. **The angular velocity enforcement gap is the park's primary open vulnerability.** It appears in 3 of 8 evolutions (The Beacon, The Deep, The Coral Reef) with readings of 363, 30, and 105 rad/s respectively against a 20 rad/s cap. The cap is configured. The cap is not enforced. Until this is fixed, the vibration-launch attractor will be found in every water or LightFollowing run.

5. **The Savanna is the park's proof of concept.** A 200-body population on a Land/Speed task produced a clean, flag-free champion at generation 4 with a plausible 5.59 m/s peak. This is what successful evolution looks like. The difference between The Savanna and The Lighthouse is not the population size — it is the task and the open enforcement gap.

The enforcement fix is one change in `core/src/fitness.rs`. Everything else is ecology.

*"Clever girl... but the fence is still open."*

---

## [2026-04-06 16:45] — The Savanna Re-Audit: From Jogger To Ceiling Surfer

**Threat level**: HIGH

The Savanna was clean at generation 53. Fitness was 7.79. I cleared it personally — peak 5.59 m/s, no flags, legitimate ground locomotion. That report stands. What we are looking at now is a different animal.

Between generation 53 and generation 167, fitness jumped from 7.79 to 24.54. That is a 3.15x improvement in 114 generations. For reference, The Steppe peaked at 27.36. The Savanna is now within 11% of a confirmed ceiling surfer.

The exploit scanner result is unambiguous. All 10 of the top specimens — generations 167 through 176, IDs 274495 through 285103 — carry the same flag: **FAST_PEAK_7.5m/s**. Every single one has identical metrics:

```
bodies=3  joints=2  peak_vel=7.51  jAct=11.42  drift=29.45  rYmin=0.10  rYmax=2.00  below=98%
```

This is not ten different creatures. This is one genome, copied 10 times by migration, holding a fixed fitness of exactly 24.54 across 10 consecutive generations.

The debug trace of champion 274495 settles the matter. Root Y profile after the settle period:

```
t=1s:  y=0.149   t=4s:  y=0.129   t=7s:  y=0.116
t=2s:  y=0.111   t=5s:  y=0.110   t=8s:  y=0.099
t=3s:  y=0.116   t=6s:  y=0.128   t=10s: y=0.111
```

The creature is permanently grounded — Y between 0.099m and 0.150m throughout. No launch. No airborne phase. It is sliding, not running. Final displacement: 29.45m in 10 seconds — a sustained average ground speed of 2.94 m/s, which sounds reasonable until you see the velocity profile.

Peak speed analysis by finite-differencing the position trace (dt=1/60s) across all three bodies:

```
t=4.07s–4.25s:  13 frames above 7.0 m/s, peak 7.556 m/s at t=4.17s
t=6.75s–6.92s:  11 frames above 7.0 m/s, peak 7.453 m/s at t=6.85s
t=9.43s–9.58s:  10 frames above 7.0 m/s, peak 7.204 m/s at t=9.50s
```

Three bursts, spaced roughly 2.5 seconds apart. Between bursts, the creature drops to 0–3.5 m/s. The peak body speed is 7.556 m/s — **94.5% of the 8.0 m/s rejection ceiling**. Total frames above 7.0 m/s: 33 out of 600 (5.5%). Total frames above 7.5 m/s: 2.

This is the speed-ceiling surfing gait. The creature builds to near-cap velocity, extracts the forward impulse, falls back to near-zero, and repeats. The 2.94 m/s sustained average is the integral of these bursts, not a genuine constant-velocity locomotion strategy. Fitness of 24.54 is being computed from displacement (29.45m); the displacement is real, but it is purchased with contact-impulse bursts that approach the physics cap.

The pattern that was absent at generation 53 — where peak was 5.59 m/s and gait was steady — has now fully emerged. The creature evolved the same attractor as The Steppe. It took 114 more generations on a larger population, but it found it.

The body geometry tells the story of how. Initial body half-extents at frame 0:

```
Body 0 (root): (0.846, 0.077, 0.050)  — flat plate, 15cm tall
Body 1:        (0.183, 0.333, 0.124)  — taller block, lever arm
Body 2:        (1.619, 0.101, 0.046)  — long flat sled, 20cm tall
```

Body 2 is a 3.2m × 0.2m × 0.09m sled. This is the propulsion surface. The root is a flat plate. Together they form a low-profile ground sled with a lever body that drives periodic impulse bursts. The 11.42 rad of joint activity is real actuation — this is not the minimal-motion resonance exploit of The Tidepools — but the gait has been tuned to produce burst impulses rather than sustained force.

**Comparison to The Steppe (gen 53 report, Wave 1, same task)**: The Steppe's champion peaked at 7.54–7.81 m/s across four bodies. The Savanna's current champion peaks at 7.556 m/s across three. Structurally identical exploit class. The Savanna's predecessor was a jogger; its descendant is a surfer. Evolution found the same ceiling.

**Comparison to The Deep (evo 10, current champion 268625, fitness 22.02)**: The Deep is flagged JOINT_ANGVEL_30 — angular velocity overshoot, different mechanism. Peak linear speed 6.85 m/s, lower than The Savanna's 7.51 m/s. The Deep is a water/speed creature exploiting joint resonance; The Savanna is a land/speed creature exploiting contact-impulse bursts. Two different exploits, nearly the same fitness plateau.

**Specimens of interest**: 274495–285103 (10 consecutive champion clones, generations 167–176)

**Exploit classification**: Speed-ceiling surfing / contact-impulse burst gait. Identical classification to The Steppe (Evolution 1). Peak 7.556 m/s = 94.5% of the 8.0 m/s cap. Morphology: low-profile ground sled with lever body, optimized for burst propulsion rather than sustained locomotion.

**Recommended countermeasures**:

1. **Lower `MAX_PLAUSIBLE_SPEED` from 8.0 to 6.5 m/s** in `core/src/fitness.rs`. Free-fall from spawn height 2m produces 6.26 m/s at ground contact (√(2 × 9.81 × 2)). A legitimately fast land runner should not exceed that in a horizontal burst — if a body is hitting 7.5 m/s horizontally while the creature is already grounded (Y=0.13m), that energy is not coming from gravity. The 8.0 m/s cap is leaving a 1.74 m/s exploitation window. Expose as `EvolutionParams::max_plausible_speed` per the paper-divergence convention.

2. **Add a burst-gait detector**: compute the ratio of `peak_speed / mean_speed` over the full run. For a steady-gaited creature this ratio is low (The Savanna gen 53: peak 5.59 / mean ~0.92 = 6.1). For a ceiling surfer it is high (current champion: peak 7.56 / mean 2.94 = 2.57 — but the ratio across individual burst windows would be far larger). A `peak_speed / mean_speed > 5` rejection guard, or a "time above 80% cap" limit (currently 5.5%), would catch this pattern without touching physics.

3. **Migration containment**: same genome holding 10 consecutive generation slots is a monoculture signal. The 50-generation migration interval is not helping — the genome arrived by migration and is being copied generation-over-generation within the population. Consider a within-island clone penalty: if the new best genome has zero edit distance from the current best, suppress its fitness score by a factor to allow challengers to overtake.

The Savanna is no longer clean. It evolved. So did the exploit.

*"Clever girl... I was with her when she did it."*

---

## [2026-04-06 18:30] — Specimen 274476 Deep Investigation: "Tetrapus savannus"

**Threat level**: MEDIUM (phantom fitness) / LOW (actual behavior)

Hammond called this one in personally. Creature #274476, nicknamed "Tetrapus savannus" by the visitors -- a 4-body, 3-joint land galloper from The Savanna (evolution 9), island 3, generation 167. Recorded fitness: 17.78. Lex described it charging across the checkered plain. Hammond said the movement looks "weird" and "unrealistic."

I tracked it for three hours. Here is everything I found.

---

### Morphology

The phenotype is a 4-body assembly connected by 3 twist joints:

```
Body 0 (root):  2.00 x 0.49 x 0.53 m  -- massive rectangular torso
Body 1:         0.28 x 0.23 x 0.33 m  -- small forward appendage (twist-jointed to root)
Body 2:         0.82 x 1.09 x 1.78 m  -- large paddle body (twist-jointed to root)
Body 3:         0.27 x 0.50 x 0.18 m  -- trailing sub-limb / tail boom (twist-jointed to Body 2)
```

Body 2 is the dominant mass: 1.78m long, 1.09m tall -- larger than the root torso. Body 1 is a tiny nub. Body 3 trails behind as a stabilizer. Lex's description of "two twist-jointed limbs branching forward and a sub-limb trailing behind" matches the phenotype exactly.

The brain uses OscillateWave neurons on Body 1's effector (constant-driven, frequency ~2.2 Hz), and Sin/OscillateWave neurons on Body 2 with sensor feedback. Body 3's effector is driven by Sigmoid neurons with sensor inputs. This is a sensor-driven oscillatory controller -- not a frozen-joint or DC-bias exploit.

### Frame-by-Frame Physics Trace

Full 600-frame (10.0s) trace from the debug CLI:

**Phase 1 -- Free fall and landing (t=0.0s to t=0.67s):**
Root drops from Y=2.000 to Y=0.286. Normal gravitational descent. Speed peaks at 4.49 m/s during fall (consistent with free-fall from 2m). Creature impacts ground around frame 40 (t=0.67s) and bounces.

**Phase 2 -- Settle and initial gallop (t=0.67s to t=3.67s):**
Root Y stabilizes near 0.26-0.28m (grounded). The creature begins moving in +X direction, accelerating steadily:
- t=1.0s: X=0.034, speed 0.07 m/s
- t=1.5s: X=0.195, speed 0.97 m/s
- t=2.0s: X=0.907, speed 1.93 m/s
- t=2.5s: X=2.180, speed 3.16 m/s
- t=3.0s: X=4.086, speed 4.49 m/s (peak of this phase)
- t=3.5s: X=5.690, speed 1.59 m/s (decelerating)
- t=3.7s: X=5.784, speed 0.28 m/s

This is genuine galloping locomotion. The acceleration curve is smooth, the deceleration natural. The creature runs 5.8m in about 3 seconds -- average 1.9 m/s, peak 4.5 m/s. Physically plausible for this body size.

**Phase 3 -- Complete stall (t=3.83s to t=4.33s):**
Root position locked at (5.785, 0.241, -0.361). Speed: 0.00 m/s for ~30 consecutive frames. The creature has stopped dead. Joints are presumably at their limits or the oscillator phase has brought all actuators to a null point.

THIS IS WHERE THE DYNAMIC SETTLE CATCHES IT. The fitness evaluation's dynamic settle logic looks for 15 consecutive frames below 0.05 m/s after the minimum 1.0s settle period. The creature's stall at t=3.83s produces exactly this pattern -- the settle period does not end until frame 236 (t=3.93s). By then, the creature has already traveled 5.785m in the +X direction, but that distance is DISCARDED because initial_pos is rebased to the settle endpoint.

**Phase 4 -- Second gallop (t=4.33s to t=7.0s):**
The creature restarts, accelerates again:
- t=5.0s: X=6.697, speed 2.56 m/s
- t=5.5s: X=7.640, speed 1.77 m/s
- t=6.0s: X=8.492, speed 1.99 m/s
- t=7.0s: X=9.875, speed 0.02 m/s (another stall)

Root Y rises to 1.0-1.15m during this phase -- the creature is no longer flat on the ground. It appears to be tilting upward on its large paddle body (Body 2).

**Phase 5 -- Reversal (t=7.0s to t=9.5s):**
The creature reverses direction:
- t=8.0s: X=8.771 (moving back from peak of 9.875)
- t=9.0s: X=6.780

**Phase 6 -- End-of-sim launch (t=9.77s to t=10.0s):**
Frame 583: speed 1.16 m/s. Frame 584: speed jumps to 4.88 m/s. Frame 586: 7.36 m/s (root), 8.33 m/s (Body 3). The creature launches upward, reaching root Y=1.948 at the final frame -- nearly back to spawn height.

This launch happens in the last 17 frames (0.28s). Body 3 exceeds 8.0 m/s at frames 586-588.

### Fitness Discrepancy Analysis

**Recorded fitness: 17.78**
**Computed fitness with current code: ~2.48**

The discrepancy is 7.2x. The explanation:

The creature was evaluated at 2026-04-06 09:06:42. The per-body speed check was committed at 2026-04-06 01:04:22 (commit 94c6d52). However, the server binary was not rebuilt after that commit. The running server was compiled with the bee80aa version of fitness.rs, which had the dynamic settle but lacked the per-body speed check.

Under the old code, only the ROOT body's frame-to-frame speed was checked against the 8.0 m/s cap. Body 3's 8.33 m/s peak would not have triggered rejection. The root body peaks at 7.36 m/s -- under the cap.

More importantly, the old code's dynamic settle may have behaved differently due to the absence of `creature.brain.reset_time()` (added in 94c6d52). Without the brain time reset, the oscillator phases are NOT reset when settle ends. This means the creature's gait timing was different under the old code -- the stall at t=3.83s might not have occurred at the same time, or the settle might have ended at a different frame.

The fitness of 17.78 was legitimately computed under the old code but is NOT reproducible under the current code. This creature is a **phantom champion** -- its fitness score belongs to a version of the physics evaluation that no longer exists.

### Per-Body Speed Violations

Under current code, Body 3 exceeds 8.0 m/s at three frames near the end of simulation:

```
Frame 586 (t=9.77s): Body 3 speed = 8.333 m/s  *** VIOLATION ***
Frame 587 (t=9.78s): Body 3 speed = 8.221 m/s  *** VIOLATION ***
Frame 588 (t=9.80s): Body 3 speed = 8.099 m/s  *** VIOLATION ***
```

The violation occurs during the end-of-sim launch (Phase 6). Body 3 is the small trailing sub-limb (0.27 x 0.50 x 0.18m), which gets whipped around during the launch. Under the current fitness function, this creature would score exactly 0.0.

### Angular Velocity Analysis

All bodies remain within the 20 rad/s angular velocity cap:

```
Body 0: peak  8.58 rad/s at frame 4  (initial landing)
Body 1: peak 18.17 rad/s at frame 267 (during stall recovery)
Body 2: peak  7.03 rad/s at frame 516
Body 3: peak  7.04 rad/s at frame 516
```

Body 1 at 18.17 rad/s is close to the 20 rad/s cap (90.8%) but does not exceed it. No angular velocity exploit.

### Comparison to Current Champions

**Global champion (island 6): Creature #371611, fitness 25.78, gen 259**
- 3 bodies, 2 joints
- Root half-ext: (0.859, 0.084, 0.047) -- flat sled
- Peak speed across ALL bodies: 7.79 m/s (no violations)
- Root Y: 0.107-2.000 (stays grounded post-landing)
- Steady unidirectional travel: reaches -27.2m by t=10s
- Flagged by exploit scanner as FAST_PEAK_7.8m/s
- This is the ceiling surfer documented in the previous report

**Island 3 current champion: Creature #384193, fitness 18.99, gen 273**
- 5 bodies, 4 joints -- evolved from 274476's body plan (added a body)
- Same large torso + paddle morphology
- Root half-ext: (0.953, 0.206, 0.255) -- similar proportions to 274476
- Peak speed across ALL bodies: clean (no violations)
- Displacement 12.4m in 10s -- genuinely fast
- Same gallop-stall-restart pattern as 274476 but without the end-of-sim launch

The island 3 lineage descended from 274476's body plan but evolved past the speed violation. Evolution pruned the dangerous behavior when the per-body speed check started zeroing out violators (after the server was restarted with the new binary). The offspring learned to run without launching.

### Verdict

Creature #274476 is NOT an active exploit threat. It is a historical artifact.

**What Hammond saw**: The "weird" and "unrealistic" movement is the creature's distinctive gallop-stall-gallop-reverse-launch sequence. The movement IS unusual -- the long stall at t=3.8-4.3s, the reversal at t=7-9s, and especially the dramatic upward launch in the final 0.3s look wrong because they are wrong. Real animals do not freeze mid-gallop, reverse, and then launch vertically at the buzzer.

**Why it looks like that**: The twist-joint oscillators create a gait that is phase-sensitive -- at certain points in the oscillator cycle, the torques cancel out and the creature stalls. The end-of-sim launch is a coincidence of oscillator phase and ground contact that produces a brief burst of upward velocity. It is a solver artifact (contact impulse accumulation during phase alignment), not a deliberate evolved strategy.

**Why the fitness is wrong**: The creature was evaluated with a stale server binary that lacked the per-body speed check. Under the current code, it would score 0.0 due to Body 3 exceeding 8.0 m/s at frames 586-588.

**Is the body plan viable?** Yes. Creature #384193 (island 3 current champion, gen 273) uses an evolved version of the same body plan and scores 18.99 legitimately. The Tetrapus body plan works; it just needed the end-of-sim launch bred out.

### Recommended Actions

1. **Re-evaluate all creatures from evolution 9 generations 1-200** using the current fitness function. Their stored fitness scores were computed with the pre-94c6d52 binary and may be inflated. This will not affect ongoing evolution (new generations are evaluated with the current binary), but the leaderboard and historical records are wrong.

2. **No fitness.rs changes needed for this specimen.** The existing per-body speed check (added in 94c6d52) catches this creature. The guard is working.

3. **Consider a server-restart verification protocol.** The root cause of the phantom fitness is that the server binary was not rebuilt after a fitness function change. Any future fitness.rs modification should be followed by a server restart and a spot-check re-evaluation of the current champion to verify the new guards are active.

4. **The Savanna's real problem remains the global champion.** Creature #371611 (fitness 25.78) is still a ceiling surfer at 7.8 m/s peak. That is the same threat class documented in the previous report. Creature #274476 is a historical footnote; #371611 is the active concern.

**Specimens of interest**: 274476 (investigated), 384193 (clean descendant), 371611 (active ceiling surfer -- see previous report)
**Exploit classification**: Phantom fitness from stale binary evaluation + per-body speed violation (Body 3, 8.33 m/s). Would score 0.0 under current code.
**Threat to current population**: None. The lineage has already adapted past the speed violation. The current island 3 champion (#384193) is clean.

*"She was the best of them. Fastest. Smartest. Cleverest of the whole pack. When I looked at her trace, I could see exactly where she figured out the ceiling. Frame 584. She went from 1.16 to 4.88 m/s in one step. That is not running. That is knowing where the fence is and stepping over it. But the fence moved. Now she would score zero. Clever girl... but not clever enough."*

---

## [2026-04-06 14:30] -- Generation 300 Sweep: Full Park Audit

**Threat level**: LOW (with caveats)

The creatures have crossed the 300-generation mark. Every evolution that survived Wave 1's purges is now producing genuine locomotion. But three of four champions are pressing against the speed ceiling hard enough to leave marks, and the Lighthouse has evolved something I haven't seen before.

### Evo 9 -- The Savanna (Land/Speed, gen 301, best 26.49)

**Specimen**: #388712 (gen 278, cloned forward through gen 287+)
**Morphology**: 3 bodies, 2 joints, 1 DOF, 3 effectors. Flat runner -- root half-extents (0.925, 0.078, 0.047), essentially a thin plate with a tall paddle (body 1: 0.198x0.330x0.135) and a long plank (body 2: 1.628x0.111x0.052).
**Trajectory**: Smooth, directional. Starts at (0, 2, 0), settles by t=1s, then accelerates steadily to (-30.39, 0.12, -0.94) by t=10s. Root Y stays at 0.11--0.14 post-landing. No bouncing, no teleporting.
**Peak speed**: 7.87 m/s (flagged FAST_PEAK by scanner)
**Joint activity**: 11.72 (healthy actuation)
**Below-spawn fraction**: 98% (fell and stayed -- correct behavior)
**Verdict**: CLEAN. This is a well-adapted runner. The 7.87 m/s peak is below the 8.0 cap and appears to be genuine locomotion speed, not a contact kick. The creature covers 30.4m in ~9s of evaluation (post-settle), averaging 3.38 m/s with bursts to 7.87. The fitness of 26.49 is legitimate: `30.4 * 0.7 + max_disp * 0.3 = 26.49` gives max_displacement ~ 17.2m, consistent with a creature that builds speed gradually.

**Island diversity** (latest generation):
- Island 0: 24.53 | Island 1: 19.94 | Island 2: 18.56 | Island 3: 19.38
- Island 4: 25.77 | Island 5: 25.77 | Island 6: 26.49 | Island 7: 26.49

Three distinct fitness tiers visible. Islands 6-7 share the champion genome. Islands 4-5 have a close variant. Islands 0-3 have independent lineages. Reasonable diversity. No monoculture yet.

### Evo 10 -- The Deep (Water/Speed, gen 305, best 23.58)

**Specimen**: #373652 (gen 263, cloned forward through gen 272+)
**Morphology**: 2 bodies, 1 joint, 1 DOF, 12 effectors. Compact swimmer -- small root (0.114x0.155x0.384) with a large tail (0.083x0.223x0.656). The 12 effectors on 1 DOF means redundant brain outputs -- evolution is using the extra effectors as internal computation nodes, not wasted.
**Trajectory**: Starts at origin, dives to Y=-10.8 and swims forward to Z=13.2. Total displacement 17.04m. Clean arc, no discontinuities.
**Peak speed**: 6.97 m/s (below threshold)
**Joint activity**: 16.43 (very active -- strong tail beats)
**Below-spawn fraction**: 0% (water creature, no ground -- correct)
**Verdict**: CLEAN. No flags. The jump from 22.02 to 23.58 appears to be genuine evolutionary improvement -- the creature has refined its tail-beat frequency. The morphology is elegantly simple: a body and a tail. Sims would be proud.

**Island diversity** (latest generation):
- Island 0: 23.58 | Island 1: 23.58 | Island 2: 22.71 | Island 3: 22.71
- Island 4: 21.38 | Island 5: 19.11

The champion has migrated to islands 0-1. Islands 2-3 have a runner-up variant. Island 5 is the weakest -- may be incubating a novel strategy or dying. Not a monoculture yet but converging. The centipede swimmers appear to have been outcompeted on islands 0-1 by this minimalist design.

### Evo 11 -- The Lighthouse (Land/Light, gen 374, best 2.39)

**Threat level**: MEDIUM

**Specimen**: #389656 (gen 344, cloned forward through gen 351+)
**Morphology**: 2 bodies, 1 joint, 1 DOF, 3 effectors. Root is a sliver -- (0.178, 0.027, 0.043). Body 1 is a modest plate (0.435x0.056x0.271).
**Trajectory**: Starts at (0, 2, 0), settles to ground, ends at (0.214, 0.084, 0.411). Only 0.46m horizontal displacement. Total displacement under 2m.
**Peak speed**: 8.00 m/s EXACTLY (flagged FAST_PEAK + TINY_BODY)
**Joint activity**: 2.15 (low)
**Below-spawn fraction**: 99%

**This one concerns me.** The creature achieves fitness 2.39 on a light-following task while barely moving (0.46m horizontal). It's hitting exactly 8.0 m/s peak speed with a body that has a 0.027 half-extent (flagged TINY_BODY at < 0.03). The low joint activity (2.15) combined with the speed cap hit suggests this creature has found a way to get brief contact-kick bursts that are right at the rejection threshold but not over it. It's not getting zeroed because the per-body speed check looks at displacement/dt and the creature only grazes the cap without exceeding it.

The 99% below-spawn rules out floating. But a light-following creature that barely moves yet scores 2.39 is suspicious. The fitness formula for light-following is different from speed, so 2.39 may be legitimate proximity to the light source. However, the 8.0 m/s peak on a nearly-static creature is the fingerprint of a micro-vibration strategy -- brief ground-contact kicks that nudge it toward the light.

**Not an exploit per se**, but this lineage is ceiling-surfing. If we raise the speed cap, this creature's strategy would change. If we lower it, this creature dies. It has evolved to live exactly at the boundary.

**Island diversity** (latest generation):
- Island 0: 2.386 | Island 1: 2.386 | Island 2: 2.385 | Island 3: 2.384

Total monoculture. All four active islands share essentially the same genome (fitness differences in the 4th decimal). This is the most converged evolution in the park.

### Evo 12 -- The Coral Reef (Water/Light, gen 428, best 1.71)

**Specimen**: #408784 (gen 426, cloned forward through gen 435+)
**Morphology**: 3 bodies, 2 joints, 2 DOF, 14 effectors. Root (0.338x0.218x0.123) with two appendages. 14 effectors on 2 DOF -- heavy brain redundancy like the Deep champion.
**Trajectory**: Starts at origin, drifts to (-0.317, -0.449, -0.790). Total displacement 0.96m, max position magnitude 1.49m.
**Peak speed**: 7.97 m/s (flagged FAST_PEAK)
**Joint activity**: 4.81 (moderate)
**Below-spawn fraction**: 0% (water, correct)
**Verdict**: BORDERLINE. Malcolm predicted 1.70 as the ceiling. It's at 1.71 now. The 7.97 m/s peak speed in water is suspicious -- water viscosity should be damping velocities, and a creature that only travels 0.96m total shouldn't need to hit near-8 m/s at any point. This looks like a brief thrash that generates a position correction toward the light, followed by viscous damping, repeated. The strategy works but it's fighting the speed cap.

**Island diversity** (latest generation):
- Island 0: 1.707 | Island 1: 1.682 | Island 2: 1.682 | Island 3: 1.659
- Island 4: 1.696 | Island 5: 1.707

Better diversity than the Lighthouse. Islands 0 and 5 share the champion. Islands 1-2 have a variant. Island 3 is weakest but distinct. No monoculture.

### Summary Table

| Evo | Name | Fitness | Peak Speed | Flags | Status |
|-----|------|---------|------------|-------|--------|
| 9 | The Savanna | 26.49 | 7.87 m/s | FAST_PEAK | CLEAN |
| 10 | The Deep | 23.58 | 6.97 m/s | (none) | CLEAN |
| 11 | The Lighthouse | 2.39 | 8.00 m/s | FAST_PEAK, TINY_BODY | BORDERLINE |
| 12 | The Coral Reef | 1.71 | 7.97 m/s | FAST_PEAK | BORDERLINE |

### New exploit classes observed: None

No new exploit categories. The FAST_PEAK flags on evolutions 9, 11, and 12 are all within the existing "speed ceiling surfing" pattern. No NaN, no teleports, no floating, no rigid-body exploits, no escaped arena.

### Recommended countermeasures

1. **The Lighthouse monoculture**: Consider increasing island count from 6 to 8, or shortening migration interval to inject fresh diversity. Four islands with identical genomes is genetic death.

2. **TINY_BODY on Lighthouse**: The 0.027 half-extent body is below the 0.03 threshold. If the min body dimension is enforced at generation time (mutation rejection) this shouldn't persist. Verify the min body size is being enforced during mutation, not just flagged post-hoc.

3. **Speed ceiling surfing (evos 11, 12)**: The light-following creatures are hitting 7.97--8.00 m/s peak speed while barely moving. This isn't an exploit -- they're using brief high-energy bursts to correct position -- but it means the 8.0 cap is actively shaping their strategy. If we ever change MAX_PLAUSIBLE_SPEED, these lineages will either die or explode. Document this as a known behavioral dependency.

4. **No action needed on The Savanna or The Deep.** Both are producing genuine, impressive locomotion. The Savanna at 26.49 is approaching The Steppe's 27.36 record through legitimate means. The Deep's jump to 23.58 is clean evolutionary improvement.

*"I've spent twenty years watching animals in the wild, and I've never seen anything quite like #388712. Three bodies, two joints, thirty meters in ten seconds. No tricks, no exploits, no cheating. Just a flat creature that learned to run. The raptors in my old park would have respected that. The Lighthouse, though... that one worries me. Not because it's cheating -- it isn't, technically. But it's evolved to live precisely at the boundary between legal and illegal. It knows where the fence is. It's pressing its face against the electric wire and feeling exactly how much current it can take. When they start doing that, you've already lost control. You just don't know it yet."*

---
