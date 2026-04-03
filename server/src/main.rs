mod coordinator;
mod db;
mod worker;

use db::init_db;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let db = init_db("karl-sims.db");
    log::info!("Database initialized");

    let num_workers = num_cpus::get().max(1);
    log::info!("Starting {num_workers} workers");

    // Spawn worker tasks.
    for i in 0..num_workers {
        let db = db.clone();
        tokio::spawn(async move {
            worker::run_worker(db, format!("worker-{i}")).await;
        });
    }

    // Create a test evolution and run it.
    let evo_id = {
        let conn = db.lock().unwrap();
        db::create_evolution(&conn, "{\"population_size\": 50}")
    };

    // Spawn coordinator.
    let db_coord = db.clone();
    tokio::spawn(async move {
        coordinator::run_evolution(db_coord, evo_id).await;
    });

    // HTTP server placeholder (implemented in Task 3).
    log::info!("Server running. Evolution {} started.", evo_id);

    // Keep running until Ctrl-C.
    tokio::signal::ctrl_c().await.ok();
    log::info!("Shutting down");
}
