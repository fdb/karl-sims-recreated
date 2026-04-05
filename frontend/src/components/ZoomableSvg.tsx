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

  /**
   * Convert a screen-space mouse event to viewBox-space coordinates.
   * Uses getScreenCTM() which is the native SVG screen→viewBox matrix —
   * correctly handles preserveAspectRatio letterboxing, element resizing,
   * and CSS transforms on ancestors. This is what d3-zoom's `pointer()`
   * does when operating on SVG targets.
   */
  const screenToSvg = (clientX: number, clientY: number): { x: number; y: number } => {
    const svg = svgRef.current;
    if (!svg) return { x: 0, y: 0 };
    const ctm = svg.getScreenCTM();
    if (!ctm) return { x: 0, y: 0 };
    const pt = svg.createSVGPoint();
    pt.x = clientX;
    pt.y = clientY;
    const p = pt.matrixTransform(ctm.inverse());
    return { x: p.x, y: p.y };
  };

  /**
   * Convert a screen-pixel delta to a viewBox-unit delta for panning.
   * With preserveAspectRatio="xMidYMid meet", both axes share a single
   * uniform scale (the MIN of the per-axis scales), so we derive
   * units-per-pixel from either axis via the inverse of the screen CTM.
   */
  const svgUnitsPerPixel = (): number => {
    const svg = svgRef.current;
    if (!svg) return 1;
    const ctm = svg.getScreenCTM();
    if (!ctm) return 1;
    // CTM.a is the horizontal scale from viewBox-units to screen pixels.
    // For uniformly-scaled SVGs a === d, so either works.
    return 1 / ctm.a;
  };

  // Wheel handler must be non-passive to call preventDefault (prevents page
  // scroll while zooming). React's onWheel is passive by default, so we
  // attach via useEffect.
  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;

    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      // Mouse position in SVG viewBox user units (d3-zoom equivalent).
      const { x: mx, y: my } = screenToSvg(e.clientX, e.clientY);
      const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
      setScale((prevScale) => {
        const newScale = Math.min(Math.max(prevScale * factor, 0.2), 8);
        const actualFactor = newScale / prevScale;
        // Anchor the point under the mouse: content under mouse stays fixed.
        //   displayed = content * k + t
        //   content_under_mouse = (m - t) / k   (from old transform)
        //   t_new = m - content_under_mouse * k_new
        //         = m - (m - t_old) * (k_new / k_old)
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
    const k = svgUnitsPerPixel();
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
