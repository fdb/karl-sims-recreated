import { useCallback, useEffect, useState } from "react";
import {
  getEvolution,
  stopEvolution,
  pauseEvolution,
  resumeEvolution,
  getBestCreatures,
  type Evolution,
  type CreatureInfo,
} from "../api";
import { useEvolutionUpdates } from "../hooks/useWebSocket";
import { navigate } from "../router";
import FitnessChart from "../components/FitnessChart";

interface Props {
  evoId: number;
}

export default function EvolutionDetail({ evoId }: Props) {
  const [evolution, setEvolution] = useState<Evolution | null>(null);
  const [bestCreatures, setBestCreatures] = useState<CreatureInfo[]>([]);
  const liveStats = useEvolutionUpdates();

  const refresh = useCallback(async () => {
    const evo = await getEvolution(evoId);
    setEvolution(evo);
    const best = await getBestCreatures(evoId);
    setBestCreatures(best);
  }, [evoId]);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 5000);
    return () => clearInterval(id);
  }, [refresh]);

  const handleStop = async () => {
    await stopEvolution(evoId);
    refresh();
  };

  const handlePause = async () => {
    await pauseEvolution(evoId);
    refresh();
  };

  const handleResume = async () => {
    await resumeEvolution(evoId);
    refresh();
  };

  const chartStats = liveStats.filter((s) => s.evolution_id === evoId);

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
        <span>Evolution #{evoId}</span>
      </div>

      {evolution && (
        <>
          <div className="dashboard-header">
            <h2>
              Evolution #{evoId}{" "}
              <span className={`status-badge ${evolution.status}`}>
                {evolution.status}
              </span>
            </h2>
            {evolution.status === "running" && (
              <>
                <button onClick={handlePause}>Pause</button>
                <button className="stop-btn" onClick={handleStop}>Stop</button>
              </>
            )}
            {evolution.status === "paused" && (
              <>
                <button className="resume-btn" onClick={handleResume}>Resume</button>
                <button className="stop-btn" onClick={handleStop}>Stop</button>
              </>
            )}
          </div>

          <p style={{ color: "#888", marginBottom: 16 }}>
            Generation {evolution.generation}
          </p>

          <FitnessChart stats={chartStats} />

          <h3 style={{ marginBottom: 8, fontWeight: 500 }}>Best Creatures</h3>
          <div className="creature-list">
            {bestCreatures.map((c) => (
              <a
                key={c.id}
                className="creature-item"
                href={`/evolutions/${evoId}/creatures/${c.id}`}
                onClick={(e) => {
                  e.preventDefault();
                  navigate(`/evolutions/${evoId}/creatures/${c.id}`);
                }}
              >
                <span>#{c.id}</span>
                <span>Fitness: {c.fitness.toFixed(4)}</span>
                <span className="view-link">View</span>
              </a>
            ))}
            {bestCreatures.length === 0 && (
              <p className="empty">No creatures yet.</p>
            )}
          </div>
        </>
      )}
    </div>
  );
}
