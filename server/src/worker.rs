use std::time::Duration;

use karl_sims_core::fitness::{evaluate_fitness, EvolutionParams};
use karl_sims_core::genotype::GenomeGraph;

use crate::db::{claim_task, complete_task, DbPool};
use crate::timing::timed_db;

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
        // `claim_task` is an `UPDATE ... RETURNING` — it takes the writer
        // lock. Its query-latency p99 is our best direct signal of writer
        // contention across the whole system.
        let task = timed_db("worker.claim_task", &db, |c| claim_task(c, &worker_id));

        match task {
            Some((task_id, genome_bytes, config_json)) => {
                match bincode::deserialize::<GenomeGraph>(&genome_bytes) {
                    Ok(genome) => {
                        let params: EvolutionParams =
                            serde_json::from_str(&config_json).unwrap_or_default();
                        // Fitness evaluation is the CPU-bound work — intentionally
                        // run with NO connection held, so we don't starve the pool.
                        // Catch panics so a single bad genome doesn't kill the
                        // entire worker thread (and with it, the park).
                        let result = std::panic::catch_unwind(
                            std::panic::AssertUnwindSafe(|| evaluate_fitness(&genome, &params)),
                        );
                        let score = match result {
                            Ok(r) => r.score,
                            Err(e) => {
                                let msg = if let Some(s) = e.downcast_ref::<&str>() {
                                    s.to_string()
                                } else if let Some(s) = e.downcast_ref::<String>() {
                                    s.clone()
                                } else {
                                    "unknown panic".to_string()
                                };
                                log::error!(
                                    "Worker {worker_id}: panic during fitness eval for task {task_id}: {msg}"
                                );
                                0.0
                            }
                        };
                        timed_db("worker.complete_task", &db, |c| {
                            complete_task(c, task_id, score)
                        });
                    }
                    Err(e) => {
                        log::error!("Worker {worker_id}: failed to deserialize genome for task {task_id}: {e}");
                        timed_db("worker.complete_task", &db, |c| {
                            complete_task(c, task_id, 0.0)
                        });
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
