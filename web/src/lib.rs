use wasm_bindgen::prelude::*;

use karl_sims_core::creature::Creature;
use karl_sims_core::genotype::GenomeGraph;

#[wasm_bindgen(start)]
pub fn wasm_main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).ok();
}

/// Opaque simulation handle exposed to JS.
#[wasm_bindgen]
pub struct SimHandle {
    creature: Creature,
}

/// Initialize a simulation from serialized genome bytes.
/// `environment` should be "Water" or "Land".
#[wasm_bindgen]
pub fn sim_init(genome_bytes: &[u8], environment: &str) -> Result<SimHandle, JsValue> {
    let genome: GenomeGraph = bincode::deserialize(genome_bytes)
        .map_err(|e| JsValue::from_str(&format!("Failed to deserialize genome: {e}")))?;
    let mut creature = Creature::from_genome(genome);
    match environment {
        "Land" => {
            creature.world.water_enabled = false;
            creature.world.gravity = glam::DVec3::new(0.0, -9.81, 0.0);
            creature.world.collisions_enabled = true;
            creature.world.ground_enabled = true;
            creature.world.set_root_transform(
                glam::DAffine3::from_translation(glam::DVec3::new(0.0, 2.0, 0.0)),
            );
            creature.world.forward_kinematics();
        }
        _ => {
            creature.world.water_enabled = true;
            creature.world.water_viscosity = 2.0;
            creature.world.gravity = glam::DVec3::ZERO;
        }
    }
    Ok(SimHandle { creature })
}

/// Advance the simulation by one frame (1/60 s).
/// Returns a flat Float64Array with 10 values per body part:
///   [px, py, pz, qw, qx, qy, qz, hx, hy, hz, ...]
/// where p = position, q = quaternion (w first), h = half_extents.
#[wasm_bindgen]
pub fn sim_step(handle: &mut SimHandle) -> Vec<f64> {
    handle.creature.step_fast(1.0 / 60.0);
    collect_transforms(&handle.creature)
}

/// Advance simulation using the accurate RK45 integrator (same as fitness evaluation).
/// Slower than sim_step but numerically stable — use for pre-computing playback frames.
#[wasm_bindgen]
pub fn sim_step_accurate(handle: &mut SimHandle) -> Vec<f64> {
    handle.creature.step(1.0 / 60.0);
    collect_transforms(&handle.creature)
}

/// Read current transforms without stepping — useful for initial frame.
#[wasm_bindgen]
pub fn sim_transforms(handle: &SimHandle) -> Vec<f64> {
    collect_transforms(&handle.creature)
}

/// Number of body parts in the simulation.
#[wasm_bindgen]
pub fn sim_body_count(handle: &SimHandle) -> usize {
    handle.creature.world.bodies.len()
}

/// Current light position as [x, y, z].
#[wasm_bindgen]
pub fn sim_light_position(handle: &SimHandle) -> Vec<f64> {
    let p = handle.creature.world.light_position;
    vec![p.x, p.y, p.z]
}

/// Set the light position (for light-following playback).
#[wasm_bindgen]
pub fn sim_set_light_position(handle: &mut SimHandle, x: f64, y: f64, z: f64) {
    handle.creature.world.light_position = glam::DVec3::new(x, y, z);
}

/// Initialize a simulation with a random creature (for demos).
#[wasm_bindgen]
pub fn sim_init_random(seed: u64) -> SimHandle {
    use karl_sims_core::rand::SeedableRng;
    let mut rng = karl_sims_core::rand_chacha::ChaCha8Rng::seed_from_u64(seed);
    let genome = karl_sims_core::genotype::GenomeGraph::random(&mut rng);
    let mut creature = Creature::from_genome(genome);
    creature.world.water_enabled = true;
    creature.world.water_viscosity = 2.0;
    creature.world.gravity = glam::DVec3::ZERO;
    SimHandle { creature }
}

/// Opaque handle for scene-based creatures (hand-crafted, no brain).
#[wasm_bindgen]
pub struct SceneHandle {
    world: karl_sims_core::world::World,
    def: karl_sims_core::creature_def::CreatureDefinition,
}

/// Initialize a scene creature by name. Environment: "Water" or "Land".
#[wasm_bindgen]
pub fn scene_init(name: &str, environment: &str) -> Result<SceneHandle, JsValue> {
    let def = karl_sims_core::creature_def::builtin(name)
        .ok_or_else(|| JsValue::from_str(&format!("Unknown scene: {name}")))?;
    let mut world = def.build_world();

    match environment {
        "Land" => {
            world.water_enabled = false;
            world.gravity = glam::DVec3::new(0.0, -9.81, 0.0);
            world.collisions_enabled = true;
            world.ground_enabled = true;
            world.set_root_transform(
                glam::DAffine3::from_translation(glam::DVec3::new(0.0, 2.0, 0.0)),
            );
            world.forward_kinematics();
        }
        _ => {
            world.water_enabled = true;
            world.water_viscosity = 2.0;
            world.gravity = glam::DVec3::ZERO;
        }
    }

    Ok(SceneHandle { world, def })
}

/// List available scene names (comma-separated).
#[wasm_bindgen]
pub fn scene_list() -> String {
    karl_sims_core::creature_def::builtin_names().join(",")
}

/// Step a scene simulation and return transforms.
#[wasm_bindgen]
pub fn scene_step(handle: &mut SceneHandle) -> Vec<f64> {
    handle.def.apply_torques(&mut handle.world);
    handle.world.step(1.0 / 60.0);
    collect_world_transforms(&handle.world)
}

/// Read current transforms from a scene handle.
#[wasm_bindgen]
pub fn scene_transforms(handle: &SceneHandle) -> Vec<f64> {
    collect_world_transforms(&handle.world)
}

/// Body count for a scene handle.
#[wasm_bindgen]
pub fn scene_body_count(handle: &SceneHandle) -> usize {
    handle.world.bodies.len()
}

fn collect_transforms(creature: &Creature) -> Vec<f64> {
    collect_world_transforms(&creature.world)
}

fn collect_world_transforms(world: &karl_sims_core::world::World) -> Vec<f64> {
    let mut out = Vec::with_capacity(world.bodies.len() * 10);

    for (i, body) in world.bodies.iter().enumerate() {
        let t = &world.transforms[i];

        // Position
        out.push(t.translation.x);
        out.push(t.translation.y);
        out.push(t.translation.z);

        // Rotation as quaternion (w, x, y, z) — convert from matrix3
        let q = glam::DQuat::from_mat3(&glam::DMat3::from_cols(
            t.matrix3.col(0),
            t.matrix3.col(1),
            t.matrix3.col(2),
        ));
        out.push(q.w);
        out.push(q.x);
        out.push(q.y);
        out.push(q.z);

        // Half-extents (box dimensions / 2)
        out.push(body.half_extents.x);
        out.push(body.half_extents.y);
        out.push(body.half_extents.z);
    }

    out
}
