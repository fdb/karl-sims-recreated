# M5: Genetic Algorithm — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Evolve swimming creatures via genetic algorithm: mutation, crossover, grafting, selection by swimming speed fitness, with an in-browser mini evolution to validate the full pipeline.

**Architecture:** New `evolution.rs` module handles population management, selection, and reproduction. `mutation.rs` implements the 5 mutation operators from the paper. `mating.rs` implements crossover and grafting. `fitness.rs` evaluates swimming speed. All operate on `GenomeGraph` from M4. The in-browser test runs a tiny evolution (pop 20, 5 generations) entirely in WASM.

**Tech Stack:** Rust, rand/rand_chacha for deterministic RNG, existing core crate types

---

## File Structure

```
core/src/
├── lib.rs          # MODIFY: add pub mod mutation, mating, fitness, evolution
├── mutation.rs     # NEW: 5 mutation operators from the paper
├── mating.rs       # NEW: crossover + grafting operators
├── fitness.rs      # NEW: swimming speed fitness evaluation
├── evolution.rs    # NEW: population management, selection, reproduction loop
└── genotype.rs     # MODIFY: add helper methods for mutation/mating

web/src/
└── lib.rs          # MODIFY: add in-browser evolution scene

frontend/src/
└── App.tsx         # MODIFY: add evolution scene to dropdown
```

---

## Task 1: Mutation Operators

**Files:**
- Create: `core/src/mutation.rs`
- Modify: `core/src/lib.rs`

Implement the 5 mutation operators from the paper, applied to GenomeGraph:

1. **Node parameter mutation** — for each MorphNode: Gaussian perturbation on scalars (dimensions, limits), random pick for joint_type, flip for terminal_only. Mutation frequency scaled inversely with graph size.
2. **New random node added** — add an unconnected node (will be garbage collected if never connected).
3. **Connection parameter mutation** — for each MorphConn: perturb scale, flip reflection, occasionally move connection pointer to different node.
4. **Random connection add/remove** — each node subject to gaining a new connection, each connection subject to removal.
5. **Garbage collection** — remove nodes unreachable from root.

Also mutate nested brain graphs: perturb neuron weights, function types, add/remove neurons.

```rust
pub fn mutate<R: Rng>(genome: &mut GenomeGraph, rng: &mut R) {
    let graph_size = genome.nodes.len().max(1);
    let mutation_scale = 1.0 / graph_size as f64; // ≥1 mutation on average
    
    mutate_node_params(genome, rng, mutation_scale);
    maybe_add_node(genome, rng, mutation_scale);
    mutate_connection_params(genome, rng, mutation_scale);
    mutate_connections(genome, rng, mutation_scale);
    garbage_collect(genome);
    
    // Mutate nested brain graphs
    for node in &mut genome.nodes {
        mutate_brain(&mut node.brain, rng, mutation_scale);
    }
    mutate_brain(&mut genome.global_brain, rng, mutation_scale);
}
```

Tests:
- `mutation_changes_genome`: mutate a genome, verify at least one parameter differs
- `garbage_collection_removes_unreachable`: add disconnected node, GC removes it
- `mutation_preserves_valid_structure`: mutate 50 times, still valid indices

- [ ] **Step 1: Implement mutation.rs with all operators and 3 tests**
- [ ] **Step 2: Add `pub mod mutation;` to lib.rs**
- [ ] **Step 3: Run tests, commit**

---

## Task 2: Mating Operators

**Files:**
- Create: `core/src/mating.rs`
- Modify: `core/src/lib.rs`

Implement crossover and grafting:

**Crossover:** Align nodes of two parents in a row. Pick 1-2 crossover points. Child gets nodes from alternating parents. Connections copied with nodes; out-of-bounds target indices randomly reassigned.

**Grafting:** Copy first parent entirely. Pick a random connection, re-point its target to a node copied from the second parent. Append that node's connected subgraph from the second parent.

```rust
pub fn crossover<R: Rng>(parent1: &GenomeGraph, parent2: &GenomeGraph, rng: &mut R) -> GenomeGraph { ... }
pub fn graft<R: Rng>(parent1: &GenomeGraph, parent2: &GenomeGraph, rng: &mut R) -> GenomeGraph { ... }
```

Tests:
- `crossover_produces_valid_genome`: crossover two random genomes → valid structure
- `graft_produces_valid_genome`: graft two random genomes → valid structure
- `crossover_combines_traits`: child has nodes from both parents

- [ ] **Step 1: Implement mating.rs with crossover + grafting and 3 tests**
- [ ] **Step 2: Add `pub mod mating;` to lib.rs**
- [ ] **Step 3: Run tests, commit**

---

## Task 3: Swimming Fitness Function

**Files:**
- Create: `core/src/fitness.rs`
- Modify: `core/src/lib.rs`

Evaluate swimming speed fitness:
- Grow creature from genome
- Simulate for ~10 seconds (600 steps at 1/60)
- Fitness = weighted combination of:
  - Distance traveled (center of mass displacement per unit time)
  - Maximum displacement from initial position (anti-circling)
  - Continuing movement bonus (velocity during final phase weighted more)
- Early termination: stop if creature not moving after first 2 seconds
- Viability check: reject if > max_parts (e.g., 20)

```rust
pub struct FitnessConfig {
    pub sim_duration: f64,        // 10.0 seconds
    pub dt: f64,                  // 1/60
    pub max_parts: usize,         // 20
    pub early_termination_time: f64, // 2.0 seconds
    pub min_movement_threshold: f64, // 0.01
}

pub struct FitnessResult {
    pub score: f64,
    pub distance: f64,
    pub max_displacement: f64,
    pub final_velocity: f64,
    pub terminated_early: bool,
}

pub fn evaluate_swimming_fitness(genome: &GenomeGraph, config: &FitnessConfig) -> FitnessResult { ... }
```

Tests:
- `stationary_creature_low_fitness`: genome that produces no movement → near-zero fitness
- `moving_creature_positive_fitness`: genome with oscillating brain → positive fitness
- `viability_rejects_large_creatures`: genome with too many parts → zero fitness

- [ ] **Step 1: Implement fitness.rs with evaluate_swimming_fitness and 3 tests**
- [ ] **Step 2: Add `pub mod fitness;` to lib.rs**
- [ ] **Step 3: Run tests, commit**

---

## Task 4: Evolution Loop

**Files:**
- Create: `core/src/evolution.rs`
- Modify: `core/src/lib.rs`

Population management and selection:

```rust
pub struct EvolutionConfig {
    pub population_size: usize,      // 300
    pub survival_ratio: f64,         // 0.2 (1/5)
    pub asexual_ratio: f64,          // 0.4
    pub crossover_ratio: f64,        // 0.3
    pub grafting_ratio: f64,         // 0.3
    pub fitness: FitnessConfig,
}

pub struct Individual {
    pub genome: GenomeGraph,
    pub fitness: f64,
}

pub struct Population {
    pub individuals: Vec<Individual>,
    pub generation: usize,
    pub config: EvolutionConfig,
}

impl Population {
    pub fn random_initial<R: Rng>(config: EvolutionConfig, rng: &mut R) -> Self { ... }
    
    /// Run one generation: evaluate fitness → select survivors → reproduce.
    pub fn evolve_generation<R: Rng>(&mut self, rng: &mut R) { ... }
    
    pub fn best(&self) -> &Individual { ... }
    pub fn average_fitness(&self) -> f64 { ... }
}
```

The `evolve_generation` method:
1. Evaluate fitness for each individual (that hasn't been evaluated yet)
2. Sort by fitness descending
3. Select top `survival_ratio * population_size` as survivors
4. Generate offspring to fill the population back to `population_size`:
   - 40% asexual: copy + mutate
   - 30% crossover: pick two parents weighted by fitness, crossover + mutate (reduced rate)
   - 30% grafting: pick two parents, graft + mutate (reduced rate)
   - Number of offspring per survivor proportional to fitness

Tests:
- `initial_population_has_correct_size`: random population → correct count
- `generation_maintains_population_size`: after evolve_generation, same size
- `fitness_improves_over_generations`: run 3 generations with tiny pop, best fitness non-decreasing (not guaranteed but very likely)

- [ ] **Step 1: Implement evolution.rs with Population and evolve_generation, 3 tests**
- [ ] **Step 2: Add `pub mod evolution;` to lib.rs**
- [ ] **Step 3: Run tests, commit**

---

## Task 5: In-Browser Mini Evolution + Web UI

**Files:**
- Modify: `web/src/lib.rs`
- Modify: `frontend/src/App.tsx`

Add a "Mini Evolution" scene that runs a tiny evolution (pop 20, 5 generations) entirely in WASM and displays the best creature from the latest generation.

In `web/src/lib.rs`:
- Add `MiniEvolution` to SceneId
- When selected: run evolution synchronously (small pop, few gens), store best creature
- Display the best creature animating
- Export a `run_mini_evolution()` WASM function that returns generation stats

In `frontend/src/App.tsx`:
- Add "Mini Evolution" to dropdown
- When selected, show a status message while evolving

- [ ] **Step 1: Add mini evolution scene to web + frontend**
- [ ] **Step 2: Build and verify**
- [ ] **Step 3: Commit**

---

## Self-Review

**Spec coverage:**
- [x] Mutation operators (all 5) → Task 1
- [x] Mating operators (crossover + grafting) → Task 2
- [x] Reproduction ratios 40/30/30 → Task 4
- [x] Selection top 1/5, offspring proportional to fitness → Task 4
- [x] Viability checks (max parts, early termination) → Task 3
- [x] Population initialization from random seeds → Task 4
- [x] Swimming speed fitness → Task 3
- [x] In-browser mini evolution → Task 5
