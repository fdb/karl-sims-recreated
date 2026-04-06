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
    let conn: PooledConnection<SqliteConnectionManager> = db.get().expect("pool get");
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
        let conn = db.get().expect("pool get");
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

/// Spawn a thread that logs the WAL file size every `WAL_INTERVAL`. If the
/// WAL grows unboundedly, autocheckpoint is falling behind — a classic
/// symptom of sustained write pressure. Under healthy operation WAL stays
/// under a few MB and resets on checkpoint.
pub fn spawn_wal_monitor(db_path: String) {
    std::thread::spawn(move || {
        let wal_path = format!("{db_path}-wal");
        loop {
            std::thread::sleep(WAL_INTERVAL);
            if let Ok(meta) = std::fs::metadata(&wal_path) {
                let mb = meta.len() as f64 / 1_048_576.0;
                log::info!("[WAL] {wal_path}: {mb:.2} MB");
            }
        }
    });
}
