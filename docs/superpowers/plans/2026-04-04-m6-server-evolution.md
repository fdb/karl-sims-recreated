# M6: Server + Evolution at Scale — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Server binary that runs multi-threaded evolution with SQLite persistence, REST+WebSocket API, and serves the frontend. Run first real evolution: population 300, 50+ generations, swimming speed.

**Architecture:** The server crate uses axum for HTTP/WS, rusqlite for SQLite, tokio for async. A coordinator task manages the evolution loop (selection → reproduction → dispatch). Worker tasks pull fitness evaluations from SQLite and run them using the core crate. The frontend is served as static files.

**Tech Stack:** Rust, axum, tokio, rusqlite, serde_json, karl-sims-core

---

## File Structure

```
server/
├── Cargo.toml          # MODIFY: add dependencies
└── src/
    ├── main.rs         # NEW: entry point, CLI args, start server
    ├── db.rs           # NEW: SQLite schema, queries, task queue
    ├── coordinator.rs  # NEW: evolution loop (selection → reproduction → dispatch)
    ├── worker.rs       # NEW: fitness evaluation worker
    ├── api.rs          # NEW: REST endpoints
    └── ws.rs           # NEW: WebSocket live updates
```

---

## Task 1: Server Dependencies + SQLite Schema

**Files:**
- Modify: `server/Cargo.toml`
- Create: `server/src/main.rs`, `server/src/db.rs`

### Cargo.toml

```toml
[package]
name = "karl-sims-server"
version = "0.1.0"
edition.workspace = true

[dependencies]
karl-sims-core = { path = "../core" }
tokio = { version = "1", features = ["full"] }
axum = { version = "0.8", features = ["ws"] }
rusqlite = { version = "0.32", features = ["bundled"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tower-http = { version = "0.6", features = ["fs", "cors"] }
chrono = { version = "0.4", features = ["serde"] }
bincode = "1"
rand = "0.8"
rand_chacha = "0.3"
num_cpus = "1"
log = "0.4"
env_logger = "0.11"
```

### db.rs — SQLite schema + queries

```rust
use rusqlite::{Connection, params};
use std::sync::{Arc, Mutex};

pub type DbPool = Arc<Mutex<Connection>>;

pub fn init_db(path: &str) -> DbPool {
    let conn = Connection::open(path).expect("Failed to open SQLite database");
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS evolutions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            config TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'running',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            generation INTEGER NOT NULL DEFAULT 0
        );
        CREATE TABLE IF NOT EXISTS genotypes (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            evolution_id INTEGER NOT NULL REFERENCES evolutions(id),
            generation INTEGER NOT NULL,
            genome BLOB NOT NULL,
            fitness REAL,
            parent_id INTEGER,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE TABLE IF NOT EXISTS tasks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            evolution_id INTEGER NOT NULL REFERENCES evolutions(id),
            genotype_id INTEGER NOT NULL REFERENCES genotypes(id),
            status TEXT NOT NULL DEFAULT 'pending',
            worker_id TEXT,
            result REAL,
            started_at TEXT,
            completed_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
        CREATE INDEX IF NOT EXISTS idx_genotypes_evolution ON genotypes(evolution_id, generation);
    ").expect("Failed to create tables");
    Arc::new(Mutex::new(conn))
}
```

Add CRUD functions:
- `create_evolution(db, config_json) -> i64`
- `create_genotype(db, evo_id, gen, genome_bytes, parent_id) -> i64`
- `create_task(db, evo_id, genotype_id) -> i64`
- `claim_task(db, worker_id) -> Option<(task_id, genotype_id, genome_bytes)>`
- `complete_task(db, task_id, fitness)`
- `get_evolution(db, id) -> EvolutionRow`
- `get_generation_genotypes(db, evo_id, gen) -> Vec<GenotypeRow>`
- `get_best_genotypes(db, evo_id, limit) -> Vec<GenotypeRow>`
- `pending_tasks_count(db, evo_id) -> usize`
- `update_evolution_generation(db, evo_id, gen)`

### main.rs (skeleton)

```rust
#[tokio::main]
async fn main() {
    env_logger::init();
    let db = db::init_db("karl-sims.db");
    log::info!("Server starting...");
    // API + workers + coordinator setup in later tasks
}
```

- [ ] **Step 1: Create Cargo.toml, db.rs with schema + queries, main.rs skeleton**
- [ ] **Step 2: Verify `cargo check -p karl-sims-server`**
- [ ] **Step 3: Commit**

---

## Task 2: Worker + Coordinator

**Files:**
- Create: `server/src/worker.rs`, `server/src/coordinator.rs`

### worker.rs

```rust
pub async fn run_worker(db: DbPool, worker_id: String) {
    loop {
        // Try to claim a task
        let task = { db.lock().unwrap().claim_task(&worker_id) };
        match task {
            Some((task_id, _genotype_id, genome_bytes)) => {
                let genome: GenomeGraph = bincode::deserialize(&genome_bytes).unwrap();
                let config = FitnessConfig::default();
                let result = evaluate_swimming_fitness(&genome, &config);
                db.lock().unwrap().complete_task(task_id, result.score);
            }
            None => {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
    }
}
```

### coordinator.rs

```rust
pub async fn run_evolution(db: DbPool, evo_id: i64, config: EvolutionConfig) {
    let mut rng = ChaCha8Rng::seed_from_u64(evo_id as u64);
    
    // Generation 0: create initial population
    let mut pop = Population::random_initial(config.clone(), &mut rng);
    insert_generation(&db, evo_id, 0, &pop);
    
    for gen in 0..100 {
        // Wait for all fitness evaluations to complete
        wait_for_tasks(&db, evo_id).await;
        
        // Read fitness results
        read_fitness_results(&db, evo_id, gen, &mut pop);
        
        // Evolve next generation
        pop.evolve_generation(&mut rng);
        
        // Insert new generation
        insert_generation(&db, evo_id, gen + 1, &pop);
        update_evolution_generation(&db, evo_id, gen + 1);
        
        log::info!("Gen {}: best={:.4}", gen, pop.stats_history.last().map(|s| s.best_fitness).unwrap_or(0.0));
    }
}
```

- [ ] **Step 1: Implement worker.rs and coordinator.rs**
- [ ] **Step 2: Wire into main.rs — spawn N workers + coordinator task**
- [ ] **Step 3: Test by running `cargo run -p karl-sims-server` and checking DB**
- [ ] **Step 4: Commit**

---

## Task 3: REST API

**Files:**
- Create: `server/src/api.rs`
- Modify: `server/src/main.rs`

### Endpoints

```rust
// POST /api/evolutions — start a new evolution
// GET /api/evolutions — list all
// GET /api/evolutions/:id — get status
// GET /api/evolutions/:id/best — get best N creatures
// POST /api/evolutions/:id/stop — stop evolution
```

Use axum Router:
```rust
pub fn router(db: DbPool) -> Router {
    Router::new()
        .route("/api/evolutions", post(create_evolution).get(list_evolutions))
        .route("/api/evolutions/:id", get(get_evolution))
        .route("/api/evolutions/:id/best", get(get_best))
        .route("/api/evolutions/:id/stop", post(stop_evolution))
        .with_state(db)
}
```

### Static file serving

Serve the frontend build from `frontend/dist/`:
```rust
use tower_http::services::ServeDir;
let app = router(db).fallback_service(ServeDir::new("frontend/dist"));
```

- [ ] **Step 1: Implement api.rs with REST endpoints**
- [ ] **Step 2: Wire into main.rs with axum server on port 3000**
- [ ] **Step 3: Verify with curl**
- [ ] **Step 4: Commit**

---

## Task 4: WebSocket Live Updates

**Files:**
- Create: `server/src/ws.rs`
- Modify: `server/src/main.rs`

### WebSocket endpoint

```rust
// GET /api/evolutions/:id/live — WebSocket for live updates
```

Sends JSON messages when:
- A generation completes (stats)
- A task completes (individual fitness)

Use axum's WebSocket support with a broadcast channel:
```rust
use tokio::sync::broadcast;

pub type UpdateSender = broadcast::Sender<String>;

// In coordinator, after each generation:
tx.send(serde_json::to_string(&stats).unwrap()).ok();
```

- [ ] **Step 1: Implement ws.rs**
- [ ] **Step 2: Wire broadcast channel from coordinator to WS handler**
- [ ] **Step 3: Commit**

---

## Task 5: Integration Test — First Real Evolution

Run the server and verify a complete evolution runs:

```bash
cargo run --release -p karl-sims-server
# In another terminal:
curl -X POST http://localhost:3000/api/evolutions -H 'Content-Type: application/json' -d '{"population_size": 50, "generations": 5}'
curl http://localhost:3000/api/evolutions/1
curl http://localhost:3000/api/evolutions/1/best
```

- [ ] **Step 1: Run server, start evolution via API**
- [ ] **Step 2: Verify generation progress in logs**
- [ ] **Step 3: Verify best creatures retrievable**
- [ ] **Step 4: Build frontend and verify static serving**
- [ ] **Step 5: Commit any fixes**

---

## Self-Review

**Spec coverage:**
- [x] SQLite schema: evolutions, generations, genotypes, tasks → Task 1
- [x] Worker loop: pull → deserialize → simulate → write fitness → Task 2
- [x] Coordinator loop: selection → reproduction → dispatch → Task 2
- [x] REST API → Task 3
- [x] WebSocket live updates → Task 4
- [x] Static file serving → Task 3
- [x] N worker threads → Task 2
- [x] Single command: `cargo run -p karl-sims-server` → Task 2
