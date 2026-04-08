use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::{Json, Router};
use serde::Deserialize;

use karl_sims_core::fitness::{Environment, EvolutionParams, FitnessGoal};

use crate::coordinator;
use crate::db::{self, DbPool};
use crate::engine::Engine;
use crate::timing::{db_read_async, timed_db};
use crate::ws::UpdateSender;

#[derive(Clone)]
pub struct AppState {
    /// Write pool — used by coordinator and mutating API calls.
    pub db: DbPool,
    /// Read-only pool — used only for genotype/phenotype endpoints.
    pub read_db: DbPool,
    /// In-memory engine — all list/get/best/stats reads go here.
    pub engine: Arc<Engine>,
    pub tx: UpdateSender,
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
    min_joint_motion: Option<f64>,
    max_joint_angular_velocity: Option<f64>,
    num_signal_channels: Option<usize>,
    growth_interval: Option<usize>,
    solver_iterations: Option<usize>,
    pgs_iterations: Option<usize>,
    friction_coefficient: Option<f64>,
    use_coulomb_friction: Option<bool>,
    friction_combine_max: Option<bool>,
    airtime_penalty: Option<f64>,
    island_strategy: Option<String>,
    exchange_interval: Option<usize>,
    diversity_pressure: Option<f64>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct PatchEvolutionRequest {
    name: Option<String>,
}

#[derive(Deserialize)]
struct PatchConfigRequest {
    max_generations: Option<usize>,
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
            "/api/evolutions/{id}/config",
            axum::routing::patch(patch_evolution_config_handler),
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

// ── Read endpoints: served from in-memory engine (zero DB) ──────────────

async fn list_evolutions(State(state): State<AppState>) -> Json<Vec<serde_json::Value>> {
    let snaps = state.engine.list_snapshots();
    Json(
        snaps
            .into_iter()
            .map(|s| {
                let config = serde_json::from_str::<serde_json::Value>(&s.config_json)
                    .unwrap_or_default();
                serde_json::json!({
                    "id": s.id,
                    "status": s.status,
                    "generation": s.current_gen,
                    "config": config,
                    "created_at": s.created_at,
                    "name": s.name,
                })
            })
            .collect(),
    )
}

async fn get_evolution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    match state.engine.get_snapshot(id) {
        Some(s) => {
            let config = serde_json::from_str::<serde_json::Value>(&s.config_json)
                .unwrap_or_default();
            Json(serde_json::json!({
                "id": s.id,
                "status": s.status,
                "generation": s.current_gen,
                "config": config,
                "name": s.name,
            }))
        }
        None => {
            // Fallback to DB for evolutions not yet loaded into engine
            let full = db_read_async("api.get_evolution", state.read_db.clone(), move |c| {
                db::get_evolution_full(c, id)
            })
            .await;
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
    }
}

async fn get_best(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<Vec<serde_json::Value>> {
    if let Some(snap) = state.engine.get_snapshot(id) {
        return Json(
            snap.best_creatures
                .into_iter()
                .map(|c| serde_json::json!({"id": c.id, "fitness": c.fitness, "island_id": c.island_id}))
                .collect(),
        );
    }
    // Fallback to DB
    let best = db_read_async("api.get_best", state.read_db.clone(), move |c| {
        db::get_best_genotypes(c, id, 10)
    })
    .await;
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
    if let Some(snap) = state.engine.get_snapshot(id) {
        return Json(
            snap.best_per_island
                .into_iter()
                .map(|c| serde_json::json!({"id": c.id, "fitness": c.fitness, "island_id": c.island_id}))
                .collect(),
        );
    }
    let best = db_read_async("api.get_best_per_island", state.read_db.clone(), move |c| {
        db::get_best_per_island(c, id)
    })
    .await;
    Json(
        best.into_iter()
            .map(|(gid, fitness, island_id)| {
                serde_json::json!({"id": gid, "fitness": fitness, "island_id": island_id})
            })
            .collect(),
    )
}

async fn get_evolution_stats(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<Vec<serde_json::Value>> {
    if let Some(snap) = state.engine.get_snapshot(id) {
        return Json(
            snap.gen_stats
                .into_iter()
                .map(|s| {
                    serde_json::json!({
                        "generation": s.generation,
                        "best_fitness": s.best_fitness,
                        "avg_fitness": s.avg_fitness,
                    })
                })
                .collect(),
        );
    }
    let stats = db_read_async("api.get_evolution_stats", state.read_db.clone(), move |c| {
        db::get_generation_stats(c, id)
    })
    .await;
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
    if let Some(snap) = state.engine.get_snapshot(id) {
        return Json(
            snap.island_stats
                .into_iter()
                .map(|s| {
                    serde_json::json!({
                        "generation": s.generation,
                        "island_id": s.island_id,
                        "best_fitness": s.best_fitness,
                        "avg_fitness": s.avg_fitness,
                    })
                })
                .collect(),
        );
    }
    let stats = db_read_async("api.get_island_stats", state.read_db.clone(), move |c| {
        db::get_island_stats(c, id)
    })
    .await;
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

// ── Mutation endpoints (still use DB for persistence) ───────────────────

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
        population_size: req.population_size.unwrap_or(300).clamp(5, 1000),
        max_generations: req.generations.unwrap_or(300).clamp(1, 10000),
        goal,
        environment: env,
        sim_duration: req.sim_duration.unwrap_or(10.0).clamp(1.0, 60.0),
        max_parts: req.max_parts.unwrap_or(5).clamp(2, 50),
        gravity: req.gravity.unwrap_or(9.81).clamp(0.0, 30.0),
        water_viscosity: req.water_viscosity.unwrap_or(2.0).clamp(0.1, 10.0),
        max_body_angular_velocity: Some(20.0),
        num_islands: req.num_islands.unwrap_or(5).clamp(1, 12),
        migration_interval: req.migration_interval.unwrap_or(20).clamp(0, 1000),
        min_joint_motion: req.min_joint_motion.or(Some(0.2)),
        settle_duration: Some(1.0),
        num_signal_channels: req.num_signal_channels.unwrap_or(0),
        growth_interval: req.growth_interval,
        max_joint_angular_velocity: req.max_joint_angular_velocity.or(Some(20.0)),
        solver_iterations: req.solver_iterations.map(|v| v.clamp(1, 64)).or(Some(8)),
        pgs_iterations: req.pgs_iterations.map(|v| v.clamp(1, 16)).or(Some(2)),
        friction_coefficient: req.friction_coefficient.map(|v| v.clamp(0.0, 10.0)).or(Some(1.5)),
        use_coulomb_friction: req.use_coulomb_friction.or(Some(true)),
        friction_combine_max: req.friction_combine_max.or(Some(true)),
        airtime_penalty: req.airtime_penalty.unwrap_or(0.0).clamp(0.0, 1.0),
        island_strategy: match req.island_strategy.as_deref() {
            Some("ring_migration") => karl_sims_core::fitness::IslandStrategy::RingMigration,
            Some("hfc") => karl_sims_core::fitness::IslandStrategy::HFC,
            _ => karl_sims_core::fitness::IslandStrategy::Isolated,
        },
        exchange_interval: req.exchange_interval.unwrap_or(10).clamp(1, 100),
        diversity_pressure: req.diversity_pressure.unwrap_or(0.0).clamp(0.0, 1.0),
    };
    let config_json = serde_json::to_string(&params).unwrap();
    let evo_id = {
        let config_json = config_json.clone();
        let name = req.name.clone();
        db_read_async("api.create_evolution", state.db.clone(), move |c| {
            db::create_evolution(c, &config_json, name.as_deref(), None)
        })
        .await
    };

    let engine = state.engine.clone();
    let db_c = state.db.clone();
    let tx = state.tx.clone();
    tokio::spawn(async move {
        coordinator::run_evolution(engine, db_c, evo_id, Some(tx)).await;
    });

    Json(serde_json::json!({"id": evo_id, "status": "running"}))
}

async fn patch_evolution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<PatchEvolutionRequest>,
) -> Json<serde_json::Value> {
    let name = req.name.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let name_for_db = name.clone();
    db_read_async("api.patch_evolution", state.db.clone(), move |c| {
        db::set_evolution_name(c, id, name_for_db.as_deref())
    })
    .await;
    Json(serde_json::json!({"id": id, "name": name}))
}

async fn patch_evolution_config_handler(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Json(req): Json<PatchConfigRequest>,
) -> Json<serde_json::Value> {
    let mut patch = serde_json::json!({});
    if let Some(mg) = req.max_generations {
        patch["max_generations"] = serde_json::json!(mg);
    }
    if patch.as_object().map(|o| o.is_empty()).unwrap_or(true) {
        return Json(serde_json::json!({"error": "no recognised fields provided"}));
    }
    db_read_async("api.patch_evolution_config", state.db.clone(), move |c| {
        db::patch_evolution_config(c, id, &patch)
    })
    .await;
    Json(serde_json::json!({"id": id, "patched": req.max_generations.map(|v| serde_json::json!({"max_generations": v}))}))
}

async fn delete_evolution_handler(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    state.engine.remove_snapshot(id);
    db_read_async("api.delete_evolution", state.db.clone(), move |c| {
        db::stop_evolution(c, id);
        db::delete_evolution(c, id);
    })
    .await;
    axum::http::StatusCode::NO_CONTENT
}

async fn stop_evolution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    state.engine.set_status(id, "stopped");
    db_read_async("api.stop_evolution", state.db.clone(), move |c| {
        db::stop_evolution(c, id)
    })
    .await;
    Json(serde_json::json!({"status": "stopped"}))
}

async fn pause_evolution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    state.engine.set_status(id, "paused");
    db_read_async("api.pause_evolution", state.db.clone(), move |c| {
        db::pause_evolution(c, id)
    })
    .await;
    Json(serde_json::json!({"status": "paused"}))
}

async fn resume_evolution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    state.engine.set_status(id, "running");
    db_read_async("api.resume_evolution", state.db.clone(), move |c| {
        db::resume_evolution(c, id)
    })
    .await;
    Json(serde_json::json!({"status": "running"}))
}

async fn replay_evolution(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Json<serde_json::Value> {
    let result = db_read_async("api.replay_evolution", state.db.clone(), move |c| {
        let full = db::get_evolution_full(c, id)?;
        let (_status, _gen, config_json, src_name) = full;
        let seed = db::get_evolution_seed(c, id);
        let new_name = Some(match src_name {
            Some(n) => format!("Replay of {n}"),
            None => format!("Replay of #{id}"),
        });
        let new_id = db::create_evolution(c, &config_json, new_name.as_deref(), Some(seed));
        Some((new_id, config_json))
    })
    .await;
    let (new_id, config_json) = match result {
        Some(x) => x,
        None => return Json(serde_json::json!({"error": "source evolution not found"})),
    };

    let engine = state.engine.clone();
    let db_c = state.db.clone();
    let tx = state.tx.clone();
    tokio::spawn(async move {
        coordinator::run_evolution(engine, db_c, new_id, Some(tx)).await;
    });

    Json(serde_json::json!({
        "id": new_id,
        "source_id": id,
        "config": serde_json::from_str::<serde_json::Value>(&config_json).unwrap_or_default(),
        "status": "running",
    }))
}

// ── Genotype endpoints (still use DB — these are user-triggered, rare) ──

async fn get_genome_bytes(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let bytes = db_read_async("api.get_genome_bytes", state.read_db.clone(), move |c| {
        db::get_genotype(c, id)
    })
    .await;
    match bytes {
        Some(bytes) => (
            [(axum::http::header::CONTENT_TYPE, "application/octet-stream")],
            bytes,
        )
            .into_response(),
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}

async fn get_phenotype_info(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let db = state.read_db.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, axum::http::StatusCode> {
        let bytes = timed_db("api.get_phenotype_info", &db, |c| db::get_genotype(c, id));
        let bytes = bytes.ok_or(axum::http::StatusCode::NOT_FOUND)?;
        let genome = bincode::deserialize::<karl_sims_core::genotype::GenomeGraph>(&bytes)
            .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)?;
        let pheno = karl_sims_core::phenotype::develop(&genome);
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
    let db = state.read_db.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<serde_json::Value, axum::http::StatusCode> {
        let bytes = timed_db("api.get_genotype_info", &db, |c| db::get_genotype(c, id));
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
