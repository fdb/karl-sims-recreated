use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection};
use serde::Serialize;

pub type DbPool = Arc<Mutex<Connection>>;

/// Open (or create) the database and ensure all tables exist.
pub fn init_db(path: &str) -> DbPool {
    let conn = Connection::open(path).expect("Failed to open database");

    // Enable WAL mode for better concurrent read performance.
    conn.execute_batch("PRAGMA journal_mode=WAL;").ok();

    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS evolutions (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            config_json TEXT    NOT NULL,
            status      TEXT    NOT NULL DEFAULT 'running',
            current_gen INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT    NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT    NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS genotypes (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            evolution_id INTEGER NOT NULL REFERENCES evolutions(id),
            generation  INTEGER NOT NULL,
            genome_bytes BLOB   NOT NULL,
            parent_id   INTEGER,
            fitness     REAL,
            created_at  TEXT    NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS tasks (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            evolution_id INTEGER NOT NULL REFERENCES evolutions(id),
            genotype_id INTEGER NOT NULL REFERENCES genotypes(id),
            status      TEXT    NOT NULL DEFAULT 'pending',
            worker_id   TEXT,
            started_at  TEXT,
            completed_at TEXT,
            fitness     REAL
        );

        CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
        CREATE INDEX IF NOT EXISTS idx_genotypes_evo_gen ON genotypes(evolution_id, generation);
        ",
    )
    .expect("Failed to create tables");

    // Reset any tasks stuck in 'running' state from a previous crash.
    // These would never be picked up otherwise.
    conn.execute(
        "UPDATE tasks SET status='pending', worker_id=NULL, started_at=NULL WHERE status='running'",
        [],
    )
    .ok();

    Arc::new(Mutex::new(conn))
}

/// Insert a new evolution run, return its ID.
pub fn create_evolution(conn: &Connection, config_json: &str) -> i64 {
    conn.execute(
        "INSERT INTO evolutions (config_json) VALUES (?1)",
        params![config_json],
    )
    .expect("Failed to create evolution");
    conn.last_insert_rowid()
}

/// Insert a genotype, return its ID.
pub fn insert_genotype(
    conn: &Connection,
    evo_id: i64,
    generation: i64,
    genome_bytes: &[u8],
    parent_id: Option<i64>,
) -> i64 {
    conn.execute(
        "INSERT INTO genotypes (evolution_id, generation, genome_bytes, parent_id) VALUES (?1, ?2, ?3, ?4)",
        params![evo_id, generation, genome_bytes, parent_id],
    )
    .expect("Failed to insert genotype");
    conn.last_insert_rowid()
}

/// Insert a genotype with a known fitness (for survivors carried forward).
pub fn insert_genotype_with_fitness(
    conn: &Connection,
    evo_id: i64,
    generation: i64,
    genome_bytes: &[u8],
    fitness: f64,
) -> i64 {
    conn.execute(
        "INSERT INTO genotypes (evolution_id, generation, genome_bytes, fitness) VALUES (?1, ?2, ?3, ?4)",
        params![evo_id, generation, genome_bytes, fitness],
    )
    .expect("Failed to insert genotype with fitness");
    let gid = conn.last_insert_rowid();
    // Also create a completed task so pending_task_count doesn't block
    conn.execute(
        "INSERT INTO tasks (evolution_id, genotype_id, status, fitness, completed_at) VALUES (?1, ?2, 'completed', ?3, datetime('now'))",
        params![evo_id, gid, fitness],
    )
    .expect("Failed to create completed task for survivor");
    gid
}

/// Create a fitness-evaluation task for a genotype.
pub fn create_task(conn: &Connection, evo_id: i64, genotype_id: i64) -> i64 {
    conn.execute(
        "INSERT INTO tasks (evolution_id, genotype_id) VALUES (?1, ?2)",
        params![evo_id, genotype_id],
    )
    .expect("Failed to create task");
    conn.last_insert_rowid()
}

/// Atomically claim the next pending task. Returns `(task_id, genome_bytes, config_json)`.
pub fn claim_task(conn: &Connection, worker_id: &str) -> Option<(i64, Vec<u8>, String)> {
    // Use a single UPDATE ... RETURNING to atomically claim a task.
    let mut stmt = conn
        .prepare(
            "UPDATE tasks SET status='running', worker_id=?1, started_at=datetime('now')
             WHERE id = (
                 SELECT id FROM tasks
                 WHERE status='pending'
                   AND evolution_id IN (SELECT id FROM evolutions WHERE status='running')
                 LIMIT 1
             )
             RETURNING id, genotype_id, evolution_id",
        )
        .ok()?;

    let result: Option<(i64, i64, i64)> = stmt
        .query_row(params![worker_id], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .ok();

    let (task_id, genotype_id, evolution_id) = result?;

    // Fetch genome bytes separately.
    let genome_bytes: Vec<u8> = conn
        .query_row(
            "SELECT genome_bytes FROM genotypes WHERE id = ?1",
            params![genotype_id],
            |row| row.get(0),
        )
        .ok()?;

    // Fetch the evolution's config_json.
    let config_json: String = conn
        .query_row(
            "SELECT config_json FROM evolutions WHERE id = ?1",
            params![evolution_id],
            |row| row.get(0),
        )
        .ok()?;

    Some((task_id, genome_bytes, config_json))
}

/// Mark a task as completed and store its fitness on both the task and genotype.
pub fn complete_task(conn: &Connection, task_id: i64, fitness: f64) {
    conn.execute(
        "UPDATE tasks SET status='completed', completed_at=datetime('now'), fitness=?1 WHERE id=?2",
        params![fitness, task_id],
    )
    .expect("Failed to complete task");

    // Also store fitness on the genotype row for convenient queries.
    conn.execute(
        "UPDATE genotypes SET fitness=?1 WHERE id=(SELECT genotype_id FROM tasks WHERE id=?2)",
        params![fitness, task_id],
    )
    .expect("Failed to update genotype fitness");
}

/// Get the status and current generation of an evolution.
pub fn get_evolution_status(conn: &Connection, evo_id: i64) -> Option<(String, i64)> {
    conn.query_row(
        "SELECT status, current_gen FROM evolutions WHERE id = ?1",
        params![evo_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .ok()
}

/// Get status, current generation, and config_json for an evolution.
pub fn get_evolution_full(conn: &Connection, evo_id: i64) -> Option<(String, i64, String)> {
    conn.query_row(
        "SELECT status, current_gen, config_json FROM evolutions WHERE id = ?1",
        params![evo_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .ok()
}

/// Get per-generation stats (best, avg fitness) for an evolution.
pub fn get_generation_stats(conn: &Connection, evo_id: i64) -> Vec<(i64, f64, f64)> {
    let mut stmt = conn
        .prepare(
            "SELECT g.generation, MAX(g.fitness), AVG(g.fitness)
             FROM genotypes g
             WHERE g.evolution_id = ?1 AND g.fitness IS NOT NULL
             GROUP BY g.generation
             ORDER BY g.generation",
        )
        .expect("Failed to prepare get_generation_stats");

    stmt.query_map(params![evo_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, f64>(2)?,
        ))
    })
    .expect("Failed to query generation stats")
    .filter_map(|r| r.ok())
    .collect()
}

/// Get all (genotype_id, fitness) pairs for a given generation.
pub fn get_generation_fitnesses(conn: &Connection, evo_id: i64, generation: i64) -> Vec<(i64, f64)> {
    let mut stmt = conn
        .prepare(
            "SELECT g.id, t.fitness
             FROM genotypes g
             JOIN tasks t ON t.genotype_id = g.id
             WHERE g.evolution_id = ?1
               AND g.generation = ?2
               AND t.status = 'completed'",
        )
        .expect("Failed to prepare get_generation_fitnesses");

    stmt.query_map(params![evo_id, generation], |row| {
        Ok((row.get(0)?, row.get::<_, f64>(1)?))
    })
    .expect("Failed to query generation fitnesses")
    .filter_map(|r| r.ok())
    .collect()
}

/// Fetch the raw genome bytes for a genotype.
pub fn get_genotype(conn: &Connection, genotype_id: i64) -> Option<Vec<u8>> {
    conn.query_row(
        "SELECT genome_bytes FROM genotypes WHERE id = ?1",
        params![genotype_id],
        |row| row.get(0),
    )
    .ok()
}

/// Get the top genotypes by fitness for an evolution.
pub fn get_best_genotypes(conn: &Connection, evo_id: i64, limit: i64) -> Vec<(i64, f64, Vec<u8>)> {
    let mut stmt = conn
        .prepare(
            "SELECT id, fitness, genome_bytes FROM genotypes
             WHERE evolution_id = ?1 AND fitness IS NOT NULL
             ORDER BY fitness DESC
             LIMIT ?2",
        )
        .expect("Failed to prepare get_best_genotypes");

    stmt.query_map(params![evo_id, limit], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })
    .expect("Failed to query best genotypes")
    .filter_map(|r| r.ok())
    .collect()
}

/// Count pending tasks for an evolution.
pub fn pending_task_count(conn: &Connection, evo_id: i64) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM tasks WHERE evolution_id = ?1 AND status IN ('pending', 'running')",
        params![evo_id],
        |row| row.get(0),
    )
    .unwrap_or(0)
}

/// Update the status and current generation of an evolution.
pub fn update_evolution(conn: &Connection, evo_id: i64, status: &str, generation: i64) {
    conn.execute(
        "UPDATE evolutions SET status=?1, current_gen=?2, updated_at=datetime('now') WHERE id=?3",
        params![status, generation, evo_id],
    )
    .expect("Failed to update evolution");
}

/// Stop an evolution permanently.
pub fn stop_evolution(conn: &Connection, evo_id: i64) {
    conn.execute(
        "UPDATE evolutions SET status='stopped', updated_at=datetime('now') WHERE id=?1",
        params![evo_id],
    )
    .expect("Failed to stop evolution");
}

/// Pause a running evolution. Workers will stop claiming its tasks.
pub fn pause_evolution(conn: &Connection, evo_id: i64) {
    conn.execute(
        "UPDATE evolutions SET status='paused', updated_at=datetime('now') WHERE id=?1 AND status='running'",
        params![evo_id],
    )
    .expect("Failed to pause evolution");
}

/// Resume a paused evolution.
pub fn resume_evolution(conn: &Connection, evo_id: i64) {
    conn.execute(
        "UPDATE evolutions SET status='running', updated_at=datetime('now') WHERE id=?1 AND status='paused'",
        params![evo_id],
    )
    .expect("Failed to resume evolution");
}

/// A row from the evolutions table.
#[derive(Debug, Serialize)]
pub struct EvolutionRow {
    pub id: i64,
    pub config_json: String,
    pub status: String,
    pub current_gen: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// List all evolutions.
pub fn list_evolutions(conn: &Connection) -> Vec<EvolutionRow> {
    let mut stmt = conn
        .prepare("SELECT id, config_json, status, current_gen, created_at, updated_at FROM evolutions ORDER BY id DESC")
        .expect("Failed to prepare list_evolutions");

    stmt.query_map([], |row| {
        Ok(EvolutionRow {
            id: row.get(0)?,
            config_json: row.get(1)?,
            status: row.get(2)?,
            current_gen: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })
    .expect("Failed to list evolutions")
    .filter_map(|r| r.ok())
    .collect()
}
