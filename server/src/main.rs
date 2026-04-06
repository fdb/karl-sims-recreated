mod api;
mod coordinator;
mod db;
mod timing;
mod worker;
mod ws;

use api::AppState;
use db::init_db;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let db_path = std::env::var("PARK_DB").unwrap_or_else(|_| "park.db".to_string());
    let db_path = db_path.as_str();
    let db = init_db(db_path);
    log::info!("Database initialized");

    // Profiling: periodic p50/p99 table + WAL-size monitor. Low overhead
    // (see `timing.rs`), runs for the lifetime of the process.
    timing::spawn_reporter();
    timing::spawn_wal_monitor(db_path.to_string());

    let num_workers = num_cpus::get().max(1);
    log::info!("Starting {num_workers} workers");

    // Spawn workers on dedicated OS threads (CPU-bound fitness evaluation).
    for i in 0..num_workers {
        worker::spawn_worker(db.clone(), format!("worker-{i}"));
    }

    // Broadcast channel for live WebSocket updates.
    let (tx, _) = tokio::sync::broadcast::channel::<String>(100);

    let state = AppState {
        db: db.clone(),
        tx: tx.clone(),
    };

    // Resume any evolutions that were running when the server last stopped.
    {
        let conn = db.get().expect("pool get");
        let evos = db::list_evolutions(&conn);
        for evo in &evos {
            if evo.status == "running" {
                let db_c = db.clone();
                let tx_c = tx.clone();
                let evo_id = evo.id;
                log::info!("Resuming evolution {evo_id} (gen {})", evo.current_gen);
                tokio::spawn(async move {
                    coordinator::run_evolution(db_c, evo_id, Some(tx_c)).await;
                });
            }
            // Paused evolutions stay paused — not resumed automatically
        }
    }

    // Build axum app: REST API + WebSocket + static file serving.
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

    // Bind to [::] (IPv6 wildcard) for dual-stack: accepts both IPv4 and
    // IPv6 clients. 0.0.0.0 is IPv4-only, which breaks browsers that
    // prefer IPv6 resolution of "localhost" (::1) — common on macOS.
    let listener = tokio::net::TcpListener::bind("[::]:3000")
        .await
        .unwrap();
    log::info!("HTTP server on http://localhost:3000 (dual-stack IPv4+IPv6)");
    axum::serve(listener, app).await.unwrap();
}
