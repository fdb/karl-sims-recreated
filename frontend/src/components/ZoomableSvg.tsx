import { useRef, useState, useEffect, type ReactNode } from "react";

interface Props {
  viewBoxWidth: number;
  viewBoxHeight: number;
  children: ReactNode;
  /** CSS height for the outer container. Width is always 100%. */
  height?: string;
}

/**
 * Wraps its SVG content with wheel-to-zoom and drag-to-pan.
 *
 * Zoom anchors on the cursor position (like maps), so scrolling up on a
 * specific node zooms into that node. Panning simply shifts the content
 * in the currently-visible scale. A "Reset" button returns to scale=1.
 *
 * Implementation: we apply a single `<g transform="translate(tx ty) scale(s)">`
 * wrapper around the children. Screen-pixel mouse deltas are converted to
 * viewBox-unit deltas using the SVG element's measured width, so pan speed
 * feels consistent regardless of container size.
 */
export default function ZoomableSvg({
  viewBoxWidth,
  viewBoxHeight,
  children,
  height = "500px",
}: Props) {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const [scale, setScale] = useState(1);
  const [tx, setTx] = useState(0);
  const [ty, setTy] = useState(0);
  const [dragging, setDragging] = useState(false);
  const dragStart = useRef<{ x: number; y: number; tx: number; ty: number } | null>(
    null,
  );

  const screenToViewBoxScale = (): number => {
    if (!svgRef.current) return 1;
    const rect = svgRef.current.getBoundingClientRect();
    return viewBoxWidth / rect.width;
  };

  // Wheel handler must be non-passive to call preventDefault (prevents page
  // scroll while zooming). React's onWheel is passive by default, so we
  // attach via useEffect.
  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;

    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const rect = svg.getBoundingClientRect();
      // Mouse position in viewBox user units.
      const mx = ((e.clientX - rect.left) / rect.width) * viewBoxWidth;
      const my = ((e.clientY - rect.top) / rect.height) * viewBoxHeight;
      const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
      setScale((prevScale) => {
        const newScale = Math.min(Math.max(prevScale * factor, 0.2), 8);
        const actualFactor = newScale / prevScale;
        // Zoom anchored at mouse position: new_t = m - (m - t) * factor
        setTx((prevTx) => mx - (mx - prevTx) * actualFactor);
        setTy((prevTy) => my - (my - prevTy) * actualFactor);
        return newScale;
      });
    };

    svg.addEventListener("wheel", onWheel, { passive: false });
    return () => svg.removeEventListener("wheel", onWheel);
  }, [viewBoxWidth, viewBoxHeight]);

  const onMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    setDragging(true);
    dragStart.current = { x: e.clientX, y: e.clientY, tx, ty };
  };

  const onMouseMove = (e: React.MouseEvent) => {
    if (!dragging || !dragStart.current) return;
    const k = screenToViewBoxScale();
    const dx = (e.clientX - dragStart.current.x) * k;
    const dy = (e.clientY - dragStart.current.y) * k;
    setTx(dragStart.current.tx + dx);
    setTy(dragStart.current.ty + dy);
  };

  const onMouseUp = () => {
    setDragging(false);
    dragStart.current = null;
  };

  const reset = () => {
    setScale(1);
    setTx(0);
    setTy(0);
  };

  return (
    <div className="relative" style={{ height }}>
      <svg
        ref={svgRef}
        width="100%"
        height="100%"
        viewBox={`0 0 ${viewBoxWidth} ${viewBoxHeight}`}
        preserveAspectRatio="xMidYMid meet"
        style={{ cursor: dragging ? "grabbing" : "grab", userSelect: "none" }}
        onMouseDown={onMouseDown}
        onMouseMove={onMouseMove}
        onMouseUp={onMouseUp}
        onMouseLeave={onMouseUp}
      >
        <g transform={`translate(${tx} ${ty}) scale(${scale})`}>{children}</g>
      </svg>
      <div className="absolute top-2 right-2 flex items-center gap-2">
        <span className="text-xs text-text-muted font-mono bg-bg-surface/80 px-1.5 py-0.5 rounded">
          {(scale * 100).toFixed(0)}%
        </span>
        <button
          type="button"
          onClick={reset}
          className="text-xs text-text-secondary hover:text-text-primary bg-bg-surface/80 hover:bg-bg-elevated px-2 py-0.5 rounded border border-border-subtle transition-colors"
        >
          Reset
        </button>
      </div>
    </div>
  );
}
