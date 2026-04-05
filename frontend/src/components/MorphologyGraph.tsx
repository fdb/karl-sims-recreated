import type { GenotypeInfo } from "../api";
import ZoomableSvg from "./ZoomableSvg";

// Color per neuron function — muted palette to blend with the dark UI.
const FUNC_COLORS: Record<string, string> = {
  Sum: "#4a6fa5",
  Product: "#7b5ea7",
  Sigmoid: "#a5694f",
  Sin: "#5a8f6a",
  OscillateWave: "#8f8f3a",
  Memory: "#7a4a5a",
};

export default function MorphologyGraph({ info }: { info: GenotypeInfo }) {
  const nodeW = 170;
  const nodeH = 96;
  const gapX = 200;
  const gapY = 130;
  const nodes = info.nodes;
  const conns = info.connections;

  // BFS layout from root (node 0). Nodes appearing via connections form a
  // tree; unreachable nodes are stacked at the bottom.
  const levels: number[][] = [];
  const visited = new Set<number>();
  const queue: { id: number; level: number }[] = [{ id: 0, level: 0 }];

  while (queue.length > 0) {
    const { id, level } = queue.shift()!;
    if (visited.has(id)) continue;
    visited.add(id);
    if (!levels[level]) levels[level] = [];
    levels[level].push(id);
    for (const c of conns) {
      if (c.source === id && !visited.has(c.target)) {
        queue.push({ id: c.target, level: level + 1 });
      }
    }
  }
  for (let i = 0; i < nodes.length; i++) {
    if (!visited.has(i)) {
      const lvl = levels.length;
      if (!levels[lvl]) levels[lvl] = [];
      levels[lvl].push(i);
    }
  }

  const positions: { x: number; y: number }[] = [];
  const maxRow = Math.max(...levels.map((r) => r.length), 1);
  const svgW = Math.max(maxRow * gapX + 80, 600);
  const svgH = levels.length * gapY + 60;
  for (let l = 0; l < levels.length; l++) {
    const row = levels[l];
    for (let i = 0; i < row.length; i++) {
      positions[row[i]] = {
        x: (i - (row.length - 1) / 2) * gapX + svgW / 2,
        y: l * gapY + 50,
      };
    }
  }

  // Short human-friendly neuron function label (fits inside 22px-wide chip)
  const funcLabel = (f: string): string => {
    switch (f) {
      case "OscillateWave":
        return "Osc";
      case "Product":
        return "Prod";
      case "Sigmoid":
        return "Sig";
      case "Memory":
        return "Mem";
      default:
        return f; // Sum, Sin — already short
    }
  };

  return (
    <ZoomableSvg viewBoxWidth={svgW} viewBoxHeight={svgH}>
      <defs>
        <marker
          id="arrow"
          viewBox="0 0 10 10"
          refX="10"
          refY="5"
          markerWidth="6"
          markerHeight="6"
          orient="auto-start-reverse"
        >
          <path d="M 0 0 L 10 5 L 0 10 z" fill="var(--color-border)" />
        </marker>
      </defs>

      {/* Edges with small connection labels */}
      {conns.map((c, i) => {
        const from = positions[c.source];
        const to = positions[c.target];
        if (!from || !to) return null;
        const label =
          c.parent_face.replace("Pos", "+").replace("Neg", "-") +
          " → " +
          c.child_face.replace("Pos", "+").replace("Neg", "-") +
          (Math.abs(c.scale - 1) > 0.01 ? ` ×${c.scale.toFixed(1)}` : "");
        return (
          <g key={`e${i}`}>
            <line
              x1={from.x}
              y1={from.y + nodeH / 2}
              x2={to.x}
              y2={to.y - nodeH / 2}
              stroke="var(--color-border)"
              strokeWidth="1.5"
              markerEnd="url(#arrow)"
              opacity="0.6"
            />
            <text
              x={(from.x + to.x) / 2 + 4}
              y={(from.y + to.y) / 2}
              fill="var(--color-text-muted)"
              fontSize="9"
            >
              {label}
            </text>
          </g>
        );
      })}

      {/* Nodes: title | dimensions | joint type | neuron chips */}
      {nodes.map((node, i) => {
        const pos = positions[i];
        if (!pos) return null;
        const dims = node.dimensions.map((d) => d.toFixed(2)).join(" × ");
        const title = i === 0 ? "Root" : `Node ${i}`;
        const neurons = node.brain.neurons;
        // Lay out neuron chips on two rows if needed.
        const chipW = 34;
        const chipH = 14;
        const chipGap = 3;
        const maxChipsPerRow = Math.floor(
          (nodeW - 12) / (chipW + chipGap),
        );

        return (
          <g key={`n${i}`}>
            <rect
              x={pos.x - nodeW / 2}
              y={pos.y - nodeH / 2}
              width={nodeW}
              height={nodeH}
              rx="6"
              fill={
                i === 0 ? "var(--color-success)" : "var(--color-bg-elevated)"
              }
              fillOpacity={i === 0 ? 0.15 : 1}
              stroke={
                i === 0 ? "var(--color-success)" : "var(--color-border)"
              }
              strokeWidth="1.5"
            />
            {/* Title + effector count */}
            <text
              x={pos.x - nodeW / 2 + 8}
              y={pos.y - nodeH / 2 + 16}
              fill="var(--color-text-primary)"
              fontSize="12"
              fontWeight="600"
            >
              {title}
            </text>
            <text
              x={pos.x + nodeW / 2 - 8}
              y={pos.y - nodeH / 2 + 16}
              fill="var(--color-text-muted)"
              fontSize="10"
              textAnchor="end"
            >
              {node.brain.num_effectors}E
            </text>
            {/* Dimensions */}
            <text
              x={pos.x - nodeW / 2 + 8}
              y={pos.y - nodeH / 2 + 32}
              fill="var(--color-text-secondary)"
              fontSize="10"
              fontFamily="ui-monospace, monospace"
            >
              {dims}
            </text>
            {/* Joint type */}
            <text
              x={pos.x - nodeW / 2 + 8}
              y={pos.y - nodeH / 2 + 48}
              fill="var(--color-accent)"
              fontSize="10"
              fontWeight="500"
            >
              {node.joint_type}
            </text>
            {/* Neuron chips */}
            {neurons.slice(0, maxChipsPerRow * 2).map((neuron, ni) => {
              const row = Math.floor(ni / maxChipsPerRow);
              const col = ni % maxChipsPerRow;
              const cx = pos.x - nodeW / 2 + 6 + col * (chipW + chipGap);
              const cy = pos.y - nodeH / 2 + 56 + row * (chipH + 2);
              const color = FUNC_COLORS[neuron.func] ?? "#555";
              return (
                <g key={`c${ni}`}>
                  <rect
                    x={cx}
                    y={cy}
                    width={chipW}
                    height={chipH}
                    rx="2"
                    fill={color}
                    opacity="0.75"
                  />
                  <text
                    x={cx + chipW / 2}
                    y={cy + chipH - 3}
                    fill="#fff"
                    fontSize="8"
                    textAnchor="middle"
                    fontFamily="ui-monospace, monospace"
                  >
                    {funcLabel(neuron.func)}
                  </text>
                </g>
              );
            })}
            {neurons.length > maxChipsPerRow * 2 && (
              <text
                x={pos.x + nodeW / 2 - 8}
                y={pos.y + nodeH / 2 - 6}
                fill="var(--color-text-muted)"
                fontSize="9"
                textAnchor="end"
              >
                +{neurons.length - maxChipsPerRow * 2} more
              </text>
            )}
          </g>
        );
      })}
    </ZoomableSvg>
  );
}
