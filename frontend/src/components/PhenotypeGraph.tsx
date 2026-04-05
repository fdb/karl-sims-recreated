import type { PhenotypeInfo } from "../api";
import ZoomableSvg from "./ZoomableSvg";

/**
 * Visualizes the *realized* phenotype — the actual bodies and joints that
 * result from developing the genotype graph. The phenotype is a tree
 * (each non-root body has exactly one incoming joint from its parent).
 * Node labels show the body's size and the joint type connecting it
 * to its parent. The genome-node origin is shown as a muted subtitle so
 * you can correlate phenotype bodies back to the genotype graph.
 */
export default function PhenotypeGraph({ info }: { info: PhenotypeInfo }) {
  const nodeW = 150;
  const nodeH = 68;
  const gapX = 180;
  const gapY = 100;

  // Build parent lookup: for each body, which joint points to it?
  const parentOf = new Map<number, number>();
  for (const j of info.joints) {
    parentOf.set(j.child, j.parent);
  }

  // BFS from the root body to determine depth levels.
  const levels: number[][] = [];
  const visited = new Set<number>();
  const queue: { id: number; level: number }[] = [
    { id: info.root, level: 0 },
  ];
  while (queue.length > 0) {
    const { id, level } = queue.shift()!;
    if (visited.has(id)) continue;
    visited.add(id);
    if (!levels[level]) levels[level] = [];
    levels[level].push(id);
    for (const j of info.joints) {
      if (j.parent === id && !visited.has(j.child)) {
        queue.push({ id: j.child, level: level + 1 });
      }
    }
  }
  // Safety: any stray body not reached via joints
  for (const b of info.bodies) {
    if (!visited.has(b.id)) {
      const lvl = levels.length;
      if (!levels[lvl]) levels[lvl] = [];
      levels[lvl].push(b.id);
    }
  }

  const positions: { x: number; y: number }[] = [];
  const maxRow = Math.max(...levels.map((r) => r.length), 1);
  const svgW = Math.max(maxRow * gapX + 80, 500);
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

  const bodyById = new Map(info.bodies.map((b) => [b.id, b]));

  return (
    <ZoomableSvg viewBoxWidth={svgW} viewBoxHeight={svgH}>
      <defs>
        <marker
          id="phen-arrow"
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

      {/* Joints (edges) */}
      {info.joints.map((j, i) => {
        const from = positions[j.parent];
        const to = positions[j.child];
        if (!from || !to) return null;
        return (
          <g key={`j${i}`}>
            <line
              x1={from.x}
              y1={from.y + nodeH / 2}
              x2={to.x}
              y2={to.y - nodeH / 2}
              stroke="var(--color-border)"
              strokeWidth="1.5"
              markerEnd="url(#phen-arrow)"
              opacity="0.6"
            />
            <text
              x={(from.x + to.x) / 2 + 4}
              y={(from.y + to.y) / 2}
              fill="var(--color-text-muted)"
              fontSize="9"
            >
              {j.joint_type}
            </text>
          </g>
        );
      })}

      {/* Bodies (nodes) */}
      {info.bodies.map((body) => {
        const pos = positions[body.id];
        if (!pos) return null;
        // Convert half-extents to full dimensions for display.
        const dims = body.half_extents
          .map((h) => (h * 2).toFixed(2))
          .join(" × ");
        const isRoot = body.id === info.root;
        const parent = parentOf.get(body.id);
        const parentBody = parent !== undefined ? bodyById.get(parent) : undefined;

        return (
          <g key={`b${body.id}`}>
            <rect
              x={pos.x - nodeW / 2}
              y={pos.y - nodeH / 2}
              width={nodeW}
              height={nodeH}
              rx="6"
              fill={
                isRoot ? "var(--color-success)" : "var(--color-bg-elevated)"
              }
              fillOpacity={isRoot ? 0.15 : 1}
              stroke={
                isRoot ? "var(--color-success)" : "var(--color-border)"
              }
              strokeWidth="1.5"
            />
            <text
              x={pos.x - nodeW / 2 + 8}
              y={pos.y - nodeH / 2 + 16}
              fill="var(--color-text-primary)"
              fontSize="12"
              fontWeight="600"
            >
              {isRoot ? "Root" : `Body ${body.id}`}
            </text>
            <text
              x={pos.x + nodeW / 2 - 8}
              y={pos.y - nodeH / 2 + 16}
              fill="var(--color-text-muted)"
              fontSize="9"
              textAnchor="end"
            >
              ← Node {body.genome_node}
            </text>
            <text
              x={pos.x - nodeW / 2 + 8}
              y={pos.y - nodeH / 2 + 32}
              fill="var(--color-text-secondary)"
              fontSize="10"
              fontFamily="ui-monospace, monospace"
            >
              {dims}
            </text>
            <text
              x={pos.x - nodeW / 2 + 8}
              y={pos.y - nodeH / 2 + 48}
              fill="var(--color-accent)"
              fontSize="10"
              fontWeight="500"
            >
              {body.joint_type}
              {!isRoot && parentBody
                ? ` ← Body ${parentBody.id}`
                : ""}
            </text>
            <text
              x={pos.x + nodeW / 2 - 8}
              y={pos.y + nodeH / 2 - 6}
              fill="var(--color-text-muted)"
              fontSize="9"
              textAnchor="end"
            >
              d={body.depth}
            </text>
          </g>
        );
      })}
    </ZoomableSvg>
  );
}
