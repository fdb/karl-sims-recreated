const API_BASE = "";

export interface EvolutionConfig {
  population_size: number;
  max_generations: number;
  goal: "SwimmingSpeed" | "LightFollowing";
  environment: "Water" | "Land";
  sim_duration: number;
  max_parts: number;
  num_islands?: number;
  migration_interval?: number;
  solver_iterations?: number;
  pgs_iterations?: number;
  friction_coefficient?: number;
  use_coulomb_friction?: boolean;
  friction_combine_max?: boolean;
  airtime_penalty?: number;
  island_strategy?: "Isolated" | "RingMigration" | "HFC";
  exchange_interval?: number;
  diversity_pressure?: number;
}

/** Extract physics solver config from EvolutionConfig for passing to WASM sim_init. */
export function physicsConfigJson(config?: EvolutionConfig): string | undefined {
  if (!config) return undefined;
  const {
    solver_iterations, pgs_iterations, friction_coefficient,
    use_coulomb_friction, friction_combine_max,
  } = config;
  // Only produce JSON if at least one field is set
  if (solver_iterations == null && pgs_iterations == null &&
      friction_coefficient == null && use_coulomb_friction == null &&
      friction_combine_max == null) {
    return undefined;
  }
  return JSON.stringify({
    solver_iterations, pgs_iterations, friction_coefficient,
    use_coulomb_friction, friction_combine_max,
  });
}

export interface Evolution {
  id: number;
  status: string;
  generation: number;
  config?: EvolutionConfig;
  name?: string;
}

export interface CreatureInfo {
  id: number;
  fitness: number;
  island_id?: number;
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

export interface CreateEvolutionParams {
  population_size: number;
  generations: number;
  goal: string;
  environment: string;
  sim_duration: number;
  max_parts: number;
  gravity?: number;
  water_viscosity?: number;
  num_islands?: number;
  migration_interval?: number;
  min_joint_motion?: number;
  max_joint_angular_velocity?: number;
  solver_iterations?: number;
  pgs_iterations?: number;
  friction_coefficient?: number;
  use_coulomb_friction?: boolean;
  friction_combine_max?: boolean;
  airtime_penalty?: number;
  island_strategy?: string;
  exchange_interval?: number;
  diversity_pressure?: number;
  name?: string;
}

export async function createEvolution(
  params: CreateEvolutionParams,
): Promise<{ id: number }> {
  const res = await fetch(`${API_BASE}/api/evolutions`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(params),
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

export async function getBestPerIsland(
  evoId: number,
): Promise<CreatureInfo[]> {
  const res = await fetch(`${API_BASE}/api/evolutions/${evoId}/best_per_island`);
  return res.json();
}

export async function getEvolutionStats(
  evoId: number,
): Promise<GenerationStats[]> {
  const res = await fetch(`${API_BASE}/api/evolutions/${evoId}/stats`);
  const data = await res.json();
  return data.map((d: { generation: number; best_fitness: number; avg_fitness: number }) => ({
    evolution_id: evoId,
    generation: d.generation,
    best_fitness: d.best_fitness,
    avg_fitness: d.avg_fitness,
  }));
}

export interface IslandStats {
  generation: number;
  island_id: number;
  best_fitness: number;
  avg_fitness: number;
}

export async function getIslandStats(evoId: number): Promise<IslandStats[]> {
  const res = await fetch(`${API_BASE}/api/evolutions/${evoId}/island_stats`);
  return res.json();
}

export async function stopEvolution(id: number): Promise<void> {
  await fetch(`${API_BASE}/api/evolutions/${id}/stop`, { method: "POST" });
}

export async function pauseEvolution(id: number): Promise<void> {
  await fetch(`${API_BASE}/api/evolutions/${id}/pause`, { method: "POST" });
}

export async function resumeEvolution(id: number): Promise<void> {
  await fetch(`${API_BASE}/api/evolutions/${id}/resume`, { method: "POST" });
}

export async function replayEvolution(id: number): Promise<{ id: number }> {
  const res = await fetch(`${API_BASE}/api/evolutions/${id}/replay`, {
    method: "POST",
  });
  return res.json();
}

export async function deleteEvolution(id: number): Promise<void> {
  await fetch(`${API_BASE}/api/evolutions/${id}`, { method: "DELETE" });
}

export async function updateEvolutionName(id: number, name: string): Promise<void> {
  await fetch(`${API_BASE}/api/evolutions/${id}`, {
    method: "PATCH",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ name }),
  });
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

export interface PhenotypeInfo {
  id: number;
  num_bodies: number;
  num_joints: number;
  root: number;
  bodies: {
    id: number;
    genome_node: number;
    depth: number;
    half_extents: [number, number, number];
    joint_type: string;
  }[];
  joints: {
    parent: number;
    child: number;
    joint_type: string;
  }[];
}

export async function getPhenotypeInfo(id: number): Promise<PhenotypeInfo> {
  const res = await fetch(`${API_BASE}/api/genotypes/${id}/phenotype`);
  return res.json();
}
