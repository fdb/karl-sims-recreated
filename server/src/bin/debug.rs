/// karl-sims-debug — simulate a creature from the DB and inspect the transforms.
///
/// Usage:
///   cargo run --bin karl-sims-debug -- --evolution 1 --creature 14201
///   cargo run --bin karl-sims-debug -- --evolution 1 --creature 14201 --output trace.json
///   cargo run --bin karl-sims-debug -- --evolution 1 --creature 14201 --fast
///   cargo run --bin karl-sims-debug -- --db /path/to/karl-sims.db --evolution 1 --creature 14201
use std::path::PathBuf;

use karl_sims_core::creature::Creature;
use karl_sims_core::fitness::{EvolutionParams, Environment};
use karl_sims_core::genotype::GenomeGraph;
use rusqlite::{Connection, params};
use serde_json::json;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let opts = parse_args(&args);

    let db_path = opts.db.unwrap_or_else(|| PathBuf::from("karl-sims.db"));
    let conn = Connection::open(&db_path)
        .unwrap_or_else(|e| panic!("Cannot open {:?}: {e}", db_path));

    // ── fetch genome bytes ──────────────────────────────────────────────────
    let genome_bytes: Vec<u8> = conn
        .query_row(
            "SELECT genome_bytes FROM genotypes WHERE id = ?1",
            params![opts.creature_id],
            |row| row.get(0),
        )
        .unwrap_or_else(|e| panic!("Creature {} not found: {e}", opts.creature_id));

    eprintln!("Genome bytes: {} bytes", genome_bytes.len());

    // ── fetch evolution config ──────────────────────────────────────────────
    let config_json: String = conn
        .query_row(
            "SELECT config_json FROM evolutions WHERE id = ?1",
            params![opts.evolution_id],
            |row| row.get(0),
        )
        .unwrap_or_else(|e| panic!("Evolution {} not found: {e}", opts.evolution_id));

    let params: EvolutionParams = serde_json::from_str(&config_json).unwrap_or_default();
    eprintln!("Evolution config: {}", config_json.trim());
    eprintln!(
        "Parsed params: env={:?} water_viscosity={} sim_duration={}s max_parts={}",
        params.environment, params.water_viscosity, params.sim_duration, params.max_parts
    );

    // ── deserialize genome ──────────────────────────────────────────────────
    let genome: GenomeGraph = bincode::deserialize(&genome_bytes)
        .unwrap_or_else(|e| panic!("bincode deserialize failed: {e}"));

    // ── build creature (same path as fitness evaluation) ───────────────────
    let mut creature = Creature::from_genome(genome);

    eprintln!("Body count: {}", creature.world.bodies.len());
    eprintln!("Joints: {}", creature.world.joints.len());

    // Apply the same environment settings as evaluate_fitness
    match params.environment {
        Environment::Water => {
            creature.world.water_enabled = true;
            creature.world.water_viscosity = params.water_viscosity;
            creature.world.gravity = glam::DVec3::ZERO;
        }
        Environment::Land => {
            creature.world.water_enabled = false;
            creature.world.gravity = glam::DVec3::new(0.0, -params.gravity, 0.0);
            creature.world.collisions_enabled = true;
            creature.world.ground_enabled = true;
            creature.world.set_root_transform(
                glam::DAffine3::from_translation(glam::DVec3::new(0.0, 2.0, 0.0)),
            );
            creature.world.forward_kinematics();
        }
    }

    // Viability check
    if creature.world.bodies.len() > params.max_parts {
        eprintln!(
            "WARN: {} bodies > max_parts {}, would get zero fitness",
            creature.world.bodies.len(), params.max_parts
        );
    }

    let dt = 1.0 / 60.0;
    let total_steps = (params.sim_duration / dt).round() as usize;
    eprintln!("Simulating {} steps ({:.1}s) using {}",
        total_steps, params.sim_duration,
        if opts.fast { "step_fast (Euler)" } else { "step (RK45)" }
    );

    // ── run simulation, collect per-frame data ──────────────────────────────
    let mut frames: Vec<serde_json::Value> = Vec::with_capacity(total_steps + 1);
    let mut first_nan_frame: Option<usize> = None;
    let mut max_position_magnitude: f64 = 0.0;

    // Frame 0 = initial state
    frames.push(collect_frame_json(&creature, 0));

    let initial_pos = creature.world.transforms[creature.world.root].translation;

    for step in 0..total_steps {
        if opts.fast {
            creature.step_fast(dt);
        } else {
            creature.step(dt);
        }

        let frame_json = collect_frame_json(&creature, step + 1);

        // Check for NaN/Inf in this frame
        if first_nan_frame.is_none() {
            let has_nan = creature.world.transforms.iter().any(|t| {
                !t.translation.is_finite()
            });
            if has_nan {
                first_nan_frame = Some(step + 1);
                eprintln!("NaN detected at frame {} (t={:.3}s)", step + 1, (step + 1) as f64 * dt);
            }
        }

        // Track max position magnitude for root body
        let root_pos = creature.world.transforms[creature.world.root].translation;
        max_position_magnitude = max_position_magnitude.max(root_pos.length());

        frames.push(frame_json);

        // Print progress every 60 frames
        if opts.verbose && (step + 1) % 60 == 0 {
            let root_t = &creature.world.transforms[creature.world.root];
            eprintln!(
                "  Frame {:4} (t={:.2}s): root=({:.3},{:.3},{:.3})",
                step + 1, (step + 1) as f64 * dt,
                root_t.translation.x, root_t.translation.y, root_t.translation.z
            );
        }
    }

    let final_pos = creature.world.transforms[creature.world.root].translation;
    let displacement = (final_pos - initial_pos).length();

    // ── summary ─────────────────────────────────────────────────────────────
    eprintln!("\n=== SUMMARY ===");
    eprintln!("Bodies: {}", creature.world.bodies.len());
    eprintln!("Initial root position: ({:.3}, {:.3}, {:.3})",
        initial_pos.x, initial_pos.y, initial_pos.z);
    eprintln!("Final root position:   ({:.3}, {:.3}, {:.3})",
        final_pos.x, final_pos.y, final_pos.z);
    eprintln!("Displacement: {:.4}", displacement);
    eprintln!("Max position magnitude: {:.4}", max_position_magnitude);
    eprintln!("First NaN frame: {}",
        first_nan_frame.map(|f| format!("{} (t={:.3}s)", f, f as f64 * dt))
            .unwrap_or_else(|| "none".to_string()));

    // Print initial transforms (from frame 0 JSON collected before sim loop)
    eprintln!("\n--- Frame 0 transforms (initial) ---");
    if let Some(f0) = frames.first() {
        if let Some(bodies) = f0["bodies"].as_array() {
            for (i, b) in bodies.iter().enumerate().take(10) {
                eprintln!(
                    "  Body {:2}: pos=({:7.3},{:7.3},{:7.3}) half_ext=({:.3},{:.3},{:.3}) nan={}",
                    i,
                    b["px"].as_f64().unwrap_or(f64::NAN),
                    b["py"].as_f64().unwrap_or(f64::NAN),
                    b["pz"].as_f64().unwrap_or(f64::NAN),
                    b["hx"].as_f64().unwrap_or(f64::NAN),
                    b["hy"].as_f64().unwrap_or(f64::NAN),
                    b["hz"].as_f64().unwrap_or(f64::NAN),
                    b["nan"].as_bool().unwrap_or(true),
                );
            }
            if bodies.len() > 10 {
                eprintln!("  ... ({} more bodies)", bodies.len() - 10);
            }
        }
    }

    // Print frame 1 transforms (first step — what the viewer shows as initial)
    eprintln!("\n--- Frame 1 transforms (after first step_fast) ---");
    if frames.len() > 1 {
        if let Some(bodies) = frames[1]["bodies"].as_array() {
            for (i, b) in bodies.iter().enumerate().take(10) {
                eprintln!(
                    "  Body {:2}: pos=({:7.3},{:7.3},{:7.3}) nan={}",
                    i,
                    b["px"].as_f64().unwrap_or(f64::NAN),
                    b["py"].as_f64().unwrap_or(f64::NAN),
                    b["pz"].as_f64().unwrap_or(f64::NAN),
                    b["nan"].as_bool().unwrap_or(true),
                );
            }
        }
    }

    // ── write trace JSON ─────────────────────────────────────────────────────
    if let Some(output_path) = opts.output {
        let trace = json!({
            "evolution_id": opts.evolution_id,
            "creature_id": opts.creature_id,
            "body_count": creature.world.bodies.len(),
            "total_frames": frames.len(),
            "dt": dt,
            "first_nan_frame": first_nan_frame,
            "displacement": displacement,
            "integrator": if opts.fast { "euler" } else { "rk45" },
            "environment": format!("{:?}", params.environment),
            "water_enabled": creature.world.water_enabled,
            "water_viscosity": creature.world.water_viscosity,
            "frames": frames,
        });
        let json_str = serde_json::to_string_pretty(&trace).unwrap();
        std::fs::write(&output_path, json_str)
            .unwrap_or_else(|e| panic!("Failed to write {:?}: {e}", output_path));
        eprintln!("\nTrace written to {:?} ({} frames)", output_path, frames.len());
    }
}

fn collect_frame_json(creature: &Creature, frame_idx: usize) -> serde_json::Value {
    let world = &creature.world;
    let bodies: Vec<serde_json::Value> = world.transforms.iter().enumerate()
        .map(|(i, t)| {
            let body = &world.bodies[i];
            let q = glam::DQuat::from_mat3(&glam::DMat3::from_cols(
                t.matrix3.col(0),
                t.matrix3.col(1),
                t.matrix3.col(2),
            ));
            json!({
                "px": t.translation.x,
                "py": t.translation.y,
                "pz": t.translation.z,
                "qw": q.w, "qx": q.x, "qy": q.y, "qz": q.z,
                "hx": body.half_extents.x,
                "hy": body.half_extents.y,
                "hz": body.half_extents.z,
                "nan": !t.translation.is_finite(),
            })
        })
        .collect();
    json!({ "frame": frame_idx, "bodies": bodies })
}

// ── minimal arg parser ───────────────────────────────────────────────────────

struct Opts {
    db: Option<PathBuf>,
    evolution_id: i64,
    creature_id: i64,
    output: Option<PathBuf>,
    fast: bool,
    verbose: bool,
}

fn parse_args(args: &[String]) -> Opts {
    let mut db = None;
    let mut evolution_id: Option<i64> = None;
    let mut creature_id: Option<i64> = None;
    let mut output = None;
    let mut fast = false;
    let mut verbose = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--db" => { db = Some(PathBuf::from(&args[i + 1])); i += 2; }
            "--evolution" | "-e" => { evolution_id = Some(args[i + 1].parse().expect("invalid evolution id")); i += 2; }
            "--creature" | "-c" => { creature_id = Some(args[i + 1].parse().expect("invalid creature id")); i += 2; }
            "--output" | "-o" => { output = Some(PathBuf::from(&args[i + 1])); i += 2; }
            "--fast" => { fast = true; i += 1; }
            "--verbose" | "-v" => { verbose = true; i += 1; }
            "--help" | "-h" => {
                eprintln!("Usage: karl-sims-debug --evolution ID --creature ID [--output trace.json] [--db path] [--fast] [--verbose]");
                std::process::exit(0);
            }
            _ => { eprintln!("Unknown arg: {}", args[i]); i += 1; }
        }
    }

    Opts {
        db,
        evolution_id: evolution_id.expect("--evolution required"),
        creature_id: creature_id.expect("--creature required"),
        output,
        fast,
        verbose,
    }
}
