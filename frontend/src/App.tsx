import { useEffect, useRef } from "react";

export default function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    if (!canvasRef.current) return;
    console.log("Canvas ready:", canvasRef.current);
  }, []);

  return (
    <div className="app">
      <canvas ref={canvasRef} id="sim-canvas" width={960} height={640} />
    </div>
  );
}
