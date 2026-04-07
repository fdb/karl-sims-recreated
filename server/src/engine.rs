//! In-memory evolution engine.
//!
//! All hot state lives here — no DB access on the critical path.
//! Workers receive tasks via crossbeam channels, coordinators keep
//! current-generation individuals in memory, and API handlers read
//! snapshots from this module.  SQLite is relegated to write-behind
//! archival (for crash recovery and creature viewer).

use std::collections::HashMap;
use std::sync::RwLock;

use crossbeam_channel::{Receiver, Sender};
use serde::Serialize;

// ── Task channel types ──────────────────────────────────────────────────

/// A fitness evaluation job sent from coordinator to worker pool.
pub struct EvalTask {
    /// Opaque index for matching results back to the originating creature.
    /// Workers echo this back in EvalResult so the coordinator can assign
    /// fitness to the correct individual regardless of arrival order.
    pub task_index: usize,
    /// Serialized genome (bincode).
    pub genome_bytes: Vec<u8>,
    /// Serialized EvolutionParams (JSON).
    pub config_json: String,
    /// Channel to send the result back to the originating coordinator.
    /// Each coordinator creates its own result channel per generation,
    /// so results are automatically routed to the right evolution.
    pub result_tx: Sender<EvalResult>,
}

/// Fitness result sent from worker back to coordinator.
pub struct EvalResult {
    /// Echoed from EvalTask — identifies which creature this result belongs to.
    pub task_index: usize,
    pub fitness: f64,
}

// ── Snapshot types (served to API handlers) ─────────────────────────────

#[derive(Clone, Serialize)]
pub struct CreatureSnapshot {
    pub id: i64,
    pub fitness: f64,
    pub island_id: i64,
}

#[derive(Clone, Serialize)]
pub struct GenStatSnapshot {
    pub generation: i64,
    pub best_fitness: f64,
    pub avg_fitness: f64,
}

#[derive(Clone, Serialize)]
pub struct IslandStatSnapshot {
    pub generation: i64,
    pub island_id: i64,
    pub best_fitness: f64,
    pub avg_fitness: f64,
}

/// In-memory snapshot of a single evolution, updated by the coordinator
/// after each generation.  API handlers clone what they need under a
/// brief read-lock — no DB round-trip.
#[derive(Clone)]
pub struct EvolutionSnapshot {
    pub id: i64,
    pub name: Option<String>,
    pub status: String,
    pub current_gen: i64,
    pub config_json: String,
    pub created_at: String,
    /// Top 10 creatures by fitness (all-time).
    pub best_creatures: Vec<CreatureSnapshot>,
    /// Best creature per island (current gen).
    pub best_per_island: Vec<CreatureSnapshot>,
    /// Per-generation aggregated stats (for fitness chart).
    pub gen_stats: Vec<GenStatSnapshot>,
    /// Per-island per-generation stats.
    pub island_stats: Vec<IslandStatSnapshot>,
}

// ── Engine ───────────────────────────────────────────────────────────────

/// Central in-memory state.  Shared via `Arc<Engine>` across coordinator
/// tasks, API handlers, and the main function.
pub struct Engine {
    /// Coordinator → worker task channel (MPMC: workers clone the Receiver).
    pub task_tx: Sender<EvalTask>,
    pub task_rx: Receiver<EvalTask>,

    /// Per-evolution snapshots for zero-DB API reads.
    pub snapshots: RwLock<HashMap<i64, EvolutionSnapshot>>,
}

impl Engine {
    pub fn new() -> Self {
        let (task_tx, task_rx) = crossbeam_channel::unbounded();
        Self {
            task_tx,
            task_rx,
            snapshots: RwLock::new(HashMap::new()),
        }
    }

    /// Insert or update a snapshot.  Called by the coordinator after each
    /// generation completes.
    pub fn update_snapshot(&self, snap: EvolutionSnapshot) {
        let mut map = self.snapshots.write().unwrap();
        map.insert(snap.id, snap);
    }

    /// Update just the status field (for stop/pause/resume from API).
    pub fn set_status(&self, evo_id: i64, status: &str) {
        let mut map = self.snapshots.write().unwrap();
        if let Some(snap) = map.get_mut(&evo_id) {
            snap.status = status.to_string();
        }
    }

    /// Remove an evolution snapshot (for delete).
    pub fn remove_snapshot(&self, evo_id: i64) {
        let mut map = self.snapshots.write().unwrap();
        map.remove(&evo_id);
    }

    /// Read-only access to all snapshots (for list endpoint).
    pub fn list_snapshots(&self) -> Vec<EvolutionSnapshot> {
        let map = self.snapshots.read().unwrap();
        let mut snaps: Vec<_> = map.values().cloned().collect();
        snaps.sort_by(|a, b| b.id.cmp(&a.id)); // newest first
        snaps
    }

    /// Read-only access to one snapshot.
    pub fn get_snapshot(&self, evo_id: i64) -> Option<EvolutionSnapshot> {
        let map = self.snapshots.read().unwrap();
        map.get(&evo_id).cloned()
    }
}
