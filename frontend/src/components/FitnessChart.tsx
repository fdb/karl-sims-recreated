interface Props {
  stats: { generation: number; best_fitness: number; avg_fitness: number }[];
  width?: number;
  height?: number;
}

export default function FitnessChart({
  stats,
  width = 600,
  height = 200,
}: Props) {
  if (stats.length < 2)
    return (
      <div className="text-text-muted italic py-5 text-center text-sm">
        Waiting for data...
      </div>
    );

  const padding = { top: 20, right: 20, bottom: 30, left: 50 };
  const w = width - padding.left - padding.right;
  const h = height - padding.top - padding.bottom;

  const maxGen = Math.max(...stats.map((s) => s.generation));
  const maxFit = Math.max(...stats.map((s) => s.best_fitness), 0.01);

  const toX = (gen: number) =>
    padding.left + (gen / Math.max(maxGen, 1)) * w;
  const toY = (fit: number) => padding.top + h - (fit / maxFit) * h;

  const bestPath = stats
    .map(
      (s, i) =>
        `${i === 0 ? "M" : "L"} ${toX(s.generation)} ${toY(s.best_fitness)}`,
    )
    .join(" ");
  const avgPath = stats
    .map(
      (s, i) =>
        `${i === 0 ? "M" : "L"} ${toX(s.generation)} ${toY(s.avg_fitness)}`,
    )
    .join(" ");

  return (
    <svg viewBox={`0 0 ${width} ${height}`} className="w-full rounded" preserveAspectRatio="xMidYMid meet">
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

      {/* Labels */}
      <text
        x={padding.left - 5}
        y={padding.top + 5}
        fill="var(--color-text-secondary)"
        fontSize="10"
        textAnchor="end"
      >
        {maxFit.toFixed(1)}
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

      {/* Lines */}
      <path
        d={bestPath}
        fill="none"
        stroke="var(--color-chart-best)"
        strokeWidth="2"
      />
      <path
        d={avgPath}
        fill="none"
        stroke="var(--color-chart-avg)"
        strokeWidth="1.5"
        strokeDasharray="4,4"
      />

      {/* Legend */}
      <line
        x1={padding.left + 10}
        y1={10}
        x2={padding.left + 30}
        y2={10}
        stroke="var(--color-chart-best)"
        strokeWidth="2"
      />
      <text
        x={padding.left + 35}
        y={14}
        fill="var(--color-chart-best)"
        fontSize="11"
      >
        Best
      </text>
      <line
        x1={padding.left + 80}
        y1={10}
        x2={padding.left + 100}
        y2={10}
        stroke="var(--color-chart-avg)"
        strokeWidth="1.5"
        strokeDasharray="4,4"
      />
      <text
        x={padding.left + 105}
        y={14}
        fill="var(--color-chart-avg)"
        fontSize="11"
      >
        Avg
      </text>
    </svg>
  );
}
