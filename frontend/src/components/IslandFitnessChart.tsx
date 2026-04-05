import type { IslandStats } from "../api";

interface Props {
  stats: IslandStats[];
  width?: number;
  height?: number;
}

// Distinct hues for up to 12 islands. Chosen to stay legible over the
// dark surface background — mid-saturation, mid-lightness colors that
// differentiate without any single line disappearing.
const ISLAND_COLORS = [
  "#4a9eff", // blue
  "#ff7a45", // orange
  "#52c41a", // green
  "#ff4d6d", // pink
  "#a855f7", // purple
  "#14b8a6", // teal
  "#eab308", // yellow
  "#ec4899", // magenta
  "#06b6d4", // cyan
  "#f97316", // dark orange
  "#84cc16", // lime
  "#8b5cf6", // violet
];

function formatAxis(v: number): string {
  if (!isFinite(v)) return "—";
  const abs = Math.abs(v);
  if (abs === 0) return "0";
  if (abs >= 1) return v.toFixed(1);
  if (abs >= 0.01) return v.toFixed(3);
  return v.toExponential(1);
}

/**
 * Overlay-line fitness chart: one line per island showing best-fitness
 * trajectory over generations. Visualizes species divergence — when
 * islands diverge onto distinct strategies, their lines separate.
 * When migration smooths differences, lines converge.
 */
export default function IslandFitnessChart({
  stats,
  width = 700,
  height = 280,
}: Props) {
  if (stats.length < 2)
    return (
      <div className="text-text-muted italic py-5 text-center text-sm">
        Waiting for data...
      </div>
    );

  // Group by island.
  const byIsland = new Map<number, IslandStats[]>();
  for (const s of stats) {
    if (!byIsland.has(s.island_id)) byIsland.set(s.island_id, []);
    byIsland.get(s.island_id)!.push(s);
  }
  for (const arr of byIsland.values()) {
    arr.sort((a, b) => a.generation - b.generation);
  }
  const islandIds = Array.from(byIsland.keys()).sort((a, b) => a - b);

  const padding = { top: 22, right: 20, bottom: 30, left: 50 };
  const w = width - padding.left - padding.right;
  const h = height - padding.top - padding.bottom;

  const maxGen = Math.max(...stats.map((s) => s.generation));
  const maxFit = Math.max(...stats.map((s) => s.best_fitness), 0.01);

  const toX = (gen: number) => padding.left + (gen / Math.max(maxGen, 1)) * w;
  const toY = (fit: number) => padding.top + h - (fit / maxFit) * h;

  // Legend layout: flow horizontally, 80px per entry.
  const legendItemW = 78;
  const legendX = padding.left + 10;

  return (
    <svg
      viewBox={`0 0 ${width} ${height}`}
      className="w-full rounded"
      preserveAspectRatio="xMidYMid meet"
    >
      {/* Axes */}
      <line
        x1={padding.left}
        y1={padding.top}
        x2={padding.left}
        y2={padding.top + h}
        stroke="var(--color-border)"
      />
      <line
        x1={padding.left}
        y1={padding.top + h}
        x2={padding.left + w}
        y2={padding.top + h}
        stroke="var(--color-border)"
      />

      {/* Y-axis labels */}
      <text
        x={padding.left - 5}
        y={padding.top + 5}
        fill="var(--color-text-secondary)"
        fontSize="10"
        textAnchor="end"
      >
        {formatAxis(maxFit)}
      </text>
      <text
        x={padding.left - 5}
        y={padding.top + h}
        fill="var(--color-text-secondary)"
        fontSize="10"
        textAnchor="end"
      >
        0
      </text>
      <text
        x={padding.left + w}
        y={padding.top + h + 15}
        fill="var(--color-text-secondary)"
        fontSize="10"
        textAnchor="end"
      >
        Gen {maxGen}
      </text>

      {/* Per-island best lines */}
      {islandIds.map((id) => {
        const rows = byIsland.get(id)!;
        const color = ISLAND_COLORS[id % ISLAND_COLORS.length];
        const path = rows
          .map(
            (s, i) =>
              `${i === 0 ? "M" : "L"} ${toX(s.generation)} ${toY(s.best_fitness)}`,
          )
          .join(" ");
        return (
          <path
            key={id}
            d={path}
            fill="none"
            stroke={color}
            strokeWidth="1.8"
            opacity="0.9"
          />
        );
      })}

      {/* Legend — one swatch per island */}
      {islandIds.map((id, i) => {
        const color = ISLAND_COLORS[id % ISLAND_COLORS.length];
        const x = legendX + i * legendItemW;
        return (
          <g key={`leg-${id}`}>
            <line
              x1={x}
              y1={12}
              x2={x + 16}
              y2={12}
              stroke={color}
              strokeWidth="2"
            />
            <text
              x={x + 20}
              y={16}
              fill="var(--color-text-secondary)"
              fontSize="10"
            >
              Island {id}
            </text>
          </g>
        );
      })}
    </svg>
  );
}
