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
    return <div className="chart-placeholder">Waiting for data...</div>;

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
    <svg width={width} height={height} className="fitness-chart">
      {/* Axes */}
      <line
        x1={padding.left}
        y1={padding.top}
        x2={padding.left}
        y2={padding.top + h}
        stroke="#555"
      />
      <line
        x1={padding.left}
        y1={padding.top + h}
        x2={padding.left + w}
        y2={padding.top + h}
        stroke="#555"
      />

      {/* Labels */}
      <text
        x={padding.left - 5}
        y={padding.top + 5}
        fill="#888"
        fontSize="10"
        textAnchor="end"
      >
        {maxFit.toFixed(1)}
      </text>
      <text
        x={padding.left - 5}
        y={padding.top + h}
        fill="#888"
        fontSize="10"
        textAnchor="end"
      >
        0
      </text>
      <text
        x={padding.left + w}
        y={padding.top + h + 15}
        fill="#888"
        fontSize="10"
        textAnchor="end"
      >
        Gen {maxGen}
      </text>

      {/* Lines */}
      <path d={bestPath} fill="none" stroke="#4caf50" strokeWidth="2" />
      <path
        d={avgPath}
        fill="none"
        stroke="#ff9800"
        strokeWidth="1.5"
        strokeDasharray="4,4"
      />

      {/* Legend */}
      <line
        x1={padding.left + 10}
        y1={10}
        x2={padding.left + 30}
        y2={10}
        stroke="#4caf50"
        strokeWidth="2"
      />
      <text x={padding.left + 35} y={14} fill="#4caf50" fontSize="11">
        Best
      </text>
      <line
        x1={padding.left + 80}
        y1={10}
        x2={padding.left + 100}
        y2={10}
        stroke="#ff9800"
        strokeWidth="1.5"
        strokeDasharray="4,4"
      />
      <text x={padding.left + 105} y={14} fill="#ff9800" fontSize="11">
        Avg
      </text>
    </svg>
  );
}
