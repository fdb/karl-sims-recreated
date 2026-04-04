/// karl-sims-simulate — simulate a creature from a JSON definition and output CSV.
///
/// Usage:
///   cargo run --bin karl-sims-simulate -- --scene swimmer-starfish
///   cargo run --bin karl-sims-simulate -- --scene walker-biped --environment land
///   cargo run --bin karl-sims-simulate -- --file creatures/custom.json --environment water
///   cargo run --bin karl-sims-simulate -- --scene walker-quadruped --environment land --frames 120
///   cargo run --bin karl-sims-simulate -- --list
///
/// Output: CSV to stdout with per-frame body positions.

use karl_sims_core::creature_def::{self, CreatureDefinition};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let opts = parse_args(&args);

    if opts.list {
        eprintln!("Built-in creatures:");
        for name in creature_def::builtin_names() {
            let def = creature_def::builtin(name).unwrap();
            eprintln!("  {:<25} ({} bodies, {} joints)", name, def.bodies.len(), def.joints.len());
        }
        return;
    }

    let def = if let Some(scene_name) = &opts.scene {
        creature_def::builtin(scene_name)
            .unwrap_or_else(|| {
                eprintln!("Unknown scene: {scene_name}. Use --list to see available scenes.");
                std::process::exit(1);
            })
    } else if let Some(file_path) = &opts.file {
        let json = std::fs::read_to_string(file_path)
            .unwrap_or_else(|e| { eprintln!("Cannot read {file_path}: {e}"); std::process::exit(1); });
        serde_json::from_str::<CreatureDefinition>(&json)
            .unwrap_or_else(|e| { eprintln!("Invalid JSON: {e}"); std::process::exit(1); })
    } else {
        eprintln!("Specify --scene <name> or --file <path>. Use --list for built-in scenes.");
        std::process::exit(1);
    };

    if opts.export_json {
        println!("{}", serde_json::to_string_pretty(&def).unwrap());
        return;
    }

    eprintln!("Creature: {} ({} bodies, {} joints, {} torque oscillators)",
        def.name, def.bodies.len(), def.joints.len(), def.torques.len());
    eprintln!("Environment: {}, gravity: {:.2} m/s², frames: {}, dt: {:.4}s",
        opts.environment, opts.gravity, opts.frames, opts.dt);

    let records = creature_def::simulate(
        &def,
        &opts.environment,
        opts.gravity,
        opts.frames,
        opts.dt,
    );

    // CSV header
    let body_count = def.bodies.len();
    print!("frame,time");
    for i in 0..body_count {
        print!(",body{i}_x,body{i}_y,body{i}_z");
    }
    println!();

    // CSV data
    for r in &records {
        print!("{},{:.4}", r.frame, r.time);
        for pos in &r.positions {
            print!(",{:.4},{:.4},{:.4}", pos[0], pos[1], pos[2]);
        }
        println!();
    }

    // Summary to stderr
    let root_start = &records[0].positions[0];
    let root_end = &records.last().unwrap().positions[0];
    let dist = ((root_end[0] - root_start[0]).powi(2)
        + (root_end[1] - root_start[1]).powi(2)
        + (root_end[2] - root_start[2]).powi(2)).sqrt();
    eprintln!("\nRoot: ({:.3},{:.3},{:.3}) → ({:.3},{:.3},{:.3}), displacement: {:.4}",
        root_start[0], root_start[1], root_start[2],
        root_end[0], root_end[1], root_end[2], dist);

    // Check for NaN
    let nan_frame = records.iter().find(|r| r.positions.iter().any(|p| !p[0].is_finite() || !p[1].is_finite() || !p[2].is_finite()));
    if let Some(r) = nan_frame {
        eprintln!("WARNING: NaN detected at frame {} (t={:.3}s)", r.frame, r.time);
    }
}

struct Opts {
    scene: Option<String>,
    file: Option<String>,
    environment: String,
    gravity: f64,
    frames: usize,
    dt: f64,
    list: bool,
    export_json: bool,
}

fn parse_args(args: &[String]) -> Opts {
    let mut scene = None;
    let mut file = None;
    let mut environment = "water".to_string();
    let mut gravity = 9.81;
    let mut frames = 600; // 10s at 60fps
    let mut dt = 1.0 / 60.0;
    let mut list = false;
    let mut export_json = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--scene" | "-s" => { scene = Some(args[i + 1].clone()); i += 2; }
            "--file" | "-f" => { file = Some(args[i + 1].clone()); i += 2; }
            "--environment" | "--env" | "-e" => { environment = args[i + 1].clone(); i += 2; }
            "--gravity" | "-g" => { gravity = args[i + 1].parse().expect("invalid gravity"); i += 2; }
            "--frames" | "-n" => { frames = args[i + 1].parse().expect("invalid frames"); i += 2; }
            "--dt" => { dt = args[i + 1].parse().expect("invalid dt"); i += 2; }
            "--list" | "-l" => { list = true; i += 1; }
            "--export-json" | "--json" => { export_json = true; i += 1; }
            "--help" | "-h" => {
                eprintln!("Usage: karl-sims-simulate [--scene NAME | --file PATH] [--environment water|land] [--gravity 9.81] [--frames 600] [--dt 0.0167] [--list]");
                std::process::exit(0);
            }
            _ => { eprintln!("Unknown arg: {}", args[i]); i += 1; }
        }
    }

    Opts { scene, file, environment, gravity, frames, dt, list, export_json }
}
