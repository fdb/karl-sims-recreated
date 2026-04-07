//! Lightweight DB profiling instrumentation.
//!
//! Every DB interaction has *three* latencies, and conflating them is the #1
//! reason pool/SQLite tuning goes wrong:
//!
//! 1. **dispatch** — for API handlers only: time from `tokio::task::spawn_blocking`
//!    being called until the closure actually starts running on a blocking thread.
//!    A high p99 here means tokio's blocking-pool is saturated (default 512 threads,
//!    hard to reach — but not impossible under lock storms).
//!
//! 2. **acquire** — time spent in `db.get()` waiting for an r2d2 pool connection.
//!    High p99 means another caller is holding connections too long, or the pool
//!    is undersized relative to demand.
//!
//! 3. **query** — time the SQL statement itself took once we had a connection.
//!    High p99 here is the interesting one: it either means the query is
//!    expensive (missing index, big BLOB), OR a writer is holding the writer
//!    lock, OR SQLite is checkpointing the WAL.
//!
//! All three are recorded per call-site label, and flushed as a p50/p99 table
//! every `REPORT_INTERVAL`. Samples are kept as raw u64 microseconds in a
//! per-label Vec; at the typical rate (a few thousand samples per 5s window)
//! the memory + sort overhead is negligible.

use r2d2::PooledConnection;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::db::DbPool;

/// How often the reporter prints its table.
const REPORT_INTERVAL: Duration = Duration::from_secs(5);
/// How often the WAL monitor logs the WAL file size.
const WAL_INTERVAL: Duration = Duration::from_secs(10);

/// One timed DB interaction. `dispatch_us=0` means the call was synchronous
/// (worker / coordinator) and had no spawn_blocking hop.
#[derive(Clone, Copy)]
struct Sample {
    dispatch_us: u64,
    acquire_us: u64,
    query_us: u64,
}

/// Global sample bucket, keyed by call-site label. `&'static str` keys mean
/// no allocation on the hot path — every call site passes a literal.
type Bucket = HashMap<&'static str, Vec<Sample>>;

fn bucket() -> &'static Mutex<Bucket> {
    static BUCKET: OnceLock<Mutex<Bucket>> = OnceLock::new();
    BUCKET.get_or_init(|| Mutex::new(HashMap::new()))
}

fn record(label: &'static str, dispatch: Duration, acquire: Duration, query: Duration) {
    let s = Sample {
        dispatch_us: dispatch.as_micros() as u64,
        acquire_us: acquire.as_micros() as u64,
        query_us: query.as_micros() as u64,
    };
    // Lock contention is not a concern — this runs ~10K/s at absolute peak,
    // and the critical section is a Vec push. Keeping it a plain Mutex avoids
    // pulling in dashmap / parking_lot just for profiling.
    let mut map = bucket().lock().unwrap();
    map.entry(label).or_default().push(s);
}

/// Synchronous DB call: measures `db.get()` then the closure.
/// Use this inside workers, the coordinator, and inside `spawn_blocking` blocks.
pub fn timed_db<F, T>(label: &'static str, db: &DbPool, f: F) -> T
where
    F: FnOnce(&Connection) -> T,
{
    let t_acq = Instant::now();
    let conn: PooledConnection<SqliteConnectionManager> = match db.get() {
        Ok(c) => c,
        Err(e) => {
            log::error!("[POOL EXHAUSTED] {label}: {e} (waited {:.1}s)",
                t_acq.elapsed().as_secs_f64());
            // Retry once after a short backoff — the pool may free up if a
            // long-running query (e.g. coord.load_gen on a bloated WAL) finishes.
            std::thread::sleep(Duration::from_millis(500));
            db.get().unwrap_or_else(|e2| {
                panic!("[POOL EXHAUSTED] {label}: retry also failed after {:.1}s: {e2}",
                    t_acq.elapsed().as_secs_f64())
            })
        }
    };
    let acquire = t_acq.elapsed();
    let t_query = Instant::now();
    let result = f(&conn);
    let query = t_query.elapsed();
    record(label, Duration::ZERO, acquire, query);
    result
}

/// Async DB call from an HTTP handler: measures spawn_blocking dispatch,
/// then `db.get()`, then the closure. Use for API handlers whose closure
/// does nothing but run a single DB function.
///
/// For handlers that do extra CPU work inside the closure (e.g. phenotype
/// development, JSON tree building), use `spawn_blocking` manually and call
/// `timed_db` inside — that way the extra work isn't charged as "query time".
pub async fn db_read_async<F, T>(label: &'static str, db: DbPool, f: F) -> T
where
    F: FnOnce(&Connection) -> T + Send + 'static,
    T: Send + 'static,
{
    let t_spawn = Instant::now();
    tokio::task::spawn_blocking(move || {
        let dispatch = t_spawn.elapsed();
        let t_acq = Instant::now();
        let conn = match db.get() {
            Ok(c) => c,
            Err(e) => {
                log::error!("[POOL EXHAUSTED] {label}: {e} (waited {:.1}s)",
                    t_acq.elapsed().as_secs_f64());
                std::thread::sleep(Duration::from_millis(500));
                db.get().unwrap_or_else(|e2| {
                    panic!("[POOL EXHAUSTED] {label}: retry also failed after {:.1}s: {e2}",
                        t_acq.elapsed().as_secs_f64())
                })
            }
        };
        let acquire = t_acq.elapsed();
        let t_query = Instant::now();
        let result = f(&conn);
        let query = t_query.elapsed();
        record(label, dispatch, acquire, query);
        result
    })
    .await
    .expect("spawn_blocking join")
}

/// Spawn the reporter task. Prints a p50/p99 table every `REPORT_INTERVAL`
/// and clears the bucket. Dropping the window-by-window model (e.g. EWMA)
/// keeps the report readable: each table shows exactly the last 5s.
pub fn spawn_reporter() {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(REPORT_INTERVAL).await;
            flush_report();
        }
    });
}

fn flush_report() {
    // Swap the bucket out under the lock so we can sort/print without holding
    // the lock across potentially-slow log I/O.
    let taken: Bucket = {
        let mut map = bucket().lock().unwrap();
        std::mem::take(&mut *map)
    };
    if taken.is_empty() {
        return;
    }

    // Sort labels so successive reports are diff-able by eye.
    let mut labels: Vec<&'static str> = taken.keys().copied().collect();
    labels.sort();

    let mut lines = Vec::with_capacity(labels.len() + 1);
    lines.push(format!(
        "[DB STATS / {}s]  {:<32} {:>6}  {:>15}  {:>15}  {:>15}",
        REPORT_INTERVAL.as_secs(),
        "label",
        "n",
        "dispatch p50/p99",
        "acquire  p50/p99",
        "query    p50/p99",
    ));
    for label in labels {
        let samples = &taken[label];
        let n = samples.len();
        let dispatch = percentiles(samples, |s| s.dispatch_us);
        let acquire = percentiles(samples, |s| s.acquire_us);
        let query = percentiles(samples, |s| s.query_us);
        lines.push(format!(
            "                  {:<32} {:>6}  {:>15}  {:>15}  {:>15}",
            label,
            n,
            fmt_pct(dispatch),
            fmt_pct(acquire),
            fmt_pct(query),
        ));
    }
    // Single multi-line log entry so lines stay together under concurrent logging.
    log::info!("\n{}", lines.join("\n"));
}

fn percentiles(samples: &[Sample], field: impl Fn(&Sample) -> u64) -> Option<(u64, u64)> {
    if samples.is_empty() {
        return None;
    }
    let mut values: Vec<u64> = samples.iter().map(&field).collect();
    values.sort_unstable();
    let p50 = values[values.len() / 2];
    // p99 with a floor at the last index so a window of <100 samples still
    // returns the tail value rather than silently collapsing to p50.
    let p99_idx = ((values.len() as f64 * 0.99) as usize).min(values.len() - 1);
    let p99 = values[p99_idx];
    Some((p50, p99))
}

fn fmt_pct(pct: Option<(u64, u64)>) -> String {
    match pct {
        // If p50 AND p99 are both zero, the field was never populated (e.g.
        // dispatch for sync call sites) — show a dash so the reader isn't
        // misled into thinking we measured something.
        Some((0, 0)) => "-".to_string(),
        Some((p50, p99)) => format!("{:.2}/{:.2} ms", p50 as f64 / 1000.0, p99 as f64 / 1000.0),
        None => "-".to_string(),
    }
}

/// Spawn a thread that monitors the WAL file size and runs periodic
/// PASSIVE checkpoints. Under sustained write + read load, SQLite's
/// autocheckpoint can't run because readers hold WAL snapshots (e.g.
/// `coord.load_gen` holding a connection for 8-18s). Without explicit
/// checkpointing, the WAL grows unboundedly (observed: 619 MB), making
/// every read scan hundreds of MB of WAL frames — a death spiral.
///
/// PASSIVE checkpoint moves committed WAL pages to the main DB file
/// without blocking writers or waiting for readers. It won't shrink the
/// WAL to zero if there are active readers, but it prevents unbounded
/// growth by checkpointing whatever frames are no longer pinned.
///
/// Every `CHECKPOINT_INTERVAL` we run a PASSIVE checkpoint. Every
/// `WAL_INTERVAL` we also log the WAL size for monitoring. Every
/// `TRUNCATE_INTERVAL` we attempt a TRUNCATE checkpoint to actually
/// reclaim disk space — this may fail if readers are active, which is
/// fine (PASSIVE keeps the WAL bounded in the meantime).
const CHECKPOINT_INTERVAL: Duration = Duration::from_secs(5);
const TRUNCATE_INTERVAL: Duration = Duration::from_secs(60);

pub fn spawn_wal_monitor(db_path: String) {
    std::thread::spawn(move || {
        let wal_path = format!("{db_path}-wal");
        // Dedicated connection for checkpointing — not from the pool, so it
        // doesn't compete with workers/coordinator for pool slots.
        let ckpt_conn = match rusqlite::Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => {
                log::error!("[WAL] Failed to open checkpoint connection: {e}");
                return;
            }
        };
        // WAL mode + short busy timeout (we don't want to block if a writer is active)
        ckpt_conn
            .execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=1000;")
            .ok();

        let mut last_log = Instant::now();
        let mut last_truncate = Instant::now();
        loop {
            std::thread::sleep(CHECKPOINT_INTERVAL);

            // Run PASSIVE checkpoint — never blocks.
            match ckpt_conn.query_row(
                "PRAGMA wal_checkpoint(PASSIVE)",
                [],
                |row| {
                    Ok((
                        row.get::<_, i32>(0)?, // busy (0=ok, 1=blocked)
                        row.get::<_, i32>(1)?, // total WAL pages
                        row.get::<_, i32>(2)?, // checkpointed pages
                    ))
                },
            ) {
                Ok((_busy, total, checkpointed)) => {
                    if total > 0 && total != checkpointed {
                        log::debug!(
                            "[WAL] PASSIVE: {checkpointed}/{total} pages checkpointed"
                        );
                    }
                }
                Err(e) => log::warn!("[WAL] PASSIVE checkpoint failed: {e}"),
            }

            // Periodically attempt TRUNCATE to reclaim disk space.
            if last_truncate.elapsed() >= TRUNCATE_INTERVAL {
                last_truncate = Instant::now();
                match ckpt_conn.query_row(
                    "PRAGMA wal_checkpoint(TRUNCATE)",
                    [],
                    |row| Ok(row.get::<_, i32>(0)?),
                ) {
                    Ok(0) => log::info!("[WAL] TRUNCATE checkpoint succeeded"),
                    Ok(_) => log::debug!("[WAL] TRUNCATE blocked by readers (OK, PASSIVE keeps WAL bounded)"),
                    Err(e) => log::warn!("[WAL] TRUNCATE checkpoint failed: {e}"),
                }
            }

            // Log WAL size periodically.
            if last_log.elapsed() >= WAL_INTERVAL {
                last_log = Instant::now();
                if let Ok(meta) = std::fs::metadata(&wal_path) {
                    let mb = meta.len() as f64 / 1_048_576.0;
                    if mb > 10.0 {
                        log::warn!("[WAL] {wal_path}: {mb:.2} MB (elevated)");
                    } else {
                        log::info!("[WAL] {wal_path}: {mb:.2} MB");
                    }
                }
            }
        }
    });
}
