import type { GenotypeInfo } from "../api";

export default function MorphologyGraph({ info }: { info: GenotypeInfo }) {
  const nodeW = 140, nodeH = 60, gapX = 180, gapY = 90;
  const nodes = info.nodes;
  const conns = info.connections;

  // Simple tree layout: BFS from root (node 0)
  const positions: { x: number; y: number }[] = [];
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
  // Also add unvisited nodes
  for (let i = 0; i < nodes.length; i++) {
    if (!visited.has(i)) {
      const lvl = levels.length;
      if (!levels[lvl]) levels[lvl] = [];
      levels[lvl].push(i);
    }
  }

  // Assign positions
  for (let l = 0; l < levels.length; l++) {
    const row = levels[l];
    for (let i = 0; i < row.length; i++) {
      positions[row[i]] = {
        x: (i - (row.length - 1) / 2) * gapX + 300,
        y: l * gapY + 40,
      };
    }
  }

  const svgW = 600, svgH = Math.max(levels.length * gapY + 80, 200);

  return (
    <svg width={svgW} height={svgH} className="morph-graph">
      {/* Arrow marker */}
      <defs>
        <marker id="arrow" viewBox="0 0 10 10" refX="10" refY="5"
          markerWidth="6" markerHeight="6" orient="auto-start-reverse">
          <path d="M 0 0 L 10 5 L 0 10 z" fill="#555" />
        </marker>
      </defs>

      {/* Edges */}
      {conns.map((c, i) => {
        const from = positions[c.source];
        const to = positions[c.target];
        if (!from || !to) return null;
        return (
          <g key={`e${i}`}>
            <line x1={from.x} y1={from.y + nodeH / 2} x2={to.x} y2={to.y - nodeH / 2}
              stroke="#555" strokeWidth="2" markerEnd="url(#arrow)" />
            <text x={(from.x + to.x) / 2 + 5} y={(from.y + to.y) / 2}
              fill="#777" fontSize="9">
              {c.parent_face.replace("Pos", "+").replace("Neg", "-")} {"\u2192"} {c.child_face.replace("Pos", "+").replace("Neg", "-")}
              {c.scale !== 1 ? ` \u00D7${c.scale.toFixed(1)}` : ""}
            </text>
          </g>
        );
      })}

      {/* Nodes */}
      {nodes.map((node, i) => {
        const pos = positions[i];
        if (!pos) return null;
        const dim = node.dimensions.map(d => d.toFixed(2)).join("\u00D7");
        return (
          <g key={`n${i}`}>
            <rect x={pos.x - nodeW / 2} y={pos.y - nodeH / 2} width={nodeW} height={nodeH}
              rx="6" fill={i === 0 ? "#2e4a3e" : "#2a2a3e"} stroke={i === 0 ? "#4caf50" : "#555"} strokeWidth="1.5" />
            <text x={pos.x} y={pos.y - 10} fill="#e0e0e0" fontSize="12" textAnchor="middle" fontWeight="600">
              {i === 0 ? "Root" : `Part ${i}`}
            </text>
            <text x={pos.x} y={pos.y + 5} fill="#999" fontSize="10" textAnchor="middle">
              {dim}
            </text>
            <text x={pos.x} y={pos.y + 18} fill="#888" fontSize="9" textAnchor="middle">
              {node.joint_type} · {node.brain.num_neurons}N {node.brain.num_effectors}E
            </text>
          </g>
        );
      })}
    </svg>
  );
}
