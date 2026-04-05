//! karl-sims-scene-trace — run a built-in creature and dump per-frame CSV traces.
//!
//! Emits two CSVs for manual inspection in a spreadsheet/plotting tool:
//!   - <prefix>_bodies.csv: frame, time, body, px, py, pz, qw, qx, qy, qz, speed
//!   - <prefix>_joints.csv: frame, time, joint, angle0, angle1, angle2, anchor_dist, torque0
//!
//! Usage:
//!   cargo run --bin karl-sims-scene-trace -- \
//!       --creature swimmer-snake --env Water --frames 600 --out trace
//!
//! --env:     Water (default)  | Land

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use glam::{DAffine3, DVec3};
use karl_sims_core::creature_def;
use karl_sims_core::world::World;

fn main() {
    let opts = parse_args(&std::env::args().collect::<Vec<_>>());

    let def = creature_def::builtin(&opts.creature)
        .unwrap_or_else(|| {
            eprintln!("Unknown creature: {}", opts.creature);
            eprintln!("Available: {:?}", creature_def::builtin_names());
            std::process::exit(1);
        });

    let mut world = def.build_world();
    match opts.env.as_str() {
        "Land" | "land" => {
            world.water_enabled = false;
            world.gravity = DVec3::new(0.0, -9.81, 0.0);
            world.ground_enabled = true;
            world.set_root_transform(
                DAffine3::from_translation(DVec3::new(0.0, 2.0, 0.0)),
            );
            world.forward_kinematics();
        }
        _ => {
            world.water_enabled = true;
            world.water_viscosity = 2.0;
            world.gravity = DVec3::ZERO;
        }
    }
    eprintln!(
        "creature={} env={} bodies={} joints={} frames={}",
        opts.creature, opts.env,
        world.bodies.len(), world.joints.len(), opts.frames,
    );

    // ── Open CSVs ─────────────────────────────────────────────────────────────
    let bodies_path = PathBuf::from(format!("{}_bodies.csv", opts.out));
    let joints_path = PathBuf::from(format!("{}_joints.csv", opts.out));
    let mut bodies_csv = BufWriter::new(File::create(&bodies_path).unwrap());
    let mut joints_csv = BufWriter::new(File::create(&joints_path).unwrap());
    writeln!(bodies_csv, "frame,time,body,px,py,pz,qw,qx,qy,qz,speed").unwrap();
    writeln!(joints_csv, "frame,time,joint,angle0,angle1,angle2,anchor_dist,torque0").unwrap();

    let dt = 1.0 / 60.0;

    // Frame 0: initial state (before any step)
    write_frame(&world, 0, &mut bodies_csv, &mut joints_csv, &[]);

    // ── Diagnostics ───────────────────────────────────────────────────────────
    let mut prev_positions: Vec<DVec3> =
        world.transforms.iter().map(|t| t.translation).collect();
    let mut first_nan_frame: Option<usize> = None;
    let mut first_explode_frame: Option<usize> = None;
    let mut max_pos_mag = 0.0_f64;
    let mut max_anchor_dist = 0.0_f64;
    let mut max_speed = 0.0_f64;

    for f in 1..=opts.frames {
        def.apply_torques(&mut world);
        let torques: Vec<[f64; 3]> = world.torques.clone();
        world.step(dt);

        // Compute speeds via finite difference (don't rely on body.velocity internals)
        let speeds: Vec<f64> = world.transforms.iter().enumerate().map(|(i, t)| {
            let v = (t.translation - prev_positions[i]) / dt;
            v.length()
        }).collect();

        // Track divergence
        for (i, t) in world.transforms.iter().enumerate() {
            if !t.translation.is_finite() && first_nan_frame.is_none() {
                first_nan_frame = Some(f);
                eprintln!("  NaN at frame {f} (body {i})");
            }
            let mag = t.translation.length();
            max_pos_mag = max_pos_mag.max(mag);
            if mag > 10.0 && first_explode_frame.is_none() {
                first_explode_frame = Some(f);
                eprintln!("  |pos|>10m at frame {f} (body {i}, |pos|={mag:.2})");
            }
            max_speed = max_speed.max(speeds[i]);
        }
        for j in &world.joints {
            let p = world.transforms[j.parent_idx].transform_point3(j.parent_anchor);
            let c = world.transforms[j.child_idx].transform_point3(j.child_anchor);
            max_anchor_dist = max_anchor_dist.max((p - c).length());
        }

        write_frame(&world, f, &mut bodies_csv, &mut joints_csv, &torques);

        prev_positions = world.transforms.iter().map(|t| t.translation).collect();
    }

    bodies_csv.flush().unwrap();
    joints_csv.flush().unwrap();

    eprintln!("\n=== SUMMARY ===");
    eprintln!("bodies CSV: {}", bodies_path.display());
    eprintln!("joints CSV: {}", joints_path.display());
    eprintln!("max |pos|       : {:.3} m", max_pos_mag);
    eprintln!("max speed       : {:.3} m/s", max_speed);
    eprintln!("max anchor dist : {:.4} m", max_anchor_dist);
    eprintln!("first NaN frame : {:?}", first_nan_frame);
    eprintln!("first |pos|>10m : {:?}", first_explode_frame);
}

fn write_frame(
    world: &World,
    frame: usize,
    bodies_csv: &mut impl Write,
    joints_csv: &mut impl Write,
    torques: &[[f64; 3]],
) {
    let time = world.time;
    for (i, t) in world.transforms.iter().enumerate() {
        let q = glam::DQuat::from_mat3(&t.matrix3);
        writeln!(
            bodies_csv,
            "{frame},{time:.5},{i},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},{:.5},",
            t.translation.x, t.translation.y, t.translation.z,
            q.w, q.x, q.y, q.z,
        ).unwrap();
        let _ = i;
    }
    for (ji, j) in world.joints.iter().enumerate() {
        let p = world.transforms[j.parent_idx].transform_point3(j.parent_anchor);
        let c = world.transforms[j.child_idx].transform_point3(j.child_anchor);
        let ad = (p - c).length();
        let tau0 = torques.get(ji).map(|t| t[0]).unwrap_or(0.0);
        writeln!(
            joints_csv,
            "{frame},{time:.5},{ji},{:.5},{:.5},{:.5},{ad:.5},{tau0:.5}",
            j.angles[0], j.angles[1], j.angles[2],
        ).unwrap();
    }
}

// ── Arg parsing ──────────────────────────────────────────────────────────────

struct Opts {
    creature: String,
    env: String,
    frames: usize,
    out: String,
}

fn parse_args(args: &[String]) -> Opts {
    let mut creature = "swimmer-starfish".to_string();
    let mut env = "Water".to_string();
    let mut frames = 600usize;
    let mut out = "trace".to_string();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--creature" | "-c" => { creature = args[i+1].clone(); i += 2; }
            "--env" | "-e"      => { env = args[i+1].clone(); i += 2; }
            "--frames" | "-f"   => { frames = args[i+1].parse().expect("bad --frames"); i += 2; }
            "--out" | "-o"      => { out = args[i+1].clone(); i += 2; }
            "--help" | "-h" => {
                eprintln!("Usage: karl-sims-scene-trace --creature NAME --env Water|Land --frames N --out PREFIX");
                eprintln!("Creatures: {:?}", creature_def::builtin_names());
                std::process::exit(0);
            }
            _ => { eprintln!("unknown arg: {}", args[i]); i += 1; }
        }
    }
    Opts { creature, env, frames, out }
}
