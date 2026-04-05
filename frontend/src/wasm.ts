import init, {
  sim_init, sim_init_random, sim_step, sim_step_accurate, sim_transforms, sim_body_count,
  sim_light_position, sim_set_light_position,
  scene_init, scene_init_rapier, scene_step, scene_transforms, scene_body_count, scene_list,
} from "karl-sims-web";

let ready: Promise<void> | null = null;

export function initWasm(): Promise<void> {
  if (!ready) {
    ready = init().then(() => {});
  }
  return ready;
}

export {
  sim_init, sim_init_random, sim_step, sim_step_accurate, sim_transforms, sim_body_count,
  sim_light_position, sim_set_light_position,
  scene_init, scene_init_rapier, scene_step, scene_transforms, scene_body_count, scene_list,
};
