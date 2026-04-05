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
 * Zoom anchors on the cursor position (like maps): the content point under
 * the mouse before the wheel event stays under the mouse after. Panning
 * simply shifts the content. Reset returns to identity (k=1, x=0, y=0).
 *
 * Implementation: we hold a single transform `{k, x, y}` — k is the zoom
 * scale, (x, y) is the translation — and apply it as
 * `<g transform="translate(x y) scale(k)">` around the children. The
 * transform is in the SVG's viewBox coordinate system. Mouse events are
 * converted into viewBox coordinates via svg.getScreenCTM().inverse(),
 * which correctly handles preserveAspectRatio letterboxing.
 *
 * Keeping k/x/y in one state object is critical: d3-zoom does the same.
 * Calling setScale + setTx + setTy as three separate calls (or nested
 * updater functions) produces intermediate states where only k has
 * updated but (x, y) haven't, which renders the transform incorrectly
 * for one frame and makes the zoom visibly drift.
 */
interface Transform {
  k: number;
  x: number;
  y: number;
}

const IDENTITY: Transform = { k: 1, x: 0, y: 0 };
const MIN_SCALE = 0.2;
const MAX_SCALE = 8;

export default function ZoomableSvg({
  viewBoxWidth,
  viewBoxHeight,
  children,
  height = "500px",
}: Props) {
  const svgRef = useRef<SVGSVGElement | null>(null);
  const [transform, setTransform] = useState<Transform>(IDENTITY);
  const transformRef = useRef<Transform>(IDENTITY);
  transformRef.current = transform;

  const [dragging, setDragging] = useState(false);
  const dragStart = useRef<{ mouseX: number; mouseY: number; x: number; y: number } | null>(
    null,
  );

  /**
   * Convert a screen mouse event to viewBox coordinates using the SVG's
   * own screen→viewBox matrix. Handles letterboxing, element resizing,
   * and any ancestor CSS transforms correctly.
   */
  const screenToSvg = (clientX: number, clientY: number): { x: number; y: number } | null => {
    const svg = svgRef.current;
    if (!svg) return null;
    const ctm = svg.getScreenCTM();
    if (!ctm) return null;
    const pt = svg.createSVGPoint();
    pt.x = clientX;
    pt.y = clientY;
    const p = pt.matrixTransform(ctm.inverse());
    return { x: p.x, y: p.y };
  };

  // Wheel handler must be non-passive to preventDefault (React onWheel is passive).
  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return;

    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const mouse = screenToSvg(e.clientX, e.clientY);
      if (!mouse) return;
      const current = transformRef.current;
      const factor = e.deltaY < 0 ? 1.15 : 1 / 1.15;
      const newK = Math.min(Math.max(current.k * factor, MIN_SCALE), MAX_SCALE);
      if (newK === current.k) return; // hit a bound, nothing to do

      // Anchor the content-point under the mouse at the same screen spot.
      //
      // Forward transform: displayed = content * k + (x, y)
      //                  ⇒ content_under_mouse = (mouse - (x, y)) / k
      //
      // Constraint: after the zoom, the same content point must still be
      // at the mouse position:
      //   mouse = content_under_mouse * newK + (x_new, y_new)
      //   (x_new, y_new) = mouse - content_under_mouse * newK
      //                  = mouse - ((mouse - (x, y)) / k) * newK
      //                  = mouse - (mouse - (x, y)) * (newK / k)
      const r = newK / current.k;
      setTransform({
        k: newK,
        x: mouse.x - (mouse.x - current.x) * r,
        y: mouse.y - (mouse.y - current.y) * r,
      });
    };

    svg.addEventListener("wheel", onWheel, { passive: false });
    return () => svg.removeEventListener("wheel", onWheel);
  }, []);

  const onMouseDown = (e: React.MouseEvent) => {
    if (e.button !== 0) return;
    const svg = svgRef.current;
    if (!svg) return;
    const ctm = svg.getScreenCTM();
    if (!ctm) return;
    // Record drag origin in screen pixels and the translation at drag start.
    setDragging(true);
    dragStart.current = {
      mouseX: e.clientX,
      mouseY: e.clientY,
      x: transform.x,
      y: transform.y,
    };
  };

  const onMouseMove = (e: React.MouseEvent) => {
    if (!dragging || !dragStart.current) return;
    const svg = svgRef.current;
    if (!svg) return;
    const ctm = svg.getScreenCTM();
    if (!ctm) return;
    // Convert screen-pixel drag delta to viewBox-unit delta via CTM.a.
    // For uniformly-scaled SVGs, CTM.a (horizontal pixel/unit) === CTM.d.
    const unitsPerPixel = 1 / ctm.a;
    const dx = (e.clientX - dragStart.current.mouseX) * unitsPerPixel;
    const dy = (e.clientY - dragStart.current.mouseY) * unitsPerPixel;
    setTransform({
      k: transformRef.current.k,
      x: dragStart.current.x + dx,
      y: dragStart.current.y + dy,
    });
  };

  const onMouseUp = () => {
    setDragging(false);
    dragStart.current = null;
  };

  const reset = () => setTransform(IDENTITY);

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
        <g
          transform={`translate(${transform.x} ${transform.y}) scale(${transform.k})`}
        >
          {children}
        </g>
      </svg>
      <div className="absolute top-2 right-2 flex items-center gap-2">
        <span className="text-xs text-text-muted font-mono bg-bg-surface/80 px-1.5 py-0.5 rounded">
          {(transform.k * 100).toFixed(0)}%
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
