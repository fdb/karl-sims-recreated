import { useCallback, useEffect, useState } from "react";
import {
  listEvolutions,
  createEvolution,
  stopEvolution,
  pauseEvolution,
  resumeEvolution,
  type Evolution,
} from "../api";
import { navigate } from "../router";

export default function EvolutionList() {
  const [evolutions, setEvolutions] = useState<Evolution[]>([]);
  const [popSize, setPopSize] = useState(50);

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

  const handlePause = async (id: number) => {
    await pauseEvolution(id);
    refresh();
  };

  const handleResume = async (id: number) => {
    await resumeEvolution(id);
    refresh();
  };

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
          <a
            key={evo.id}
            className="evo-item"
            href={`/evolutions/${evo.id}`}
            onClick={(e) => {
              e.preventDefault();
              navigate(`/evolutions/${evo.id}`);
            }}
          >
            <span className={`status-badge ${evo.status}`}>{evo.status}</span>
            <span>Evolution #{evo.id}</span>
            <span>Gen {evo.generation}</span>
            {evo.status === "running" && (
              <>
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    e.preventDefault();
                    handlePause(evo.id);
                  }}
                >
                  Pause
                </button>
                <button
                  className="stop-btn"
                  onClick={(e) => {
                    e.stopPropagation();
                    e.preventDefault();
                    handleStop(evo.id);
                  }}
                >
                  Stop
                </button>
              </>
            )}
            {evo.status === "paused" && (
              <>
                <button
                  className="resume-btn"
                  onClick={(e) => {
                    e.stopPropagation();
                    e.preventDefault();
                    handleResume(evo.id);
                  }}
                >
                  Resume
                </button>
                <button
                  className="stop-btn"
                  onClick={(e) => {
                    e.stopPropagation();
                    e.preventDefault();
                    handleStop(evo.id);
                  }}
                >
                  Stop
                </button>
              </>
            )}
          </a>
        ))}
        {evolutions.length === 0 && (
          <p className="empty">No evolutions yet. Create one!</p>
        )}
      </div>
    </div>
  );
}
