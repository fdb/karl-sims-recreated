import init, { sim_init, sim_init_random, sim_step, sim_transforms, sim_body_count } from "karl-sims-web";

let initialized = false;

export async function initWasm(): Promise<void> {
  if (initialized) return;
  await init();
  initialized = true;
}

export { sim_init, sim_init_random, sim_step, sim_transforms, sim_body_count };
