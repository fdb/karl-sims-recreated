import { useEffect, useRef, useState } from "react";
import { initWasm, create_renderer, set_rendering_active } from "./wasm";
import { useRoute } from "./hooks/useRoute";
import { navigate } from "./router";
import EvolutionList from "./pages/EvolutionList";
import EvolutionDetail from "./pages/EvolutionDetail";
import CreatureDetail from "./pages/CreatureDetail";
import Viewer from "./pages/Viewer";
import "./App.css";

export default function App() {
  const route = useRoute();
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const initedRef = useRef(false);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    if (initedRef.current) return;
    initedRef.current = true;

    (async () => {
      await initWasm();
      await create_renderer("sim-canvas");
      console.log("Renderer created");
      setReady(true);
    })();
  }, []);

  const showCanvas = route.path === "viewer" || route.path === "creature";

  // Pause rendering when canvas is not visible — saves CPU
  useEffect(() => {
    if (ready) {
      set_rendering_active(showCanvas);
    }
  }, [showCanvas, ready]);

  return (
    <div className="min-h-screen bg-bg-base text-text-primary">
      {/* Header */}
      <header className="bg-bg-header border-b border-border sticky top-0 z-50">
        <div className="max-w-[1600px] mx-auto px-6 h-14 flex items-center justify-between">
          <a
            href="/"
            onClick={(e) => {
              e.preventDefault();
              navigate("/");
            }}
            className="text-lg font-semibold text-text-primary hover:text-accent transition-colors"
          >
            Evolving Virtual Creatures
          </a>
          <nav className="flex gap-1">
            <a
              href="/"
              onClick={(e) => {
                e.preventDefault();
                navigate("/");
              }}
              className={`px-4 py-2 rounded-md text-sm transition-colors ${
                route.path === "home" ||
                route.path === "evolution" ||
                route.path === "creature"
                  ? "bg-bg-elevated text-text-primary"
                  : "text-text-secondary hover:text-text-primary hover:bg-bg-surface"
              }`}
            >
              Dashboard
            </a>
            <a
              href="/viewer"
              onClick={(e) => {
                e.preventDefault();
                navigate("/viewer");
              }}
              className={`px-4 py-2 rounded-md text-sm transition-colors ${
                route.path === "viewer"
                  ? "bg-bg-elevated text-text-primary"
                  : "text-text-secondary hover:text-text-primary hover:bg-bg-surface"
              }`}
            >
              Viewer
            </a>
          </nav>
        </div>
      </header>

      {/* Main content */}
      <main className="max-w-[1600px] mx-auto px-6 py-6">
        {route.path === "home" && <EvolutionList />}
        {route.path === "evolution" && (
          <EvolutionDetail evoId={Number(route.params.evoId)} />
        )}
        {route.path === "creature" && (
          <CreatureDetail
            evoId={Number(route.params.evoId)}
            creatureId={Number(route.params.creatureId)}
          />
        )}
        {route.path === "viewer" && <Viewer ready={ready} />}
      </main>

      {/* Canvas always in DOM — wgpu renderer is bound to it */}
      <div className={showCanvas ? "" : "absolute -left-[9999px]"}>
        <canvas
          ref={canvasRef}
          id="sim-canvas"
          width={960}
          height={640}
          className="border border-border rounded"
        />
      </div>
    </div>
  );
}
