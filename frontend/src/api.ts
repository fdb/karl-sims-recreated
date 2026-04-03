const API_BASE = "http://localhost:3000";

export interface Evolution {
  id: number;
  status: string;
  generation: number;
}

export interface CreatureInfo {
  id: number;
  fitness: number;
}

export interface GenerationStats {
  evolution_id: number;
  generation: number;
  best_fitness: number;
  avg_fitness: number;
}

export async function listEvolutions(): Promise<Evolution[]> {
  const res = await fetch(`${API_BASE}/api/evolutions`);
  return res.json();
}

export async function createEvolution(
  populationSize: number = 50,
): Promise<{ id: number }> {
  const res = await fetch(`${API_BASE}/api/evolutions`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ population_size: populationSize }),
  });
  return res.json();
}

export async function getEvolution(id: number): Promise<Evolution> {
  const res = await fetch(`${API_BASE}/api/evolutions/${id}`);
  return res.json();
}

export async function getBestCreatures(
  evoId: number,
): Promise<CreatureInfo[]> {
  const res = await fetch(`${API_BASE}/api/evolutions/${evoId}/best`);
  return res.json();
}

export async function stopEvolution(id: number): Promise<void> {
  await fetch(`${API_BASE}/api/evolutions/${id}/stop`, { method: "POST" });
}
