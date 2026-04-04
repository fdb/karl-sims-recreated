use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use karl_sims_core::fitness::{Environment, EvolutionParams, FitnessGoal};

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
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Deserialize)]
struct CreateEvolutionRequest {
    population_size: Option<usize>,
    generations: Option<usize>,
    goal: Option<String>,
    environment: Option<String>,
    sim_duration: Option<f64>,
    max_parts: Option<usize>,
    gravity: Option<f64>,
    water_viscosity: Option<f64>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct PatchEvolutionRequest {
    name: Option<String>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/evolutions",
            axum::routing::get(list_evolutions).post(create_evolution),
        )
        .route(
            "/api/evolutions/{id}",
            axum::routing::get(get_evolution)
                .patch(patch_evolution)
                .delete(delete_evolution_handler),
        )
        .route("/api/evolutions/{id}/best", axum::routing::get(get_best))
        .route(
            "/api/evolutions/{id}/stop",
            axum::routing::post(stop_evolution),
        )
        .route(
            "/api/evolutions/{id}/pause",
            axum::routing::post(pause_evolution),
        )
        .route(
            "/api/evolutions/{id}/resume",
            axum::routing::post(resume_evolution),
        )
        .route(
            "/api/evolutions/{id}/stats",
            axum::routing::get(get_evolution_stats),
        )
        .route("/api/genotypes/{id}", axum::routing::get(get_genotype_info))
        .route(
            "/api/genotypes/{id}/genome",
            axum::routing::get(get_genome_bytes),
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
                name: e.name,
            })
            .collect(),
    )
}

async fn create_evolution(
    State(state): State<AppState>,
    Json(req): Json<CreateEvolutionRequest>,
) -> Json<serde_json::Value> {
    let goal = match req.goal.as_deref() {
        Some("light_following") => FitnessGoal::LightFollowing,
        _ => FitnessGoal::SwimmingSpeed,
    };
    let env = match req.environment.as_deref() {
        Some("land") => Environment::Land,
        _ => Environment::Water,
    };
    let params = EvolutionParams {
        population_size: req.population_size.unwrap_or(50).clamp(5, 1000),
        max_generations: req.generations.unwrap_or(100).clamp(1, 10000),
        goal,
        environment: env,
        sim_duration: req.sim_duration.unwrap_or(10.0).clamp(1.0, 60.0),
        max_parts: req.max_parts.unwrap_or(20).clamp(2, 50),
        gravity: req.gravity.unwrap_or(9.81).clamp(0.0, 30.0),
        water_viscosity: req.water_viscosity.unwrap_or(2.0).clamp(0.1, 10.0),
    };
    let config_json = serde_json::to_string(&params).unwrap();
    let evo_id = {
        let conn = state.db.lock().unwrap();
        db::create_evolution(&conn, &config_json, req.name.as_deref())
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
    match db::get_evolution_full(&conn, id) {
        Some((status, generation, config_json, name)) => {
            let config = serde_json::from_str::<serde_json::Value>(&config_json)
                .unwrap_or_default();
            Json(serde_json::json!({
                "id": id,
                "status": status,
                "generation": generation,
                "config": config,
                "name": name,
            }))
        }
        None => Json(serde_json::json!({"error": "not found"})),
    }
}

async fn patch_evolution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<PatchEvolutionRequest>,
) -> Json<serde_json::Value> {
    let conn = state.db.lock().unwrap();
    // Trim whitespace; treat empty string as None (remove name)
    let name = req.name.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    db::set_evolution_name(&conn, id, name.as_deref());
    Json(serde_json::json!({"id": id, "name": name}))
}

async fn delete_evolution_handler(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let conn = state.db.lock().unwrap();
    // Stop first so the coordinator task exits on its next status check.
    db::stop_evolution(&conn, id);
    db::delete_evolution(&conn, id);
    axum::http::StatusCode::NO_CONTENT
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

async fn pause_evolution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let conn = state.db.lock().unwrap();
    db::pause_evolution(&conn, id);
    Json(serde_json::json!({"status": "paused"}))
}

async fn resume_evolution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let conn = state.db.lock().unwrap();
    db::resume_evolution(&conn, id);
    Json(serde_json::json!({"status": "running"}))
}

async fn get_evolution_stats(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<Vec<serde_json::Value>> {
    let conn = state.db.lock().unwrap();
    let stats = db::get_generation_stats(&conn, id);
    Json(
        stats
            .into_iter()
            .map(|(generation, best, avg)| {
                serde_json::json!({
                    "generation": generation,
                    "best_fitness": best,
                    "avg_fitness": avg,
                })
            })
            .collect(),
    )
}

async fn get_genome_bytes(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let conn = state.db.lock().unwrap();
    match db::get_genotype(&conn, id) {
        Some(bytes) => (
            [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
            bytes,
        )
            .into_response(),
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

async fn get_genotype_info(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let conn = state.db.lock().unwrap();
    match db::get_genotype(&conn, id) {
        Some(bytes) => {
            match bincode::deserialize::<karl_sims_core::genotype::GenomeGraph>(&bytes) {
                Ok(genome) => {
                    let info = serde_json::json!({
                        "id": id,
                        "num_nodes": genome.nodes.len(),
                        "num_connections": genome.connections.len(),
                        "nodes": genome.nodes.iter().enumerate().map(|(i, n)| {
                            serde_json::json!({
                                "id": i,
                                "dimensions": [n.dimensions.x, n.dimensions.y, n.dimensions.z],
                                "joint_type": format!("{:?}", n.joint_type),
                                "recursive_limit": n.recursive_limit,
                                "terminal_only": n.terminal_only,
                                "brain": {
                                    "num_neurons": n.brain.neurons.len(),
                                    "num_effectors": n.brain.effectors.len(),
                                    "neurons": n.brain.neurons.iter().enumerate().map(|(j, neuron)| {
                                        serde_json::json!({
                                            "id": j,
                                            "func": format!("{:?}", neuron.func),
                                            "inputs": neuron.inputs.iter().map(|(inp, w)| {
                                                serde_json::json!({
                                                    "source": format!("{:?}", inp),
                                                    "weight": w,
                                                })
                                            }).collect::<Vec<_>>(),
                                        })
                                    }).collect::<Vec<_>>(),
                                }
                            })
                        }).collect::<Vec<_>>(),
                        "connections": genome.connections.iter().map(|c| {
                            serde_json::json!({
                                "source": c.source,
                                "target": c.target,
                                "parent_face": format!("{:?}", c.parent_face),
                                "child_face": format!("{:?}", c.child_face),
                                "scale": c.scale,
                                "reflection": c.reflection,
                            })
                        }).collect::<Vec<_>>(),
                    });
                    Json(info).into_response()
                }
                Err(_) => axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response(),
            }
        }
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}
