import { useCallback, useEffect, useState } from "react";
import {
  listEvolutions,
  stopEvolution,
  pauseEvolution,
  resumeEvolution,
  type Evolution,
} from "../api";
import { navigate } from "../router";
import StatusBadge from "../components/StatusBadge";
import CreateEvolutionForm from "../components/CreateEvolutionForm";

export default function EvolutionList() {
  const [evolutions, setEvolutions] = useState<Evolution[]>([]);
  const [showForm, setShowForm] = useState(false);

  const refresh = useCallback(async () => {
    const evos = await listEvolutions();
    setEvolutions(evos);
  }, []);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 5000);
    return () => clearInterval(id);
  }, [refresh]);

  const handleStop = async (id: number) => {
    await stopEvolution(id);
    refresh();
  };

  const handlePause = async (id: number) => {
    await pauseEvolution(id);
    refresh();
  };

  const handleResume = async (id: number) => {
    await resumeEvolution(id);
    refresh();
  };

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h1 className="text-2xl font-semibold">Evolution Runs</h1>
        {!showForm && (
          <button
            onClick={() => setShowForm(true)}
            className="px-4 py-1.5 bg-accent text-white rounded-md text-sm font-medium hover:bg-accent-hover transition-colors"
          >
            New Evolution
          </button>
        )}
      </div>

      {showForm && (
        <CreateEvolutionForm
          onCreated={() => {
            setShowForm(false);
            refresh();
          }}
          onCancel={() => setShowForm(false)}
        />
      )}

      <div className="space-y-2">
        {evolutions.map((evo) => (
          <a
            key={evo.id}
            href={`/evolutions/${evo.id}`}
            onClick={(e) => {
              e.preventDefault();
              navigate(`/evolutions/${evo.id}`);
            }}
            className="flex flex-wrap items-center gap-x-4 gap-y-2 px-4 py-3 bg-bg-surface border border-border-subtle rounded-lg hover:bg-bg-elevated hover:border-border transition-all no-underline text-inherit"
          >
            <StatusBadge status={evo.status} />
            <span className="font-medium">Evolution #{evo.id}</span>
            <span className="text-text-secondary text-sm">
              Gen {evo.generation}
            </span>
            <div className="ml-auto flex gap-2">
              {evo.status === "running" && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    e.preventDefault();
                    handlePause(evo.id);
                  }}
                  className="px-3 py-1 text-xs bg-warning/20 text-warning rounded hover:bg-warning/30 transition-colors"
                >
                  Pause
                </button>
              )}
              {evo.status === "paused" && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    e.preventDefault();
                    handleResume(evo.id);
                  }}
                  className="px-3 py-1 text-xs bg-success/20 text-success rounded hover:bg-success/30 transition-colors"
                >
                  Resume
                </button>
              )}
            </div>
          </a>
        ))}
        {evolutions.length === 0 && !showForm && (
          <p className="text-text-muted italic py-8 text-center">
            No evolutions yet. Create one!
          </p>
        )}
      </div>
    </div>
  );
}
