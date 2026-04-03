use axum::extract::{Path, State};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::coordinator;
use crate::db::{self, DbPool};
use crate::ws::UpdateSender;

#[derive(Clone)]
pub struct AppState {
    pub db: DbPool,
    pub tx: UpdateSender,
}

#[derive(Serialize)]
struct EvolutionInfo {
    id: i64,
    status: String,
    generation: i64,
    config: String,
    created_at: String,
}

#[derive(Deserialize)]
struct CreateEvolutionRequest {
    population_size: Option<usize>,
    generations: Option<usize>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/evolutions",
            axum::routing::get(list_evolutions).post(create_evolution),
        )
        .route("/api/evolutions/{id}", axum::routing::get(get_evolution))
        .route("/api/evolutions/{id}/best", axum::routing::get(get_best))
        .route(
            "/api/evolutions/{id}/stop",
            axum::routing::post(stop_evolution),
        )
}

async fn list_evolutions(State(state): State<AppState>) -> Json<Vec<EvolutionInfo>> {
    let conn = state.db.lock().unwrap();
    let evos = db::list_evolutions(&conn);
    Json(
        evos.into_iter()
            .map(|e| EvolutionInfo {
                id: e.id,
                status: e.status,
                generation: e.current_gen,
                config: e.config_json,
                created_at: e.created_at,
            })
            .collect(),
    )
}

async fn create_evolution(
    State(state): State<AppState>,
    Json(req): Json<CreateEvolutionRequest>,
) -> Json<serde_json::Value> {
    let config = serde_json::json!({
        "population_size": req.population_size.unwrap_or(50),
        "generations": req.generations.unwrap_or(50),
    });
    let evo_id = {
        let conn = state.db.lock().unwrap();
        db::create_evolution(&conn, &config.to_string())
    };

    // Spawn coordinator for this evolution.
    let db_c = state.db.clone();
    let tx = state.tx.clone();
    tokio::spawn(async move {
        coordinator::run_evolution(db_c, evo_id, Some(tx)).await;
    });

    Json(serde_json::json!({"id": evo_id, "status": "running"}))
}

async fn get_evolution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let conn = state.db.lock().unwrap();
    match db::get_evolution_status(&conn, id) {
        Some((status, generation)) => {
            Json(serde_json::json!({"id": id, "status": status, "generation": generation}))
        }
        None => Json(serde_json::json!({"error": "not found"})),
    }
}

async fn get_best(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<Vec<serde_json::Value>> {
    let conn = state.db.lock().unwrap();
    let best = db::get_best_genotypes(&conn, id, 10);
    Json(
        best.into_iter()
            .map(|(gid, fitness, _bytes)| serde_json::json!({"id": gid, "fitness": fitness}))
            .collect(),
    )
}

async fn stop_evolution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let conn = state.db.lock().unwrap();
    db::stop_evolution(&conn, id);
    Json(serde_json::json!({"status": "stopped"}))
}
