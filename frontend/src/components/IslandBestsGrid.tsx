import { navigate } from "../router";
import type { CreatureInfo } from "../api";

interface Props {
  evoId: number;
  bestPerIsland: CreatureInfo[];
}

const ISLAND_COLORS = [
  "#4a9eff", "#ff7a45", "#52c41a", "#ff4d6d",
  "#a855f7", "#14b8a6", "#eab308", "#ec4899",
  "#06b6d4", "#f97316", "#84cc16", "#8b5cf6",
];

function formatFitness(v: number): string {
  if (!isFinite(v)) return "—";
  const abs = Math.abs(v);
  if (abs === 0) return "0";
  if (abs >= 1e6) return v.toExponential(2);
  if (abs >= 1) return v.toFixed(2);
  if (abs >= 0.01) return v.toFixed(4);
  return v.toExponential(2);
}

/**
 * Grid of the top creature from each island, color-matched to the
 * IslandFitnessChart legend so the two panels read as a pair. Click any
 * tile to drill into that creature's detail page.
 */
export default function IslandBestsGrid({ evoId, bestPerIsland }: Props) {
  if (bestPerIsland.length === 0) {
    return (
      <p className="text-text-muted italic text-sm py-4 text-center">
        No evaluated creatures yet.
      </p>
    );
  }

  return (
    <div className="grid grid-cols-2 sm:grid-cols-3 gap-2">
      {bestPerIsland.map((c) => {
        const id = c.island_id ?? 0;
        const color = ISLAND_COLORS[id % ISLAND_COLORS.length];
        return (
          <a
            key={c.id}
            href={`/evolutions/${evoId}/creatures/${c.id}`}
            onClick={(e) => {
              e.preventDefault();
              navigate(`/evolutions/${evoId}/creatures/${c.id}`);
            }}
            className="block rounded border border-border-subtle bg-bg-elevated hover:border-accent transition-colors p-2 no-underline text-inherit"
            style={{ borderLeftColor: color, borderLeftWidth: 3 }}
          >
            <div className="flex items-center justify-between text-xs text-text-muted mb-0.5">
              <span style={{ color }}>Island {id}</span>
              <span className="font-mono">#{c.id}</span>
            </div>
            <div className="text-sm font-mono text-text-primary">
              {formatFitness(c.fitness)}
            </div>
          </a>
        );
      })}
    </div>
  );
}
