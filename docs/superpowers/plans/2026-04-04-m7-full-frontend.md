# M7: Full Frontend — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** React dashboard for managing evolution runs: start/stop evolutions, view fitness-over-generations charts, browse creature gallery, replay best creatures in 3D, and watch live evolution progress via WebSocket.

**Architecture:** The existing React+Vite frontend is extended with new pages/components. The server REST API provides evolution data. WebSocket provides live generation updates. The WASM wgpu renderer displays selected creatures. A simple client-side router (or tab UI) switches between dashboard and viewer.

**Tech Stack:** React 19, TypeScript, Vite, existing WASM wgpu renderer, server REST API

---

## Task 1: Evolution Dashboard + API Client

**Files:**
- Create: `frontend/src/api.ts` — typed API client
- Create: `frontend/src/pages/Dashboard.tsx` — evolution list, create/stop
- Modify: `frontend/src/App.tsx` — tab navigation between dashboard and sim viewer

The dashboard shows:
- List of evolution runs (from GET /api/evolutions) with status, generation, created_at
- "New Evolution" button that POSTs to create one
- "Stop" button per running evolution
- Click an evolution to switch to the viewer tab showing the best creature

### api.ts

```typescript
const BASE = "";  // same origin

export interface Evolution {
  id: number;
  status: string;
  generation: number;
  config: string;
  created_at: string;
}

export interface CreatureInfo {
  id: number;
  fitness: number;
}

export async function listEvolutions(): Promise<Evolution[]> {
  const res = await fetch(`${BASE}/api/evolutions`);
  return res.json();
}

export async function createEvolution(populationSize: number): Promise<{ id: number }> {
  const res = await fetch(`${BASE}/api/evolutions`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ population_size: populationSize }),
  });
  return res.json();
}

export async function getEvolution(id: number): Promise<Evolution> {
  const res = await fetch(`${BASE}/api/evolutions/${id}`);
  return res.json();
}

export async function getBestCreatures(id: number): Promise<CreatureInfo[]> {
  const res = await fetch(`${BASE}/api/evolutions/${id}/best`);
  return res.json();
}

export async function stopEvolution(id: number): Promise<void> {
  await fetch(`${BASE}/api/evolutions/${id}/stop`, { method: "POST" });
}
```

- [ ] **Step 1: Create api.ts, Dashboard.tsx, update App.tsx with tab navigation**
- [ ] **Step 2: Verify with `npm run dev` pointing at running server**
- [ ] **Step 3: Commit**

---

## Task 2: Fitness Chart + Live WebSocket

**Files:**
- Create: `frontend/src/components/FitnessChart.tsx` — SVG line chart
- Create: `frontend/src/hooks/useWebSocket.ts` — WS hook for live updates
- Modify: `frontend/src/pages/Dashboard.tsx` — add chart and live status

A simple SVG-based line chart showing best/avg fitness per generation. No external charting library — just SVG paths.

WebSocket hook connects to `/api/live` and accumulates generation stats.

- [ ] **Step 1: Create FitnessChart.tsx (SVG line chart) and useWebSocket.ts**
- [ ] **Step 2: Integrate into Dashboard**
- [ ] **Step 3: Commit**

---

## Task 3: Creature Viewer Integration

**Files:**
- Modify: `frontend/src/App.tsx` — creature viewer tab loads best creature from server
- Modify: `frontend/src/wasm.ts` — add function to load creature from genome bytes
- Modify: `web/src/lib.rs` — add `load_creature_genome(bytes)` WASM function

When a user clicks a creature in the dashboard, the viewer tab loads its genome from the server, passes it to the WASM module, which grows the creature and starts simulating it.

New WASM export:
```rust
#[wasm_bindgen]
pub fn load_creature_genome(genome_bytes: &[u8]) {
    // Deserialize genome, create Creature, store in AppState
}
```

The API needs to return genome bytes for a specific creature. Add endpoint:
```
GET /api/evolutions/:id/creatures/:creature_id/genome → binary blob
```

- [ ] **Step 1: Add genome download endpoint to server**
- [ ] **Step 2: Add load_creature_genome to web crate**
- [ ] **Step 3: Wire creature loading in frontend**
- [ ] **Step 4: Build and test full flow**
- [ ] **Step 5: Commit**

---

## Self-Review

- [x] Evolution dashboard: list/create/stop → Task 1
- [x] Fitness chart → Task 2
- [x] WebSocket live viewer → Task 2
- [x] Creature viewer (3D playback) → Task 3
- [ ] Creature gallery with animated thumbnails → Deferred (requires multiple simultaneous renderers)
- [ ] Genotype/brain visualization → Deferred (graph layout is complex, not critical for MVP)
- [ ] In-browser sandbox → Already exists from M5 (mini evolution scene)
