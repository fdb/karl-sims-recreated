mod api;
mod coordinator;
mod db;
mod engine;
mod timing;
mod worker;
mod ws;

use std::sync::Arc;

use api::AppState;
use db::{init_db, init_read_pool};
use engine::Engine;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let db_path = std::env::var("PARK_DB").unwrap_or_else(|_| "park.db".to_string());
    let db_path = db_path.as_str();
    let db = init_db(db_path);
    let read_db = init_read_pool(db_path);
    log::info!("Database initialized");

    // Profiling + WAL checkpoint thread.
    timing::spawn_reporter();
    timing::spawn_wal_monitor(db_path.to_string());

    // In-memory engine — all hot state lives here.
    let engine = Arc::new(Engine::new());

    // Spawn workers on dedicated OS threads. They receive tasks from the
    // engine's crossbeam channel — no DB access on the hot path.
    let num_workers = num_cpus::get().max(1);
    log::info!("Starting {num_workers} channel-based workers");
    for i in 0..num_workers {
        worker::spawn_worker(engine.task_rx.clone(), format!("worker-{i}"));
    }

    // Broadcast channel for live WebSocket updates.
    let (tx, _) = tokio::sync::broadcast::channel::<String>(100);

    let state = AppState {
        db: db.clone(),
        read_db: read_db.clone(),
        engine: engine.clone(),
        tx: tx.clone(),
    };

    // Load existing evolutions into engine snapshots and resume running ones.
    {
        let conn = db.get().expect("pool get");
        let evos = db::list_evolutions(&conn);
        for evo in &evos {
            // Load a basic snapshot for stopped/completed/paused evolutions
            // so the list endpoint shows them.
            let config_json = &evo.config_json;
            let snap = engine::EvolutionSnapshot {
                id: evo.id,
                name: evo.name.clone(),
                status: evo.status.clone(),
                current_gen: evo.current_gen,
                config_json: config_json.clone(),
                created_at: evo.created_at.clone(),
                best_creatures: Vec::new(), // will be populated on resume or lazy
                best_per_island: Vec::new(),
                gen_stats: Vec::new(),
                island_stats: Vec::new(),
            };
            engine.update_snapshot(snap);

            if evo.status == "running" {
                let engine_c = engine.clone();
                let db_c = db.clone();
                let tx_c = tx.clone();
                let evo_id = evo.id;
                log::info!("Resuming evolution {evo_id} (gen {})", evo.current_gen);
                tokio::spawn(async move {
                    coordinator::run_evolution(engine_c, db_c, evo_id, Some(tx_c)).await;
                });
            }
        }
    }

    // Build axum app.
    let cors = tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    let app = api::routes()
        .route("/api/live", ws::ws_route())
        .with_state(state)
        .layer(cors)
        .fallback_service(
            tower_http::services::ServeDir::new("frontend/dist")
                .fallback(tower_http::services::ServeFile::new(
                    "frontend/dist/index.html",
                )),
        );

    let listener = tokio::net::TcpListener::bind("[::]:3000")
        .await
        .unwrap();
    log::info!("HTTP server on http://localhost:3000 (dual-stack IPv4+IPv6)");
    axum::serve(listener, app).await.unwrap();
}
