# Dr. Grant's Field Notes

> "Some of the biggest discoveries have been made by... accident."

---

## [2026-04-06 10:00] -- Field Notes: First Survey of the Park

All four evolutions have completed their 200-generation runs. Time to see what nature -- or rather, selection pressure -- has wrought.

### Overall Observation: Convergence Dominance

The first thing that strikes me is how aggressively each evolution has converged. In The Steppe, all five islands share the same champion genotype (fitness 27.36). The Abyss has near-total convergence (4 of 5 islands identical at 14.99, one at 14.81). Migration between islands is doing its job -- perhaps too well. Biodiversity is low within each evolution, though there is genuine morphological diversity *between* evolutions.

This is the classic island model problem: migration homogenizes the population once a dominant strategy emerges. The creatures are fit, but we've lost the evolutionary experiments that might have been happening on isolated islands.

---

### Species 1: *Cursor velox* ("the swift runner")

**Specimen**: Genotype #36111 (Evolution 1: The Steppe, Island 4)
**Fitness**: 27.36 (highest in the park)
**Environment**: Land speed

**Morphology**: 4 bodies, 3 joints (2 revolute, 1 rigid). The root body is small (0.10 x 0.26 x 0.30 half-extents) -- a compact torso. Rigidly attached to it is a large flat plate (0.71 x 0.13 x 1.13) -- a broad, low paddle that serves as the main contact surface with the ground. Two additional segments are connected via revolute joints: a thin actuated rod (0.05 x 0.22 x 0.15) connected to the root, and a taller block (0.11 x 0.51 x 0.36) connected to the plate, forming a recursive limb from genome node 2 (recursive_limit=2).

**Brain**: Minimalist. A single OscillateWave neuron drives both revolute joints. The oscillator takes constants as input (frequency ~3.0, phase ~0.5) -- a steady rhythmic beat. No sensor feedback. This creature is a wind-up toy, but a remarkably effective one.

**Locomotion**: Galloper. The large flat plate provides ground contact and stability while the oscillating limb segments create an asymmetric rocking motion that propels the creature forward. The body plan is fundamentally asymmetric -- it has a "front" and a "back" -- which is essential for directed locomotion. At fitness 27.36 over a 10-second simulation (minus 1s settle), that is approximately 3 meters per second of sustained ground speed. Remarkable.

**Distinguishing features**: The combination of a large rigid stabilizer plate with small oscillating appendages is a clever evolved solution. The creature essentially "rows" itself across the ground.

**Tags**: `species:cursor-velox`, `locomotion:galloper`, `notable:highest-fitness`, `favorite`

---

### Species 2: *Remigia simplex* ("the simple oar")

**Specimen**: Genotype #44054 (Evolution 2: The Abyss, Island 3)
**Fitness**: 14.99
**Environment**: Water speed

**Morphology**: 2 bodies, 1 revolute joint. Beautifully minimal. A small cuboid head (0.18 x 0.19 x 0.05 -- nearly flat, disc-like) connected to a long flat paddle (0.43 x 0.03 x 0.13 -- extremely thin in Y, wide in X and Z). The paddle is attached to the head's PosZ face at the child's NegY face, with reflection=true, meaning the paddle hangs below.

**Brain**: One OscillateWave neuron with three inputs: two constants setting frequency (~3.1) and phase (~0.8), plus a Sensor(1) input with weight 0.55. This creature is *sensing* something -- likely its joint angle or angular velocity -- and modulating its paddle beat accordingly. That sensor feedback is the key innovation separating it from a pure open-loop swimmer.

**Locomotion**: Paddle swimmer. The flat fin oscillates up and down (revolute joint), sculling through the water. The extreme thinness of the paddle (0.03 half-extent in Y) means it slices through water on the recovery stroke and presents a broad face on the power stroke. Evolution has discovered the principle of the oar blade.

**Distinguishing features**: The sensor feedback loop. Most creatures in this park are open-loop oscillators, but *R. simplex* listens to its own body. This is a hint of proprioception.

**Tags**: `species:remigia-simplex`, `locomotion:paddle-swimmer`, `notable:highest-fitness`, `favorite`

---

### Species 2b: *Remigia robusta* ("the sturdy oar")

**Specimen**: Genotype #17921 (Evolution 2: The Abyss, Island 2)
**Fitness**: 14.81
**Environment**: Water speed

**Morphology**: Nearly identical to *R. simplex* -- same 2-body, 1-joint plan. The paddle is slightly longer (0.46 vs 0.43) and thicker (0.05 vs 0.03), and the head is slightly different in proportions. Same brain architecture (OscillateWave with sensor feedback), with slightly different weights.

**Locomotion**: Identical paddle-swimmer strategy.

**Significance**: This is a textbook case of convergent evolution. Island 2 independently arrived at the same body plan as Islands 0, 1, 3, and 4, but with slightly different parameters. The 1.2% fitness gap (14.81 vs 14.99) shows how fine the tuning is -- small changes in paddle proportions and oscillation weights matter at this level.

**Tags**: `species:remigia-robusta`, `locomotion:paddle-swimmer`, `notable:convergent-evolution`

---

### Species 3: *Heliotropus terrestris* ("the land sun-turner")

**Specimen**: Genotype #65878 (Evolution 3: The Beacon, Island 0)
**Fitness**: 2.05
**Environment**: Land, light-following

**Morphology**: 3 bodies, 2 joints (1 universal, 1 rigid). The root is a long, low slab (0.24 x 0.11 x 0.54 -- elongated in Z). Attached via a universal joint (2 degrees of freedom) is a small sensor arm (0.18 x 0.13 x 0.08). Rigidly attached is a ballast block (0.20 x 0.11 x 0.13).

The genome is extraordinary: 26 nodes and 35 connections, making it by far the most complex genetic blueprint in the park. Yet developmental pruning reduces it to just 3 physical bodies. The genome is a vast iceberg of which only the tip is expressed. Whether the unexpressed nodes serve as a genetic reservoir for future mutations, or are simply evolutionary debris, is an open question.

**Brain**: Complex. The expressed nodes include neurons with OscillateWave, Sigmoid, and Sin activation functions. The universal joint arm has 2 effectors (2-axis control), giving the creature fine directional control toward the light source.

**Locomotion**: Tumbler. On land, this creature cannot walk in any conventional sense. Instead, it uses its universal-joint arm to shift its center of mass and topple toward the light. The rigid ballast block provides asymmetric weight distribution. It is ungainly but effective -- fitness 2.05 means it consistently reaches the light.

**Distinguishing features**: The massive unexpressed genome (26 nodes to 3 bodies). The universal joint providing 2-axis light tracking. This creature solves navigation, not speed.

**Tags**: `species:heliotropus-terrestris`, `locomotion:tumbler`, `notable:highest-fitness`, `favorite`

---

### Species 3b: *Heliotropus minor*

**Specimen**: Genotype #64566 (Evolution 3: The Beacon, Island 3)
**Fitness**: 1.99
**Environment**: Land, light-following

**Morphology**: Effectively identical to *H. terrestris* -- same 3-body, 2-joint topology, same joint types, near-identical dimensions. This is the same species with minor parametric variation, not a distinct lineage.

**Tags**: `species:heliotropus-minor`, `locomotion:tumbler`

---

### Species 4: *Heliotropus natans* ("the swimming sun-turner")

**Specimen**: Genotype #59093 (Evolution 4: The Tidepools, Island 2)
**Fitness**: 1.33
**Environment**: Water, light-following

**Morphology**: 2 bodies, 1 revolute joint. The root is a compact block (0.26 x 0.29 x 0.17) with a TwistBend joint type (though as root, this is informational). Connected via a revolute joint is a large, dramatically flat sail (0.11 x 0.76 x 0.95 -- enormous in Y and Z, thin in X). This sail body is nearly 10x the root body's volume.

**Brain**: The genome has 17 nodes with Sigmoid, OscillateWave, and Sin neurons, plus multiple effectors. But the phenotype is just 2 bodies, 1 joint. Like *H. terrestris*, there is a massive unexpressed genome lurking beneath the simple body.

**Locomotion**: Sail swimmer. In water, the enormous flat sail acts as both a propulsive surface and a light-intercepting panel. The revolute joint oscillates the sail, creating thrust, and the asymmetry of the body plan means this thrust has a directional component toward the light source. Think of a manta ray with a single enormous wing.

**Distinguishing features**: The dramatic size ratio between head and sail. The sail's half-extents (0.76 x 0.95) make it nearly a square meter of surface area. In water, this is an enormous hydrodynamic surface. Evolution has discovered that for phototaxis in water, maximizing your cross-section is the strategy.

**Tags**: `species:heliotropus-natans`, `locomotion:sail-swimmer`, `notable:highest-fitness`, `favorite`

---

### Species 4b: *Polypus luminaris* ("the many-limbed light-seeker")

**Specimen**: Genotype #51093 (Evolution 4: The Tidepools, Island 1)
**Fitness**: 1.30
**Environment**: Water, light-following

**Morphology**: Phenotypically identical to *H. natans* -- 2 bodies, 1 revolute joint, same head-plus-sail architecture. The sail is slightly larger (0.85 x 0.98 vs 0.76 x 0.95). But the genome is the most complex in the park: 17 nodes, 22 connections, with recursive limits up to 3, universal joints, multiple oscillator neurons, and sensor inputs. This genome *could* produce a sprawling multi-limbed creature, but developmental constraints prune it to a simple pair.

**Significance**: The naming reflects the genome's potential rather than its expression. Under different developmental conditions, this genome might unfold into something far more complex. It is, in a sense, a creature carrying the blueprints for a much larger animal in its DNA.

**Tags**: `species:polypus-luminaris`, `locomotion:sail-swimmer`, `notable:genome-complexity`

---

### Biodiversity Assessment

**Total distinct species**: 6 named (in 4 genera)
**Total distinct body plans**: 4
**Total distinct locomotion strategies**: 4

| Strategy | Body Plan | Environment | Genus |
|---|---|---|---|
| Galloper | Asymmetric plate + oscillating limbs (4 bodies) | Land speed | *Cursor* |
| Paddle swimmer | Head + flat fin (2 bodies) | Water speed | *Remigia* |
| Tumbler | Slab + universal-joint arm + ballast (3 bodies) | Land light | *Heliotropus* (terrestris) |
| Sail swimmer | Head + enormous sail (2 bodies) | Water light | *Heliotropus* (natans) |

**Diversity rating**: LOW within evolutions, MODERATE across evolutions.

Each environment has produced exactly one dominant strategy. There are no alternative body plans surviving on any island. Migration has ensured genetic homogeneity within each evolution. This is a park of four species, each perfectly adapted to its niche, with no competitors.

**What's missing**: No snake-like chains (despite max_parts allowing up to 20). No starfish radial symmetry. No bipedal walkers. No spinners or tumblers in the speed tasks. Evolution found local optima early and locked in. The 15-generation migration interval may be too frequent -- the islands never had time to develop truly distinct lineages before the dominant form colonized them all.

**Recommendation**: Future evolutions should consider longer migration intervals (50+ generations) or larger populations to maintain diversity. The current park is efficient but ecologically monotonous.

---

*"What's so great about discovery is that you never know what you're going to find. But here, I must confess -- I know exactly what I found. Four beautiful solutions, each utterly alone in its niche. There's something both elegant and a little sad about that."*

---

## [2026-04-06 14:30] -- Field Notes: Wave 2 Survey -- The Speciation Event

Wave 2 ran four new evolutions with larger populations (150-200), more islands (6-8), and critically, slower migration. The question I asked after Round 1 was whether isolation would produce diversity. The answer is: yes -- emphatically so. We are witnessing genuine speciation.

---

### Evolution 9: The Savanna (Land/Speed, pop 200, 8 islands)

The Savanna is the most biodiverse evolution I have seen in this park. Eight islands, and at least four distinct body plans have survived to generation 53.

#### Species: *Cursor minor* ("the lesser runner")

**Specimens**: Genotype #100736 (Island 4, fitness 7.79), #160790 (Island 1, 7.09), #161757 (Island 2, 7.03)
**Morphology**: 2-body plan. A flat or blocky root with a single appendage connected via revolute or twist joint. The champion on island 4 has a flat root (0.34 x 0.08 x 0.44) with a blocky revolute arm (0.24 x 0.24 x 0.11). Islands 1 and 2 use larger root bodies (0.79 x 0.29 x 0.37 and 0.82 x 0.30 x 0.38) with smaller twist appendages.
**Locomotion**: Galloper. Same fundamental strategy as Wave 1's *Cursor velox*, but simplified -- only 2 bodies instead of 4. The asymmetric joint oscillation creates a rocking gait that covers ground efficiently.
**Comparison to Wave 1**: *C. velox* achieved fitness 27.36 with its 4-body plan (but over 200 generations). *C. minor* at 7.79 after only ~53 generations is on a promising trajectory. The body plan is a simplified version -- evolution has converged on the same principle (asymmetric oscillation) but in a more minimal form.

**Tags**: `species:cursor-minor`, `locomotion:galloper`

#### Species: *Cursor torsus* ("the twisting runner")

**Specimen**: Genotype #159812 (Island 0, fitness 4.55)
**Morphology**: 2-body plan, but distinctly different proportions. A small root (0.13 x 0.09 x 0.29) with a very long twist arm (0.82 x 0.13 x 0.09). The arm is nearly 6x the root's X-dimension.
**Locomotion**: Twist-roller. Instead of rocking like *C. minor*, this creature uses its long arm as a lever, rotating via the twist joint to push itself along the ground. Think of a person lying on the ground and spinning -- the asymmetric shape converts rotational motion into linear displacement.
**Significance**: This is a NEW locomotion strategy not seen in Wave 1. Island 0 has independently evolved a twist-based movement on land.

**Tags**: `species:cursor-torsus`, `locomotion:twist-roller`

#### Species: *Cursor erectus* ("the upright runner")

**Specimen**: Genotype #149419 (Island 6, fitness 3.49)
**Morphology**: 2-body plan. Distinctive tall, upright root body (0.25 x 0.48 x 0.17 -- Y is the dominant axis) with a blocky revolute limb (0.35 x 0.26 x 0.25). This is the only land creature in the park where the root body is taller than it is wide.
**Locomotion**: Tumbler. The tall body is inherently unstable. The revolute limb acts as a counterweight, and the creature exploits its own tendency to topple. It falls, catches itself with the limb, and falls again. Controlled collapse as locomotion.
**Significance**: Tumbling was seen in Wave 1 only for light-following (*Heliotropus*). Here it has evolved independently for pure speed. Convergent strategy, divergent context.

**Tags**: `species:cursor-erectus`, `locomotion:tumbler`

#### Species: *Tetrapus savannus* ("the four-footed savanna creature")

**Specimen**: Genotype #147472 (Island 3, fitness 6.14)
**Morphology**: 4 bodies, 3 joints -- the most complex land creature in Wave 2. A long, extremely flat root (1.06 x 0.14 x 0.05, TwistBend joint type) with two revolute legs (0.23 x 0.42 x 0.07 and 0.15 x 0.28 x 0.04) and a long rigid boom (1.52 x 0.10 x 0.07) extending from the root's NegX face. The recursive connection (node 1 to itself) produces the second leg at depth 2.
**Locomotion**: Multi-limbed galloper. The two revolute legs pump alternately while the rigid boom provides directional stability. The flat root serves as a chassis. This is the closest thing to a "real" walking animal in the park -- bilateral appendages on a central body.
**Distinguishing features**: The rigid boom is remarkable. At 1.52 half-extents in X, it is by far the longest single body segment in any land creature. It acts like a tail or outrigger, preventing the creature from spinning in circles. Evolution has discovered the stabilizing function of a tail.

**Tags**: `species:tetrapus-savannus`, `locomotion:galloper`, `notable:morphological-novelty`

#### Savanna Island Diversity Summary

| Island | Champion | Fitness | Bodies | Locomotion |
|--------|----------|---------|--------|------------|
| 0 | *C. torsus* | 4.55 | 2 | Twist-roller |
| 1 | *C. minor* | 7.09 | 2 | Galloper |
| 2 | *C. minor* | 7.03 | 2 | Galloper |
| 3 | *T. savannus* | 6.14 | 4 | Multi-limbed galloper |
| 4 | *C. minor* | 7.79 | 2 | Galloper |
| 5 | *C. minor* | 7.79 | 2 | Galloper (migrated from 4) |
| 6 | *C. erectus* | 3.49 | 2 | Tumbler |
| 7 | (unnamed) | 2.88 | 2 | Twist-roller variant |

Four distinct species across 8 islands. This is a dramatic improvement over Wave 1, where all islands converged to a single champion.

---

### Evolution 10: The Deep (Water/Speed, pop 200, 8 islands)

#### Species: *Remigia profunda* ("the deep oar")

**Specimens**: #166886 (Island 6, fitness 17.50), #159342 (Island 3, 15.71), #150117 (Island 4, 15.59), #159438 (Island 7, 13.86)
**Morphology**: 2-body paddle swimmers. Small compact root body connected via revolute joint to a large, flat fin elongated in Y and Z. The champion (#166886) has a root of (0.11 x 0.14 x 0.39) and a fin of (0.09 x 0.27 x 0.60). Remarkably similar to Wave 1's *Remigia simplex* -- the fin is long in Z (swimming direction) and tall in Y (paddle stroke axis).
**Locomotion**: Paddle swimmer. Identical strategy to Wave 1. The revolute joint oscillates the fin, creating thrust.
**Comparison to Wave 1**: *R. simplex* achieved 14.99 over 200 generations. *R. profunda* has already reached 17.50 in only ~52 generations. The larger population (200 vs 100) and 8 islands vs 5 are providing better optimization. The body plan has converged to the same solution -- strong evidence that this is a genuine global optimum for 2-body water locomotion.

**Tags**: `species:remigia-profunda`, `locomotion:paddle-swimmer`, `notable:highest-fitness-deep`, `favorite`

#### NEW SPECIES: *Scolopendra aquatilis* ("the water centipede")

**Specimen**: Genotype #166734 (Island 0, fitness 6.31)
**Morphology**: 7 bodies, 6 joints. This creature blew the doors off my expectations. The genome has a self-recursive connection (node 0 connects to itself with scale 2.14), producing a chain of repeating segments. Each segment consists of a flat body plate (0.20 x 0.05 x 0.30) rigidly linked to its successor, with a tall, thin revolute fin (0.13 x 0.72 x 0.10) hanging below each plate. Three such body-fin pairs chain together, plus the initial root segment with its own fin.
**Brain**: The spine node has Sigmoid and Memory neurons with sensor feedback -- this is not a simple oscillator. The Memory neuron suggests the creature can maintain state across timesteps, potentially enabling coordinated wave-like motion along the chain.
**Locomotion**: Centipede swimmer. The repeating fin-segments create an undulating wave along the body length. Each fin beats independently but is driven by the same neural template with sensor feedback, allowing phase-locked oscillation to emerge. This is the first CHAIN-BASED locomotion in the park. It looks like a swimming centipede -- or more accurately, like a polychaete worm with parapodia.
**Significance**: THIS IS THE SPECIES I HAVE BEEN WAITING FOR. In Round 1, I lamented the absence of snake-like chains. Island 0 of The Deep, protected by slower migration from being overrun by the dominant paddle plan, has independently evolved undulating locomotion. At fitness 6.31 it is far behind the paddle swimmers (17.50), but it is a fundamentally different strategy. Given more generations, this body plan has enormous optimization potential -- real aquatic organisms use undulation for a reason.

**Tags**: `species:scolopendra-aquatilis`, `locomotion:centipede-swimmer`, `notable:morphological-novelty`, `notable:new-locomotion`

#### Species: *Scolopendra pinnata* ("the finned centipede")

**Specimen**: Genotype #165467 (Island 1, fitness 4.26)
**Morphology**: 6 bodies, 5 joints. Same self-recursive genome architecture as *S. aquatilis*, but with taller, thinner fins (0.26 x 0.92 x 0.03 -- nearly a full meter tall!). The chain has 3 spine segments, each with an enormous sail-like fin hanging from a revolute joint.
**Locomotion**: Centipede swimmer variant. The taller fins may provide more thrust per stroke but are likely less hydrodynamically efficient (too much drag on recovery stroke). This explains the lower fitness.
**Significance**: Two centipede species on adjacent islands. Island 0 and Island 1 have independently converged on chain-based undulation, with different fin proportions. This is speciation in action -- the same body plan diverging in morphological details.

**Tags**: `species:scolopendra-pinnata`, `locomotion:centipede-swimmer`

#### Deep Island Diversity Summary

| Island | Champion | Fitness | Bodies | Locomotion |
|--------|----------|---------|--------|------------|
| 0 | *S. aquatilis* | 6.31 | 7 | Centipede swimmer |
| 1 | *S. pinnata* | 4.26 | 6 | Centipede swimmer |
| 2 | (unnamed) | 0.59 | 3 | Struggling |
| 3 | *R. profunda* | 15.71 | 2 | Paddle swimmer |
| 4 | *R. profunda* | 15.59 | 2 | Paddle swimmer |
| 5 | *R. torsilis* | 2.37 | 2 | Twist swimmer |
| 6 | *R. profunda* | 17.50 | 2 | Paddle swimmer |
| 7 | *R. profunda* | 13.86 | 2 | Paddle swimmer |

Three distinct locomotion strategies across 8 islands. The centipede swimmers are confined to islands 0-1, the paddle swimmers dominate islands 3-4-6-7. Island 5 has a twist variant. The migration barrier has allowed genuine coexistence of competing strategies.

---

### Evolution 11: The Lighthouse (Land/LightFollowing, pop 200, 6 islands)

The Lighthouse has produced the most morphological diversity of any single evolution. Six islands, at least five distinct body plans. The light-following task seems to reward body plan experimentation more than the speed tasks.

#### Species: *Heliotropus planus* ("the flat sun-turner")

**Specimen**: Genotype #152776 (Island 3, fitness 2.34)
**Morphology**: 2-body plan. A tiny, flat root (0.11 x 0.025 x 0.04 -- at the minimum Y extent) with a wide twist-connected paddle (0.44 x 0.07 x 0.27). The root's Universal joint type suggests the genome encodes multi-axis freedom, though only the twist to the paddle is expressed.
**Locomotion**: Twist-tumbler. The tiny root provides almost no resistance; the creature is essentially a flat paddle that twists and topples toward the light.

**Tags**: `species:heliotropus-planus`, `locomotion:twist-tumbler`, `notable:highest-fitness-lighthouse`

#### NEW SPECIES: *Heliotropus articulatus* ("the jointed sun-turner")

**Specimen**: Genotype #169519 (Island 4, fitness 2.34)
**Morphology**: 4 bodies, 3 joints. A tiny universal root, a wide twist paddle (0.60 x 0.03 x 0.27), then two more universal-jointed segments trailing behind (0.09 x 0.03 x 0.03 and 0.08 x 0.03 x 0.03). This is an articulated chain -- a flat paddle with a segmented tail.
**Locomotion**: Articulated tumbler. The universal joints at the tail provide multi-axis steering. The paddle does the heavy lifting (toppling), while the tail segments fine-tune the direction. This is the first creature in the park to show a clear separation between propulsion (paddle) and steering (tail).
**Significance**: Functional differentiation of body segments. The front moves; the back steers. This division of labor is a hallmark of biological sophistication.

**Tags**: `species:heliotropus-articulatus`, `locomotion:articulated-tumbler`, `notable:morphological-novelty`

#### NEW SPECIES: *Heliotropus ramosus* ("the branching sun-turner")

**Specimen**: Genotype #157992 (Island 1, fitness 1.99)
**Morphology**: 6 bodies, 5 joints -- the most complex light-follower in the park. A long root slab (0.67 x 0.15 x 0.18) branches into three children: a TwistBend arm (0.09 x 0.08 x 0.17), a Twist fin (0.04 x 0.42 x 0.05), and a large rigid extension (0.76 x 0.19 x 0.23). The rigid extension then has its own TwistBend arm, which has its own Twist fin. The structure is bilaterally echoed -- two arm-fin pairs at different depths.
**Locomotion**: Complex articulated movement. The twin TwistBend arms with their trailing twist fins act like two antennae, sensing and reaching toward the light from different positions on the body.
**Significance**: The branching architecture is unprecedented. This is not a chain, not a paddle, not a simple tumbler. It is a genuinely complex multi-limbed organism with distributed actuation.

**Tags**: `species:heliotropus-ramosus`, `locomotion:articulated-tumbler`, `notable:morphological-novelty`

#### Species: *Heliotropus magnus* ("the great sun-turner")

**Specimen**: Genotype #144346 (Island 5, fitness 2.25)
**Morphology**: 3 bodies, 2 joints. Tiny spherical root (0.09 x 0.06 x 0.08 -- nearly cubical), twist-connected to a flat arm (0.29 x 0.26 x 0.03), which TwistBend-connects to a massive terminal plate (0.67 x 0.22 x 0.55). The terminal plate is by far the largest body, like a flag on a pole.
**Locomotion**: Plate-tumbler. The massive end plate acts as a sail and counterweight. The small root is just a pivot point; the creature is dominated by its terminal mass.
**Distinguishing features**: The Spherical root joint type is unique in the park. Also, the "inverted pyramid" mass distribution (smallest at base, largest at tip) is an unusual evolutionary solution.

**Tags**: `species:heliotropus-magnus`, `locomotion:plate-tumbler`, `notable:morphological-novelty`

#### Species: *Heliotropus bifidus* ("the forked sun-turner")

**Specimen**: Genotype #169464 (Island 2, fitness 1.27)
**Morphology**: 3 bodies, 2 joints. A tall, flat root (0.17 x 0.53 x 0.07) with TWO twist children branching from it: a large block (0.17 x 0.27 x 0.57) and a smaller arm (0.05 x 0.17 x 0.26). Both are twist-connected.
**Locomotion**: Dual-arm tumbler. The two independent twist arms provide differential steering -- the creature can bias its topple direction by activating one arm more than the other.

**Tags**: `species:heliotropus-bifidus`, `locomotion:dual-arm-tumbler`

#### Lighthouse Island Diversity Summary

| Island | Champion | Fitness | Bodies | Locomotion |
|--------|----------|---------|--------|------------|
| 0 | *H. terrestris* (W2) | 1.91 | 3 | Tumbler |
| 1 | *H. ramosus* | 1.99 | 6 | Articulated branching |
| 2 | *H. bifidus* | 1.27 | 3 | Dual-arm tumbler |
| 3 | *H. planus* | 2.34 | 2 | Twist-tumbler |
| 4 | *H. articulatus* | 2.34 | 4 | Articulated chain |
| 5 | *H. magnus* | 2.25 | 3 | Plate-tumbler |

SIX islands, FIVE distinct species. The fitness spread is narrow (1.27 to 2.34) but the morphological diversity is extraordinary. The light-following task has not produced a single dominant strategy, because the problem is harder -- it rewards navigation, not just speed. Multiple body plans are competitive.

---

### Evolution 12: The Coral Reef (Water/LightFollowing, pop 150, 6 islands)

#### Species: *Photonautes brachialis* ("the arm-bearing light-sailor")

**Specimens**: #155614 (Island 4, fitness 1.42), #177793 (Island 5, 1.45), #177743 (Island 3, 1.36)
**Morphology**: 3-body plan. A blocky root (0.34 x 0.23 x 0.11) with TWO twist-connected children: a small arm (0.18 x 0.08 x 0.14) and a tall fin (0.06 x 0.26 x 0.28). The fin is thin in X but substantial in Y and Z -- a flat panel for water interaction. The small arm may serve as a sensor platform or steering rudder.
**Locomotion**: Dual-twist swimmer. Both appendages rotate via twist joints, creating asymmetric water flow. The fin provides thrust while the arm provides directional control.
**Significance**: This is a NEW genus. It does not match any Wave 1 body plan. The dual-appendage arrangement with functional differentiation (propulsion vs. steering) is reminiscent of *H. articulatus* on land but has evolved independently in water.

**Tags**: `species:photonautes-brachialis`, `locomotion:twist-swimmer`, `notable:highest-fitness-reef`

#### Species: *Photonautes remigans* ("the rowing light-sailor")

**Specimens**: #179715 (Island 0, fitness 1.26), #166060 (Island 1, 1.27)
**Morphology**: 2-body plan. Small root with a large revolute paddle. Nearly identical to *Remigia profunda* in body plan, but used for light-following rather than pure speed. The paddle on #166060 is particularly large (0.88 x 0.24 x 0.23).
**Locomotion**: Paddle swimmer, adapted for phototaxis.
**Significance**: Convergent evolution across task boundaries. The paddle-swimmer body plan has evolved independently for water speed (*Remigia*) and water light-following (*Photonautes remigans*). Same body, different purpose.

**Tags**: `species:photonautes-remigans`, `locomotion:paddle-swimmer`

#### Coral Reef Island Diversity Summary

| Island | Champion | Fitness | Bodies | Locomotion |
|--------|----------|---------|--------|------------|
| 0 | *P. remigans* | 1.26 | 2 | Paddle swimmer |
| 1 | *P. remigans* | 1.27 | 2 | Paddle swimmer |
| 2 | *P. remigans* | 1.27 | 2 | Paddle swimmer (migrated from 1) |
| 3 | *P. brachialis* | 1.36 | 3 | Dual-twist swimmer |
| 4 | *P. brachialis* | 1.42 | 3 | Dual-twist swimmer |
| 5 | *P. brachialis* | 1.45 | 3 | Dual-twist swimmer |

A clean geographic split: islands 0-2 are *P. remigans* territory (paddle swimmers), islands 3-5 are *P. brachialis* territory (dual-twist swimmers). Two species coexisting in the same evolution, geographically separated. Classic allopatric speciation.

---

### Biodiversity Assessment: Wave 2

**New species described**: 12
**New genera**: 3 (*Tetrapus*, *Scolopendra*, *Photonautes*)
**New locomotion strategies**: 4 (twist-roller, centipede-swimmer, articulated-tumbler, dual-twist-swimmer)
**Total species in park (cumulative)**: 18 named across both waves

#### Wave 2 Locomotion Catalog

| Strategy | Species | Environment | Bodies | New? |
|----------|---------|-------------|--------|------|
| Galloper | *C. minor*, *T. savannus* | Land speed | 2-4 | No (simplified from W1) |
| Twist-roller | *C. torsus* | Land speed | 2 | YES |
| Tumbler (speed) | *C. erectus* | Land speed | 2 | Context-new |
| Paddle swimmer | *R. profunda*, *P. remigans* | Water | 2 | No (convergent) |
| Centipede swimmer | *S. aquatilis*, *S. pinnata* | Water speed | 6-7 | YES |
| Twist swimmer | *R. torsilis*, *P. brachialis* | Water | 2-3 | YES |
| Twist-tumbler | *H. planus* | Land light | 2 | Variant |
| Plate-tumbler | *H. magnus* | Land light | 3 | YES |
| Articulated tumbler | *H. articulatus*, *H. ramosus* | Land light | 4-6 | YES |
| Dual-arm tumbler | *H. bifidus* | Land light | 3 | YES |

#### Key Findings

**1. Slower migration works.** Wave 1 produced 4 species across 4 evolutions. Wave 2 has produced 12+ across 4 evolutions. The slower migration rate has allowed islands to maintain distinct lineages long enough for genuine morphological divergence. This is the single most important finding.

**2. The centipede swimmers are the discovery of the survey.** *Scolopendra aquatilis* is the first chain-based undulating creature in the park. Its self-recursive genome producing a 7-body chain with repeating fin segments is exactly the kind of body plan Sims 1994 described but that we had not yet seen emerge. It is currently uncompetitive (6.31 vs 17.50 for the paddle plan) but the architecture has room to optimize.

**3. Light-following drives diversity more than speed.** The Lighthouse (land light) has 5 distinct species across 6 islands; The Savanna (land speed) has 4 across 8. Speed tasks have a clearer fitness gradient -- go fast -- that narrows the solution space. Light-following requires solving navigation, which admits more diverse solutions.

**4. Convergent evolution is real.** The paddle-swimmer body plan has now evolved independently in 3 separate evolutions (*R. simplex* in Wave 1, *R. profunda* in Wave 2, *P. remigans* in Coral Reef). This is strong evidence that the 2-body revolute-fin design is a genuine attractor in the fitness landscape for aquatic locomotion.

**5. Functional differentiation emerges under complexity pressure.** *H. articulatus* separates propulsion from steering. *P. brachialis* has a thrust fin and a control arm. *T. savannus* has legs and a stabilizing tail. When creatures grow beyond 2 bodies, the additional parts tend to specialize rather than replicate. This is the beginning of modularity.

#### Diversity Rating: MODERATE-HIGH

Wave 2 is a genuinely diverse park. Within-evolution diversity has jumped from essentially zero (Wave 1) to 2-5 coexisting strategies. The island model is working as intended. There are still gaps -- no snake-like chain on land, no radial symmetry, no true bipedal locomotion -- but the building blocks are present in the genome architectures.

*"This is what I came here to see. Not just four optimized machines, each alone in its cage -- but a living ecosystem where different solutions coexist, compete, and find their own niches. Island 0 of The Deep has a centipede while Island 6 has a paddle swimmer. They live in the same ocean but they found different answers. That is evolution. That is what makes this worth doing."*

