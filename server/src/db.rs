use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::{params, Connection};
use serde::Serialize;

/// A pool of SQLite connections. Every pool member opens the same database
/// file and shares its WAL; locking is handled by SQLite itself, not by us.
///
/// Previously this was `Arc<Mutex<Connection>>` — a single connection behind
/// a global mutex, which meant every worker thread, every API handler, and
/// the coordinator all serialized through one lock. With a pool, each caller
/// holds its own connection and SQLite's WAL lets readers proceed in parallel
/// with a single writer.
pub type DbPool = r2d2::Pool<SqliteConnectionManager>;

/// Pragmas applied to every connection the pool hands out. `synchronous=NORMAL`
/// is the standard companion to WAL — still crash-safe, roughly 10× faster
/// than the `FULL` default because it skips an fsync per commit. `busy_timeout`
/// makes concurrent writers wait instead of failing with `SQLITE_BUSY`.
fn apply_pragmas(conn: &mut Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA busy_timeout=5000;
         PRAGMA foreign_keys=ON;
         PRAGMA mmap_size=268435456;
         PRAGMA cache_size=-65536;",
    )
}

/// Pragmas for read-only connections. No busy_timeout needed (WAL readers
/// never hit SQLITE_BUSY), `query_only` prevents accidental writes, and
/// `mmap_size` gives fast BLOB reads via memory-mapped I/O.
fn apply_read_pragmas(conn: &mut Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA query_only=ON;
         PRAGMA mmap_size=268435456;
         PRAGMA cache_size=-65536;",
    )
}

/// Open (or create) the database, ensure all tables exist, and return a
/// write pool. Size is `num_cpus + 4` so every worker thread + the
/// coordinator + a few mutating API calls always have a connection
/// without blocking. Previously this was fixed at 16, which was smaller
/// than `num_cpus` on machines with 20+ cores — causing pool starvation
/// that cascaded into multi-second API stalls.
pub fn init_db(path: &str) -> DbPool {
    let manager = SqliteConnectionManager::file(path).with_init(|c| {
        apply_pragmas(c)
    });
    let write_pool_size = (num_cpus::get() + 4) as u32;
    let pool = r2d2::Pool::builder()
        .max_size(write_pool_size)
        .build(manager)
        .expect("Failed to build SQLite write pool");
    log::info!("Write pool: {write_pool_size} connections");

    let conn = pool.get().expect("Failed to acquire initial connection");

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

    // Migrate: add name column if it doesn't exist yet (safe to run multiple times).
    conn.execute(
        "ALTER TABLE evolutions ADD COLUMN name TEXT",
        [],
    )
    .ok(); // Silently ignores "duplicate column" error on subsequent startups.

    // Migrate: add island_id column for islands-model evolution. Default 0
    // so existing evolutions (single-pool) keep working transparently.
    conn.execute(
        "ALTER TABLE genotypes ADD COLUMN island_id INTEGER NOT NULL DEFAULT 0",
        [],
    )
    .ok();

    // Migrate: add seed column so evolutions are reproducible independently
    // of their auto-increment id. Existing rows get NULL; the coordinator
    // falls back to `evo_id as u64` for NULL seeds, preserving prior behavior.
    conn.execute(
        "ALTER TABLE evolutions ADD COLUMN seed INTEGER",
        [],
    )
    .ok();

    // Index to speed up per-island, per-generation lookups.
    conn.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_genotypes_evo_island_gen
         ON genotypes(evolution_id, island_id, generation);",
    )
    .ok();

    // Reset any tasks stuck in 'running' state from a previous crash.
    // These would never be picked up otherwise.
    conn.execute(
        "UPDATE tasks SET status='pending', worker_id=NULL, started_at=NULL WHERE status='running'",
        [],
    )
    .ok();

    drop(conn);
    pool
}

/// Create a separate read-only pool for API GET handlers.
///
/// Under WAL, read-only connections NEVER contend for the writer lock
/// and NEVER hit SQLITE_BUSY, so they don't need `busy_timeout` and
/// can't be blocked by the coordinator's write bursts. This completely
/// decouples UI read latency from evolution write throughput.
pub fn init_read_pool(path: &str) -> DbPool {
    let manager = SqliteConnectionManager::file(path).with_init(|c| {
        apply_read_pragmas(c)
    });
    let pool = r2d2::Pool::builder()
        .max_size(8)
        .build(manager)
        .expect("Failed to build SQLite read pool");
    log::info!("Read pool: 8 connections");
    pool
}

/// Insert a new evolution run, return its ID.
///
/// If `seed` is `None`, the evolution's RNG seed defaults to its row id (the
/// original behavior). Pass `Some(seed)` when replaying from another run so the
/// initial population + mutation stream are byte-identical to the source.
pub fn create_evolution(
    conn: &Connection,
    config_json: &str,
    name: Option<&str>,
    seed: Option<u64>,
) -> i64 {
    conn.execute(
        "INSERT INTO evolutions (config_json, name, seed) VALUES (?1, ?2, ?3)",
        // SQLite stores i64; cast from u64 is fine, bit pattern preserved.
        params![config_json, name, seed.map(|s| s as i64)],
    )
    .expect("Failed to create evolution");
    conn.last_insert_rowid()
}

/// Get the seed used by this evolution's RNG. Returns the stored `seed` if
/// present, otherwise falls back to `evo_id as u64` for backward compatibility
/// with rows created before the seed column existed.
pub fn get_evolution_seed(conn: &Connection, evo_id: i64) -> u64 {
    let stored: Option<i64> = conn
        .query_row(
            "SELECT seed FROM evolutions WHERE id = ?1",
            params![evo_id],
            |row| row.get(0),
        )
        .unwrap_or(None);
    stored.map(|s| s as u64).unwrap_or(evo_id as u64)
}

/// Update the name of an evolution.
pub fn set_evolution_name(conn: &Connection, evo_id: i64, name: Option<&str>) {
    conn.execute(
        "UPDATE evolutions SET name=?1, updated_at=datetime('now') WHERE id=?2",
        params![name, evo_id],
    )
    .expect("Failed to update evolution name");
}

/// Patch a single field in config_json.
///
/// Reads the stored JSON, merges `patch` into it (top-level keys only), and
/// writes it back. This is the safe way to update individual params (e.g.
/// `max_generations`) without clobbering the rest of the config.
pub fn patch_evolution_config(conn: &Connection, evo_id: i64, patch: &serde_json::Value) {
    let config_json: String = conn
        .query_row(
            "SELECT config_json FROM evolutions WHERE id = ?1",
            params![evo_id],
            |row| row.get(0),
        )
        .expect("Failed to read config_json for patch");

    let mut config: serde_json::Value =
        serde_json::from_str(&config_json).unwrap_or(serde_json::json!({}));

    if let (Some(obj), Some(patch_obj)) = (config.as_object_mut(), patch.as_object()) {
        for (k, v) in patch_obj {
            obj.insert(k.clone(), v.clone());
        }
    }

    let new_json = serde_json::to_string(&config).expect("Failed to serialise patched config");
    conn.execute(
        "UPDATE evolutions SET config_json=?1, updated_at=datetime('now') WHERE id=?2",
        params![new_json, evo_id],
    )
    .expect("Failed to write patched config_json");
}

/// Read max_generations from the stored config_json. Returns `None` if the
/// evolution doesn't exist or the field is absent.
pub fn get_max_generations(conn: &Connection, evo_id: i64) -> Option<usize> {
    let config_json: String = conn
        .query_row(
            "SELECT config_json FROM evolutions WHERE id = ?1",
            params![evo_id],
            |row| row.get(0),
        )
        .ok()?;
    let v: serde_json::Value = serde_json::from_str(&config_json).ok()?;
    v.get("max_generations")?.as_u64().map(|n| n as usize)
}

/// Insert a genotype, return its ID. `island_id` is 0 for single-pool
/// evolutions, or the island index for islands-model runs.
pub fn insert_genotype(
    conn: &Connection,
    evo_id: i64,
    generation: i64,
    genome_bytes: &[u8],
    parent_id: Option<i64>,
    island_id: i64,
) -> i64 {
    conn.execute(
        "INSERT INTO genotypes (evolution_id, generation, genome_bytes, parent_id, island_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![evo_id, generation, genome_bytes, parent_id, island_id],
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
    island_id: i64,
) -> i64 {
    conn.execute(
        "INSERT INTO genotypes (evolution_id, generation, genome_bytes, fitness, island_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![evo_id, generation, genome_bytes, fitness, island_id],
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

/// Get status, current generation, config_json, and name for an evolution.
pub fn get_evolution_full(conn: &Connection, evo_id: i64) -> Option<(String, i64, String, Option<String>)> {
    conn.query_row(
        "SELECT status, current_gen, config_json, name FROM evolutions WHERE id = ?1",
        params![evo_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .ok()
}

/// Per-generation stats grouped by island.
/// Returns rows of (generation, island_id, best_fitness, avg_fitness).
pub fn get_island_stats(conn: &Connection, evo_id: i64) -> Vec<(i64, i64, f64, f64)> {
    let mut stmt = conn
        .prepare(
            "SELECT g.generation, g.island_id, MAX(g.fitness), AVG(g.fitness)
             FROM genotypes g
             WHERE g.evolution_id = ?1 AND g.fitness IS NOT NULL
             GROUP BY g.generation, g.island_id
             ORDER BY g.generation, g.island_id",
        )
        .expect("Failed to prepare get_island_stats");

    stmt.query_map(params![evo_id], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, f64>(2)?,
            row.get::<_, f64>(3)?,
        ))
    })
    .expect("Failed to query island stats")
    .filter_map(|r| r.ok())
    .collect()
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

/// Get all (genotype_id, fitness) pairs for a given generation (across all islands).
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

/// Get (genotype_id, fitness) pairs for a given generation, scoped to one island.
pub fn get_generation_fitnesses_by_island(
    conn: &Connection,
    evo_id: i64,
    island_id: i64,
    generation: i64,
) -> Vec<(i64, f64)> {
    let mut stmt = conn
        .prepare(
            "SELECT g.id, t.fitness
             FROM genotypes g
             JOIN tasks t ON t.genotype_id = g.id
             WHERE g.evolution_id = ?1
               AND g.island_id = ?2
               AND g.generation = ?3
               AND t.status = 'completed'",
        )
        .expect("Failed to prepare get_generation_fitnesses_by_island");

    stmt.query_map(params![evo_id, island_id, generation], |row| {
        Ok((row.get(0)?, row.get::<_, f64>(1)?))
    })
    .expect("Failed to query island fitnesses")
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

/// Load an entire island's generation in a single query: returns
/// `(genotype_id, fitness, genome_bytes)` for every completed creature.
///
/// This replaces the N+1 pattern of `get_generation_fitnesses_by_island`
/// followed by N individual `get_genotype` calls. On a large WAL, each
/// individual BLOB read scans WAL frames independently — 50 reads × 600MB
/// WAL = catastrophic. A single query does one WAL scan for all rows.
pub fn load_island_generation(
    conn: &Connection,
    evo_id: i64,
    island_id: i64,
    generation: i64,
) -> Vec<(i64, f64, Vec<u8>)> {
    let mut stmt = conn
        .prepare(
            "SELECT g.id, t.fitness, g.genome_bytes
             FROM genotypes g
             JOIN tasks t ON t.genotype_id = g.id
             WHERE g.evolution_id = ?1
               AND g.island_id = ?2
               AND g.generation = ?3
               AND t.status = 'completed'",
        )
        .expect("Failed to prepare load_island_generation");

    stmt.query_map(params![evo_id, island_id, generation], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, f64>(1)?,
            row.get::<_, Vec<u8>>(2)?,
        ))
    })
    .expect("Failed to query island generation")
    .filter_map(|r| r.ok())
    .collect()
}

/// Get the top genotypes by fitness for an evolution.
/// Returns (id, fitness, genome_bytes, island_id).
pub fn get_best_genotypes(
    conn: &Connection,
    evo_id: i64,
    limit: i64,
) -> Vec<(i64, f64, Vec<u8>, i64)> {
    let mut stmt = conn
        .prepare(
            "SELECT id, fitness, genome_bytes, island_id FROM genotypes
             WHERE evolution_id = ?1 AND fitness IS NOT NULL
             ORDER BY fitness DESC
             LIMIT ?2",
        )
        .expect("Failed to prepare get_best_genotypes");

    stmt.query_map(params![evo_id, limit], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
    })
    .expect("Failed to query best genotypes")
    .filter_map(|r| r.ok())
    .collect()
}

/// Get the single best genotype from each island (top by fitness per island).
/// Returns (id, fitness, island_id) sorted by island_id.
pub fn get_best_per_island(conn: &Connection, evo_id: i64) -> Vec<(i64, f64, i64)> {
    let mut stmt = conn
        .prepare(
            "SELECT id, fitness, island_id FROM genotypes g
             WHERE evolution_id = ?1 AND fitness IS NOT NULL
               AND fitness = (
                 SELECT MAX(fitness) FROM genotypes
                 WHERE evolution_id = ?1 AND island_id = g.island_id
                   AND fitness IS NOT NULL
               )
             GROUP BY island_id
             ORDER BY island_id",
        )
        .expect("Failed to prepare get_best_per_island");

    stmt.query_map(params![evo_id], |row| {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    })
    .expect("Failed to query best per island")
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

/// Delete an evolution and all its genotypes/tasks from the database.
/// Caller should set status to 'stopped' first so the coordinator exits.
pub fn delete_evolution(conn: &Connection, evo_id: i64) {
    conn.execute("DELETE FROM tasks WHERE evolution_id=?1", params![evo_id])
        .expect("Failed to delete tasks");
    conn.execute("DELETE FROM genotypes WHERE evolution_id=?1", params![evo_id])
        .expect("Failed to delete genotypes");
    conn.execute("DELETE FROM evolutions WHERE id=?1", params![evo_id])
        .expect("Failed to delete evolution");
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
    pub name: Option<String>,
}

/// List all evolutions.
pub fn list_evolutions(conn: &Connection) -> Vec<EvolutionRow> {
    let mut stmt = conn
        .prepare("SELECT id, config_json, status, current_gen, created_at, updated_at, name FROM evolutions ORDER BY id DESC")
        .expect("Failed to prepare list_evolutions");

    stmt.query_map([], |row| {
        Ok(EvolutionRow {
            id: row.get(0)?,
            config_json: row.get(1)?,
            status: row.get(2)?,
            current_gen: row.get(3)?,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
            name: row.get(6)?,
        })
    })
    .expect("Failed to list evolutions")
    .filter_map(|r| r.ok())
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn mem_db() -> Connection {
        // in-memory DB with the schema that init_db applies
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE evolutions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                config_json TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'running',
                current_gen INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                updated_at TEXT NOT NULL DEFAULT (datetime('now')),
                name TEXT,
                seed INTEGER
            );
            CREATE TABLE genotypes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                evolution_id INTEGER NOT NULL,
                generation INTEGER NOT NULL,
                genome_bytes BLOB NOT NULL,
                parent_id INTEGER,
                fitness REAL,
                created_at TEXT NOT NULL DEFAULT (datetime('now')),
                island_id INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE tasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                evolution_id INTEGER NOT NULL,
                genotype_id INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                worker_id TEXT,
                started_at TEXT,
                completed_at TEXT,
                fitness REAL
            );"
        ).unwrap();
        conn
    }

    #[test]
    fn island_fitness_query_returns_only_matching_island() {
        let conn = mem_db();
        let evo_id = create_evolution(&conn, "{}", None, None);
        let bytes = vec![0u8, 1, 2];
        // Insert 2 creatures in island 0 and 3 in island 1, all in gen 5.
        let a = insert_genotype_with_fitness(&conn, evo_id, 5, &bytes, 1.0, 0);
        let b = insert_genotype_with_fitness(&conn, evo_id, 5, &bytes, 2.0, 0);
        let c = insert_genotype_with_fitness(&conn, evo_id, 5, &bytes, 10.0, 1);
        let d = insert_genotype_with_fitness(&conn, evo_id, 5, &bytes, 20.0, 1);
        let e = insert_genotype_with_fitness(&conn, evo_id, 5, &bytes, 30.0, 1);
        let island0 = get_generation_fitnesses_by_island(&conn, evo_id, 0, 5);
        let island1 = get_generation_fitnesses_by_island(&conn, evo_id, 1, 5);
        assert_eq!(island0.len(), 2);
        assert_eq!(island1.len(), 3);
        let ids0: Vec<i64> = island0.iter().map(|(id, _)| *id).collect();
        assert!(ids0.contains(&a) && ids0.contains(&b));
        let ids1: Vec<i64> = island1.iter().map(|(id, _)| *id).collect();
        assert!(ids1.contains(&c) && ids1.contains(&d) && ids1.contains(&e));
        // And get_generation_fitnesses (all-islands) returns all 5.
        let all = get_generation_fitnesses(&conn, evo_id, 5);
        assert_eq!(all.len(), 5);
    }
}
