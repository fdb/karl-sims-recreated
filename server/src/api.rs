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
    num_islands: Option<usize>,
    migration_interval: Option<usize>,
    /// Joint-motion stddev threshold (radians). `None` to disable.
    /// Default: 0.3. See `EvolutionParams::min_joint_motion`.
    min_joint_motion: Option<f64>,
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
            "/api/evolutions/{id}/best_per_island",
            axum::routing::get(get_best_per_island_handler),
        )
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
            "/api/evolutions/{id}/replay",
            axum::routing::post(replay_evolution),
        )
        .route(
            "/api/evolutions/{id}/stats",
            axum::routing::get(get_evolution_stats),
        )
        .route(
            "/api/evolutions/{id}/island_stats",
            axum::routing::get(get_island_stats_handler),
        )
        .route("/api/genotypes/{id}", axum::routing::get(get_genotype_info))
        .route(
            "/api/genotypes/{id}/genome",
            axum::routing::get(get_genome_bytes),
        )
        .route(
            "/api/genotypes/{id}/phenotype",
            axum::routing::get(get_phenotype_info),
        )
}

async fn list_evolutions(State(state): State<AppState>) -> Json<Vec<EvolutionInfo>> {
    // All DB calls in handlers go through `spawn_blocking`. They are
    // synchronous rusqlite calls, and we don't want them occupying a tokio
    // runtime worker (which would block unrelated HTTP traffic).
    let db = state.db.clone();
    let evos = tokio::task::spawn_blocking(move || {
        let conn = db.get().expect("pool get");
        db::list_evolutions(&conn)
    })
    .await
    .expect("spawn_blocking join");
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
        max_body_angular_velocity: Some(20.0),
        num_islands: req.num_islands.unwrap_or(1).clamp(1, 12),
        migration_interval: req.migration_interval.unwrap_or(20).clamp(0, 1000),
        // If the caller omits min_joint_motion we keep the default (Some(0.3)).
        // Callers can pass `null` (→ None) to disable, or a concrete number.
        min_joint_motion: req.min_joint_motion.or(Some(0.3)),
    };
    let config_json = serde_json::to_string(&params).unwrap();
    let evo_id = {
        let db = state.db.clone();
        let config_json = config_json.clone();
        let name = req.name.clone();
        tokio::task::spawn_blocking(move || {
            let conn = db.get().expect("pool get");
            // Pre-creation we don't know the row id, so we can't store a specific
            // seed yet. Pass None — get_evolution_seed will fall back to the id.
            db::create_evolution(&conn, &config_json, name.as_deref(), None)
        })
        .await
        .expect("spawn_blocking join")
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
    let db = state.db.clone();
    let full = tokio::task::spawn_blocking(move || {
        let conn = db.get().expect("pool get");
        db::get_evolution_full(&conn, id)
    })
    .await
    .expect("spawn_blocking join");
    match full {
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
    // Trim whitespace; treat empty string as None (remove name)
    let name = req.name.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let db = state.db.clone();
    let name_for_db = name.clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.get().expect("pool get");
        db::set_evolution_name(&conn, id, name_for_db.as_deref());
    })
    .await
    .expect("spawn_blocking join");
    Json(serde_json::json!({"id": id, "name": name}))
}

async fn delete_evolution_handler(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.get().expect("pool get");
        // Stop first so the coordinator task exits on its next status check.
        db::stop_evolution(&conn, id);
        db::delete_evolution(&conn, id);
    })
    .await
    .expect("spawn_blocking join");
    axum::http::StatusCode::NO_CONTENT
}

async fn get_best(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<Vec<serde_json::Value>> {
    let db = state.db.clone();
    let best = tokio::task::spawn_blocking(move || {
        let conn = db.get().expect("pool get");
        db::get_best_genotypes(&conn, id, 10)
    })
    .await
    .expect("spawn_blocking join");
    Json(
        best.into_iter()
            .map(|(gid, fitness, _bytes, island_id)| {
                serde_json::json!({"id": gid, "fitness": fitness, "island_id": island_id})
            })
            .collect(),
    )
}

async fn get_best_per_island_handler(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<Vec<serde_json::Value>> {
    let db = state.db.clone();
    let best = tokio::task::spawn_blocking(move || {
        let conn = db.get().expect("pool get");
        db::get_best_per_island(&conn, id)
    })
    .await
    .expect("spawn_blocking join");
    Json(
        best.into_iter()
            .map(|(gid, fitness, island_id)| {
                serde_json::json!({"id": gid, "fitness": fitness, "island_id": island_id})
            })
            .collect(),
    )
}

async fn stop_evolution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.get().expect("pool get");
        db::stop_evolution(&conn, id);
    })
    .await
    .expect("spawn_blocking join");
    Json(serde_json::json!({"status": "stopped"}))
}

async fn pause_evolution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.get().expect("pool get");
        db::pause_evolution(&conn, id);
    })
    .await
    .expect("spawn_blocking join");
    Json(serde_json::json!({"status": "paused"}))
}

async fn resume_evolution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || {
        let conn = db.get().expect("pool get");
        db::resume_evolution(&conn, id);
    })
    .await
    .expect("spawn_blocking join");
    Json(serde_json::json!({"status": "running"}))
}

/// Spawn a new evolution with the same config + seed as an existing one.
///
/// Deterministic re-run: the initial population and mutation stream will be
/// byte-identical to the source evolution — every random draw feeds from
/// `ChaCha8Rng::seed_from_u64(source_seed)`, same as the original run.
/// Useful for reproducing bugs, testing the effect of a code change on
/// otherwise-identical evolutionary pressure, or saving interesting seeds.
///
/// The new evolution gets a name prefixed with "Replay of {source}".
async fn replay_evolution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    // Read source config + name + seed, then insert new row in a single
    // connection checkout. Seed is inherited verbatim.
    let db = state.db.clone();
    let result = tokio::task::spawn_blocking(move || {
        let conn = db.get().expect("pool get");
        let full = db::get_evolution_full(&conn, id)?;
        let (_status, _gen, config_json, src_name) = full;
        let seed = db::get_evolution_seed(&conn, id);
        let new_name = Some(match src_name {
            Some(n) => format!("Replay of {n}"),
            None => format!("Replay of #{id}"),
        });
        let new_id = db::create_evolution(
            &conn,
            &config_json,
            new_name.as_deref(),
            Some(seed),
        );
        Some((new_id, config_json))
    })
    .await
    .expect("spawn_blocking join");
    let (new_id, config_json) = match result {
        Some(x) => x,
        None => return Json(serde_json::json!({"error": "source evolution not found"})),
    };

    // Spawn coordinator for the replay.
    let db_c = state.db.clone();
    let tx = state.tx.clone();
    tokio::spawn(async move {
        coordinator::run_evolution(db_c, new_id, Some(tx)).await;
    });

    Json(serde_json::json!({
        "id": new_id,
        "source_id": id,
        "config": serde_json::from_str::<serde_json::Value>(&config_json).unwrap_or_default(),
        "status": "running",
    }))
}

async fn get_evolution_stats(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<Vec<serde_json::Value>> {
    let db = state.db.clone();
    let stats = tokio::task::spawn_blocking(move || {
        let conn = db.get().expect("pool get");
        db::get_generation_stats(&conn, id)
    })
    .await
    .expect("spawn_blocking join");
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

async fn get_island_stats_handler(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<Vec<serde_json::Value>> {
    let db = state.db.clone();
    let stats = tokio::task::spawn_blocking(move || {
        let conn = db.get().expect("pool get");
        db::get_island_stats(&conn, id)
    })
    .await
    .expect("spawn_blocking join");
    Json(
        stats
            .into_iter()
            .map(|(generation, island_id, best, avg)| {
                serde_json::json!({
                    "generation": generation,
                    "island_id": island_id,
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
    let db = state.db.clone();
    let bytes = tokio::task::spawn_blocking(move || {
        let conn = db.get().expect("pool get");
        db::get_genotype(&conn, id)
    })
    .await
    .expect("spawn_blocking join");
    match bytes {
        Some(bytes) => (
            [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
            bytes,
        )
            .into_response(),
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

/// Develops a genome into its realized phenotype and returns body + joint info.
/// This shows what the creature *actually becomes* after BFS expansion,
/// respecting recursive_limit / terminal_only / connectivity pruning.
async fn get_phenotype_info(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    // Everything — DB read, deserialization, `develop()`, JSON building —
    // runs on a blocking-pool thread. Tokio runtime workers stay free to
    // handle other HTTP traffic. The pool connection is released as soon as
    // the BLOB read completes, before the CPU-heavy JSON tree is built.
    let db = state.db.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, axum::http::StatusCode> {
        let bytes = {
            let conn = db.get().expect("pool get");
            db::get_genotype(&conn, id)
        };
        let bytes = bytes.ok_or(axum::http::StatusCode::NOT_FOUND)?;
        let genome = bincode::deserialize::<karl_sims_core::genotype::GenomeGraph>(&bytes)
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
        let pheno = karl_sims_core::phenotype::develop(&genome);
        // Pair each body with its originating genome node.
        let bodies: Vec<_> = pheno
            .world
            .bodies
            .iter()
            .enumerate()
            .map(|(i, body)| {
                let (geno_idx, depth) = pheno.body_node_map[i];
                let jt = format!("{:?}", genome.nodes[geno_idx].joint_type);
                serde_json::json!({
                    "id": i,
                    "genome_node": geno_idx,
                    "depth": depth,
                    "half_extents": [body.half_extents.x, body.half_extents.y, body.half_extents.z],
                    "joint_type": jt,
                })
            })
            .collect();
        let joints: Vec<_> = pheno
            .world
            .joints
            .iter()
            .map(|j| {
                serde_json::json!({
                    "parent": j.parent_idx,
                    "child": j.child_idx,
                    "joint_type": format!("{:?}", j.joint_type),
                })
            })
            .collect();
        Ok(serde_json::json!({
            "id": id,
            "num_bodies": bodies.len(),
            "num_joints": joints.len(),
            "root": pheno.world.root,
            "bodies": bodies,
            "joints": joints,
        }))
    })
    .await
    .expect("spawn_blocking join");
    match result {
        Ok(info) => Json(info).into_response(),
        Err(status) => status.into_response(),
    }
}

async fn get_genotype_info(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    // All DB + CPU work happens on the blocking pool. Connection is dropped
    // immediately after fetching the BLOB, before the (often large) JSON
    // tree over neurons/connections is built.
    let db = state.db.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, axum::http::StatusCode> {
        let bytes = {
            let conn = db.get().expect("pool get");
            db::get_genotype(&conn, id)
        };
        let bytes = bytes.ok_or(axum::http::StatusCode::NOT_FOUND)?;
        let genome = bincode::deserialize::<karl_sims_core::genotype::GenomeGraph>(&bytes)
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
        Ok(serde_json::json!({
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
        }))
    })
    .await
    .expect("spawn_blocking join");
    match result {
        Ok(info) => Json(info).into_response(),
        Err(status) => status.into_response(),
    }
}
