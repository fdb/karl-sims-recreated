use rand::Rng;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use karl_sims_core::evolution::EvolutionConfig;
use karl_sims_core::fitness::{EvolutionParams, FitnessConfig};
use karl_sims_core::genotype::GenomeGraph;
use karl_sims_core::evolution::Population;

use tokio::sync::broadcast;

use crate::db::{
    create_task, get_evolution_status, get_evolution_full, get_genotype, get_generation_fitnesses,
    insert_genotype, pending_task_count, update_evolution, DbPool,
};

/// Run a full evolution loop, persisting every generation to the database.
pub async fn run_evolution(db: DbPool, evo_id: i64, tx: Option<broadcast::Sender<String>>) {
    let mut rng = ChaCha8Rng::seed_from_u64(evo_id as u64);

    // Read params from DB.
    let params: EvolutionParams = {
        let conn = db.lock().unwrap();
        get_evolution_full(&conn, evo_id)
            .and_then(|(_, _, config_json)| serde_json::from_str(&config_json).ok())
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

    // Generation 0: create initial random population and enqueue tasks.
    let pop = Population::random_initial(config.clone(), &mut rng);
    {
        let conn = db.lock().unwrap();
        for ind in &pop.individuals {
            let bytes = bincode::serialize(&ind.genome).unwrap();
            let gid = insert_genotype(&conn, evo_id, 0, &bytes, None);
            create_task(&conn, evo_id, gid);
        }
    }

    let max_generations = params.max_generations;
    for cur_gen in 0..max_generations {
        // Check if the evolution has been stopped or paused.
        loop {
            let status = {
                let conn = db.lock().unwrap();
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
                let conn = db.lock().unwrap();
                pending_task_count(&conn, evo_id)
            };
            if pending == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        // Read fitness results for the current generation.
        let fitnesses = {
            let conn = db.lock().unwrap();
            get_generation_fitnesses(&conn, evo_id, cur_gen as i64)
        };

        // Reconstruct (genome, fitness) pairs from the database.
        let mut individuals: Vec<(GenomeGraph, f64)> = Vec::new();
        {
            let conn = db.lock().unwrap();
            for (gid, fitness) in &fitnesses {
                if let Some(bytes) = get_genotype(&conn, *gid) {
                    if let Ok(genome) = bincode::deserialize(&bytes) {
                        individuals.push((genome, *fitness));
                    }
                }
            }
        }

        // Sort by fitness descending.
        individuals.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        let best = individuals.first().map(|(_, f)| *f).unwrap_or(0.0);
        let avg = individuals.iter().map(|(_, f)| f).sum::<f64>()
            / individuals.len().max(1) as f64;
        log::info!(
            "Evolution {evo_id} Gen {cur_gen}: best={best:.4}, avg={avg:.4}, pop={}",
            individuals.len()
        );

        // Broadcast generation stats to WebSocket clients.
        if let Some(ref tx) = tx {
            let msg = serde_json::json!({
                "type": "generation",
                "evolution_id": evo_id,
                "generation": cur_gen,
                "best_fitness": best,
                "avg_fitness": avg,
            });
            tx.send(msg.to_string()).ok();
        }

        // Handle the all-zero-fitness case: regenerate a random population.
        if individuals.is_empty() || best <= 0.0 {
            log::warn!(
                "Evolution {evo_id} Gen {cur_gen}: all zero fitness, regenerating random population"
            );
            let fresh = Population::random_initial(config.clone(), &mut rng);
            let next_gen = cur_gen + 1;
            {
                let conn = db.lock().unwrap();
                update_evolution(&conn, evo_id, "running", next_gen as i64);
                for ind in &fresh.individuals {
                    let bytes = bincode::serialize(&ind.genome).unwrap();
                    let gid = insert_genotype(&conn, evo_id, next_gen as i64, &bytes, None);
                    create_task(&conn, evo_id, gid);
                }
            }
            continue;
        }

        // Select survivors (top fraction by fitness).
        let num_survivors =
            (config.survival_ratio * individuals.len() as f64).ceil() as usize;
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
            break;
        }

        // Prepare next generation.
        let next_gen = cur_gen + 1;
        let mut offspring_count = 0;
        {
            let conn = db.lock().unwrap();
            update_evolution(&conn, evo_id, "running", next_gen as i64);
        }

        // Keep survivors (they get re-evaluated).
        for genome in &survivors {
            let bytes = bincode::serialize(genome).unwrap();
            let conn = db.lock().unwrap();
            let gid = insert_genotype(&conn, evo_id, next_gen as i64, &bytes, None);
            create_task(&conn, evo_id, gid);
            offspring_count += 1;
        }

        // Generate new offspring to fill the population.
        while offspring_count < config.population_size {
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
            let conn = db.lock().unwrap();
            let gid = insert_genotype(&conn, evo_id, next_gen as i64, &bytes, None);
            create_task(&conn, evo_id, gid);
            offspring_count += 1;
        }
    }

    {
        let conn = db.lock().unwrap();
        update_evolution(&conn, evo_id, "completed", -1);
    }
    log::info!("Evolution {evo_id} completed");
}

/// Fitness-proportional (roulette-wheel) parent selection.
fn pick_weighted<'a, R: Rng>(
    parents: &'a [GenomeGraph],
    fitnesses: &[f64],
    rng: &mut R,
) -> &'a GenomeGraph {
    let total: f64 = fitnesses.iter().map(|f| f.max(0.001)).sum();
    let mut pick = rng.r#gen::<f64>() * total;
    for (i, &f) in fitnesses.iter().enumerate() {
        pick -= f.max(0.001);
        if pick <= 0.0 {
            return &parents[i];
        }
    }
    &parents[parents.len() - 1]
}
