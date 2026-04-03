import { useCallback, useEffect, useState } from "react";
import {
  listEvolutions,
  createEvolution,
  stopEvolution,
  getBestCreatures,
  type Evolution,
  type CreatureInfo,
} from "../api";
import { useEvolutionUpdates } from "../hooks/useWebSocket";
import FitnessChart from "../components/FitnessChart";

interface Props {
  onViewCreature?: (evoId: number, creatureId: number) => void;
}

export default function Dashboard({ onViewCreature }: Props) {
  const [evolutions, setEvolutions] = useState<Evolution[]>([]);
  const [selectedEvo, setSelectedEvo] = useState<number | null>(null);
  const [bestCreatures, setBestCreatures] = useState<CreatureInfo[]>([]);
  const [popSize, setPopSize] = useState(50);
  const liveStats = useEvolutionUpdates();

  const refresh = useCallback(async () => {
    const evos = await listEvolutions();
    setEvolutions(evos);
  }, []);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 5000);
    return () => clearInterval(id);
  }, [refresh]);

  const handleCreate = async () => {
    await createEvolution(popSize);
    refresh();
  };

  const handleStop = async (id: number) => {
    await stopEvolution(id);
    refresh();
  };

  const handleSelectEvo = async (id: number) => {
    setSelectedEvo(id);
    const best = await getBestCreatures(id);
    setBestCreatures(best);
  };

  // Filter live stats for selected evolution
  const chartStats = selectedEvo
    ? liveStats.filter((s) => s.evolution_id === selectedEvo)
    : [];

  return (
    <div className="dashboard">
      <div className="dashboard-header">
        <h2>Evolution Runs</h2>
        <div className="create-evo">
          <label>Pop size:</label>
          <input
            type="number"
            value={popSize}
            onChange={(e) => setPopSize(Number(e.target.value))}
            min={10}
            max={500}
          />
          <button onClick={handleCreate}>New Evolution</button>
        </div>
      </div>

      <div className="evo-list">
        {evolutions.map((evo) => (
          <div
            key={evo.id}
            className={`evo-item ${selectedEvo === evo.id ? "selected" : ""}`}
            onClick={() => handleSelectEvo(evo.id)}
          >
            <span className={`status-badge ${evo.status}`}>{evo.status}</span>
            <span>Evolution #{evo.id}</span>
            <span>Gen {evo.generation}</span>
            {evo.status === "running" && (
              <button
                onClick={(e) => {
                  e.stopPropagation();
                  handleStop(evo.id);
                }}
              >
                Stop
              </button>
            )}
          </div>
        ))}
        {evolutions.length === 0 && (
          <p className="empty">No evolutions yet. Create one!</p>
        )}
      </div>

      {selectedEvo && (
        <div className="evo-detail">
          <h3>Evolution #{selectedEvo}</h3>
          <FitnessChart stats={chartStats} />

          <h4>Best Creatures</h4>
          <div className="creature-list">
            {bestCreatures.map((c) => (
              <div
                key={c.id}
                className="creature-item"
                onClick={() => onViewCreature?.(selectedEvo, c.id)}
              >
                <span>#{c.id}</span>
                <span>Fitness: {c.fitness.toFixed(4)}</span>
                <button>View</button>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
