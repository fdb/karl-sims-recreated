import { useEffect, useRef, useState } from "react";
import { initWasm, create_renderer, set_scene, set_paused, reset_scene } from "./wasm";
import "./App.css";

const SCENES = [
  { id: "starfish", label: "Starfish (4 flippers)" },
  { id: "hinged_pair", label: "Hinged Pair" },
  { id: "triple_chain", label: "Triple Chain" },
  { id: "universal", label: "Universal Joint (2-DOF)" },
  { id: "spherical", label: "Spherical Joint (3-DOF)" },
  { id: "swimming_starfish", label: "Swimming Starfish (water)" },
  { id: "single_box", label: "Single Box" },
  { id: "random_creature", label: "Random Creature" },
];

export default function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const initedRef = useRef(false);
  const [currentScene, setCurrentScene] = useState("starfish");
  const [paused, setPaused] = useState(false);
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

  const handleSceneChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const scene = e.target.value;
    setCurrentScene(scene);
    set_scene(scene);
    setPaused(false);
    set_paused(false);
  };

  const handlePlayPause = () => {
    const next = !paused;
    setPaused(next);
    set_paused(next);
  };

  const handleReset = () => {
    reset_scene();
    setPaused(false);
    set_paused(false);
  };

  return (
    <div className="app">
      <h1>Evolving Virtual Creatures</h1>
      <div className="controls">
        <select value={currentScene} onChange={handleSceneChange} disabled={!ready}>
          {SCENES.map((s) => (
            <option key={s.id} value={s.id}>
              {s.label}
            </option>
          ))}
        </select>
        <button onClick={handlePlayPause} disabled={!ready}>
          {paused ? "Play" : "Pause"}
        </button>
        <button onClick={handleReset} disabled={!ready}>
          Reset
        </button>
      </div>
      <canvas ref={canvasRef} id="sim-canvas" width={960} height={640} />
      <p className="hint">Drag to orbit. Scroll to zoom.</p>
    </div>
  );
}
