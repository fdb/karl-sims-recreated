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
/// Water drag is enabled so the sim is stable — creatures are evolved in water.
#[wasm_bindgen]
pub fn sim_init(genome_bytes: &[u8]) -> Result<SimHandle, JsValue> {
    let genome: GenomeGraph = bincode::deserialize(genome_bytes)
        .map_err(|e| JsValue::from_str(&format!("Failed to deserialize genome: {e}")))?;
    let mut creature = Creature::from_genome(genome);
    creature.world.water_enabled = true;
    creature.world.water_viscosity = 2.0;
    creature.world.gravity = glam::DVec3::ZERO;
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

fn collect_transforms(creature: &Creature) -> Vec<f64> {
    let world = &creature.world;
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
