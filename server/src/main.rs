mod api;
mod coordinator;
mod db;
mod worker;
mod ws;

use api::AppState;
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

    // Broadcast channel for live WebSocket updates.
    let (tx, _) = tokio::sync::broadcast::channel::<String>(100);

    let state = AppState {
        db: db.clone(),
        tx: tx.clone(),
    };

    // Build axum app: REST API + WebSocket + static file serving.
    let cors = tower_http::cors::CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods(tower_http::cors::Any)
        .allow_headers(tower_http::cors::Any);

    let app = api::routes()
        .route("/api/live", ws::ws_route())
        .with_state(state)
        .layer(cors)
        .fallback_service(tower_http::services::ServeDir::new("frontend/dist"));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .unwrap();
    log::info!("HTTP server on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
