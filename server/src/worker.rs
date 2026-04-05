use std::time::Duration;

use karl_sims_core::fitness::{evaluate_fitness, EvolutionParams};
use karl_sims_core::genotype::GenomeGraph;

use crate::db::{claim_task, complete_task, DbPool};

/// Run a worker on a dedicated OS thread (not tokio async — this is CPU-bound).
pub fn spawn_worker(db: DbPool, worker_id: String) {
    std::thread::spawn(move || {
        run_worker_loop(db, worker_id);
    });
}

fn run_worker_loop(db: DbPool, worker_id: String) {
    loop {
        // Each `db.get()` returns a PooledConnection that derefs to &Connection.
        // No global lock — other workers, API handlers, and the coordinator
        // each hold their own connection, and SQLite's WAL serializes the
        // actual writes internally.
        let task = {
            let conn = db.get().expect("pool get (claim)");
            claim_task(&conn, &worker_id)
        };

        match task {
            Some((task_id, genome_bytes, config_json)) => {
                match bincode::deserialize::<GenomeGraph>(&genome_bytes) {
                    Ok(genome) => {
                        let params: EvolutionParams =
                            serde_json::from_str(&config_json).unwrap_or_default();
                        // Fitness evaluation is the CPU-bound work — intentionally
                        // run with NO connection held, so we don't starve the pool.
                        let result = evaluate_fitness(&genome, &params);
                        let conn = db.get().expect("pool get (complete)");
                        complete_task(&conn, task_id, result.score);
                    }
                    Err(e) => {
                        log::error!("Worker {worker_id}: failed to deserialize genome for task {task_id}: {e}");
                        let conn = db.get().expect("pool get (complete-err)");
                        complete_task(&conn, task_id, 0.0);
                    }
                }
            }
            None => {
                // No work — sleep briefly before polling again
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}
