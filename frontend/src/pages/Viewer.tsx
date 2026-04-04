import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { initWasm, scene_init, scene_step, scene_body_count, scene_transforms } from "../wasm";

const DEMOS = [
  { name: "swimmer-starfish", label: "Swimming Starfish", env: "Water" },
  { name: "swimmer-snake", label: "Swimming Snake", env: "Water" },
  { name: "walker-inchworm", label: "Land Inchworm", env: "Land" },
  { name: "walker-lizard", label: "Sprawling Lizard", env: "Land" },
];

const SIM_DURATION = 10.0;
const DT = 1.0 / 60.0;
const TOTAL_FRAMES = Math.round(SIM_DURATION / DT);
const STRIDE = 10; // values per body

export default function Viewer() {
  const mountRef = useRef<HTMLDivElement>(null);
  const [currentDemo, setCurrentDemo] = useState(0);
  const [progress, setProgress] = useState(0);
  const [isComputing, setIsComputing] = useState(true);
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

      const demo = DEMOS[currentDemo];

      // Scene
      const scene = new THREE.Scene();
      const bgColor = demo.env === "Water" ? 0x0d1e2e : 0x1a2a30;
      scene.background = new THREE.Color(bgColor);
      scene.fog = new THREE.Fog(bgColor, 30, 120);

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
      const fill = new THREE.DirectionalLight(0x8ab4c0, 0.3);
      fill.position.set(-3, 2, -4);
      scene.add(fill);

      // Ground/water plane
      if (demo.env === "Land") {
        const checker = buildCheckerTexture();
        const ground = new THREE.Mesh(
          new THREE.PlaneGeometry(80, 80),
          new THREE.MeshLambertMaterial({ map: checker })
        );
        ground.rotation.x = -Math.PI / 2;
        ground.position.y = 0;
        ground.receiveShadow = true;
        scene.add(ground);
        const grid = new THREE.GridHelper(80, 40, 0x4a5a50, 0x3a4a40);
        grid.position.y = 0.002;
        scene.add(grid);
      } else {
        const waterMat = new THREE.MeshLambertMaterial({
          color: 0x1a5080, transparent: true, opacity: 0.35, side: THREE.DoubleSide,
        });
        const water = new THREE.Mesh(new THREE.PlaneGeometry(80, 80), waterMat);
        water.rotation.x = -Math.PI / 2;
        scene.add(water);
        const grid = new THREE.GridHelper(80, 40, 0x2a6090, 0x1a4060);
        (grid.material as THREE.LineBasicMaterial).transparent = true;
        (grid.material as THREE.LineBasicMaterial).opacity = 0.4;
        grid.position.y = 0.01;
        scene.add(grid);
      }

      // Init scene creature
      let handle;
      try {
        handle = scene_init(demo.name, demo.env);
      } catch (e) {
        console.error("scene_init failed:", e);
        return;
      }

      const bodyCount = scene_body_count(handle);
      const bodyMat = new THREE.MeshLambertMaterial({ color: 0xeae5d9 });
      const meshes: THREE.Mesh[] = [];
      for (let i = 0; i < bodyCount; i++) {
        const mesh = new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), bodyMat);
        mesh.castShadow = true;
        mesh.receiveShadow = true;
        scene.add(mesh);
        meshes.push(mesh);
      }

      // Initial state
      const initialTransforms = scene_transforms(handle);
      applyTransforms(initialTransforms, meshes);

      // Auto-fit camera
      const box = new THREE.Box3();
      scene.updateMatrixWorld(true);
      for (const mesh of meshes) box.expandByObject(mesh);
      if (!box.isEmpty()) {
        const center = new THREE.Vector3();
        box.getCenter(center);
        const size = new THREE.Vector3();
        box.getSize(size);
        const maxDim = Math.max(size.x, size.y, size.z, 0.5);
        const dist = maxDim * 3.5;
        controls.target.copy(center);
        camera.position.set(center.x + dist, center.y + dist * 0.6, center.z + dist);
        camera.lookAt(center);
        camera.updateProjectionMatrix();
        controls.update();
      }

      // Resize
      const resizeObs = new ResizeObserver(() => {
        camera.aspect = mount.clientWidth / mount.clientHeight;
        camera.updateProjectionMatrix();
        renderer.setSize(mount.clientWidth, mount.clientHeight);
      });
      resizeObs.observe(mount);

      renderer.render(scene, camera);

      // Pre-compute all frames
      const allFrames: Float64Array[] = [initialTransforms.slice()];
      const BATCH_SIZE = 30;
      for (let start = 0; start < TOTAL_FRAMES; start += BATCH_SIZE) {
        if (cancelled) return;
        await new Promise<void>((r) => setTimeout(r, 0));

        const end = Math.min(start + BATCH_SIZE, TOTAL_FRAMES);
        for (let i = start; i < end; i++) {
          const t = scene_step(handle);
          allFrames.push(t.slice());
        }
        setProgress(end / TOTAL_FRAMES);
        renderer.render(scene, camera);
      }

      if (cancelled) return;
      setIsComputing(false);

      // Playback loop
      let frameIdx = 0;
      let animId: number;
      const animate = () => {
        animId = requestAnimationFrame(animate);
        frameIdx = (frameIdx + 1) % allFrames.length;
        applyTransforms(allFrames[frameIdx], meshes);
        controls.update();
        renderer.render(scene, camera);
      };
      animId = requestAnimationFrame(animate);
      stateRef.current = { renderer, animId };
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
  }, [currentDemo]);

  return (
    <div>
      <div className="flex items-center gap-4 mb-4">
        <select
          value={currentDemo}
          onChange={(e) => { setCurrentDemo(Number(e.target.value)); setIsComputing(true); setProgress(0); }}
          className="px-3 py-1.5 bg-bg-surface border border-border rounded-md text-sm text-text-primary focus:outline-none focus:border-accent"
        >
          {DEMOS.map((d, i) => (
            <option key={d.name} value={i}>
              {d.label} ({d.env})
            </option>
          ))}
        </select>
        <span className="text-text-muted text-xs">Drag to orbit · Scroll to zoom</span>
      </div>
      <div
        ref={mountRef}
        className="bg-bg-surface border border-border-subtle rounded-lg overflow-hidden relative"
        style={{ height: "600px" }}
      >
        {isComputing && (
          <div
            style={{
              position: "absolute", bottom: 0, left: 0, right: 0,
              padding: "8px 12px", background: "rgba(0,0,0,0.5)",
              display: "flex", alignItems: "center", gap: 8, zIndex: 10,
            }}
          >
            <div style={{ flex: 1, height: 4, background: "#2a3d44", borderRadius: 2, overflow: "hidden" }}>
              <div style={{ height: "100%", width: `${progress * 100}%`, background: "#4a9eca", borderRadius: 2, transition: "width 0.1s" }} />
            </div>
            <span style={{ color: "#aac", fontSize: 11, whiteSpace: "nowrap" }}>
              {Math.round(progress * 100)}%
            </span>
          </div>
        )}
      </div>
    </div>
  );
}

function buildCheckerTexture(): THREE.CanvasTexture {
  const size = 512;
  const tileCount = 8;
  const tileSize = size / tileCount;
  const canvas = document.createElement("canvas");
  canvas.width = size;
  canvas.height = size;
  const ctx = canvas.getContext("2d")!;
  for (let r = 0; r < tileCount; r++) {
    for (let c = 0; c < tileCount; c++) {
      ctx.fillStyle = (r + c) % 2 === 0 ? "#5a5248" : "#2e2b27";
      ctx.fillRect(c * tileSize, r * tileSize, tileSize, tileSize);
    }
  }
  const tex = new THREE.CanvasTexture(canvas);
  tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
  tex.repeat.set(12, 12);
  return tex;
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
