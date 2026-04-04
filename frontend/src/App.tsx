import { useEffect, useRef, useState } from "react";
import { initWasm, create_renderer } from "./wasm";
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

  return (
    <div className="app">
      <nav className="nav">
        <a
          href="/"
          onClick={(e) => {
            e.preventDefault();
            navigate("/");
          }}
          className="nav-brand"
        >
          Evolving Virtual Creatures
        </a>
        <div className="nav-links">
          <a
            href="/"
            onClick={(e) => {
              e.preventDefault();
              navigate("/");
            }}
            className={
              route.path === "home" ||
              route.path === "evolution" ||
              route.path === "creature"
                ? "active"
                : ""
            }
          >
            Dashboard
          </a>
          <a
            href="/viewer"
            onClick={(e) => {
              e.preventDefault();
              navigate("/viewer");
            }}
            className={route.path === "viewer" ? "active" : ""}
          >
            Viewer
          </a>
        </div>
      </nav>

      <main className="main">
        {route.path === "home" && <EvolutionList />}
        {route.path === "evolution" && (
          <EvolutionDetail evoId={Number(route.params.evoId)} />
        )}
        {route.path === "creature" && (
          <CreatureDetail
            evoId={Number(route.params.evoId)}
            creatureId={Number(route.params.creatureId)}
            canvasVisible={route.path === "creature"}
            onShowCanvas={() => {}}
          />
        )}
        {route.path === "viewer" && <Viewer ready={ready} />}
      </main>

      {/* Canvas is always in DOM — wgpu renderer is bound to it */}
      <div style={{ display: (route.path === "viewer" || route.path === "creature") ? "block" : "none" }}>
        <canvas ref={canvasRef} id="sim-canvas" width={960} height={640} />
      </div>
    </div>
  );
}
