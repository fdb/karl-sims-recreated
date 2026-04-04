import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { initWasm, sim_init_random, sim_step, sim_body_count } from "../wasm";

const SEEDS = [
  { seed: 42, label: "Random Creature A" },
  { seed: 7, label: "Random Creature B" },
  { seed: 1337, label: "Random Creature C" },
  { seed: 99, label: "Random Creature D" },
  { seed: 256, label: "Random Creature E" },
];

export default function Viewer() {
  const mountRef = useRef<HTMLDivElement>(null);
  const [currentSeed, setCurrentSeed] = useState(42);
  const stateRef = useRef<{
    renderer: THREE.WebGLRenderer;
    animId: number;
  } | null>(null);

  useEffect(() => {
    const mount = mountRef.current;
    if (!mount) return;
    let cancelled = false;

    (async () => {
      await initWasm();
      if (cancelled) return;

      // Scene
      const scene = new THREE.Scene();
      scene.background = new THREE.Color(0x1a2a30);
      scene.fog = new THREE.Fog(0x1a2a30, 12, 40);

      const camera = new THREE.PerspectiveCamera(45, mount.clientWidth / mount.clientHeight, 0.01, 100);
      camera.position.set(3, 2.5, 5);
      camera.lookAt(0, 0, 0);

      const renderer = new THREE.WebGLRenderer({ antialias: true });
      renderer.setPixelRatio(window.devicePixelRatio);
      renderer.setSize(mount.clientWidth, mount.clientHeight);
      renderer.shadowMap.enabled = true;
      renderer.shadowMap.type = THREE.PCFSoftShadowMap;
      mount.appendChild(renderer.domElement);

      const controls = new OrbitControls(camera, renderer.domElement);
      controls.enableDamping = true;
      controls.dampingFactor = 0.08;

      // Lighting
      scene.add(new THREE.AmbientLight(0xffffff, 0.4));
      const sun = new THREE.DirectionalLight(0xfff5e0, 1.2);
      sun.position.set(4, 8, 5);
      sun.castShadow = true;
      scene.add(sun);
      scene.add(Object.assign(new THREE.DirectionalLight(0x8ab4c0, 0.3), { position: new THREE.Vector3(-3, 2, -4) }));

      // Ground
      const checkerCanvas = document.createElement("canvas");
      checkerCanvas.width = 256; checkerCanvas.height = 256;
      const ctx = checkerCanvas.getContext("2d")!;
      for (let r = 0; r < 8; r++) for (let c = 0; c < 8; c++) {
        ctx.fillStyle = (r + c) % 2 === 0 ? "#2a3d44" : "#243540";
        ctx.fillRect(c * 32, r * 32, 32, 32);
      }
      const tex = new THREE.CanvasTexture(checkerCanvas);
      tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
      tex.repeat.set(8, 8);
      const ground = new THREE.Mesh(new THREE.PlaneGeometry(40, 40), new THREE.MeshLambertMaterial({ map: tex }));
      ground.rotation.x = -Math.PI / 2;
      ground.receiveShadow = true;
      scene.add(ground);

      // Body meshes (rebuilt on seed change via closure over meshesRef)
      const meshesRef = { current: [] as THREE.Mesh[] };
      const bodyMat = new THREE.MeshLambertMaterial({ color: 0xeae5d9 });

      const buildCreature = (seed: number) => {
        meshesRef.current.forEach((m) => { m.geometry.dispose(); scene.remove(m); });
        meshesRef.current = [];
        const handle = sim_init_random(BigInt(seed));
        const count = sim_body_count(handle);
        for (let i = 0; i < count; i++) {
          const mesh = new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), bodyMat);
          mesh.castShadow = true;
          mesh.receiveShadow = true;
          scene.add(mesh);
          meshesRef.current.push(mesh);
        }
        applyTransforms(sim_step(handle), meshesRef.current);
        return handle;
      };

      let handle = buildCreature(currentSeed);

      // Listen to seed changes via a ref so the animation loop picks it up
      const seedRef = { current: currentSeed, changed: false, next: currentSeed };

      // Expose rebuild to the React state update — via a custom event
      const onSeedChange = (e: Event) => {
        const seed = (e as CustomEvent<number>).detail;
        handle = buildCreature(seed);
      };
      mount.addEventListener("seedchange", onSeedChange);

      // Resize
      const resizeObs = new ResizeObserver(() => {
        camera.aspect = mount.clientWidth / mount.clientHeight;
        camera.updateProjectionMatrix();
        renderer.setSize(mount.clientWidth, mount.clientHeight);
      });
      resizeObs.observe(mount);

      // Render loop
      let animId: number;
      const animate = () => {
        animId = requestAnimationFrame(animate);
        applyTransforms(sim_step(handle), meshesRef.current);
        controls.update();
        renderer.render(scene, camera);
      };
      animId = requestAnimationFrame(animate);
      stateRef.current = { renderer, animId };

      return () => {
        mount.removeEventListener("seedchange", onSeedChange);
        resizeObs.disconnect();
      };
    })();

    return () => {
      cancelled = true;
      if (stateRef.current) {
        cancelAnimationFrame(stateRef.current.animId);
        stateRef.current.renderer.dispose();
        stateRef.current.renderer.domElement.remove();
        stateRef.current = null;
      }
    };
  }, []);

  // When seed changes from UI, fire a custom event into the canvas mount
  const handleSeedChange = (seed: number) => {
    setCurrentSeed(seed);
    mountRef.current?.dispatchEvent(new CustomEvent("seedchange", { detail: seed }));
  };

  return (
    <div>
      <div className="flex items-center gap-4 mb-4">
        <select
          value={currentSeed}
          onChange={(e) => handleSeedChange(Number(e.target.value))}
          className="px-3 py-1.5 bg-bg-surface border border-border rounded-md text-sm text-text-primary focus:outline-none focus:border-accent"
        >
          {SEEDS.map((s) => (
            <option key={s.seed} value={s.seed}>{s.label}</option>
          ))}
        </select>
        <span className="text-text-muted text-xs">Drag to orbit · Scroll to zoom</span>
      </div>
      <div
        ref={mountRef}
        className="bg-bg-surface border border-border-subtle rounded-lg overflow-hidden"
        style={{ height: "600px" }}
      />
    </div>
  );
}

function applyTransforms(data: Float64Array, meshes: THREE.Mesh[]) {
  const STRIDE = 10;
  for (let i = 0; i < meshes.length; i++) {
    const b = i * STRIDE;
    meshes[i].position.set(data[b], data[b + 1], data[b + 2]);
    meshes[i].quaternion.set(data[b + 4], data[b + 5], data[b + 6], data[b + 3]);
    meshes[i].scale.set(data[b + 7] * 2, data[b + 8] * 2, data[b + 9] * 2);
  }
}
