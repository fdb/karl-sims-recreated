use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use karl_sims_core::evolution::EvolutionConfig;
use karl_sims_core::fitness::{EvolutionParams, FitnessConfig};
use karl_sims_core::genotype::GenomeGraph;

use tokio::sync::broadcast;

use crate::db::{
    create_task, get_evolution_status, get_evolution_full, get_genotype,
    get_generation_fitnesses, get_generation_fitnesses_by_island, insert_genotype,
    insert_genotype_with_fitness, pending_task_count, update_evolution, DbPool,
};

/// Run a full evolution loop, persisting every generation to the database.
pub async fn run_evolution(db: DbPool, evo_id: i64, tx: Option<broadcast::Sender<String>>) {
    let mut rng = ChaCha8Rng::seed_from_u64(evo_id as u64);

    // Read params from DB.
    let params: EvolutionParams = {
        let conn = db.get().expect("pool get");
        get_evolution_full(&conn, evo_id)
            .and_then(|(_, _, config_json, _)| serde_json::from_str(&config_json).ok())
            .unwrap_or_default()
    };

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
    let start_gen = {
        let conn = db.get().expect("pool get");
        get_evolution_status(&conn, evo_id)
            .map(|(_, current_gen)| current_gen)
            .unwrap_or(0) as usize
    };

    // Only create generation 0 if the evolution is brand new (no genotypes yet).
    let has_genotypes = {
        let conn = db.get().expect("pool get");
        !get_generation_fitnesses(&conn, evo_id, 0).is_empty()
            || pending_task_count(&conn, evo_id) > 0
    };

    // Split population across islands. `num_islands=1` preserves single-pool behavior.
    let num_islands = params.num_islands.max(1);
    let per_island_pop = (params.population_size / num_islands).max(1);

    if !has_genotypes {
        log::info!(
            "Evolution {evo_id}: creating initial population ({} islands × {} creatures = {} total)",
            num_islands, per_island_pop, num_islands * per_island_pop,
        );
        let conn = db.get().expect("pool get");
        for island_id in 0..num_islands {
            for _ in 0..per_island_pop {
                let genome = GenomeGraph::random(&mut rng);
                let bytes = bincode::serialize(&genome).unwrap();
                let gid = insert_genotype(&conn, evo_id, 0, &bytes, None, island_id as i64);
                create_task(&conn, evo_id, gid);
            }
        }
    } else {
        log::info!("Evolution {evo_id}: resuming from generation {start_gen}");
        // Advance the RNG to match where we left off (approximate)
        for _ in 0..start_gen * params.population_size {
            let _: f64 = rng.r#gen();
        }
    }

    let max_generations = params.max_generations;
    for cur_gen in start_gen..max_generations {
        // Check if the evolution has been stopped or paused.
        loop {
            let status = {
                let conn = db.get().expect("pool get");
                get_evolution_status(&conn, evo_id).map(|(s, _)| s)
            };
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
            let pending = {
                let conn = db.get().expect("pool get");
                pending_task_count(&conn, evo_id)
            };
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
            let conn = db.get().expect("pool get");
            let pairs = get_generation_fitnesses_by_island(
                &conn, evo_id, island_id as i64, cur_gen as i64,
            );
            for (gid, fitness) in &pairs {
                if let Some(bytes) = get_genotype(&conn, *gid) {
                    if let Ok(genome) = bincode::deserialize(&bytes) {
                        bucket.push((genome, *fitness));
                    }
                }
            }
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
            let conn = db.get().expect("pool get");
            update_evolution(&conn, evo_id, "running", next_gen as i64);
            for island_id in 0..num_islands {
                for _ in 0..per_island_pop {
                    let genome = GenomeGraph::random(&mut rng);
                    let bytes = bincode::serialize(&genome).unwrap();
                    let gid = insert_genotype(
                        &conn, evo_id, next_gen as i64, &bytes, None, island_id as i64,
                    );
                    create_task(&conn, evo_id, gid);
                }
            }
            continue;
        }

        // Prepare next generation.
        let next_gen = cur_gen + 1;
        {
            let conn = db.get().expect("pool get");
            update_evolution(&conn, evo_id, "running", next_gen as i64);
        }

        // ── Migration: ring topology, every `migration_interval` gens ───
        // best_of[i] = island i's elite (to be shipped to island (i+1)%N)
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

        // ── Per-island reproduction ─────────────────────────────────────
        for island_id in 0..num_islands {
            let individuals = &island_individuals[island_id];
            if individuals.is_empty() {
                continue;
            }

            // Select survivors (top survival_ratio of this island).
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
                let conn = db.get().expect("pool get");
                insert_genotype_with_fitness(
                    &conn, evo_id, next_gen as i64, &bytes, *fitness, island_id as i64,
                );
                offspring_count += 1;
            }

            // Keep survivors with their existing fitness (no re-evaluation needed).
            for (genome, fitness) in survivors.iter().zip(survivor_fitnesses.iter()) {
                if offspring_count >= per_island_pop { break; }
                let bytes = bincode::serialize(genome).unwrap();
                let conn = db.get().expect("pool get");
                insert_genotype_with_fitness(
                    &conn, evo_id, next_gen as i64, &bytes, *fitness, island_id as i64,
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
                let conn = db.get().expect("pool get");
                let gid = insert_genotype(
                    &conn, evo_id, next_gen as i64, &bytes, None, island_id as i64,
                );
                create_task(&conn, evo_id, gid);
                offspring_count += 1;
            }

            // Fill remaining slots with offspring.
            while offspring_count < per_island_pop {
                let roll: f64 = rng.r#gen();
                let child = if roll < config.asexual_ratio {
                    let parent = pick_weighted(&survivors, &survivor_fitnesses, &mut rng);
                    let mut child = parent.clone();
                    karl_sims_core::mutation::mutate(&mut child, &mut rng);
                    child
                } else if roll < config.asexual_ratio + config.crossover_ratio {
                    let p1 = pick_weighted(&survivors, &survivor_fitnesses, &mut rng);
                    let p2 = pick_weighted(&survivors, &survivor_fitnesses, &mut rng);
                    let mut child = karl_sims_core::mating::crossover(p1, p2, &mut rng);
                    karl_sims_core::mutation::mutate(&mut child, &mut rng);
                    child
                } else {
                    let p1 = pick_weighted(&survivors, &survivor_fitnesses, &mut rng);
                    let p2 = pick_weighted(&survivors, &survivor_fitnesses, &mut rng);
                    let mut child = karl_sims_core::mating::graft(p1, p2, &mut rng);
                    karl_sims_core::mutation::mutate(&mut child, &mut rng);
                    child
                };

                let bytes = bincode::serialize(&child).unwrap();
                let conn = db.get().expect("pool get");
                let gid = insert_genotype(
                    &conn, evo_id, next_gen as i64, &bytes, None, island_id as i64,
                );
                create_task(&conn, evo_id, gid);
                offspring_count += 1;
            }
        }
    }

    {
        let conn = db.get().expect("pool get");
        update_evolution(&conn, evo_id, "completed", -1);
    }
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
