import { useEffect, useState } from "react";
import { getGenotypeInfo, getGenomeBytes, getBestCreatures, type GenotypeInfo, type CreatureInfo } from "../api";
import { navigate } from "../router";
import { load_creature_from_bytes } from "../wasm";
import MorphologyGraph from "../components/MorphologyGraph";
import BrainGraph from "../components/BrainGraph";

interface Props {
  evoId: number;
  creatureId: number;
  canvasVisible: boolean;
  onShowCanvas: () => void;
}

export default function CreatureDetail({ evoId, creatureId, canvasVisible, onShowCanvas }: Props) {
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
        const c = creatures.find(c => c.id === creatureId);
        if (c) setCreature(c);

        // Get genotype structure for visualization
        const genoInfo = await getGenotypeInfo(creatureId);
        setInfo(genoInfo);

        // Load genome bytes into WASM for 3D rendering
        const bytes = await getGenomeBytes(creatureId);
        load_creature_from_bytes(new Uint8Array(bytes));
        onShowCanvas();
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    })();
  }, [evoId, creatureId]);

  return (
    <div className="creature-detail">
      <div className="breadcrumb">
        <a href="/" onClick={e => { e.preventDefault(); navigate("/"); }}>Dashboard</a>
        <span>/</span>
        <a href={`/evolutions/${evoId}`} onClick={e => { e.preventDefault(); navigate(`/evolutions/${evoId}`); }}>
          Evolution #{evoId}
        </a>
        <span>/</span>
        <span>Creature #{creatureId}</span>
      </div>

      <h2>Creature #{creatureId}</h2>
      {creature && <p className="fitness-score">Fitness: {creature.fitness.toFixed(4)}</p>}

      {loading && <p>Loading creature...</p>}
      {error && <p className="error">Error: {error}</p>}

      {info && (
        <div className="genome-panels">
          <div className="genome-panel">
            <h3>Morphology Graph</h3>
            <MorphologyGraph info={info} />
          </div>
          <div className="genome-panel">
            <h3>Neural Network</h3>
            <BrainGraph info={info} />
          </div>
        </div>
      )}
    </div>
  );
}
