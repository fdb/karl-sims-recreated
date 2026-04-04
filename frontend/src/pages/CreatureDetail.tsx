import { useEffect, useState } from "react";
import { getBestCreatures, type CreatureInfo } from "../api";
import { navigate } from "../router";

interface Props {
  evoId: number;
  creatureId: number;
}

export default function CreatureDetail({ evoId, creatureId }: Props) {
  const [creature, setCreature] = useState<CreatureInfo | null>(null);

  useEffect(() => {
    (async () => {
      // Fetch best creatures and find the one we want.
      // TODO: Add a dedicated GET /api/evolutions/:id/creatures/:creatureId endpoint.
      const creatures = await getBestCreatures(evoId);
      const found = creatures.find((c) => c.id === creatureId);
      setCreature(found ?? null);
    })();
  }, [evoId, creatureId]);

  return (
    <div className="dashboard">
      <div className="breadcrumb">
        <a
          href="/"
          onClick={(e) => {
            e.preventDefault();
            navigate("/");
          }}
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
        >
          Evolution #{evoId}
        </a>
        <span>/</span>
        <span>Creature #{creatureId}</span>
      </div>

      <h2>Creature #{creatureId}</h2>

      {creature ? (
        <div className="creature-info">
          <p>
            <strong>Fitness:</strong> {creature.fitness.toFixed(4)}
          </p>
          <p>
            <strong>Evolution:</strong> #{evoId}
          </p>
          {/* TODO: Add 3D rendering of this creature once genome download endpoint
              is available. The WASM renderer would need a loadGenome() function
              to display a specific creature from its serialized genome data. */}
          <div className="creature-3d-placeholder">
            <p>3D viewer for individual creatures coming soon.</p>
            <p style={{ fontSize: "0.8rem", color: "#666" }}>
              Requires genome download endpoint and WASM genome loading support.
            </p>
          </div>
        </div>
      ) : (
        <p className="empty">Loading creature data...</p>
      )}
    </div>
  );
}
