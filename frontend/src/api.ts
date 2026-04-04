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

export interface GenotypeInfo {
  id: number;
  num_nodes: number;
  num_connections: number;
  nodes: {
    id: number;
    dimensions: [number, number, number];
    joint_type: string;
    recursive_limit: number;
    terminal_only: boolean;
    brain: {
      num_neurons: number;
      num_effectors: number;
      neurons: {
        id: number;
        func: string;
        inputs: { source: string; weight: number }[];
      }[];
    };
  }[];
  connections: {
    source: number;
    target: number;
    parent_face: string;
    child_face: string;
    scale: number;
    reflection: boolean;
  }[];
}

export async function getGenotypeInfo(id: number): Promise<GenotypeInfo> {
  const res = await fetch(`${API_BASE}/api/genotypes/${id}`);
  return res.json();
}

export async function getGenomeBytes(id: number): Promise<ArrayBuffer> {
  const res = await fetch(`${API_BASE}/api/genotypes/${id}/genome`);
  return res.arrayBuffer();
}
