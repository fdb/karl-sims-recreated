import { useEffect, useRef } from "react";
import { initWasm, create_renderer } from "./wasm";

export default function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const initedRef = useRef(false);

  useEffect(() => {
    if (initedRef.current) return;
    initedRef.current = true;

    (async () => {
      await initWasm();
      await create_renderer("sim-canvas");
      console.log("Renderer created");
    })();
  }, []);

  return (
    <div className="app">
      <canvas ref={canvasRef} id="sim-canvas" width={960} height={640} />
    </div>
  );
}
