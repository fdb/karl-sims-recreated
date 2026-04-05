import { useEffect, useState } from "react";
import {
  getGenotypeInfo,
  getGenomeBytes,
  getBestCreatures,
  getEvolution,
  getPhenotypeInfo,
  type GenotypeInfo,
  type CreatureInfo,
  type PhenotypeInfo,
} from "../api";
import { Link } from "@tanstack/react-router";
import MorphologyGraph from "../components/MorphologyGraph";
import PhenotypeGraph from "../components/PhenotypeGraph";
import CreatureViewer from "../components/CreatureViewer";

function formatFitness(v: number): string {
  if (!isFinite(v)) return "—";
  const abs = Math.abs(v);
  if (abs === 0) return "0";
  if (abs >= 1e6) return v.toExponential(2);
  if (abs >= 1) return v.toFixed(2);
  if (abs >= 0.01) return v.toFixed(4);
  return v.toExponential(2);
}

interface Props {
  evoId: number;
  creatureId: number;
  islandId?: number;
}

const ISLAND_COLORS = [
  "#4a9eff", "#ff7a45", "#52c41a", "#ff4d6d",
  "#a855f7", "#14b8a6", "#eab308", "#ec4899",
  "#06b6d4", "#f97316", "#84cc16", "#8b5cf6",
];

export default function CreatureDetail({ evoId, creatureId, islandId }: Props) {
  const [info, setInfo] = useState<GenotypeInfo | null>(null);
  const [phenotype, setPhenotype] = useState<PhenotypeInfo | null>(null);
  const [creature, setCreature] = useState<CreatureInfo | null>(null);
  const [genomeBytes, setGenomeBytes] = useState<Uint8Array | null>(null);
  const [environment, setEnvironment] = useState<"Water" | "Land">("Water");
  const [goal, setGoal] = useState<"SwimmingSpeed" | "LightFollowing">("SwimmingSpeed");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      setLoading(true);
      setGenomeBytes(null);
      try {
        const [creatures, evo, genoInfo, phenoInfo, bytes] = await Promise.all([
          getBestCreatures(evoId),
          getEvolution(evoId),
          getGenotypeInfo(creatureId),
          getPhenotypeInfo(creatureId),
          getGenomeBytes(creatureId),
        ]);

        const c = creatures.find((c) => c.id === creatureId);
        if (c) setCreature(c);
        if (evo.config?.environment) setEnvironment(evo.config.environment);
        if (evo.config?.goal) setGoal(evo.config.goal);
        setInfo(genoInfo);
        setPhenotype(phenoInfo);
        setGenomeBytes(new Uint8Array(bytes));
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    })();
  }, [evoId, creatureId]);

  return (
    <div>
      {/* Breadcrumb */}
      <div className="flex items-center gap-2 text-sm text-text-muted mb-4">
        <Link to="/" className="hover:text-text-secondary transition-colors">
          Dashboard
        </Link>
        <span>/</span>
        <Link
          to="/evolutions/$evoId"
          params={{ evoId: String(evoId) }}
          className="hover:text-text-secondary transition-colors"
        >
          Evolution #{evoId}
        </Link>
        {islandId !== undefined && (
          <>
            <span>/</span>
            <span
              className="inline-flex items-center gap-1.5"
              style={{ color: ISLAND_COLORS[islandId % ISLAND_COLORS.length] }}
            >
              <span
                className="inline-block w-2 h-2 rounded-full"
                style={{
                  background: ISLAND_COLORS[islandId % ISLAND_COLORS.length],
                }}
              />
              Island {islandId}
            </span>
          </>
        )}
        <span>/</span>
        <span className="text-text-secondary">Creature #{creatureId}</span>
      </div>

      <div className="flex items-center gap-4 mb-6">
        <h1 className="text-2xl font-semibold">Creature #{creatureId}</h1>
        {creature && (
          <span className="text-success text-lg font-mono">
            Fitness: {formatFitness(creature.fitness)}
          </span>
        )}
      </div>

      {error && <p className="text-danger mb-4">Error: {error}</p>}

      {/* Two-column: 3D viewer (large) + genome info (sidebar) */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="lg:col-span-2">
          <div className="bg-bg-surface border border-border-subtle rounded-lg overflow-hidden aspect-[4/3] sm:aspect-[3/2] relative">
            {loading && (
              <div className="absolute inset-0 flex items-center justify-center z-10">
                <p className="text-text-muted text-sm">Loading creature...</p>
              </div>
            )}
            {genomeBytes && <CreatureViewer genomeBytes={genomeBytes} environment={environment} goal={goal} />}
          </div>
        </div>
        <div className="lg:col-span-1 space-y-4 lg:overflow-y-auto lg:max-h-[calc(100vh-200px)]">
          {info && (
            <div className="bg-bg-surface border border-border-subtle rounded-lg p-4">
              <h2 className="text-sm font-medium text-text-secondary mb-1">
                Genotype Graph
              </h2>
              <p className="text-xs text-text-muted mb-3">
                {info.num_nodes} nodes · {info.num_connections} connections ·
                neurons inlined as colored chips
              </p>
              <MorphologyGraph info={info} />
            </div>
          )}
          {phenotype && (
            <div className="bg-bg-surface border border-border-subtle rounded-lg p-4">
              <h2 className="text-sm font-medium text-text-secondary mb-1">
                Phenotype Graph
              </h2>
              <p className="text-xs text-text-muted mb-3">
                {phenotype.num_bodies} bodies · {phenotype.num_joints} joints
                — after BFS expansion with recursion / terminal pruning
              </p>
              <PhenotypeGraph info={phenotype} />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
