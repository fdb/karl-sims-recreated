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
import { navigate } from "../router";
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
}

export default function CreatureDetail({ evoId, creatureId }: Props) {
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
        <a
          href="/"
          onClick={(e) => {
            e.preventDefault();
            navigate("/");
          }}
          className="hover:text-text-secondary transition-colors"
        >
          Dashboard
        </a>
        <span>/</span>
        <a
          href={`/evolutions/${evoId}`}
          onClick={(e) => {
            e.preventDefault();
            navigate(`/evolutions/${evoId}`);
          }}
          className="hover:text-text-secondary transition-colors"
        >
          Evolution #{evoId}
        </a>
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
