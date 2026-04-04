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
import StatusBadge from "../components/StatusBadge";

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
        <span className="text-text-secondary">Evolution #{evoId}</span>
      </div>

      {/* Header with actions */}
      <div className="flex items-center justify-between mb-6">
        <div className="flex items-center gap-3">
          <h1 className="text-2xl font-semibold">Evolution #{evoId}</h1>
          {evolution && <StatusBadge status={evolution.status} />}
        </div>
        <div className="flex gap-2">
          {evolution?.status === "running" && (
            <>
              <button
                onClick={handlePause}
                className="px-4 py-1.5 text-sm bg-warning/20 text-warning rounded-md hover:bg-warning/30 transition-colors"
              >
                Pause
              </button>
              <button
                onClick={handleStop}
                className="px-4 py-1.5 text-sm bg-danger/20 text-danger rounded-md hover:bg-danger/30 transition-colors"
              >
                Stop
              </button>
            </>
          )}
          {evolution?.status === "paused" && (
            <>
              <button
                onClick={handleResume}
                className="px-4 py-1.5 text-sm bg-success/20 text-success rounded-md hover:bg-success/30 transition-colors"
              >
                Resume
              </button>
              <button
                onClick={handleStop}
                className="px-4 py-1.5 text-sm bg-danger/20 text-danger rounded-md hover:bg-danger/30 transition-colors"
              >
                Stop
              </button>
            </>
          )}
        </div>
      </div>

      {evolution && (
        <p className="text-text-secondary mb-6">
          Generation {evolution.generation}
        </p>
      )}

      {/* Two-column layout: chart left, creatures right */}
      <div className="grid grid-cols-5 gap-6">
        <div className="col-span-3">
          <div className="bg-bg-surface border border-border-subtle rounded-lg p-4">
            <h2 className="text-sm font-medium text-text-secondary mb-3">
              Fitness Over Generations
            </h2>
            <FitnessChart stats={chartStats} width={700} height={250} />
          </div>
        </div>
        <div className="col-span-2">
          <div className="bg-bg-surface border border-border-subtle rounded-lg p-4">
            <h2 className="text-sm font-medium text-text-secondary mb-3">
              Best Creatures
            </h2>
            <div className="space-y-1">
              {bestCreatures.map((c) => (
                <a
                  key={c.id}
                  href={`/evolutions/${evoId}/creatures/${c.id}`}
                  onClick={(e) => {
                    e.preventDefault();
                    navigate(`/evolutions/${evoId}/creatures/${c.id}`);
                  }}
                  className="flex items-center justify-between px-3 py-2 rounded hover:bg-bg-elevated transition-colors no-underline text-inherit"
                >
                  <span className="text-sm font-mono">#{c.id}</span>
                  <span className="text-sm text-text-secondary">
                    {c.fitness.toFixed(4)}
                  </span>
                </a>
              ))}
              {bestCreatures.length === 0 && (
                <p className="text-text-muted italic text-sm py-4 text-center">
                  No creatures yet.
                </p>
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
