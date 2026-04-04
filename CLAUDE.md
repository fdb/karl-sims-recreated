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

## Physics changes

Any changes to the physics engine (`core/src/world.rs`, `core/src/featherstone.rs`, `core/src/collision.rs`) must include sanity checks:

1. Write TDD tests that prove the bug exists (RED), then fix (GREEN)
2. Use the debug CLI to export a physics trace and visually verify the trajectory makes sense:
   ```bash
   cargo run --bin karl-sims-debug -- -e <ID> -c <ID> -v --output trace.json
   ```
3. Check the root body Y position over time — it should fall under gravity, bounce realistically off the ground, and never exceed its starting height without external energy input

The root body is integrated per-substep alongside the RK45 joint integrator. Do NOT integrate the root with the full frame_dt — this causes explosive bouncing on ground contact.

**Critical: the viewer MUST use the same physics as the server (bit-identical results).** Never use `step_fast` or a different integrator in the viewer — always use `step()` (RK45). The viewer pre-computes frames using `sim_step_accurate`, and the scene viewer uses `scene_step` which calls `world.step()`. Do NOT introduce a separate "fast" physics path for rendering.

## Key conventions

- WASM uses `--target web`; `initWasm()` in `wasm.ts` explicitly calls the default `init()` export (do NOT make it a no-op)
- `sim_init(genome_bytes, environment)` accepts "Water" or "Land" to match fitness evaluation physics
- Ground plane is at y=0 in physics; land creatures start at y=2.0
- Fitness values can be pathological (NaN, Inf) from physics blowup — always guard and format
