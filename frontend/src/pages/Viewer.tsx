import { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import {
  initWasm,
  scene_init, scene_init_rapier,
  scene_step, scene_body_count, scene_transforms,
} from "../wasm";

// Four creatures shown simultaneously, spaced apart on X axis
const CREATURES = [
  { name: "swimmer-starfish", label: "Starfish", env: "Water" },
  { name: "swimmer-snake",    label: "Snake",    env: "Water" },
  { name: "walker-inchworm",  label: "Inchworm", env: "Land"  },
  { name: "walker-lizard",    label: "Lizard",   env: "Land"  },
];

const SPACING = 6; // world-space X offset between creatures
const STRIDE  = 10; // floats per body: px py pz qw qx qy qz hx hy hz

// Position offsets so creatures don't overlap
const OFFSETS: THREE.Vector3[] = CREATURES.map(
  (_, i) => new THREE.Vector3((i - (CREATURES.length - 1) / 2) * SPACING, 0, 0)
);

export default function Viewer() {
  const mountRef  = useRef<HTMLDivElement>(null);
  const [useRapier, setUseRapier] = useState(false);
  const [paused,    setPaused]    = useState(false);
  // Use refs for values that need to be read inside the rAF closure without re-creating it
  const pausedRef   = useRef(false);
  const useRapierRef = useRef(false);

  // Keep refs in sync with state
  useEffect(() => { pausedRef.current   = paused;    }, [paused]);
  useEffect(() => { useRapierRef.current = useRapier; }, [useRapier]);

  // restartKey increments to force a full sim re-init
  const [restartKey, setRestartKey] = useState(0);

  useEffect(() => {
    const mount = mountRef.current;
    if (!mount) return;
    let animId = 0;
    let disposed = false;
    let rendererRef: THREE.WebGLRenderer | null = null;

    (async () => {
      await initWasm();
      if (disposed) return;

      // ── Scene setup ────────────────────────────────────────────────────────
      const scene = new THREE.Scene();
      scene.background = new THREE.Color(0x111820);
      scene.fog = new THREE.Fog(0x111820, 40, 120);

      const camera = new THREE.PerspectiveCamera(
        45, mount.clientWidth / mount.clientHeight, 0.01, 200
      );
      camera.position.set(0, 4, 18);
      camera.lookAt(0, 1, 0);

      const renderer = new THREE.WebGLRenderer({ antialias: true });
      rendererRef = renderer;
      renderer.setPixelRatio(window.devicePixelRatio);
      renderer.setSize(mount.clientWidth, mount.clientHeight);
      renderer.shadowMap.enabled = true;
      renderer.shadowMap.type = THREE.PCFSoftShadowMap;
      mount.appendChild(renderer.domElement);

      const controls = new OrbitControls(camera, renderer.domElement);
      controls.target.set(0, 1, 0);
      controls.enableDamping = true;
      controls.dampingFactor = 0.08;

      // Lighting
      scene.add(new THREE.AmbientLight(0xffffff, 0.45));
      const sun = new THREE.DirectionalLight(0xfff5e0, 1.1);
      sun.position.set(6, 12, 8);
      sun.castShadow = true;
      scene.add(sun);
      const fill = new THREE.DirectionalLight(0x8ab4c0, 0.25);
      fill.position.set(-4, 3, -6);
      scene.add(fill);

      // Ground plane (shared, at y=0)
      const ground = new THREE.Mesh(
        new THREE.PlaneGeometry(120, 120),
        new THREE.MeshLambertMaterial({ map: buildCheckerTexture() })
      );
      ground.rotation.x = -Math.PI / 2;
      ground.receiveShadow = true;
      scene.add(ground);
      const grid = new THREE.GridHelper(120, 60, 0x3a4a40, 0x2a3830);
      grid.position.set(0, 0.003, 0);
      scene.add(grid);

      // ── Labels ─────────────────────────────────────────────────────────────
      CREATURES.forEach((c, i) => {
        const sprite = makeLabel(`${c.label} (${c.env})`, useRapierRef.current ? "⚡" : "");
        sprite.position.set(OFFSETS[i].x, 3.5, 0);
        scene.add(sprite);
      });

      // ── Init sim handles ───────────────────────────────────────────────────
      type Handle = ReturnType<typeof scene_init>;
      const handles: Handle[] = [];
      const meshGroups: THREE.Mesh[][] = [];

      const bodyMats = [
        new THREE.MeshLambertMaterial({ color: 0xd4c8b8 }), // starfish
        new THREE.MeshLambertMaterial({ color: 0xb8c8d4 }), // snake
        new THREE.MeshLambertMaterial({ color: 0xc8d4b8 }), // inchworm
        new THREE.MeshLambertMaterial({ color: 0xd4b8c8 }), // lizard
      ];

      for (let ci = 0; ci < CREATURES.length; ci++) {
        const c = CREATURES[ci];
        let h: Handle;
        try {
          h = useRapierRef.current
            ? scene_init_rapier(c.name, c.env)
            : scene_init(c.name, c.env);
        } catch (e) {
          console.error(`Failed to init ${c.name}:`, e);
          continue;
        }
        handles.push(h);

        const count = scene_body_count(h);
        const group: THREE.Mesh[] = [];
        for (let bi = 0; bi < count; bi++) {
          const mesh = new THREE.Mesh(
            new THREE.BoxGeometry(1, 1, 1),
            bodyMats[ci]
          );
          mesh.castShadow = true;
          mesh.receiveShadow = true;
          scene.add(mesh);
          group.push(mesh);
        }
        meshGroups.push(group);

        // Apply initial positions with creature offset
        const t = scene_transforms(h);
        applyTransforms(t, group, OFFSETS[ci]);
      }

      // ── Resize observer ────────────────────────────────────────────────────
      const resizeObs = new ResizeObserver(() => {
        camera.aspect = mount.clientWidth / mount.clientHeight;
        camera.updateProjectionMatrix();
        renderer.setSize(mount.clientWidth, mount.clientHeight);
      });
      resizeObs.observe(mount);

      // ── Real-time game loop ────────────────────────────────────────────────
      const animate = () => {
        if (disposed) return;
        animId = requestAnimationFrame(animate);

        if (!pausedRef.current) {
          for (let ci = 0; ci < handles.length; ci++) {
            try {
              const t = scene_step(handles[ci]);
              applyTransforms(t, meshGroups[ci], OFFSETS[ci]);
            } catch (e) {
              console.error(`Step error on creature ${ci}:`, e);
            }
          }
        }

        controls.update();
        renderer.render(scene, camera);
      };
      animId = requestAnimationFrame(animate);

    })();

    return () => {
      disposed = true;
      cancelAnimationFrame(animId);
      if (rendererRef) {
        rendererRef.dispose();
        rendererRef.domElement.remove();
        rendererRef = null;
      }
    };
  // restartKey changes when user clicks Restart or toggles Rapier
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [restartKey]);

  const handleToggleRapier = () => {
    setUseRapier(r => !r);
    setRestartKey(k => k + 1);
  };

  const handleRestart = () => setRestartKey(k => k + 1);

  return (
    <div>
      <div className="flex items-center gap-3 mb-4">
        <button
          onClick={() => setPaused(p => !p)}
          className="px-3 py-1.5 text-sm rounded-md border border-border bg-bg-surface text-text-primary hover:border-accent transition-colors inline-flex items-center gap-2"
        >
          {paused ? (
            <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor">
              <path d="M4 2 L13 8 L4 14 Z" />
            </svg>
          ) : (
            <svg width="12" height="12" viewBox="0 0 16 16" fill="currentColor">
              <rect x="3" y="2" width="3.5" height="12" rx="0.5" />
              <rect x="9.5" y="2" width="3.5" height="12" rx="0.5" />
            </svg>
          )}
          {paused ? "Play" : "Pause"}
        </button>
        <button
          onClick={handleRestart}
          className="px-3 py-1.5 text-sm rounded-md border border-border bg-bg-surface text-text-muted hover:border-accent hover:text-text-primary transition-colors inline-flex items-center gap-2"
        >
          <svg width="12" height="12" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
            <path d="M3 8 a5 5 0 1 0 1.5 -3.5" />
            <path d="M3 3 L3 5 L5 5" />
          </svg>
          Restart
        </button>
        <button
          onClick={handleToggleRapier}
          className={`px-3 py-1.5 text-sm rounded-md border transition-colors ${
            useRapier
              ? "bg-accent/20 border-accent text-accent"
              : "bg-bg-surface border-border text-text-muted hover:border-accent hover:text-text-primary"
          }`}
        >
          {useRapier ? "⚡ Rapier" : "Featherstone"}
        </button>
        <span className="text-text-muted text-xs">
          {CREATURES.map(c => c.label).join(" · ")} · Drag to orbit
        </span>
      </div>
      <div
        ref={mountRef}
        className="bg-bg-surface border border-border-subtle rounded-lg overflow-hidden"
        style={{ height: "600px" }}
      />
    </div>
  );
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function applyTransforms(
  data: Float64Array,
  meshes: THREE.Mesh[],
  offset: THREE.Vector3
) {
  for (let i = 0; i < meshes.length; i++) {
    const b = i * STRIDE;
    const px = data[b], py = data[b + 1], pz = data[b + 2];

    // Guard against NaN/Inf — keep last known good position
    if (!isFinite(px) || !isFinite(py) || !isFinite(pz)) continue;

    meshes[i].position.set(px + offset.x, py + offset.y, pz + offset.z);
    meshes[i].quaternion.set(data[b + 4], data[b + 5], data[b + 6], data[b + 3]);
    meshes[i].scale.set(data[b + 7] * 2, data[b + 8] * 2, data[b + 9] * 2);
  }
}

function makeLabel(text: string, badge: string): THREE.Sprite {
  const canvas = document.createElement("canvas");
  canvas.width = 256;
  canvas.height = 48;
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "rgba(0,0,0,0)";
  ctx.fillRect(0, 0, 256, 48);
  ctx.font = "bold 18px sans-serif";
  ctx.fillStyle = badge ? "#7accff" : "#aabbcc";
  ctx.textAlign = "center";
  ctx.fillText((badge ? badge + " " : "") + text, 128, 30);
  const tex = new THREE.CanvasTexture(canvas);
  const mat = new THREE.SpriteMaterial({ map: tex, transparent: true, depthTest: false });
  const sprite = new THREE.Sprite(mat);
  sprite.scale.set(4, 0.75, 1);
  return sprite;
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
      ctx.fillStyle = (r + c) % 2 === 0 ? "#3a3733" : "#252320";
      ctx.fillRect(c * tileSize, r * tileSize, tileSize, tileSize);
    }
  }
  const tex = new THREE.CanvasTexture(canvas);
  tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
  tex.repeat.set(20, 20);
  return tex;
}
