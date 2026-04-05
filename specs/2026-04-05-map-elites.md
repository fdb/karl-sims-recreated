# MAP-Elites: Quality-Diversity Evolution

**Status**: Proposed
**Date**: 2026-04-05
**Prerequisite**: Islands model (landed in `feat(evolution): islands-model GA with migration + per-island UI`)

## Why

Islands give us N parallel species — 6–12 divergent gaits from a single run.
That's good for "multiple species emerge", but islands still reward
*fitness within each niche*: each island collapses to its own single elite.

**MAP-Elites** answers a different question: *what are the best creatures
we can find in each region of behavior space?* Instead of one elite per
island, you get one elite per **bin** of a behavior grid — potentially
hundreds of distinct, niche-optimal creatures from a single run.

From Mouret & Clune 2015 ("Illuminating Search Spaces with MAP-Elites"):
the algorithm *illuminates* the map — you literally see which combinations
of behavior traits are achievable, and how fit each one can be. It's a
visual, navigable zoo of strategies instead of a leaderboard.

## What this unlocks

- **"Show me the best short creature" / "the best tall one"** — query
  any bin and get the champion for that morphology
- **Locomotion style atlas** — heatmap where rows = body-part-count,
  columns = height-above-ground, fills tell you which shapes are viable
- **Stepping-stone discovery** — creatures in "weak" bins may be
  ancestors of high-fitness creatures in neighboring bins; MAP-Elites
  preserves them as mutation parents where a fitness-only GA would delete
  them in one generation
- **Illuminated failure modes** — empty bins are visible gaps in the
  grid, pointing to morphologies evolution hasn't reached yet

## How it differs from islands

| | Islands | MAP-Elites |
|---|---|---|
| Population structure | N sub-populations by index | 1 global archive, indexed by behavior |
| Selection pressure | Local within each island | Global (any filled bin) |
| Diversity mechanism | Migration between islands | Bin independence |
| Replacement rule | Fitness-proportional in own island | Win the bin by fitness, or fill empty bin |
| Output | N elites | Up to K_total bins' worth of elites (often 100s) |
| Mutation parents | Sampled from own island | Sampled uniformly from filled bins |
| Natural metric | Per-island best | Coverage (% filled bins) + QD-score |

Islands and MAP-Elites are **not mutually exclusive** — MAP-Elites
can replace the classical GA entirely, or run alongside it as a
separate evolution mode.

## Behavior descriptor (the grid axes)

We need 2–3 numeric traits that summarize a creature's *behavior* (not
genotype — two genomes with identical behavior belong in the same bin).
Each trait gets discretized into N bins, product is the total grid size.

**Proposed axes** (start with 2 for simplicity):

### Axis 1: `avg_height` — typical center-of-mass height during gait
- Range: 0.0 m – 2.0 m
- Bins: 16
- Computed as: mean root-body Y during seconds 2..sim_duration
  (skip the first 2 s of settling-in)
- Discriminates: crawlers (low) from walkers (mid) from hoppers (high)

### Axis 2: `horizontal_efficiency` — distance traveled per unit
absolute motion
- Range: 0.0 – 1.0
- Bins: 10
- Computed as: `final_horizontal_distance / total_path_length`
- total_path_length = sum of per-frame root displacements
- Discriminates: straight-line efficient (near 1.0) from
  oscillating/spinning (near 0.0)

**Total bins**: 16 × 10 = 160 cells

### Alternative/additional axis candidates

- `num_bodies` — morphology complexity (discrete, 1 bin per count up to 20)
- `aspect_ratio` — max dimension / min dimension of bounding box
- `sideways_motion` — |lateral_drift / forward_distance|
- `joint_usage_entropy` — how evenly actuation is distributed across joints
- `total_body_mass` — proxy for size budget spent

We should pick axes that:
1. Correlate weakly with fitness (otherwise MAP-Elites collapses to a
   fitness-sorted 1D list)
2. Are cheap to compute (ideally from existing trace data)
3. Discriminate interesting behaviors a human can describe verbally

Avoid axes that are noise-dominated (e.g. final_x alone — a creature
that walks left vs right isn't meaningfully "different").

## Data model

New table: `map_elites_cells`.

```sql
CREATE TABLE map_elites_cells (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    evolution_id    INTEGER NOT NULL REFERENCES evolutions(id),
    -- Bin coordinates in the grid. NULL = this axis isn't used.
    bin_axis0       INTEGER NOT NULL,
    bin_axis1       INTEGER NOT NULL,
    bin_axis2       INTEGER,
    -- The current elite of this cell.
    genotype_id     INTEGER NOT NULL REFERENCES genotypes(id),
    fitness         REAL    NOT NULL,
    -- Behavior descriptors (exact values, not binned) for diagnostics.
    bd_axis0        REAL    NOT NULL,
    bd_axis1        REAL    NOT NULL,
    bd_axis2        REAL,
    updated_at      TEXT    NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX idx_map_elites_bin
ON map_elites_cells(evolution_id, bin_axis0, bin_axis1, bin_axis2);
```

Notes:
- One row per filled cell (empty cells are absent — sparse representation)
- `genotype_id` points to the winning creature; we can always recover
  its genome from `genotypes` table
- `bd_axis*` stored for tooltip display and re-binning (if we want to
  change grid resolution later without re-evaluating creatures)

We also extend the `genotypes` table with behavior descriptor fields:

```sql
ALTER TABLE genotypes ADD COLUMN bd_axis0 REAL;
ALTER TABLE genotypes ADD COLUMN bd_axis1 REAL;
ALTER TABLE genotypes ADD COLUMN bd_axis2 REAL;
```

This lets fitness-evaluation write the BD alongside fitness in one pass.

## Evolution mode

Add a new field to `EvolutionParams`:

```rust
/// Evolution search mode. None = classical fitness-based GA (islands
/// model). MapElites enables quality-diversity search with a 2- or
/// 3-axis behavior grid.
#[serde(default)]
pub search_mode: SearchMode,

pub enum SearchMode {
    /// Classical GA (with islands if num_islands > 1). Default.
    Classical,
    /// MAP-Elites quality-diversity. Replaces islands.
    MapElites { grid: MapElitesGrid },
}

pub struct MapElitesGrid {
    pub axes: Vec<MapElitesAxis>,  // 2 or 3 axes
}

pub struct MapElitesAxis {
    pub name: String,         // e.g. "avg_height"
    pub bins: usize,          // discretization resolution
    pub min: f64,
    pub max: f64,
}
```

When `search_mode = MapElites`, the coordinator runs a **different loop**:

```
initial: generate `population_size` random creatures → evaluate → place
         each into its bin (keep if empty OR higher fitness than current occupant)

for each iteration step:
    batch = pop a random sample of filled cells (uniform over cells, not fitness)
    for each cell in batch:
        mutate its elite → evaluate → place into its new (possibly different) bin
```

Key differences from classical GA:
- **No generation boundaries** — evolution is continuous, one genome at a time
- **Parents sampled uniformly from filled cells**, not by fitness
- **Placement rule**: `if bin_is_empty OR new_fitness > current_fitness: replace`
- **No "survivors" concept** — the grid IS the population

## Fitness evaluation changes

The `evaluate_fitness` function must now also return the behavior
descriptor. Extend `FitnessResult`:

```rust
pub struct FitnessResult {
    pub score: f64,
    pub distance: f64,
    pub max_displacement: f64,
    pub terminated_early: bool,
    // NEW:
    pub behavior: Option<[f64; 3]>,  // None for terminated_early
}
```

Computing the behavior descriptor during `evaluate_speed_fitness`:
- Track `path_length: f64` — sum of per-frame root displacement
- Track `height_sum: f64`, `height_samples: usize` — for avg_height
- At end: `avg_height = height_sum / height_samples`,
  `horizontal_efficiency = horizontal_distance / path_length`

For rejected creatures (score=0, terminated_early), return `behavior: None`
and skip placement in the grid. MAP-Elites doesn't track garbage.

## Coordinator changes

Add a `run_map_elites` function alongside `run_evolution`, dispatched by
`search_mode`. Shared infrastructure: task queue, workers, DB persistence.

```rust
pub async fn run_map_elites(db: DbPool, evo_id: i64, grid: MapElitesGrid,
                            tx: Option<broadcast::Sender<String>>) {
    // Initial seed: random creatures, evaluate all
    seed_initial_population(db, evo_id, grid.pop_size).await;

    loop {
        // Wait for all pending tasks to complete
        wait_for_pending(db, evo_id).await;

        // For each completed task: place its creature in the grid
        for (gid, fitness, behavior) in read_completed_results(db, evo_id) {
            if let Some(bd) = behavior {
                let cell = grid.bin_for(&bd);
                maybe_place_in_cell(db, evo_id, cell, gid, fitness, bd);
            }
        }

        // Generate a new batch of offspring from filled cells
        let parents = sample_parents_from_grid(db, evo_id, batch_size);
        for parent_gid in parents {
            let mutated = mutate(load_genome(parent_gid));
            let child_gid = insert_genotype(db, evo_id, /* generation */ -1, bytes, None, /* island_id */ 0);
            create_task(db, evo_id, child_gid);
        }
    }
}
```

`generation` field in `genotypes` stays as-is but means nothing for
MAP-Elites — we can use it as an "evaluation batch counter" for
stats/debugging. Alternatively: set `generation = -1` for MAP-Elites
creatures and disambiguate in queries.

Better: **add `evaluation_index INTEGER` to genotypes** that increments
per creature created, replacing generation's role for MAP-Elites.

## API endpoints

```
GET  /api/evolutions/:id/map_elites       → full grid state
GET  /api/evolutions/:id/map_elites/cell/:bin0_:bin1 → cell detail
```

Response shape for the grid:

```json
{
  "axes": [
    { "name": "avg_height", "bins": 16, "min": 0.0, "max": 2.0 },
    { "name": "horizontal_efficiency", "bins": 10, "min": 0.0, "max": 1.0 }
  ],
  "cells": [
    { "bin_axis0": 5, "bin_axis1": 8, "genotype_id": 1234, "fitness": 12.3,
      "bd_axis0": 0.62, "bd_axis1": 0.85 },
    ...
  ],
  "coverage": 0.47,          // filled bins / total bins
  "total_bins": 160,
  "qd_score": 1847.5         // sum of fitness over filled cells
}
```

## UI

New route: `/evolutions/:evoId/map`

**Main visualization**: grid heatmap, one cell per bin.
- Rows = axis 1 (e.g. `horizontal_efficiency`, 10 rows)
- Columns = axis 0 (e.g. `avg_height`, 16 columns)
- Color intensity ∝ cell fitness (normalized to global max)
- Empty cells drawn dimmer (indicate unreached behavior)
- Click any cell → creature detail page (reuses existing
  `/evolutions/:evoId/creatures/:creatureId` but with a
  `?cell=row_col` query param for back-navigation)

**Axis labels**: axis name + domain below/left.

**Legend**: fitness gradient scale, total coverage %, QD-score.

**Hover tooltip**: exact BD values, fitness, genotype id.

## Example UX narrative

1. User creates evolution with `search_mode=MapElites`, axes
   = [avg_height 0–2m/16 bins, efficiency 0–1/10 bins], `pop_size=50`,
   `max_evaluations=10000`.
2. Server seeds 50 random creatures, workers evaluate, each creature is
   placed in (its bin) or discarded.
3. Coordinator loops: sample 50 filled cells, mutate their elites,
   evaluate the 50 new offspring, place each.
4. Every ~1 s the UI polls the map endpoint and redraws the heatmap.
5. User watches the grid fill: first, low-height/low-efficiency
   (static creatures) appears. Over minutes, higher-height cells fill as
   mutations find jumping morphologies. After an hour: a rich map
   showing the Pareto-ish frontier of achievable (height, efficiency)
   combinations.
6. User clicks the high-fitness cell at (height=1.2m, eff=0.9) and
   sees a creature standing upright moving in a straight line.

## Open questions

- **Parent sampling strategy**: uniform over filled cells (Mouret's
  default) or biased toward boundaries of filled regions
  (like curiosity-driven exploration)? Start with uniform.
- **Batch size per generation**: 50 is reasonable and keeps worker
  throughput high. Could scale with worker count.
- **Stopping criterion**: `max_evaluations` (total creature count) is
  more natural than `max_generations` for MAP-Elites. Need a new config
  field.
- **Bin resolution changes mid-run**: if we decide later to use 32×20
  bins instead of 16×10, we'd re-bin all existing creatures using
  their stored `bd_axis*` fields. No re-evaluation needed. That's why
  we store raw BD values.
- **Do we need islands + MAP-Elites simultaneously?** Probably not —
  islands give diversity by population structure; MAP-Elites gives
  diversity by behavior. Pick one. If combining: one grid per island.

## Implementation milestones

### Milestone A — Data plumbing (2–3 hours)
1. Schema: `ALTER TABLE genotypes ADD COLUMN bd_axis0/1/2 REAL`
2. Schema: `CREATE TABLE map_elites_cells`
3. Extend `FitnessResult` with `behavior: Option<[f64; 3]>`
4. Compute BD in `evaluate_speed_fitness` (path_length + height_sum loops)
5. Tests: BD values are computed, bins-for-BD math is correct

### Milestone B — Coordinator (3–4 hours)
1. `SearchMode` enum in `EvolutionParams`
2. `run_map_elites` function next to `run_evolution`
3. Dispatch by `search_mode` in coordinator entry point
4. `maybe_place_in_cell` DB helper (atomic UPSERT with fitness comparison)
5. `sample_parents_from_grid` DB helper (random filled cells)
6. Worker stores BD into `genotypes` via extended `complete_task`
7. Test: seeded run for 200 evaluations converges coverage > 30%

### Milestone C — API + UI (2–3 hours)
1. `GET /api/evolutions/:id/map_elites` endpoint
2. `MapGrid.tsx` heatmap component with hover/click handlers
3. Route `/evolutions/:id/map` with grid view
4. Config form: search mode selector, axis selection UI
5. Breadcrumb + back-link from creature detail when coming from grid

### Milestone D — Polish (2 hours)
1. WebSocket broadcast on cell updates (real-time heatmap)
2. Coverage % / QD-score shown on evolution detail card
3. Restart handling (resume MAP-Elites after server crash)
4. `karl-sims-debug` support for dumping map state as JSON

**Total**: ~10 hours of focused work. Ship Milestone A + B as one commit,
C + D as a second commit.

## Non-goals for the first implementation

- **CVT-MAP-Elites** (centroidal Voronoi tessellation for non-uniform
  bins) — grid-based is simpler and sufficient at our scale
- **CMA-ME** (covariance matrix adaptation emitters) — Gaussian
  mutation is fine
- **Multi-emitter schemes** (improvement emitters, curiosity emitters)
  — uniform sampling first, specialize later if it plateaus
- **Cross-grid migration** between MAP-Elites and islands
  (architectural complexity, unclear benefit)

## References

- Mouret, J.-B. & Clune, J. (2015). *Illuminating search spaces by
  mapping elites.* arXiv:1504.04909
- Cully, A. et al. (2015). *Robots that can adapt like animals.*
  Nature 521, 503–507 (MAP-Elites for damage recovery — the paper
  that put QD on the map)
- Pyribs (Python QD library) — reference implementation patterns:
  https://pyribs.org/
