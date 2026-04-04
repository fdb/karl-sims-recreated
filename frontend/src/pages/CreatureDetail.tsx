import { useEffect, useState } from "react";
import {
  getGenotypeInfo,
  getGenomeBytes,
  getBestCreatures,
  type GenotypeInfo,
  type CreatureInfo,
} from "../api";
import { navigate } from "../router";
import { load_creature_from_bytes } from "../wasm";
import MorphologyGraph from "../components/MorphologyGraph";
import BrainGraph from "../components/BrainGraph";

interface Props {
  evoId: number;
  creatureId: number;
}

export default function CreatureDetail({ evoId, creatureId }: Props) {
  const [info, setInfo] = useState<GenotypeInfo | null>(null);
  const [creature, setCreature] = useState<CreatureInfo | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      setLoading(true);
      try {
        // Get creature fitness info
        const creatures = await getBestCreatures(evoId);
        const c = creatures.find((c) => c.id === creatureId);
        if (c) setCreature(c);

        // Get genotype structure for visualization
        const genoInfo = await getGenotypeInfo(creatureId);
        setInfo(genoInfo);

        // Load genome bytes into WASM for 3D rendering
        const bytes = await getGenomeBytes(creatureId);
        load_creature_from_bytes(new Uint8Array(bytes));
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
          className="hover:text-text-secondary transition-colors no-underline text-inherit"
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
          className="hover:text-text-secondary transition-colors no-underline text-inherit"
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
            Fitness: {creature.fitness.toFixed(4)}
          </span>
        )}
      </div>

      {loading && (
        <p className="text-text-muted">Loading creature...</p>
      )}
      {error && <p className="text-danger">Error: {error}</p>}

      {/* Two-column: 3D viewer (large) + genome info (sidebar) */}
      <div className="grid grid-cols-3 gap-6">
        <div className="col-span-2">
          <div className="bg-bg-surface border border-border-subtle rounded-lg p-2 aspect-[3/2] flex items-center justify-center">
            <p className="text-text-muted text-sm">3D Viewer</p>
          </div>
        </div>
        <div className="col-span-1 space-y-4">
          {info && (
            <>
              <div className="bg-bg-surface border border-border-subtle rounded-lg p-4">
                <h2 className="text-sm font-medium text-text-secondary mb-3">
                  Morphology Graph
                </h2>
                <MorphologyGraph info={info} />
              </div>
              <div className="bg-bg-surface border border-border-subtle rounded-lg p-4">
                <h2 className="text-sm font-medium text-text-secondary mb-3">
                  Neural Network
                </h2>
                <BrainGraph info={info} />
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
