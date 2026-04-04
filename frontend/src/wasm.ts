import init, { sim_init, sim_init_random, sim_step, sim_step_accurate, sim_transforms, sim_body_count, sim_light_position, sim_set_light_position } from "karl-sims-web";

let initialized: Promise<void> | null = null;

export function initWasm(): Promise<void> {
  if (!initialized) {
    initialized = init().then(() => {});
  }
  return initialized;
}

export { sim_init, sim_init_random, sim_step, sim_step_accurate, sim_transforms, sim_body_count, sim_light_position, sim_set_light_position };
