import { useCallback, useEffect, useRef, useState } from "react";
import {
  getEvolution,
  stopEvolution,
  pauseEvolution,
  resumeEvolution,
  getBestCreatures,
  getBestPerIsland,
  getEvolutionStats,
  getIslandStats,
  updateEvolutionName,
  deleteEvolution,
  type Evolution,
  type CreatureInfo,
  type GenerationStats,
  type IslandStats,
} from "../api";
import { useEvolutionUpdates } from "../hooks/useWebSocket";
import { navigate } from "../router";
import FitnessChart from "../components/FitnessChart";
import IslandFitnessChart from "../components/IslandFitnessChart";
import IslandBestsGrid from "../components/IslandBestsGrid";
import StatusBadge from "../components/StatusBadge";

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
}

export default function EvolutionDetail({ evoId }: Props) {
  const [evolution, setEvolution] = useState<Evolution | null>(null);
  const [bestCreatures, setBestCreatures] = useState<CreatureInfo[]>([]);
  const [bestPerIsland, setBestPerIsland] = useState<CreatureInfo[]>([]);
  const [historicalStats, setHistoricalStats] = useState<GenerationStats[]>([]);
  const [islandStats, setIslandStats] = useState<IslandStats[]>([]);
  const liveStats = useEvolutionUpdates();

  // Inline name editing
  const [editingName, setEditingName] = useState(false);
  const [nameInput, setNameInput] = useState("");
  const nameInputRef = useRef<HTMLInputElement>(null);

  const refresh = useCallback(async () => {
    const evo = await getEvolution(evoId);
    setEvolution(evo);
    const useIslands = (evo.config?.num_islands ?? 1) > 1;
    const [best, stats] = await Promise.all([
      getBestCreatures(evoId),
      getEvolutionStats(evoId),
    ]);
    setBestCreatures(best);
    setHistoricalStats(stats);
    if (useIslands) {
      const [perIsland, isStats] = await Promise.all([
        getBestPerIsland(evoId),
        getIslandStats(evoId),
      ]);
      setBestPerIsland(perIsland);
      setIslandStats(isStats);
    } else {
      setBestPerIsland([]);
      setIslandStats([]);
    }
  }, [evoId]);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 5000);
    return () => clearInterval(id);
  }, [refresh]);

  const startEditName = () => {
    setNameInput(evolution?.name ?? "");
    setEditingName(true);
    setTimeout(() => nameInputRef.current?.focus(), 0);
  };

  const commitName = async () => {
    setEditingName(false);
    await updateEvolutionName(evoId, nameInput.trim());
    refresh();
  };

  const handleStop = async () => {
    await stopEvolution(evoId);
    refresh();
  };

  const handleDelete = async () => {
    if (!window.confirm("Are you sure you want to delete this evolution and all its creatures? This cannot be undone.")) return;
    await deleteEvolution(evoId);
    navigate("/");
  };

  const handlePause = async () => {
    await pauseEvolution(evoId);
    refresh();
  };

  const handleResume = async () => {
    await resumeEvolution(evoId);
    refresh();
  };

  // Merge historical stats with live WS updates (dedup by generation)
  const liveForEvo = liveStats.filter((s) => s.evolution_id === evoId);
  const mergedMap = new Map<number, GenerationStats>();
  for (const s of historicalStats) mergedMap.set(s.generation, s);
  for (const s of liveForEvo) mergedMap.set(s.generation, s); // WS overrides
  const chartStats = Array.from(mergedMap.values()).sort(
    (a, b) => a.generation - b.generation,
  );

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
      <div className="flex flex-wrap items-center justify-between gap-3 mb-6">
        <div className="flex items-center gap-3 min-w-0">
          {editingName ? (
            <input
              ref={nameInputRef}
              value={nameInput}
              onChange={(e) => setNameInput(e.target.value)}
              onBlur={commitName}
              onKeyDown={(e) => {
                if (e.key === "Enter") commitName();
                if (e.key === "Escape") setEditingName(false);
              }}
              placeholder={`Evolution #${evoId}`}
              maxLength={100}
              className="text-2xl font-semibold bg-transparent border-b border-accent outline-none text-text-primary w-64"
            />
          ) : (
            <h1
              className="text-2xl font-semibold cursor-pointer hover:text-accent transition-colors"
              title="Click to rename"
              onClick={startEditName}
            >
              {evolution?.name ?? `Evolution #${evoId}`}
              {evolution?.name && (
                <span className="text-text-muted text-sm font-normal ml-2">
                  #{evoId}
                </span>
              )}
            </h1>
          )}
          {evolution && <StatusBadge status={evolution.status} />}
        </div>
        <div className="flex gap-2">
          {evolution?.status === "running" && (
            <button
              onClick={handlePause}
              className="px-4 py-1.5 text-sm bg-warning/20 text-warning rounded-md hover:bg-warning/30 transition-colors"
            >
              Pause
            </button>
          )}
          {evolution?.status === "paused" && (
            <button
              onClick={handleResume}
              className="px-4 py-1.5 text-sm bg-success/20 text-success rounded-md hover:bg-success/30 transition-colors"
            >
              Resume
            </button>
          )}
        </div>
      </div>

      {evolution && (
        <div className="flex flex-wrap gap-x-4 gap-y-1 text-sm text-text-secondary mb-6">
          <span>Generation {evolution.generation}</span>
          {evolution.config && (
            <>
              <span>·</span>
              <span>
                {evolution.config.goal === "SwimmingSpeed"
                  ? evolution.config.environment === "Land"
                    ? "Locomotion Speed"
                    : "Swimming Speed"
                  : "Light Following"}
              </span>
              <span>·</span>
              <span>
                {evolution.config.environment === "Water" ? "Water" : "Land"}
              </span>
              <span>·</span>
              <span>Pop {evolution.config.population_size}</span>
              <span>·</span>
              <span>{evolution.config.sim_duration}s sim</span>
            </>
          )}
        </div>
      )}

      {/* Two-column layout: chart left, creatures right */}
      <div className="grid grid-cols-1 lg:grid-cols-5 gap-6 mb-6">
        <div className="lg:col-span-3">
          <div className="bg-bg-surface border border-border-subtle rounded-lg p-4">
            <h2 className="text-sm font-medium text-text-secondary mb-3">
              {islandStats.length > 0
                ? "Per-Island Best Fitness"
                : "Fitness Over Generations"}
            </h2>
            {islandStats.length > 0 ? (
              <IslandFitnessChart stats={islandStats} width={700} height={280} />
            ) : (
              <FitnessChart stats={chartStats} width={700} height={250} />
            )}
          </div>
        </div>
        <div className="lg:col-span-2 space-y-4">
          {bestPerIsland.length > 0 && (
            <div className="bg-bg-surface border border-border-subtle rounded-lg p-4">
              <h2 className="text-sm font-medium text-text-secondary mb-3">
                Best Per Island
              </h2>
              <IslandBestsGrid evoId={evoId} bestPerIsland={bestPerIsland} />
            </div>
          )}
          <div className="bg-bg-surface border border-border-subtle rounded-lg p-4">
            <h2 className="text-sm font-medium text-text-secondary mb-3">
              Top Creatures
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
                  <span className="text-sm font-mono">
                    #{c.id}
                    {c.island_id !== undefined && bestPerIsland.length > 0 && (
                      <span className="text-text-muted ml-2 text-xs">
                        i{c.island_id}
                      </span>
                    )}
                  </span>
                  <span className="text-sm text-text-secondary">
                    {formatFitness(c.fitness)}
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
      {/* Danger zone */}
      <div className="border-t border-border-subtle pt-6 mt-2">
        <button
          onClick={handleDelete}
          className="px-4 py-1.5 text-sm border border-danger text-danger rounded-md hover:bg-danger/10 transition-colors"
        >
          Delete Evolution
        </button>
      </div>
    </div>
  );
}
