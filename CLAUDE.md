# Karl Sims Recreated

## Pre-handoff checklist

Before presenting code to the user, **always** verify:

1. **Rust tests pass**: `cargo test -p karl-sims-core --lib`
2. **WASM builds**: `wasm-pack build web --target web`
3. **TypeScript compiles cleanly**: `cd frontend && npx tsc --noEmit`

Do not hand off code that fails any of these checks.

## Build & run

- **Server**: `cargo run --release -p karl-sims-server`
- **Frontend dev**: `cd frontend && npm run dev`
- **WASM rebuild**: `wasm-pack build web --target web` (output symlinked into `frontend/node_modules/karl-sims-web`)

## Architecture

- `core/` — Rust physics engine, creature genotype/phenotype, fitness evaluation, evolution
- `web/` — WASM bindings (wasm-bindgen) exposing simulation to JS
- `server/` — Axum HTTP/WebSocket server, SQLite persistence, evolution orchestration
- `frontend/` — React + Three.js viewer, Vite bundler

## Physics is stable — do not modify

The physics engine (`core/src/rapier_world.rs`, `core/src/world.rs`, `core/src/collision.rs`) is working correctly as of 2026-04-05 (post-Rapier migration + velocity clamps + 8 m/s fitness rejection). **Do NOT modify physics code** when working on actuator tuning, brain/control changes, fitness tweaks, or behavioral improvements. The "creatures launching into orbit" class of bugs has been fixed and must not regress.

If you believe a physics change is genuinely required, surface it explicitly to the user and wait for approval before touching any physics file. Always prefer changing the control/effector/brain layer first.

### If physics changes are ever approved

1. Write TDD tests that prove the bug exists (RED), then fix (GREEN)
2. Use the debug CLI to export a physics trace and visually verify the trajectory makes sense:
   ```bash
   cargo run --bin karl-sims-debug -- -e <ID> -c <ID> -v --output trace.json
   ```
3. Check the root body Y position over time — it should fall under gravity, bounce realistically off the ground, and never exceed its starting height without external energy input

**Critical: the viewer MUST use the same physics as the server (bit-identical results).** The viewer uses real-time Rapier stepping via `scene_step` — do NOT introduce a separate "fast" physics path for rendering.

## Paper divergences must be configurable

When the implementation diverges from Sims 1994 "Evolving Virtual Creatures" (e.g. safety caps, tuning parameters, selection-method changes, additional fitness penalties), expose the divergence as an **opt-in config field** on `EvolutionParams` (or the relevant config struct), with a doc comment that **clearly states the paper's behavior and our variant**.

Template for the doc comment:

```rust
/// <Field description>.
///
/// Sims 1994: <what the paper says / the paper's default>.
/// Our variant: <what we do, and why>.
/// Set to `None` / disable to reproduce the paper faithfully.
pub some_knob: Option<T>,
```

This keeps the codebase honest about what is Sims and what is ours, and lets you run paper-faithful experiments by leaving the knobs unset. Use `#[serde(default)]` so old saved configs still deserialize.

Examples of existing paper divergences that should be configurable (most still TODO):
- `MAX_PLAUSIBLE_SPEED = 8.0` linear body speed rejection (`fitness.rs`) — not in paper, contact-kick safety net
- `TORQUE_PER_METER = 6.0` actuator scaling constant (`brain.rs`) — paper only says "proportional to max cross-sectional dimension", doesn't specify constant
- Tournament selection with `k=3` (`coordinator.rs`) — paper uses various methods, we picked one
- Random-injection interval of 10 generations (`coordinator.rs`) — not in paper
- `MIN_MUTATION_SCALE = 0.05` floor (`mutation.rs`) — paper uses pure `1/graph_size`

## Key conventions

- WASM uses `--target web`; `initWasm()` in `wasm.ts` explicitly calls the default `init()` export (do NOT make it a no-op)
- `sim_init(genome_bytes, environment)` accepts "Water" or "Land" to match fitness evaluation physics
- Ground plane is at y=0 in physics; land creatures start at y=2.0
- Fitness values can be pathological (NaN, Inf) from physics blowup — always guard and format
