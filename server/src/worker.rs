use karl_sims_core::fitness::{evaluate_swimming_fitness, FitnessConfig};
use karl_sims_core::genotype::GenomeGraph;

use crate::db::{claim_task, complete_task, DbPool};

pub async fn run_worker(db: DbPool, worker_id: String) {
    loop {
        let task = {
            let conn = db.lock().unwrap();
            claim_task(&conn, &worker_id)
        };

        match task {
            Some((task_id, genome_bytes)) => {
                // Deserialize and evaluate fitness (CPU-bound, runs on the tokio thread).
                match bincode::deserialize::<GenomeGraph>(&genome_bytes) {
                    Ok(genome) => {
                        let result =
                            evaluate_swimming_fitness(&genome, &FitnessConfig::default());
                        let conn = db.lock().unwrap();
                        complete_task(&conn, task_id, result.score);
                    }
                    Err(e) => {
                        log::error!(
                            "Failed to deserialize genome for task {task_id}: {e}"
                        );
                        let conn = db.lock().unwrap();
                        complete_task(&conn, task_id, 0.0);
                    }
                }
            }
            None => {
                // No work available — back off briefly.
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
    }
}
