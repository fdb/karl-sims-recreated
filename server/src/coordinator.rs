use std::sync::Arc;

use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use karl_sims_core::evolution::EvolutionConfig;
use karl_sims_core::fitness::{EvolutionParams, FitnessConfig};
use karl_sims_core::genotype::GenomeGraph;

use tokio::sync::broadcast;

use crate::db::{
    create_task, get_evolution_seed, get_evolution_status, get_evolution_full,
    get_max_generations, insert_genotype, insert_genotype_with_fitness,
    load_island_generation, update_evolution, DbPool,
};
use crate::engine::{
    CreatureSnapshot, Engine, EvalTask,
    EvolutionSnapshot, GenStatSnapshot, IslandStatSnapshot,
};
use crate::timing::timed_db;

/// Run a full evolution loop.  Hot state lives in memory; SQLite is used
/// only for archival writes (so the creature viewer can fetch genomes)
/// and status reads (so PATCH /stop works across restart).
pub async fn run_evolution(
    engine: Arc<Engine>,
    db: DbPool,
    evo_id: i64,
    tx: Option<broadcast::Sender<String>>,
) {
    let seed = timed_db("coord.init", &db, |c| get_evolution_seed(c, evo_id));
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let (params, config_json_str, evo_name, created_at): (EvolutionParams, String, Option<String>, String) =
        timed_db("coord.init", &db, |c| {
            let full = get_evolution_full(c, evo_id)
                .expect("evolution not found");
            let (_, _, config_json, name) = full;
            let params: EvolutionParams =
                serde_json::from_str(&config_json).unwrap_or_default();
            // Get created_at for the snapshot
            let created_at: String = c
                .query_row(
                    "SELECT created_at FROM evolutions WHERE id = ?1",
                    rusqlite::params![evo_id],
                    |row| row.get(0),
                )
                .unwrap_or_default();
            (params, config_json, name, created_at)
        });

    let config_json_for_workers = config_json_str.clone();

    let config = EvolutionConfig {
        population_size: params.population_size,
        fitness: FitnessConfig {
            sim_duration: params.sim_duration,
            max_parts: params.max_parts,
            ..Default::default()
        },
        ..Default::default()
    };

    let start_gen = timed_db("coord.init", &db, |c| {
        get_evolution_status(c, evo_id)
            .map(|(_, current_gen)| current_gen)
            .unwrap_or(0) as usize
    });

    let num_islands = params.num_islands.max(1);
    let per_island_pop = (params.population_size / num_islands).max(1);

    // ── Initial population or resume ────────────────────────────────────
    // `island_individuals` is the in-memory state we carry across generations.
    let mut island_individuals: Vec<Vec<(GenomeGraph, f64, i64)>> =
        vec![Vec::new(); num_islands];

    let has_genotypes = timed_db("coord.init", &db, |c| {
        !load_island_generation(c, evo_id, 0, 0).is_empty()
    });

    if !has_genotypes {
        log::info!(
            "Evolution {evo_id}: creating initial population ({num_islands} islands × {per_island_pop} creatures)"
        );
        timed_db("coord.init_pop", &db, |conn| {
            for island_id in 0..num_islands {
                for _ in 0..per_island_pop {
                    let genome = GenomeGraph::random(&mut rng);
                    let bytes = bincode::serialize(&genome).unwrap();
                    let gid = insert_genotype(conn, evo_id, 0, &bytes, None, island_id as i64);
                    create_task(conn, evo_id, gid);
                    island_individuals[island_id].push((genome, 0.0, gid));
                }
            }
        });

        // Evaluate generation 0 via channels
        evaluate_generation(
            &engine,
            &config_json_for_workers,
            &mut island_individuals,
        );

        // Write fitness back to DB
        archive_fitness(&db, &island_individuals);
    } else {
        log::info!("Evolution {evo_id}: resuming from generation {start_gen}");
        // Load current gen from DB (one-time on resume)
        for island_id in 0..num_islands {
            let rows = timed_db("coord.resume_load", &db, |conn| {
                load_island_generation(conn, evo_id, island_id as i64, start_gen as i64)
            });
            for (gid, fitness, bytes) in rows {
                if let Ok(genome) = bincode::deserialize(&bytes) {
                    island_individuals[island_id].push((genome, fitness, gid));
                }
            }
        }
        // Advance RNG to match
        for _ in 0..start_gen * params.population_size {
            let _: f64 = rng.r#gen();
        }
    }

    // Sort each island by fitness (descending)
    for island in &mut island_individuals {
        island.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    }

    // Initial snapshot
    update_engine_snapshot(
        &engine, evo_id, &evo_name, "running", start_gen as i64,
        &config_json_str, &created_at, &island_individuals, &[],
    );

    let mut gen_stats: Vec<GenStatSnapshot> = Vec::new();
    let mut all_island_stats: Vec<IslandStatSnapshot> = Vec::new();

    let mut cur_gen = start_gen;
    loop {
        let max_generations = timed_db("coord.poll_max_gen", &db, |c| {
            get_max_generations(c, evo_id).unwrap_or(params.max_generations)
        });
        if cur_gen >= max_generations {
            break;
        }

        // Check stop/pause status
        loop {
            let status = timed_db("coord.poll_status", &db, |c| {
                get_evolution_status(c, evo_id).map(|(s, _)| s)
            });
            match status.as_deref() {
                Some("stopped") => {
                    log::info!("Evolution {evo_id} stopped");
                    engine.set_status(evo_id, "stopped");
                    return;
                }
                Some("paused") => {
                    engine.set_status(evo_id, "paused");
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    continue;
                }
                _ => break,
            }
        }

        // ── Log + broadcast stats for this completed generation ─────────
        let (global_best, global_avg) = compute_stats(&island_individuals);

        if num_islands == 1 {
            log::info!(
                "Evolution {evo_id} Gen {cur_gen}: best={global_best:.4}, avg={global_avg:.4}, pop={}",
                island_individuals.iter().map(|i| i.len()).sum::<usize>()
            );
        } else {
            let per_island: Vec<String> = island_individuals
                .iter()
                .enumerate()
                .map(|(i, pop)| {
                    let b = pop.first().map(|(_, f, _)| *f).unwrap_or(0.0);
                    format!("i{i}={b:.2}")
                })
                .collect();
            log::info!(
                "Evolution {evo_id} Gen {cur_gen}: best={global_best:.4} avg={global_avg:.4} islands[{}]",
                per_island.join(" ")
            );
        }

        gen_stats.push(GenStatSnapshot {
            generation: cur_gen as i64,
            best_fitness: global_best,
            avg_fitness: global_avg,
        });
        for (island_id, pop) in island_individuals.iter().enumerate() {
            let (ibest, iavg) = island_stats_for(pop);
            all_island_stats.push(IslandStatSnapshot {
                generation: cur_gen as i64,
                island_id: island_id as i64,
                best_fitness: ibest,
                avg_fitness: iavg,
            });
        }

        if let Some(ref tx) = tx {
            let msg = serde_json::json!({
                "type": "generation",
                "evolution_id": evo_id,
                "generation": cur_gen,
                "best_fitness": global_best,
                "avg_fitness": global_avg,
            });
            tx.send(msg.to_string()).ok();
        }

        // ── All-zero recovery ───────────────────────────────────────────
        let any_progress = island_individuals
            .iter()
            .any(|pop| pop.iter().any(|(_, f, _)| *f > 0.0));
        if !any_progress {
            log::warn!(
                "Evolution {evo_id} Gen {cur_gen}: all zero fitness, regenerating"
            );
            let next_gen = cur_gen + 1;
            island_individuals = vec![Vec::new(); num_islands];
            timed_db("coord.regen_all_zero", &db, |conn| {
                update_evolution(conn, evo_id, "running", next_gen as i64);
                for island_id in 0..num_islands {
                    for _ in 0..per_island_pop {
                        let genome = GenomeGraph::random(&mut rng);
                        let bytes = bincode::serialize(&genome).unwrap();
                        let gid = insert_genotype(conn, evo_id, next_gen as i64, &bytes, None, island_id as i64);
                        create_task(conn, evo_id, gid);
                        island_individuals[island_id].push((genome, 0.0, gid));
                    }
                }
            });
            evaluate_generation(&engine, &config_json_for_workers, &mut island_individuals);
            archive_fitness(&db, &island_individuals);
            cur_gen = next_gen;
            continue;
        }

        // ── Reproduce next generation ───────────────────────────────────
        let next_gen = cur_gen + 1;

        // Migration: ring topology
        let migration_active = num_islands > 1
            && params.migration_interval > 0
            && next_gen > 0
            && next_gen % params.migration_interval == 0;
        let best_of: Vec<Option<(GenomeGraph, f64)>> = if migration_active {
            island_individuals
                .iter()
                .map(|pop| pop.first().map(|(g, f, _)| (g.clone(), *f)))
                .collect()
        } else {
            vec![None; num_islands]
        };
        if migration_active {
            log::info!("Evolution {evo_id} Gen {next_gen}: migrating elites along ring");
        }

        // Build next generation in memory, INSERT to DB for archival
        let mut next_island_individuals: Vec<Vec<(GenomeGraph, f64, i64)>> =
            vec![Vec::new(); num_islands];

        timed_db("coord.reproduce_gen", &db, |conn| {
            conn.execute_batch("BEGIN IMMEDIATE").expect("begin txn");
            update_evolution(conn, evo_id, "running", next_gen as i64);

            for island_id in 0..num_islands {
                let individuals = &island_individuals[island_id];
                if individuals.is_empty() {
                    continue;
                }

                let num_survivors = (config.survival_ratio * individuals.len() as f64)
                    .ceil() as usize;
                let survivors: Vec<&GenomeGraph> = individuals
                    [..num_survivors.min(individuals.len())]
                    .iter()
                    .map(|(g, _, _)| g)
                    .collect();
                let survivor_fitnesses: Vec<f64> = individuals
                    [..num_survivors.min(individuals.len())]
                    .iter()
                    .map(|(_, f, _)| *f)
                    .collect();
                if survivors.is_empty() {
                    continue;
                }

                let mut offspring_count = 0usize;
                let bucket = &mut next_island_individuals[island_id];

                // Inbound migrant
                if let Some((genome, fitness)) = if migration_active {
                    let src = (island_id + num_islands - 1) % num_islands;
                    best_of[src].clone()
                } else {
                    None
                } {
                    let bytes = bincode::serialize(&genome).unwrap();
                    let gid = insert_genotype_with_fitness(
                        conn, evo_id, next_gen as i64, &bytes, fitness, island_id as i64,
                    );
                    bucket.push((genome, fitness, gid));
                    offspring_count += 1;
                }

                // Keep survivors
                for (genome, fitness, _old_id) in individuals[..num_survivors.min(individuals.len())].iter() {
                    if offspring_count >= per_island_pop { break; }
                    let bytes = bincode::serialize(genome).unwrap();
                    let gid = insert_genotype_with_fitness(
                        conn, evo_id, next_gen as i64, &bytes, *fitness, island_id as i64,
                    );
                    bucket.push((genome.clone(), *fitness, gid));
                    offspring_count += 1;
                }

                // Random injection
                const INJECTION_INTERVAL: usize = 10;
                const INJECTION_FRACTION: f64 = 0.10;
                let inject_count = if next_gen > 0 && next_gen % INJECTION_INTERVAL == 0 {
                    (per_island_pop as f64 * INJECTION_FRACTION).round() as usize
                } else {
                    0
                };
                for _ in 0..inject_count {
                    if offspring_count >= per_island_pop { break; }
                    let child = GenomeGraph::random(&mut rng);
                    let bytes = bincode::serialize(&child).unwrap();
                    let gid = insert_genotype(
                        conn, evo_id, next_gen as i64, &bytes, None, island_id as i64,
                    );
                    // Needs evaluation — fitness starts at 0
                    bucket.push((child, 0.0, gid));
                    offspring_count += 1;
                }

                // Fill with offspring
                while offspring_count < per_island_pop {
                    let roll: f64 = rng.r#gen();
                    let child = if roll < config.asexual_ratio {
                        let parent = pick_weighted(&survivors, &survivor_fitnesses, &mut rng);
                        let mut child = parent.clone();
                        karl_sims_core::mutation::mutate_with_signals(&mut child, &mut rng, params.num_signal_channels);
                        child
                    } else if roll < config.asexual_ratio + config.crossover_ratio {
                        let p1 = pick_weighted(&survivors, &survivor_fitnesses, &mut rng);
                        let p2 = pick_weighted(&survivors, &survivor_fitnesses, &mut rng);
                        let mut child = karl_sims_core::mating::crossover(p1, p2, &mut rng);
                        karl_sims_core::mutation::mutate_with_signals(&mut child, &mut rng, params.num_signal_channels);
                        child
                    } else {
                        let p1 = pick_weighted(&survivors, &survivor_fitnesses, &mut rng);
                        let p2 = pick_weighted(&survivors, &survivor_fitnesses, &mut rng);
                        let mut child = karl_sims_core::mating::graft(p1, p2, &mut rng);
                        karl_sims_core::mutation::mutate_with_signals(&mut child, &mut rng, params.num_signal_channels);
                        child
                    };

                    let bytes = bincode::serialize(&child).unwrap();
                    let gid = insert_genotype(
                        conn, evo_id, next_gen as i64, &bytes, None, island_id as i64,
                    );
                    bucket.push((child, 0.0, gid));
                    offspring_count += 1;
                }
            }

            conn.execute_batch("COMMIT").expect("commit txn");
        });

        // ── Evaluate new creatures via channel ──────────────────────────
        // Survivors already have fitness; only creatures with fitness==0 need eval.
        evaluate_generation(&engine, &config_json_for_workers, &mut next_island_individuals);

        // ── Write fitness back to DB (for creature viewer) ──────────────
        archive_fitness(&db, &next_island_individuals);

        // ── Sort and update in-memory state ─────────────────────────────
        for island in &mut next_island_individuals {
            island.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        }
        island_individuals = next_island_individuals;

        // Update engine snapshot for API
        update_engine_snapshot(
            &engine, evo_id, &evo_name, "running", next_gen as i64,
            &config_json_str, &created_at, &island_individuals,
            &gen_stats,
        );

        cur_gen = next_gen;
    }

    timed_db("coord.finish", &db, |c| {
        update_evolution(c, evo_id, "completed", -1)
    });
    engine.set_status(evo_id, "completed");
    log::info!("Evolution {evo_id} completed");
}

/// Send creatures that need evaluation to workers via channel and collect results.
fn evaluate_generation(
    engine: &Engine,
    config_json: &str,
    island_individuals: &mut Vec<Vec<(GenomeGraph, f64, i64)>>,
) {
    // Collect indices of creatures needing evaluation (fitness == 0)
    let mut tasks_sent = 0usize;
    let (result_tx, result_rx) = crossbeam_channel::bounded(
        island_individuals.iter().map(|i| i.len()).sum::<usize>() + 1,
    );

    // We need to track which (island, index) each task corresponds to.
    // Since results come back in arbitrary order, we use a flat index.
    let mut task_map: Vec<(usize, usize)> = Vec::new(); // (island_id, idx_within_island)

    for (island_id, island) in island_individuals.iter().enumerate() {
        for (idx, (genome, fitness, _gid)) in island.iter().enumerate() {
            if *fitness != 0.0 {
                continue; // survivor — already has fitness
            }
            let bytes = bincode::serialize(genome).unwrap();
            engine
                .task_tx
                .send(EvalTask {
                    genome_bytes: bytes,
                    config_json: config_json.to_string(),
                    result_tx: result_tx.clone(),
                })
                .expect("task channel closed");
            task_map.push((island_id, idx));
            tasks_sent += 1;
        }
    }

    // Drop our copy of result_tx so the channel closes when all workers are done
    drop(result_tx);

    // Collect exactly `tasks_sent` results
    for i in 0..tasks_sent {
        match result_rx.recv() {
            Ok(result) => {
                let (island_id, idx) = task_map[i];
                island_individuals[island_id][idx].1 = result.fitness;
            }
            Err(_) => {
                log::error!("Result channel closed prematurely ({i}/{tasks_sent} received)");
                break;
            }
        }
    }
}

/// Write fitness values back to DB for creatures that were just evaluated.
fn archive_fitness(db: &DbPool, island_individuals: &[Vec<(GenomeGraph, f64, i64)>]) {
    timed_db("coord.archive_fitness", db, |conn| {
        conn.execute_batch("BEGIN").ok();
        for island in island_individuals {
            for (_genome, fitness, gid) in island {
                conn.execute(
                    "UPDATE genotypes SET fitness = ?1 WHERE id = ?2 AND fitness IS NULL",
                    rusqlite::params![fitness, gid],
                )
                .ok();
            }
        }
        conn.execute_batch("COMMIT").ok();
    });
}

fn compute_stats(island_individuals: &[Vec<(GenomeGraph, f64, i64)>]) -> (f64, f64) {
    let all: Vec<f64> = island_individuals
        .iter()
        .flatten()
        .map(|(_, f, _)| *f)
        .collect();
    if all.is_empty() {
        return (0.0, 0.0);
    }
    let best = all.iter().copied().fold(f64::NEG_INFINITY, f64::max).max(0.0);
    let avg = all.iter().sum::<f64>() / all.len() as f64;
    (best, avg)
}

fn island_stats_for(pop: &[(GenomeGraph, f64, i64)]) -> (f64, f64) {
    if pop.is_empty() {
        return (0.0, 0.0);
    }
    let best = pop.iter().map(|(_, f, _)| *f).fold(f64::NEG_INFINITY, f64::max).max(0.0);
    let avg = pop.iter().map(|(_, f, _)| f).sum::<f64>() / pop.len() as f64;
    (best, avg)
}

fn update_engine_snapshot(
    engine: &Engine,
    evo_id: i64,
    name: &Option<String>,
    status: &str,
    current_gen: i64,
    config_json: &str,
    created_at: &str,
    island_individuals: &[Vec<(GenomeGraph, f64, i64)>],
    gen_stats: &[GenStatSnapshot],
) {
    // Build best creatures (top 10 across all islands)
    let mut all_creatures: Vec<CreatureSnapshot> = island_individuals
        .iter()
        .enumerate()
        .flat_map(|(island_id, pop)| {
            pop.iter().map(move |(_, fitness, gid)| CreatureSnapshot {
                id: *gid,
                fitness: *fitness,
                island_id: island_id as i64,
            })
        })
        .collect();
    all_creatures.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap_or(std::cmp::Ordering::Equal));
    let best_creatures: Vec<CreatureSnapshot> = all_creatures.iter().take(10).cloned().collect();

    // Best per island
    let best_per_island: Vec<CreatureSnapshot> = island_individuals
        .iter()
        .enumerate()
        .filter_map(|(island_id, pop)| {
            pop.first().map(|(_, fitness, gid)| CreatureSnapshot {
                id: *gid,
                fitness: *fitness,
                island_id: island_id as i64,
            })
        })
        .collect();

    // Island stats for current gen
    let island_stats: Vec<IslandStatSnapshot> = island_individuals
        .iter()
        .enumerate()
        .map(|(island_id, pop)| {
            let (best, avg) = island_stats_for(pop);
            IslandStatSnapshot {
                generation: current_gen,
                island_id: island_id as i64,
                best_fitness: best,
                avg_fitness: avg,
            }
        })
        .collect();

    // Merge with existing snapshot's historical stats
    let existing = engine.get_snapshot(evo_id);
    let mut merged_gen_stats = existing
        .as_ref()
        .map(|s| s.gen_stats.clone())
        .unwrap_or_default();
    for stat in gen_stats {
        if !merged_gen_stats.iter().any(|s| s.generation == stat.generation) {
            merged_gen_stats.push(stat.clone());
        }
    }
    let mut merged_island_stats = existing
        .as_ref()
        .map(|s| s.island_stats.clone())
        .unwrap_or_default();
    for stat in &island_stats {
        if !merged_island_stats.iter().any(|s| s.generation == stat.generation && s.island_id == stat.island_id) {
            merged_island_stats.push(stat.clone());
        }
    }

    // Also merge all-time best creatures
    let mut merged_best = existing
        .as_ref()
        .map(|s| s.best_creatures.clone())
        .unwrap_or_default();
    for c in &all_creatures {
        if !merged_best.iter().any(|b| b.id == c.id) {
            merged_best.push(c.clone());
        }
    }
    merged_best.sort_by(|a, b| b.fitness.partial_cmp(&a.fitness).unwrap_or(std::cmp::Ordering::Equal));
    merged_best.truncate(10);

    engine.update_snapshot(EvolutionSnapshot {
        id: evo_id,
        name: name.clone(),
        status: status.to_string(),
        current_gen,
        config_json: config_json.to_string(),
        created_at: created_at.to_string(),
        best_creatures: merged_best,
        best_per_island,
        gen_stats: merged_gen_stats,
        island_stats: merged_island_stats,
    });
}

const TOURNAMENT_SIZE: usize = 3;

fn tournament_pick<R: Rng>(fitnesses: &[f64], rng: &mut R) -> usize {
    let n = fitnesses.len();
    assert!(n > 0, "tournament_pick called with empty fitnesses");
    let mut best_idx = rng.gen_range(0..n);
    let mut best_fit = fitnesses[best_idx];
    for _ in 1..TOURNAMENT_SIZE.min(n) {
        let candidate = rng.gen_range(0..n);
        if fitnesses[candidate] > best_fit {
            best_idx = candidate;
            best_fit = fitnesses[candidate];
        }
    }
    best_idx
}

fn pick_weighted<'a, R: Rng>(
    parents: &'a [&GenomeGraph],
    fitnesses: &[f64],
    rng: &mut R,
) -> &'a GenomeGraph {
    parents[tournament_pick(fitnesses, rng)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn tournament_pick_favors_higher_fitness() {
        let fitnesses: Vec<f64> = (0..10).map(|i| i as f64).collect();
        let mut rng = ChaCha8Rng::seed_from_u64(1);
        let trials = 10_000;
        let mut top_wins = 0;
        let mut bottom_wins = 0;
        for _ in 0..trials {
            let idx = tournament_pick(&fitnesses, &mut rng);
            if idx == 9 { top_wins += 1; }
            if idx == 0 { bottom_wins += 1; }
        }
        assert!(top_wins > 2000, "top wins {top_wins}, expected >2000");
        assert!(bottom_wins < 200, "bottom wins {bottom_wins}, expected <200");
    }

    #[test]
    fn tournament_pick_uniform_when_all_equal_fitness() {
        let fitnesses: Vec<f64> = vec![5.0; 10];
        let mut rng = ChaCha8Rng::seed_from_u64(2);
        let mut counts = vec![0usize; 10];
        for _ in 0..10_000 {
            counts[tournament_pick(&fitnesses, &mut rng)] += 1;
        }
        for (i, &c) in counts.iter().enumerate() {
            assert!(c > 700 && c < 1300, "bucket {i}: {c} (expected ~1000)");
        }
    }

    #[test]
    fn tournament_pick_single_candidate() {
        let fitnesses = vec![42.0];
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        for _ in 0..100 {
            assert_eq!(tournament_pick(&fitnesses, &mut rng), 0);
        }
    }
}
