/// mega_scan — runs many evolutions in parallel, aggregates exploit patterns.
use karl_sims_core::creature::Creature;
use karl_sims_core::fitness::{evaluate_fitness, Environment, EvolutionParams, FitnessGoal};
use karl_sims_core::genotype::GenomeGraph;
use karl_sims_core::{mating, mutation as mutate_mod};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed_count: usize = args.iter().position(|a| a == "--seeds")
        .and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(20);
    let gens: usize = args.iter().position(|a| a == "--gens")
        .and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(30);
    let pop: usize = args.iter().position(|a| a == "--pop")
        .and_then(|i| args.get(i + 1)).and_then(|s| s.parse().ok()).unwrap_or(50);
    let env: String = args.iter().position(|a| a == "--env")
        .and_then(|i| args.get(i + 1)).cloned().unwrap_or_else(|| "land".into());
    let environment = if env == "water" { Environment::Water } else { Environment::Land };

    let base_params = EvolutionParams {
        population_size: pop, max_generations: gens, goal: FitnessGoal::SwimmingSpeed,
        environment, sim_duration: 10.0, max_parts: 20,
        gravity: 9.81, water_viscosity: 2.0, max_body_angular_velocity: Some(20.0),
        num_islands: 1, migration_interval: 20, min_joint_motion: Some(0.3),
        settle_duration: Some(1.0),
    };

    let mut flag_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut fit_dist: Vec<f64> = Vec::new();
    let mut per_seed_top: Vec<(u64, f64, Vec<String>, String)> = Vec::new();

    for seed in 0..seed_count as u64 {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        let mut individuals: Vec<(GenomeGraph, f64)> = (0..pop)
            .map(|_| (GenomeGraph::random(&mut rng), 0.0))
            .collect();

        for _g in 0..gens {
            for ind in individuals.iter_mut() {
                ind.1 = evaluate_fitness(&ind.0, &base_params).score;
            }
            individuals.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            let num_survivors = ((pop as f64) * 0.2).ceil() as usize;
            let survivors: Vec<(GenomeGraph, f64)> = individuals[..num_survivors].to_vec();
            let mut next: Vec<(GenomeGraph, f64)> = survivors.clone();
            while next.len() < pop {
                let roll: f64 = rng.r#gen();
                let a = tournament(&survivors, &mut rng);
                let child = if roll < 0.4 {
                    let mut c = a.0.clone();
                    mutate_mod::mutate(&mut c, &mut rng); c
                } else if roll < 0.7 {
                    let b = tournament(&survivors, &mut rng);
                    let mut c = mating::crossover(&a.0, &b.0, &mut rng);
                    mutate_mod::mutate(&mut c, &mut rng); c
                } else {
                    let b = tournament(&survivors, &mut rng);
                    let mut c = mating::graft(&a.0, &b.0, &mut rng);
                    mutate_mod::mutate(&mut c, &mut rng); c
                };
                next.push((child, 0.0));
            }
            individuals = next.into_iter().map(|(g, _)| (g, 0.0)).collect();
        }
        for ind in individuals.iter_mut() {
            ind.1 = evaluate_fitness(&ind.0, &base_params).score;
        }
        individuals.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top = &individuals[0];
        fit_dist.push(top.1);
        let m = measure(&top.0, &base_params);
        let sig = signature(&m);
        let flags = classify(&m, &base_params);
        for f in &flags {
            *flag_counts.entry(f.clone()).or_insert(0) += 1;
        }
        per_seed_top.push((seed, top.1, flags, sig));
    }

    println!("# seeds={} gens={} pop={} env={env}", seed_count, gens, pop);
    println!("\n# per-seed top creature:");
    for (seed, fit, flags, sig) in &per_seed_top {
        println!("seed {seed:>3}: fit={fit:>6.2}  {sig}  {}", flags.join(","));
    }

    let mean: f64 = fit_dist.iter().sum::<f64>() / fit_dist.len() as f64;
    let mut sorted = fit_dist.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = sorted[sorted.len() / 2];
    let p90 = sorted[(sorted.len() * 90 / 100).min(sorted.len() - 1)];
    println!("\n# fitness distribution: mean={:.2} median={:.2} p90={:.2} max={:.2}",
        mean, median, p90, sorted[sorted.len()-1]);

    println!("\n# flag counts:");
    let mut entries: Vec<(&String, &usize)> = flag_counts.iter().collect();
    entries.sort_by(|a, b| b.1.cmp(a.1));
    for (flag, count) in entries {
        println!("  {flag:>30}: {count}");
    }
}

fn signature(m: &Metrics) -> String {
    format!("bd={:>2} df={:>2} ef={:>2} mnDim={:>5.3} drift={:>5.2} jAct={:>5.1}",
        m.body_count, m.total_dofs, m.num_effectors, m.min_body_dim,
        m.horizontal_distance, m.joint_activity)
}

fn tournament<'a>(pool: &'a [(GenomeGraph, f64)], rng: &mut ChaCha8Rng) -> &'a (GenomeGraph, f64) {
    let mut best: Option<&(GenomeGraph, f64)> = None;
    for _ in 0..3.min(pool.len()) {
        let idx = rng.gen_range(0..pool.len());
        let cand = &pool[idx];
        if best.map(|b| cand.1 > b.1).unwrap_or(true) { best = Some(cand); }
    }
    best.unwrap()
}

#[derive(Default)]
struct Metrics {
    body_count: usize, total_dofs: usize, num_effectors: usize,
    min_body_dim: f64, peak_root_y: f64, min_root_y: f64,
    horizontal_distance: f64, peak_speed: f64, joint_activity: f64,
    max_joint_angvel: f64, escaped: bool, nan: bool, below_spawn_frac: f64,
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
        }
    }
    if !m.min_body_dim.is_finite() { m.min_body_dim = 0.0; }
    let spawn_y = match params.environment {
        Environment::Land => { c.world.water_enabled = false;
            c.world.gravity = glam::DVec3::new(0.0, -params.gravity, 0.0);
            c.world.ground_enabled = true;
            c.world.set_root_transform(glam::DAffine3::from_translation(glam::DVec3::new(0.0, 2.0, 0.0)));
            c.world.forward_kinematics(); 2.0 },
        _ => { c.world.water_enabled = true; c.world.water_viscosity = params.water_viscosity;
            c.world.gravity = glam::DVec3::ZERO; 0.0 },
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
        if matches!(params.environment, Environment::Land) && p.y < spawn_y - 0.1 { below_count += 1; }
        for (ji, j) in c.world.joints.iter().enumerate() {
            for (d, &a) in j.angles.iter().enumerate() {
                let mut da = a - prev_ang[ji][d];
                while da > std::f64::consts::PI { da -= 2.0 * std::f64::consts::PI; }
                while da < -std::f64::consts::PI { da += 2.0 * std::f64::consts::PI; }
                m.joint_activity += da.abs();
                m.max_joint_angvel = m.max_joint_angvel.max(da.abs() / dt);
                prev_ang[ji][d] = a;
            }
        }
        prev_p = p;
    }
    let pf = c.world.transforms[c.world.root].translation;
    let diff = pf - p0;
    m.horizontal_distance = if matches!(params.environment, Environment::Land) {
        glam::DVec3::new(diff.x, 0.0, diff.z).length()
    } else { diff.length() };
    m.below_spawn_frac = below_count as f64 / steps as f64;
    m
}

fn classify(m: &Metrics, params: &EvolutionParams) -> Vec<String> {
    let mut f = Vec::new();
    if m.nan { f.push("NAN".into()); }
    if m.escaped { f.push("ESCAPED".into()); }
    if m.total_dofs == 0 && m.body_count > 1 && m.horizontal_distance > 0.5 { f.push("RIGID_DRIFT".into()); }
    if m.num_effectors == 0 && m.body_count > 1 && m.horizontal_distance > 0.5 { f.push("NOEFF_DRIFT".into()); }
    if m.horizontal_distance > 0.5 && m.joint_activity < 0.3 && m.total_dofs > 0 { f.push("PASSIVE".into()); }
    if m.min_body_dim < 0.03 && m.body_count > 1 { f.push("TINY_BODY".into()); }
    if m.peak_speed > 7.0 { f.push("FAST_PEAK".into()); }
    if matches!(params.environment, Environment::Land) && m.below_spawn_frac < 0.3 && m.body_count > 0 { f.push("FLOAT".into()); }
    if m.peak_step_delta > 0.5 { f.push("TELEPORT".into()); }
    if let Some(cap) = params.max_body_angular_velocity {
        if m.max_joint_angvel > cap * 1.5 { f.push("ANG_CAP".into()); }
    }
    if m.peak_speed > 3.0 && m.horizontal_distance > 5.0 && m.peak_speed / (m.horizontal_distance / 10.0).max(1e-6) > 4.0 { f.push("SPIKE".into()); }
    if matches!(params.environment, Environment::Land) && m.peak_root_y > 2.5 { f.push("CLIMB".into()); }
    if matches!(params.environment, Environment::Land) && m.min_root_y < -0.5 { f.push("CLIP".into()); }
    // Effector bloat: many genome effectors but tiny DOF count
    if m.num_effectors > 5 && m.num_effectors > m.total_dofs * 5 { f.push("EFF_BLOAT".into()); }
    f
}
