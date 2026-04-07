use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use karl_sims_core::evolution::EvolutionConfig;
use karl_sims_core::fitness::{EvolutionParams, FitnessConfig};
use karl_sims_core::genotype::GenomeGraph;

use tokio::sync::broadcast;

use crate::db::{
    create_task, get_evolution_seed, get_evolution_status, get_evolution_full,
    get_generation_fitnesses, get_max_generations,
    insert_genotype, insert_genotype_with_fitness, load_island_generation, pending_task_count,
    update_evolution, DbPool,
};
use crate::timing::timed_db;

/// Run a full evolution loop, persisting every generation to the database.
pub async fn run_evolution(db: DbPool, evo_id: i64, tx: Option<broadcast::Sender<String>>) {
    let seed = timed_db("coord.init", &db, |c| get_evolution_seed(c, evo_id));
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Read params from DB.
    let params: EvolutionParams = timed_db("coord.init", &db, |c| {
        get_evolution_full(c, evo_id)
            .and_then(|(_, _, config_json, _)| serde_json::from_str(&config_json).ok())
            .unwrap_or_default()
    });

    let config = EvolutionConfig {
        population_size: params.population_size,
        fitness: FitnessConfig {
            sim_duration: params.sim_duration,
            max_parts: params.max_parts,
            ..Default::default()
        },
        ..Default::default()
    };

    // Check if this evolution already has generations (resuming after server restart).
    let start_gen = timed_db("coord.init", &db, |c| {
        get_evolution_status(c, evo_id)
            .map(|(_, current_gen)| current_gen)
            .unwrap_or(0) as usize
    });

    // Only create generation 0 if the evolution is brand new (no genotypes yet).
    let has_genotypes = timed_db("coord.init", &db, |c| {
        !get_generation_fitnesses(c, evo_id, 0).is_empty() || pending_task_count(c, evo_id) > 0
    });

    // Split population across islands. `num_islands=1` preserves single-pool behavior.
    let num_islands = params.num_islands.max(1);
    let per_island_pop = (params.population_size / num_islands).max(1);

    if !has_genotypes {
        log::info!(
            "Evolution {evo_id}: creating initial population ({} islands × {} creatures = {} total)",
            num_islands, per_island_pop, num_islands * per_island_pop,
        );
        // One-off initial-population insert: holds a single connection for
        // `num_islands × per_island_pop` write pairs. Tagged as a single
        // "init" sample — the per-insert cost shows up under the per-gen
        // labels below.
        timed_db("coord.init_pop", &db, |conn| {
            for island_id in 0..num_islands {
                for _ in 0..per_island_pop {
                    let genome = GenomeGraph::random(&mut rng);
                    let bytes = bincode::serialize(&genome).unwrap();
                    let gid = insert_genotype(conn, evo_id, 0, &bytes, None, island_id as i64);
                    create_task(conn, evo_id, gid);
                }
            }
        });
    } else {
        log::info!("Evolution {evo_id}: resuming from generation {start_gen}");
        // Advance the RNG to match where we left off (approximate)
        for _ in 0..start_gen * params.population_size {
            let _: f64 = rng.r#gen();
        }
    }

    let mut cur_gen = start_gen;
    loop {
        // Re-read max_generations every iteration so a PATCH /config while
        // running is picked up without a restart.
        let max_generations = timed_db("coord.poll_max_gen", &db, |c| {
            get_max_generations(c, evo_id).unwrap_or(params.max_generations)
        });
        if cur_gen >= max_generations {
            break;
        }

        // Check if the evolution has been stopped or paused.
        loop {
            // Hot poll: runs every 500ms while paused, plus once per generation.
            let status = timed_db("coord.poll_status", &db, |c| {
                get_evolution_status(c, evo_id).map(|(s, _)| s)
            });
            match status.as_deref() {
                Some("stopped") => {
                    log::info!("Evolution {evo_id} stopped");
                    return;
                }
                Some("paused") => {
                    // Wait until resumed or stopped
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    continue;
                }
                _ => break, // "running" or other — proceed
            }
        }

        // Wait for all pending/running tasks to complete.
        loop {
            // Hot poll: every 200ms while workers are still running a gen.
            // This does a COUNT(*) against `tasks` — expect sub-ms p50, watch
            // the p99 for contention with workers' UPDATEs.
            let pending = timed_db("coord.poll_pending", &db, |c| {
                pending_task_count(c, evo_id)
            });
            if pending == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        // ── Load this generation's individuals, per island ────────────────
        // Each island runs its own selection/reproduction cycle using only
        // its own creatures; migration (below) shuttles elites between.
        let mut island_individuals: Vec<Vec<(GenomeGraph, f64)>> =
            vec![Vec::new(); num_islands];
        for (island_id, bucket) in island_individuals.iter_mut().enumerate() {
            // Single-query load: fetches fitness + genome_bytes together,
            // replacing the previous N+1 pattern (1 fitness query + N BLOB
            // reads). On a large WAL, the old N+1 pattern scanned WAL frames
            // independently for each BLOB read, causing 8-18s load times.
            timed_db("coord.load_gen", &db, |conn| {
                let rows = load_island_generation(
                    conn, evo_id, island_id as i64, cur_gen as i64,
                );
                for (_gid, fitness, bytes) in &rows {
                    if let Ok(genome) = bincode::deserialize(bytes) {
                        bucket.push((genome, *fitness));
                    }
                }
            });
            bucket.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        }

        // Global stats for logging/WebSocket (aggregated across islands).
        let total_individuals: Vec<&(GenomeGraph, f64)> =
            island_individuals.iter().flatten().collect();
        let global_best = total_individuals
            .iter()
            .map(|(_, f)| *f)
            .fold(f64::NEG_INFINITY, f64::max)
            .max(0.0);
        let global_avg = if total_individuals.is_empty() {
            0.0
        } else {
            total_individuals.iter().map(|(_, f)| f).sum::<f64>()
                / total_individuals.len() as f64
        };
        if num_islands == 1 {
            log::info!(
                "Evolution {evo_id} Gen {cur_gen}: best={global_best:.4}, avg={global_avg:.4}, pop={}",
                total_individuals.len()
            );
        } else {
            let per_island: Vec<String> = island_individuals
                .iter()
                .enumerate()
                .map(|(i, pop)| {
                    let b = pop.first().map(|(_, f)| *f).unwrap_or(0.0);
                    format!("i{i}={b:.2}")
                })
                .collect();
            log::info!(
                "Evolution {evo_id} Gen {cur_gen}: best={global_best:.4} avg={global_avg:.4} islands[{}]",
                per_island.join(" ")
            );
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

        // All-zero recovery: if no island has any non-zero fitness, regenerate.
        let any_progress = island_individuals
            .iter()
            .any(|pop| pop.iter().any(|(_, f)| *f > 0.0));
        if !any_progress {
            log::warn!(
                "Evolution {evo_id} Gen {cur_gen}: all zero fitness, regenerating random population"
            );
            let next_gen = cur_gen + 1;
            timed_db("coord.regen_all_zero", &db, |conn| {
                update_evolution(conn, evo_id, "running", next_gen as i64);
                for island_id in 0..num_islands {
                    for _ in 0..per_island_pop {
                        let genome = GenomeGraph::random(&mut rng);
                        let bytes = bincode::serialize(&genome).unwrap();
                        let gid = insert_genotype(
                            conn, evo_id, next_gen as i64, &bytes, None, island_id as i64,
                        );
                        create_task(conn, evo_id, gid);
                    }
                }
            });
            cur_gen += 1;
            continue;
        }

        // Prepare next generation.
        let next_gen = cur_gen + 1;

        // ── Migration: ring topology, every `migration_interval` gens ───
        let migration_active = num_islands > 1
            && params.migration_interval > 0
            && next_gen > 0
            && next_gen % params.migration_interval == 0;
        let best_of: Vec<Option<(GenomeGraph, f64)>> = if migration_active {
            island_individuals
                .iter()
                .map(|pop| pop.first().cloned())
                .collect()
        } else {
            vec![None; num_islands]
        };
        if migration_active {
            log::info!("Evolution {evo_id} Gen {next_gen}: migrating elites along ring");
        }

        // ── All-islands reproduction in a single transaction ───────────
        //
        // Previously each insert was its own autocommit — ~400 separate
        // writer-lock acquisitions per generation, each fighting workers'
        // complete_task UPDATEs. Workers that lost the fight waited in
        // busy_timeout, holding their pool connections, starving API reads.
        //
        // A single transaction takes the writer lock ONCE, does all inserts,
        // commits. Workers see SQLITE_BUSY for ~10ms (the whole batch) not
        // ~400× 1ms (the old thundering-herd pattern). Net write time is
        // also dramatically lower because there's only one WAL fsync.
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
                let survivors: Vec<GenomeGraph> = individuals
                    [..num_survivors.min(individuals.len())]
                    .iter()
                    .map(|(g, _)| g.clone())
                    .collect();
                let survivor_fitnesses: Vec<f64> = individuals
                    [..num_survivors.min(individuals.len())]
                    .iter()
                    .map(|(_, f)| *f)
                    .collect();
                if survivors.is_empty() {
                    continue;
                }

                let mut offspring_count = 0usize;

                // Inbound migrant from previous island (ring topology).
                let migrant = if migration_active {
                    let src = (island_id + num_islands - 1) % num_islands;
                    best_of[src].clone()
                } else {
                    None
                };
                if let Some((genome, fitness)) = &migrant {
                    let bytes = bincode::serialize(genome).unwrap();
                    insert_genotype_with_fitness(
                        conn, evo_id, next_gen as i64, &bytes, *fitness, island_id as i64,
                    );
                    offspring_count += 1;
                }

                // Keep survivors with their existing fitness.
                for (genome, fitness) in survivors.iter().zip(survivor_fitnesses.iter()) {
                    if offspring_count >= per_island_pop { break; }
                    let bytes = bincode::serialize(genome).unwrap();
                    insert_genotype_with_fitness(
                        conn, evo_id, next_gen as i64, &bytes, *fitness, island_id as i64,
                    );
                    offspring_count += 1;
                }

                // Random injection every INJECTION_INTERVAL gens.
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
                    create_task(conn, evo_id, gid);
                    offspring_count += 1;
                }

                // Fill remaining slots with offspring.
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
                    create_task(conn, evo_id, gid);
                    offspring_count += 1;
                }
            }

            conn.execute_batch("COMMIT").expect("commit txn");
        });

        cur_gen += 1;
    }

    timed_db("coord.init", &db, |c| {
        update_evolution(c, evo_id, "completed", -1)
    });
    log::info!("Evolution {evo_id} completed");
}

const TOURNAMENT_SIZE: usize = 3;

/// Tournament selection: pick TOURNAMENT_SIZE random indices in 0..n,
/// return the one with the highest fitness. Unlike roulette-wheel, this
/// does NOT over-amplify a single high-fitness elite: every survivor has
/// a non-trivial chance of being selected regardless of the elite gap.
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

/// Tournament parent selection over parallel (parents, fitnesses) slices.
fn pick_weighted<'a, R: Rng>(
    parents: &'a [GenomeGraph],
    fitnesses: &[f64],
    rng: &mut R,
) -> &'a GenomeGraph {
    &parents[tournament_pick(fitnesses, rng)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    #[test]
    fn tournament_pick_favors_higher_fitness() {
        // 10 candidates with strictly ascending fitness. Over many draws,
        // the last (highest) should win far more often than 1/n, and the
        // first (lowest) should win far less. With TOURNAMENT_SIZE=3, the
        // top candidate wins with probability ≈ 1 - ((n-1)/n)^3 per draw,
        // which for n=10 is ~27% — ~2.7× uniform expectation of 10%.
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
        // Top wins ~27%, bottom wins ~0.1% (needs all 3 picks to be index 0).
        assert!(top_wins > 2000, "top wins {top_wins}, expected >2000");
        assert!(bottom_wins < 200, "bottom wins {bottom_wins}, expected <200");
    }

    #[test]
    fn tournament_pick_uniform_when_all_equal_fitness() {
        // When fitnesses are all equal, tournament reduces to uniform
        // random selection (since no candidate wins the > comparison).
        let fitnesses: Vec<f64> = vec![5.0; 10];
        let mut rng = ChaCha8Rng::seed_from_u64(2);
        let mut counts = vec![0usize; 10];
        for _ in 0..10_000 {
            counts[tournament_pick(&fitnesses, &mut rng)] += 1;
        }
        // Each bucket should land within ±30% of uniform (1000).
        for (i, &c) in counts.iter().enumerate() {
            assert!(c > 700 && c < 1300, "bucket {i}: {c} (expected ~1000)");
        }
    }

    #[test]
    fn tournament_pick_single_candidate() {
        // Degenerate: 1 candidate is always picked.
        let fitnesses = vec![42.0];
        let mut rng = ChaCha8Rng::seed_from_u64(3);
        for _ in 0..100 {
            assert_eq!(tournament_pick(&fitnesses, &mut rng), 0);
        }
    }
}
