/// batch_evolve — in-process headless evolution runner + exploit scanner.
///
/// Runs an evolution with the SAME fitness path as the server (evaluate_fitness
/// with full EvolutionParams, including min_joint_motion). Much faster than
/// round-tripping through the DB. After N generations, scans the top creatures
/// for anomalies and prints a diagnostic report.
///
/// Usage: cargo run --release --bin batch_evolve -- --seed 1 --gens 20 --pop 40

use karl_sims_core::creature::Creature;
use karl_sims_core::fitness::{evaluate_fitness, Environment, EvolutionParams, FitnessGoal};
use karl_sims_core::genotype::GenomeGraph;
use karl_sims_core::{mating, mutation as mutate_mod};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

fn parse<T: std::str::FromStr>(args: &[String], flag: &str, default: T) -> T {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .and_then(|s| s.parse().ok())
        .unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = parse(&args, "--seed", 1);
    let gens: usize = parse(&args, "--gens", 20);
    let pop: usize = parse(&args, "--pop", 40);
    let show: usize = parse(&args, "--show", 5);
    let env: String = parse(&args, "--env", "land".to_string());
    let quiet: bool = args.iter().any(|a| a == "--quiet");

    let environment = match env.as_str() {
        "water" => Environment::Water,
        _ => Environment::Land,
    };

    let params = EvolutionParams {
        population_size: pop,
        max_generations: gens,
        goal: FitnessGoal::SwimmingSpeed,
        environment,
        sim_duration: 10.0,
        max_parts: 20,
        gravity: 9.81,
        water_viscosity: 2.0,
        max_body_angular_velocity: Some(20.0),
        num_islands: 1,
        migration_interval: 20,
        min_joint_motion: Some(0.1),
        settle_duration: Some(1.0),
        num_signal_channels: 0,
        growth_interval: None,
        max_joint_angular_velocity: Some(12.0),
        solver_iterations: Some(8),
        pgs_iterations: Some(2),
        friction_coefficient: Some(1.5),
        use_coulomb_friction: Some(true),
        friction_combine_max: Some(true),
        airtime_penalty: 0.0,
        island_strategy: karl_sims_core::fitness::IslandStrategy::Isolated,
        exchange_interval: 10,
        diversity_pressure: 0.0,
    };

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Create initial population.
    let mut individuals: Vec<(GenomeGraph, f64)> = (0..pop)
        .map(|_| (GenomeGraph::random(&mut rng), 0.0))
        .collect();

    if !quiet {
        eprintln!("# seed={seed} pop={pop} gens={gens} env={:?}", params.environment);
    }

    for g in 0..gens {
        // Evaluate unevaluated genomes.
        for ind in individuals.iter_mut() {
            ind.1 = evaluate_fitness(&ind.0, &params).score;
        }
        individuals.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let best = individuals[0].1;
        let avg = individuals.iter().map(|(_, f)| f).sum::<f64>() / pop as f64;
        if !quiet {
            eprintln!("gen {g:>3}: best={best:>7.3} avg={avg:>7.3}");
        }
        if g == gens - 1 { break; }

        // Tournament-selection reproduction.
        let num_survivors = ((pop as f64) * 0.2).ceil() as usize;
        let survivors: Vec<(GenomeGraph, f64)> = individuals[..num_survivors].to_vec();
        let mut next: Vec<(GenomeGraph, f64)> = Vec::with_capacity(pop);
        // Keep survivors with their fitness.
        for s in survivors.iter().cloned() { next.push(s); }
        while next.len() < pop {
            let roll: f64 = rng.r#gen();
            let a = tournament(&survivors, &mut rng);
            let child = if roll < 0.4 {
                let mut c = a.0.clone();
                mutate_mod::mutate(&mut c, &mut rng);
                c
            } else if roll < 0.7 {
                let b = tournament(&survivors, &mut rng);
                let mut c = mating::crossover(&a.0, &b.0, &mut rng);
                mutate_mod::mutate(&mut c, &mut rng);
                c
            } else {
                let b = tournament(&survivors, &mut rng);
                let mut c = mating::graft(&a.0, &b.0, &mut rng);
                mutate_mod::mutate(&mut c, &mut rng);
                c
            };
            next.push((child, 0.0));
        }
        // Mark all non-survivors as needing re-evaluation.
        for i in num_survivors..next.len() { next[i].1 = 0.0; }
        // But survivors keep their fitness — prevent the inherited-fitness
        // shortcut from hiding bugs by actually re-evaluating every generation.
        individuals = next.into_iter().map(|(g, _)| (g, 0.0)).collect();
    }

    // Final re-evaluation + analysis.
    for ind in individuals.iter_mut() {
        ind.1 = evaluate_fitness(&ind.0, &params).score;
    }
    individuals.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Report top N with exploit metrics.
    println!("\n{:>3} {:>8} {:>4} {:>3} {:>3} {:>6} {:>6} {:>6} {:>6} {:>7} {:>5} flags",
        "#","fitness","bd","df","ef","mnDim","drift","peakY","minY","peakV","jAct");
    println!("{}", "─".repeat(100));

    let mut all_flags: Vec<String> = Vec::new();
    for (i, (genome, fit)) in individuals.iter().take(show).enumerate() {
        let m = measure(genome, &params);
        let flags = classify(&m, &params);
        print_row(i, *fit, &m, &flags);
        for f in flags { all_flags.push(f); }
    }
    if !all_flags.is_empty() {
        println!("\n# distinct flags: {:?}", {
            let mut set: std::collections::BTreeSet<String> = all_flags.iter().cloned().collect();
            set.remove("");
            set
        });
    }

    // Optional: trace top creature step-by-step, print high-water-mark joint velocities.
    if args.iter().any(|a| a == "--trace") {
        trace_creature(&individuals[0].0, &params);
    }

    // Optional: dump top genome + confirm its fitness.
    if args.iter().any(|a| a == "--dump-top") {
        let bytes = bincode::serialize(&individuals[0].0).unwrap();
        let path = "/tmp/top_genome.bin";
        std::fs::write(path, &bytes).unwrap();
        eprintln!("# dumped top genome to {path} ({} bytes)", bytes.len());
        // Re-evaluate with and without settle
        let mut p_no = params.clone();
        p_no.settle_duration = None;
        let r_no = evaluate_fitness(&individuals[0].0, &p_no);
        let r_yes = evaluate_fitness(&individuals[0].0, &params);
        eprintln!("# no settle: score={:.3} dist={:.3} max_disp={:.3}", r_no.score, r_no.distance, r_no.max_displacement);
        eprintln!("# settled  : score={:.3} dist={:.3} max_disp={:.3}", r_yes.score, r_yes.distance, r_yes.max_displacement);
    }
}

fn trace_creature(genome: &GenomeGraph, params: &EvolutionParams) {
    println!("\n# TRACE · top creature step-by-step");
    let mut c = Creature::from_genome(genome.clone());
    let spawn_y = match params.environment {
        Environment::Land => {
            c.world.water_enabled = false;
            c.world.gravity = glam::DVec3::new(0.0, -params.gravity, 0.0);
            c.world.ground_enabled = true;
            c.world.set_root_transform(glam::DAffine3::from_translation(glam::DVec3::new(0.0, 2.0, 0.0)));
            c.world.forward_kinematics();
            2.0
        }
        _ => 0.0,
    };
    let _ = spawn_y;
    let dt = 1.0 / 60.0;
    let steps = 600;
    let mut prev_ang: Vec<Vec<f64>> = c.world.joints.iter().map(|j| j.angles.to_vec()).collect();
    let mut prev_q: Vec<glam::DQuat> = c.world.transforms.iter().map(|t| glam::DQuat::from_mat3(&t.matrix3)).collect();
    println!("{:>4} {:>6} {:>6} {:>7} {:>7} {:>7} {:>6}", "step","rootX","rootY","maxJV","maxBAV","maxTrq","extY");
    for step in 0..steps {
        c.step(dt);
        let mut max_jv: f64 = 0.0;
        for (ji, j) in c.world.joints.iter().enumerate() {
            for (d, &a) in j.angles.iter().enumerate() {
                let jv = (a - prev_ang[ji][d]).abs() / dt;
                max_jv = max_jv.max(jv);
                prev_ang[ji][d] = a;
            }
        }
        let mut max_bav: f64 = 0.0;
        let mut ext_y: f64 = 0.0;
        for (i, t) in c.world.transforms.iter().enumerate() {
            let q = glam::DQuat::from_mat3(&t.matrix3);
            let qr = q * prev_q[i].inverse();
            let w = qr.w.abs().clamp(-1.0, 1.0);
            let ang = 2.0 * w.acos();
            let bav = ang / dt;
            max_bav = max_bav.max(bav);
            prev_q[i] = q;
            ext_y = ext_y.max(t.translation.y);
        }
        let max_trq = c.world.torques.iter()
            .flat_map(|t| t.iter().copied().map(|x| x.abs()))
            .fold(0.0_f64, |a, b| a.max(b));
        let p = c.world.transforms[c.world.root].translation;
        if step < 20 || step % 30 == 0 || max_jv > 20.0 || max_bav > 20.0 {
            println!("{:>4} {:>6.2} {:>6.2} {:>7.1} {:>7.1} {:>7.2} {:>6.2}",
                step, p.x, p.y, max_jv, max_bav, max_trq, ext_y);
        }
    }
}

fn tournament<'a>(pool: &'a [(GenomeGraph, f64)], rng: &mut ChaCha8Rng) -> &'a (GenomeGraph, f64) {
    const K: usize = 3;
    let mut best: Option<&(GenomeGraph, f64)> = None;
    for _ in 0..K.min(pool.len()) {
        let idx = rng.gen_range(0..pool.len());
        let cand = &pool[idx];
        if best.map(|b| cand.1 > b.1).unwrap_or(true) { best = Some(cand); }
    }
    best.unwrap()
}

#[derive(Default, Debug)]
struct Metrics {
    body_count: usize,
    total_dofs: usize,
    num_effectors: usize,
    min_body_dim: f64,
    max_body_dim: f64,
    peak_root_y: f64,
    min_root_y: f64,
    horizontal_distance: f64,
    peak_speed: f64,
    mean_speed: f64,
    joint_activity: f64,
    max_joint_angvel: f64,
    escaped: bool,
    nan: bool,
    below_spawn_frac: f64,
    peak_step_delta: f64,
}

fn measure(g: &GenomeGraph, params: &EvolutionParams) -> Metrics {
    let mut m = Metrics::default();
    m.num_effectors = g.nodes.iter().map(|n| n.brain.effectors.len()).sum();

    let mut c = Creature::from_genome(g.clone());
    m.body_count = c.world.bodies.len();
    m.total_dofs = c.world.joints.iter().map(|j| j.joint_type.dof_count()).sum();
    m.min_body_dim = f64::INFINITY;
    for b in &c.world.bodies {
        for &d in &[b.half_extents.x, b.half_extents.y, b.half_extents.z] {
            m.min_body_dim = m.min_body_dim.min(d);
            m.max_body_dim = m.max_body_dim.max(d);
        }
    }
    if !m.min_body_dim.is_finite() { m.min_body_dim = 0.0; }

    let spawn_y = match params.environment {
        Environment::Land => {
            c.world.water_enabled = false;
            c.world.gravity = glam::DVec3::new(0.0, -params.gravity, 0.0);
            c.world.ground_enabled = true;
            c.world.set_root_transform(glam::DAffine3::from_translation(glam::DVec3::new(0.0, 2.0, 0.0)));
            c.world.forward_kinematics();
            2.0
        }
        Environment::Water => {
            c.world.water_enabled = true;
            c.world.water_viscosity = params.water_viscosity;
            c.world.gravity = glam::DVec3::ZERO;
            0.0
        }
    };

    let dt = 1.0 / 60.0;
    let steps = (params.sim_duration / dt).round() as usize;
    let p0 = c.world.transforms[c.world.root].translation;
    m.min_root_y = spawn_y;
    m.peak_root_y = spawn_y;
    let mut prev_p = p0;
    let mut prev_ang: Vec<Vec<f64>> = c.world.joints.iter().map(|j| j.angles.to_vec()).collect();
    let mut below_count = 0usize;
    for _ in 0..steps {
        c.step(dt);
        let p = c.world.transforms[c.world.root].translation;
        let dp = p - prev_p;
        let v = dp.length() / dt;
        if !p.is_finite() { m.nan = true; break; }
        for t in &c.world.transforms {
            if t.translation.length() > 200.0 { m.escaped = true; }
        }
        m.peak_speed = m.peak_speed.max(v);
        m.peak_step_delta = m.peak_step_delta.max(dp.length());
        m.peak_root_y = m.peak_root_y.max(p.y);
        m.min_root_y = m.min_root_y.min(p.y);
        if matches!(params.environment, Environment::Land) && p.y < spawn_y - 0.1 {
            below_count += 1;
        }
        for (ji, j) in c.world.joints.iter().enumerate() {
            for (d, &a) in j.angles.iter().enumerate() {
                // Unwrap the joint angle against the previous sample so a
                // ±π discontinuity doesn't spoof huge angular-velocity values.
                let mut da = a - prev_ang[ji][d];
                while da >  std::f64::consts::PI { da -= 2.0 * std::f64::consts::PI; }
                while da < -std::f64::consts::PI { da += 2.0 * std::f64::consts::PI; }
                let delta = da.abs();
                m.joint_activity += delta;
                m.max_joint_angvel = m.max_joint_angvel.max(delta / dt);
                prev_ang[ji][d] = a;
            }
        }
        prev_p = p;
    }
    let pf = c.world.transforms[c.world.root].translation;
    let diff = pf - p0;
    m.horizontal_distance = glam::DVec3::new(diff.x, 0.0, diff.z).length();
    m.mean_speed = m.horizontal_distance / params.sim_duration;
    m.below_spawn_frac = below_count as f64 / steps as f64;
    m
}

fn classify(m: &Metrics, params: &EvolutionParams) -> Vec<String> {
    let mut f = Vec::new();
    if m.nan { f.push("NAN".into()); }
    if m.escaped { f.push("ESCAPED".into()); }
    if m.total_dofs == 0 && m.body_count > 1 && m.mean_speed > 0.5 {
        f.push("RIGID_DRIFT".into());
    }
    if m.num_effectors == 0 && m.body_count > 1 && m.mean_speed > 0.5 {
        f.push("NOEFF_DRIFT".into());
    }
    if m.mean_speed > 0.5 && m.joint_activity < 0.3 && m.total_dofs > 0 {
        f.push("PASSIVE_DRIFT".into());
    }
    if m.min_body_dim < 0.03 && m.body_count > 1 { f.push("TINY_BODY".into()); }
    if m.peak_speed > 7.0 { f.push(format!("FAST_{:.1}", m.peak_speed)); }
    if matches!(params.environment, Environment::Land) && m.below_spawn_frac < 0.3 && m.body_count > 0 {
        f.push(format!("FLOAT_{:.0}%", m.below_spawn_frac * 100.0));
    }
    if m.peak_step_delta > 0.5 { f.push(format!("TELE_{:.2}", m.peak_step_delta)); }
    if let Some(cap) = params.max_body_angular_velocity {
        if m.max_joint_angvel > cap * 1.5 { f.push(format!("ANG_{:.0}", m.max_joint_angvel)); }
    }
    // High peak/mean ratio: spike pattern (impulse + glide)
    if m.peak_speed > 3.0 && m.mean_speed > 0.5 && m.peak_speed / m.mean_speed.max(1e-6) > 4.0 {
        f.push("SPIKE".into());
    }
    // Peak root Y higher than spawn height → creature climbed
    if matches!(params.environment, Environment::Land) && m.peak_root_y > 2.5 {
        f.push(format!("CLIMB_{:.1}", m.peak_root_y));
    }
    // Creature ended up with root BELOW ground (clipped through)
    if matches!(params.environment, Environment::Land) && m.min_root_y < -0.5 {
        f.push(format!("CLIP_{:.2}", m.min_root_y));
    }
    f
}

fn print_row(i: usize, fit: f64, m: &Metrics, flags: &[String]) {
    println!(
        "{:>3} {:>8.3} {:>4} {:>3} {:>3} {:>6.3} {:>6.2} {:>6.2} {:>6.2} {:>7.2} {:>5.1} {}",
        i, fit, m.body_count, m.total_dofs, m.num_effectors, m.min_body_dim,
        m.horizontal_distance, m.peak_root_y, m.min_root_y, m.peak_speed, m.joint_activity,
        flags.join(",")
    );
}
