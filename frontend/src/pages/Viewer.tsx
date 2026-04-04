import { useState } from "react";
import { set_scene, set_paused, reset_scene } from "../wasm";

const SCENES = [
  { id: "starfish", label: "Starfish (4 flippers)" },
  { id: "hinged_pair", label: "Hinged Pair" },
  { id: "triple_chain", label: "Triple Chain" },
  { id: "universal", label: "Universal Joint (2-DOF)" },
  { id: "spherical", label: "Spherical Joint (3-DOF)" },
  { id: "swimming_starfish", label: "Swimming Starfish (water)" },
  { id: "single_box", label: "Single Box" },
  { id: "random_creature", label: "Random Creature" },
  { id: "following", label: "Following (light target)" },
  { id: "mini_evolution", label: "Mini Evolution (in-browser)" },
];

interface Props {
  ready: boolean;
}

export default function Viewer({ ready }: Props) {
  const [currentScene, setCurrentScene] = useState("starfish");
  const [paused, setPaused] = useState(false);

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
    <div>
      <div className="controls">
        <select
          value={currentScene}
          onChange={handleSceneChange}
          disabled={!ready}
        >
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
      <p className="hint">Drag to orbit. Scroll to zoom.</p>
    </div>
  );
}
