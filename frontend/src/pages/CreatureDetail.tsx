import { useEffect, useRef, useState } from "react";
import {
  getGenotypeInfo,
  getGenomeBytes,
  getBestCreatures,
  type GenotypeInfo,
  type CreatureInfo,
} from "../api";
import { navigate } from "../router";
import { load_creature_from_bytes, clear_scene } from "../wasm";
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
  const viewerRef = useRef<HTMLDivElement>(null);
  const genomeBytesRef = useRef<Uint8Array | null>(null);

  // Reparent the canvas into the viewer container
  useEffect(() => {
    const canvas = document.getElementById("sim-canvas");
    const container = viewerRef.current;
    if (!canvas || !container) return;

    // Clear scene first (show empty background while loading)
    clear_scene();

    // Move canvas into viewer container
    const originalParent = canvas.parentElement;
    container.appendChild(canvas);
    canvas.style.width = "100%";
    canvas.style.height = "100%";
    canvas.style.borderRadius = "0.375rem";

    return () => {
      // Move canvas back when leaving this page
      if (originalParent) {
        originalParent.appendChild(canvas);
        canvas.style.width = "";
        canvas.style.height = "";
      }
    };
  }, []);

  // Load creature data
  useEffect(() => {
    (async () => {
      setLoading(true);
      try {
        const creatures = await getBestCreatures(evoId);
        const c = creatures.find((c) => c.id === creatureId);
        if (c) setCreature(c);

        const genoInfo = await getGenotypeInfo(creatureId);
        setInfo(genoInfo);

        const bytes = await getGenomeBytes(creatureId);
        const genomeBytes = new Uint8Array(bytes);
        genomeBytesRef.current = genomeBytes;
        load_creature_from_bytes(genomeBytes);
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
            Fitness: {creature.fitness.toFixed(4)}
          </span>
        )}
        {!loading && genomeBytesRef.current && (
          <button
            onClick={() => {
              if (genomeBytesRef.current) {
                load_creature_from_bytes(genomeBytesRef.current);
              }
            }}
            className="px-3 py-1.5 text-sm bg-bg-surface border border-border rounded-md hover:bg-bg-elevated transition-colors"
          >
            Restart
          </button>
        )}
      </div>

      {error && <p className="text-danger mb-4">Error: {error}</p>}

      {/* Two-column: 3D viewer (large) + genome info (sidebar) */}
      <div className="grid grid-cols-3 gap-6">
        <div className="col-span-2">
          <div
            ref={viewerRef}
            className="bg-bg-surface border border-border-subtle rounded-lg overflow-hidden aspect-[3/2] relative"
          >
            {loading && (
              <div className="absolute inset-0 flex items-center justify-center z-10">
                <p className="text-text-muted text-sm">Loading creature...</p>
              </div>
            )}
          </div>
        </div>
        <div className="col-span-1 space-y-4 overflow-y-auto max-h-[calc(100vh-200px)]">
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
