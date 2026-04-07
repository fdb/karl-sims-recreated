use crossbeam_channel::Receiver;

use karl_sims_core::fitness::{evaluate_fitness, EvolutionParams};
use karl_sims_core::genotype::GenomeGraph;

use crate::engine::{EvalResult, EvalTask};

/// Run a worker on a dedicated OS thread (not tokio — this is CPU-bound).
///
/// Workers receive tasks from a crossbeam channel (no DB access) and send
/// results back via the per-generation result channel embedded in each task.
/// This eliminates all DB contention on the hot path: no claim_task polls,
/// no complete_task writes, no connection pool pressure.
pub fn spawn_worker(task_rx: Receiver<EvalTask>, worker_id: String) {
    std::thread::spawn(move || {
        run_worker_loop(task_rx, worker_id);
    });
}

fn run_worker_loop(task_rx: Receiver<EvalTask>, worker_id: String) {
    loop {
        // Block until a task is available.  crossbeam's MPMC semantics
        // ensure each task is consumed by exactly one worker.  When the
        // channel is empty, the thread sleeps (zero CPU) — no busy-polling.
        let task = match task_rx.recv() {
            Ok(t) => t,
            Err(_) => {
                // Channel closed — server is shutting down.
                log::info!("Worker {worker_id}: task channel closed, exiting");
                return;
            }
        };

        let fitness = match bincode::deserialize::<GenomeGraph>(&task.genome_bytes) {
            Ok(genome) => {
                let params: EvolutionParams =
                    serde_json::from_str(&task.config_json).unwrap_or_default();
                // Catch panics so a single bad genome doesn't kill the worker.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    evaluate_fitness(&genome, &params)
                }));
                match result {
                    Ok(r) => r.score,
                    Err(e) => {
                        let msg = if let Some(s) = e.downcast_ref::<&str>() {
                            s.to_string()
                        } else if let Some(s) = e.downcast_ref::<String>() {
                            s.clone()
                        } else {
                            "unknown panic".to_string()
                        };
                        log::error!("Worker {worker_id}: panic during fitness eval: {msg}");
                        0.0
                    }
                }
            }
            Err(e) => {
                log::error!("Worker {worker_id}: bincode deserialize failed: {e}");
                0.0
            }
        };

        // Send result back to the coordinator.  If the receiver is dropped
        // (coordinator gave up on this generation), silently discard.
        task.result_tx.send(EvalResult { fitness }).ok();
    }
}
