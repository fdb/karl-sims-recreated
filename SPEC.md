# Evolving Virtual Creatures — Recreation of Karl Sims (SIGGRAPH '94)

## Overview

A faithful recreation of Karl Sims' "Evolving Virtual Creatures" (SIGGRAPH 1994), which co-evolves the morphology and neural control systems of 3D virtual creatures using genetic algorithms. Creatures are built from directed graphs of rectangular rigid parts connected by joints, with dataflow-graph neural "brains" controlling joint actuators. Fitness is evaluated via physics simulation.

**Initial goal:** Evolve swimming creatures optimized for straight-line speed, then progress to light-following behavior (swimming toward a moving target dot that repositions every ~5 seconds).

## Architecture

### Crate Layout (Rust workspace)

```
karl-sims/
├── core/       # Physics, genetics, brain, simulation (no_std-compatible, compiles to native + WASM)
├── server/     # Async runtime, SQLite, HTTP/WS API, worker orchestration
├── web/        # WASM bindings, wgpu renderer, JS interop
└── frontend/   # React + TypeScript UI shell
```

- **`core`**: The deterministic heart. Contains physics engine, genotype/phenotype representation, neural brain evaluation, genetic operators, and fitness evaluation. Must compile identically to native and `wasm32-unknown-unknown`. No platform-specific code. No floating-point non-determinism (no SIMD, no platform-dependent FP).
- **`server`**: Depends on `core`. Runs evolution loops, manages distributed fitness evaluation via SQLite task queue, exposes REST + WebSocket API. Built with tokio + axum (or similar).
- **`web`**: Depends on `core`. WASM module providing simulation playback and interactive sandbox in the browser. Renders via wgpu (WebGPU). Exposes bindings consumed by the React frontend.
- **`frontend`**: React + TypeScript. Hosts the wgpu canvas, provides evolution controls, creature gallery, fitness graphs, parameter tuning UI. Communicates with server via REST + WebSocket.

### Determinism Requirement

**Bit-exact reproducibility** between native (server) and WASM (browser) is required. The same genotype + same simulation parameters must produce identical physics frame-by-frame on both targets.

Implications:
- **Custom physics engine** (no Rapier) — full control over all floating-point operations.
- No SIMD — all math uses scalar f64 operations.
- No platform-dependent FP rounding — avoid operations where compilers may reorder or fuse (e.g., FMA). Use explicit intermediate variables where needed. Consider `#[cfg(target_arch = "wasm32")]` tests to verify cross-platform agreement.
- Fixed RNG seeding — all stochastic operations (mutation, selection) use a seeded PRNG (e.g., `rand_chacha`) for reproducibility.
- **Adaptive but deterministic timestep** — the RK4-Fehlberg adaptive step size logic from the paper is preserved, but the adaptation is purely a function of simulation state (same inputs → same step choices → same results).

---

## Creature Morphology (Genotype → Phenotype)

### Genotype Representation

The genotype is a **directed graph** stored as an **arena-based adjacency list**:

```
GenomeGraph {
    nodes: Vec<MorphNode>,       // arena-allocated
    connections: Vec<MorphConn>, // edges referencing node indices
    root: NodeIndex,
}
```

Each **`MorphNode`** contains:
- **Dimensions**: width, height, depth of the rectangular solid part.
- **Joint type**: rigid, revolute, twist, universal, bend-twist, twist-bend, or spherical (all 7 from the paper).
- **Joint limits**: min/max angle per DOF, beyond which restoring spring forces apply.
- **Recursive limit**: max times this node generates a phenotype part in a recursive cycle.
- **Terminal-only flag**: connection only applied at the end of a recursive chain.
- **Neural graph**: a nested `BrainGraph` (see below) describing local neurons for this part.

Each **`MorphConn`** contains:
- **Source/target node indices**.
- **Attachment position**: constrained to a face of the parent part.
- **Orientation, scale, reflection**: positioning of child relative to parent. Reflections enable symmetric sub-trees.

### Phenotype Development

The phenotype is grown by traversing the directed graph from the root node:
1. Create a 3D rigid part from the root node.
2. Follow connections to child nodes, recursively creating parts.
3. Recursive/cyclic connections produce repeated structures (chains, fractal limbs) up to the recursive limit.
4. Neural graphs are instantiated per part, with local neurons replicated along with the morphology.
5. Connections between neural elements of adjacent parts (parent ↔ child) are allowed.
6. Unassociated (centralized) neurons are instantiated once for global coordination.

---

## Creature Control (Neural Brain)

### Brain Model

The brain is a **dataflow graph** evaluated at each simulation timestep. It maps sensor inputs to joint effector outputs.

**Initial neuron function set (minimal viable subset):**
1. **sum** — weighted sum of inputs
2. **product** — product of inputs
3. **sigmoid** — logistic sigmoid of weighted input
4. **sin** — sine of weighted input
5. **oscillate-wave** — time-varying sine wave (has internal phase state)
6. **memory** — retains previous output, blends with input

Additional functions from the paper (to be added later): divide, sum-threshold, greater-than, sign-of, min, max, abs, if, interpolate, cos, atan, log, expt, differentiate, smooth, oscillate-saw.

### Brain Evaluation

- Each neuron has up to 3 inputs, each with a weight.
- Inputs can be: another neuron's output, a sensor value, or a constant.
- **Two brain timesteps per physics timestep** (per the paper) to reduce signal propagation delay.

### Sensors (Initial Set)

1. **Joint angle sensors** — current angle per DOF of each joint.
2. **Contact sensors** — ±1.0 per face, activated on collision (not needed for swimming initially, but included in the representation).

Photosensors (3 signals per part, normalized light direction) will be added when implementing the following behavior.

### Effectors

- One effector per joint DOF.
- Each effector takes a neuron/sensor input, scales by a weight, and applies as joint torque.
- Maximum torque is proportional to the cross-sectional area of the two connected parts (per the paper — strength scales with area, mass with volume).

---

## Physics Engine (Custom, Deterministic)

Built from scratch in the `core` crate. No external physics dependencies.

### Components

1. **Articulated body dynamics** — Featherstone's O(N) recursive algorithm for computing accelerations from velocities and external forces in a hierarchy of connected rigid parts.

2. **Numerical integration** — Runge-Kutta-Fehlberg method (4th order with 5th order error estimate for adaptive step size). Typically 1–5 substeps per 1/30s frame. The step adaptation is deterministic: purely a function of the current state and error estimate.

3. **Collision detection** — AABB (axis-aligned bounding box) hierarchies for broad phase. Narrow phase tests between rectangular solid pairs. O(N²) reduced via bounding box culling. Collisions with self and environment. Connected parts use adjusted shapes (child shape clipped halfway back from attachment point) to prevent trivial self-collision but allow swinging.

4. **Collision response** — Hybrid impulse + penalty spring model:
   - High velocities: instantaneous impulse forces.
   - Low velocities: penalty spring forces.
   - Configurable elasticity and friction parameters.
   - Penetration resolution: reduce timestep to keep new penetrations below tolerance.

5. **Viscous water model** (for swimming):
   - Gravity is **off**.
   - For each exposed moving surface of each part, apply a **viscous drag force** that resists the normal component of velocity, proportional to surface area × normal velocity magnitude.
   - This is a simple per-face approximation, not fluid dynamics. Sufficient for realistic paddling and sculling behavior.

### Part Shapes

All parts are **rectangular solids** (boxes). Dimensions are mutable parameters in the genotype.

---

## Genetic Algorithm

### Population Parameters

- **Population size**: 300
- **Survival ratio**: 1/5 (60 survivors per generation)
- **Generations**: 50–100 per evolution run
- All parameters stored in config, but defaults match the paper.

### Reproduction Ratios (per the paper)

- **40% asexual** — copy parent, apply mutations.
- **30% crossover** — align two parent graphs, apply crossover points to swap segments.
- **30% grafting** — copy first parent, connect a random node to a random node in the second parent, append descendants.

Offspring from mating are subjected to mutations afterwards, with reduced frequencies.

Number of offspring per survivor is proportional to its fitness.

### Mutation Operators (Directed Graphs)

Applied in sequence per the paper:

1. **Node parameter mutation** — each parameter subjected to possible alteration. Scalars: Gaussian perturbation (scale relative to value). Booleans: flip. Enums: random pick. Mutation frequency per parameter type is tunable. Frequency scaled inversely with graph size so ≥1 mutation occurs on average.

2. **New random node added** — initially disconnected (will be garbage collected if never connected).

3. **Connection parameter mutation** — same as node params. Connection pointer occasionally moved to a different random node.

4. **Random connection add/remove** — each node subject to gaining a new connection; each existing connection subject to removal.

5. **Garbage collection** — unreachable nodes (not connected from root or effectors) are removed.

For nested neural graphs: outer (morphology) graph mutated first, then inner (neural) graphs. Inner graph mutations respect the topology constraints of the outer graph.

### Mating Operators

**Crossover:** Nodes of two parents aligned in a row. One or more crossover points determine when to switch copying source. Connections copied with their nodes; out-of-bounds indices randomly reassigned.

**Grafting:** First parent copied. One of its connections re-pointed to a random node in the second parent. Second parent's connected subgraph appended. Disconnected nodes from first parent removed.

### Viability Checks (Early Termination)

Before full fitness evaluation:
1. Reject creatures exceeding a maximum part count.
2. Run brief collision-resolution sim to separate interpenetrating parts. Discard creatures with persistent interpenetration.
3. During fitness evaluation, periodically estimate fitness. Terminate creatures that:
   - Are not moving at all.
   - Have fitness worse than the minimum of previously surviving individuals.

This conserves compute by discontinuing hopeless simulations.

---

## Fitness Evaluation

### Phase 1: Swimming Speed

- **Environment**: no gravity, viscous water resistance.
- **Fitness metric**: distance traveled by center of mass per unit time.
- **Anti-circling bonus**: reward maximum displacement from initial position (straight-line distance) in addition to cumulative travel.
- **Continuing movement bonus**: velocities during the final phase of the test period weighted more heavily (prevents coasting from an initial push).
- **Test duration**: ~10 seconds of virtual time.

### Phase 2: Following (Future Milestone)

- Enable **photosensors** (3 per part: normalized light direction relative to part orientation).
- Place a light source at varying positions. Run multiple trials.
- **Fitness metric**: average speed toward the light source across trials.
- Light source repositions every ~5 seconds.

---

## Server Architecture

### SQLite as Coordination Layer

A single SQLite database serves as both persistent storage and task queue:

**Tables (conceptual):**
- `evolutions` — evolution run metadata (parameters, status, created_at).
- `generations` — generation number, parent evolution, aggregate fitness stats.
- `genotypes` — serialized genotype blobs, parent lineage, generation, fitness score.
- `tasks` — fitness evaluation work items: genotype_id, status (pending/running/complete/failed), worker_id, result.

**Worker loop:**
1. Pull a pending task from `tasks` (atomic UPDATE ... WHERE status = 'pending' RETURNING ...).
2. Deserialize genotype from `genotypes`.
3. Run physics simulation (using `core`), compute fitness.
4. Write fitness result back to `tasks` and update `genotypes.fitness`.

**Coordinator loop:**
1. Create generation 0 with random seed genotypes → insert into `genotypes` + `tasks`.
2. Poll `tasks` until all complete for current generation.
3. Apply selection (top 1/5 survive).
4. Generate offspring via reproduction operators → insert next generation.
5. Repeat for N generations.

Workers are Rust async tasks (tokio) running on the same machine, pulling from SQLite. The abstraction supports future distribution if needed.

### HTTP/WebSocket API

**REST endpoints:**
- `POST /evolutions` — start a new evolution run with parameters.
- `GET /evolutions` — list all runs.
- `GET /evolutions/:id` — get evolution status, current generation, fitness stats.
- `GET /evolutions/:id/generations/:gen` — get generation details.
- `GET /evolutions/:id/creatures/:id` — get creature genotype + phenotype + fitness.
- `GET /evolutions/:id/best` — get the top N creatures from the latest generation.
- `POST /evolutions/:id/stop` — stop a running evolution.

**WebSocket:**
- `ws://.../evolutions/:id/live` — stream generation progress, fitness updates, and optionally live simulation frames for the current best creature.

**Static file serving:**
- Serve the React frontend + WASM bundle.

A simple web dashboard shows all tasks and their status (running, complete, failed).

### Deployment

Self-hosted single machine. No cloud abstractions. The server binary includes:
- The coordinator logic.
- N worker threads (configurable, defaults to num_cpus).
- The HTTP/WS server.
- SQLite database stored locally.

Run with a single command: `cargo run --release -p server`.

---

## Frontend (React + TypeScript)

### Progressive Capabilities

1. **Result explorer** (MVP): Browse completed evolution runs. View fitness-over-generations graphs. Select and replay the best creatures in 3D.

2. **Live evolution viewer**: Connect via WebSocket to a running evolution. Watch generational progress in real-time. See the current best creature simulated live.

3. **Interactive sandbox**: Tweak evolution parameters. Run small evolutions in-browser (using the WASM `core` module directly). Hand-design seed creatures. Compare creatures side-by-side.

### 3D Rendering

- **wgpu** compiled to WASM, targeting WebGPU in the browser.
- Camera controls (orbit, zoom, follow creature).
- The WASM `core` module runs the physics; the renderer reads the resulting part transforms each frame.

### Visual Style (Recreating the Karl Sims Look)

The original video has a distinctive early-90s SGI aesthetic that we want to faithfully recreate:

**Creatures:**
- Rectangular solids (boxes) with sharp edges — no rounding, no textures.
- Cream/white base color with **face-dependent shading**: faces oriented toward the light are bright white/cream; faces angled away shift toward a muted yellow-green/olive tint.
- Single directional light source (slightly warm) plus soft ambient fill.
- Subtle flat shading per face (not smooth/Phong — each face of a box has a uniform color determined by its normal vs light direction).

**Environment (underwater):**
- **Background**: gradient from medium teal-blue (top) to darker blue-green (bottom). No skybox — just a color gradient suggesting depth.
- **Ground plane**: large checkered pattern in muted blue-gray and lighter gray-blue tones. Not high contrast — the checks should feel like they're seen through water.
- **Depth fog**: objects fade toward the background color with distance, simulating underwater visibility falloff. This is key to the atmosphere.
- **No specular highlights** on creatures. The look is matte/diffuse throughout.
- **No water surface** — the camera is fully submerged. The environment just *is* underwater.

**Lighting:**
- One directional light (above-right, slightly warm).
- Low ambient fill so shadow-facing sides aren't pure black but rather take on the blue-green of the environment.
- No cast shadows needed initially (the original video has some, but they're subtle and can be added later).

**Overall mood:** Calm, clinical, slightly dreamlike. The muted color palette lets the creature morphology be the focus.

### UI Components

- **Evolution dashboard**: list of runs, status, start/stop controls.
- **Fitness graph**: line chart of best/average/worst fitness per generation.
- **Creature gallery**: grid of thumbnails (or small animated previews) of top creatures.
- **Creature detail view**: 3D viewport with playback controls, genotype graph visualization, neural brain visualization, fitness stats.
- **Parameter editor**: configure population size, generations, mutation rates, etc.
- **Task monitor**: table of current fitness evaluation tasks, their status, which worker is running them.

---

## Development Milestones

Milestones are ordered to get **visual output in the browser as early as possible**, so we can inspect and validate each layer as it's built. The principle: never go more than one milestone without being able to *see* something.

### M1: Minimal Physics + WASM Renderer (Visual Proof of Life)
The goal is a browser window showing animated rectangular solids in the Karl Sims underwater style. Hard-coded test scenes, no evolution.

**Core crate (minimal physics):**
- Rigid body representation: rectangular solid with position, orientation (quaternion), mass, inertia tensor.
- Simple Euler integration (upgrade to RK4-Fehlberg in M3).
- Single revolute joint constraint (1-DOF hinge) with spring-damper limits.
- No collision detection yet.
- Compile to both native (for tests) and `wasm32-unknown-unknown`.

**Web crate (wgpu renderer):**
- wgpu WebGPU renderer compiling to WASM.
- Render rectangular solids with the Karl Sims visual style: flat per-face shading, cream/white bodies with face-dependent olive tint, single directional light.
- Underwater environment: teal-blue gradient background, checkered ground plane, depth fog.
- Orbit camera controls (mouse drag to rotate, scroll to zoom).

**Hard-coded test scenes:**
1. A single box floating and slowly rotating (verify rendering pipeline).
2. Two boxes connected by a revolute joint, with a sinusoidal torque driving the joint (verify joint + integration).
3. A "starfish" — a central box with 4 hinged flippers, each driven by a phase-offset sine wave (preview of what creatures will look like).

**Frontend shell:**
- Minimal React + TypeScript app hosting the wgpu canvas.
- Scene selector dropdown to switch between test scenes.
- Play/pause/reset controls.

**Deliverable:** Open `localhost:3000`, see animated boxes underwater in the Karl Sims style.

### M2: Full Joint Types + Articulated Body Dynamics
Upgrade the physics from "minimal" to "paper-faithful" while keeping the visual test harness.

- Featherstone's O(N) articulated body method (replacing naive Euler constraint solving).
- All 7 joint types: rigid, revolute, twist, universal, bend-twist, twist-bend, spherical.
- Joint limits with restoring spring forces.
- Effector torque application (strength proportional to cross-sectional area).
- New test scenes showcasing each joint type (visually inspect DOFs).
- Unit tests: energy conservation, joint limit behavior, torque response.

### M3: Robust Integration + Water Physics
- RK4-Fehlberg adaptive integration (deterministic: same state → same step choices).
- Viscous water drag model (per-face, resists normal velocity component, proportional to area × speed).
- AABB collision detection + hybrid impulse/penalty response.
- Connected part adjusted shapes (child clipped halfway back from attachment).
- **Cross-platform determinism test**: run identical sim in native + WASM, assert bit-exact match.
- New test scenes: creature in water with drag visible (flippers pushing against water), collision between parts.

### M4: Genotype, Phenotype & Brain
Now we can build creatures that are *described* by genotypes and *grown* into physics bodies.

**Genotype & phenotype:**
- Directed graph genotype with arena-based adjacency list.
- All 7 joint types in the genotype node representation.
- Phenotype development: graph traversal with recursive expansion.
- Serialization (serde + bincode).
- Random genotype generation (seeded PRNG).

**Neural brain:**
- Dataflow graph (arena-based), nested inside morphology nodes.
- Initial 6 neuron functions: sum, product, sigmoid, sin, oscillate-wave, memory.
- Brain evaluation: 2 brain timesteps per physics timestep.
- Joint angle sensors.
- Effector → joint torque with strength scaling.

**Test scenes (in browser):**
- Generate a random creature from a seed, grow its phenotype, render it.
- Manually wire a simple brain (e.g., oscillate-wave → effectors) to make it paddle.
- "Random creature gallery": generate N random creatures and display them side-by-side (static poses) to visually inspect morphological diversity.

### M5: Genetic Algorithm
- Mutation operators (all 5 from the paper, frequency scaled by graph size).
- Mating operators (crossover + grafting).
- Reproduction with exact paper ratios: 40% asexual, 30% crossover, 30% grafting.
- Selection: top 1/5 survive, offspring count proportional to fitness.
- Viability checks: max part count, interpenetration rejection, early termination of non-movers.
- Population initialization from random seeds.
- Swimming speed fitness function (distance/time, anti-circling, continuing movement bonus).
- **In-browser test**: run a tiny evolution (pop 20, 5 generations) entirely in WASM and visualize the best creature each generation. This validates the full pipeline before scaling up.

### M6: Server + Evolution at Scale
- SQLite schema: evolutions, generations, genotypes, tasks.
- Worker loop: pull task → deserialize → simulate → write fitness.
- Coordinator loop: selection → reproduction → dispatch → repeat.
- REST API for evolution management (CRUD on evolutions, query creatures/generations).
- WebSocket for live generation progress streaming.
- Task status dashboard.
- Run first real evolution: population 300, 50+ generations, swimming speed.

### M7: Full Frontend
- Evolution dashboard: list runs, start/stop, configure parameters.
- Fitness-over-generations line chart (best/avg/worst).
- Creature gallery with animated thumbnails.
- Creature detail view: 3D playback + genotype graph visualization + brain visualization.
- WebSocket live viewer: watch running evolution in real-time.
- In-browser sandbox: parameter tuning, small local evolutions, seed creature design.

### M8: Following Behavior
- Photosensors (3 signals per part: normalized light direction relative to part orientation).
- Following fitness function: multiple trials with light at different positions, average speed toward light.
- Moving light source (repositions every ~5 seconds).
- Frontend: visualize light source position, creature paths across trials.

---

## Open Questions / Future Considerations

- **Cross-platform FP testing**: Need a CI step that compiles `core` to both native and WASM, runs identical simulations, and asserts bit-exact output agreement.
- **Genotype visualization**: How to render the directed graph (morphology + nested neural graphs) in the UI. Consider force-directed graph layout or a tree view.
- **Simulation recording**: Should we store full simulation traces (part transforms per frame) for playback, or always re-simulate from the genotype? Re-simulation is deterministic so recording is optional, but pre-recorded traces enable faster browsing.
- **Walking/jumping behaviors**: Future phases after swimming + following are working.
- **Aesthetic selection**: The paper mentions interactive/aesthetic selection as an alternative to automatic fitness. Could be a compelling feature for the web UI.
