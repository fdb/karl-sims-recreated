import { sim_init, sim_init_random, sim_step, sim_step_accurate, sim_transforms, sim_body_count } from "karl-sims-web";

// New bundler-target wasm-pack output initializes the WASM module synchronously
// at import time via __wbindgen_start — no explicit init() call needed.
export async function initWasm(): Promise<void> {
  // No-op: WASM is initialized when the module is first imported.
}

export { sim_init, sim_init_random, sim_step, sim_step_accurate, sim_transforms, sim_body_count };
