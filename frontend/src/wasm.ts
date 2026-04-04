import init, {
  create_renderer,
  renderer_resize,
  set_scene,
  set_paused,
  reset_scene,
  load_creature_from_bytes,
} from "karl-sims-web";

let initialized = false;

export async function initWasm(): Promise<void> {
  if (initialized) return;
  await init();
  initialized = true;
}

export { create_renderer, renderer_resize, set_scene, set_paused, reset_scene, load_creature_from_bytes };
