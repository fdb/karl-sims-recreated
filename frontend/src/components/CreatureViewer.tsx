import { useEffect, useRef, useState, useCallback } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { initWasm, sim_init, sim_step_accurate, sim_body_count, sim_transforms } from "../wasm";

interface Props {
  genomeBytes: Uint8Array;
}

const SIM_DURATION = 10.0; // seconds
const DT = 1.0 / 60.0;
const TOTAL_FRAMES = Math.round(SIM_DURATION / DT); // 600

export default function CreatureViewer({ genomeBytes }: Props) {
  const mountRef = useRef<HTMLDivElement>(null);
  const rendererRef = useRef<THREE.WebGLRenderer | null>(null);
  const animIdRef = useRef<number>(0);

  const [progress, setProgress] = useState(0); // 0..1 during pre-computation
  const [isComputing, setIsComputing] = useState(true);
  const [currentFrame, setCurrentFrame] = useState(0);
  const [totalFrames, setTotalFrames] = useState(TOTAL_FRAMES);
  const [isPlaying, setIsPlaying] = useState(true);
  const [hasNan, setHasNan] = useState(false);
  const [nanFrame, setNanFrame] = useState<number | null>(null);

  // Mutable refs for animation loop (avoids stale closures)
  const framesRef = useRef<Float64Array[]>([]);
  const currentFrameRef = useRef(0);
  const isPlayingRef = useRef(true);
  const totalFramesRef = useRef(TOTAL_FRAMES);
  const meshesRef = useRef<THREE.Mesh[]>([]);

  // Keep play ref in sync
  useEffect(() => {
    isPlayingRef.current = isPlaying;
  }, [isPlaying]);

  // Seek: set frame from slider
  const seekTo = useCallback((frame: number) => {
    currentFrameRef.current = frame;
    setCurrentFrame(frame);
    if (meshesRef.current.length > 0 && framesRef.current[frame]) {
      applyTransforms(framesRef.current[frame], meshesRef.current);
    }
  }, []);

  useEffect(() => {
    const mount = mountRef.current;
    if (!mount) return;

    let cancelled = false;

    (async () => {
      await initWasm();
      if (cancelled) return;

      // --- Three.js scene ---
      const scene = new THREE.Scene();
      scene.background = new THREE.Color(0x1a2a30);
      scene.fog = new THREE.Fog(0x1a2a30, 12, 40);

      const w = mount.clientWidth || 400;
      const h = mount.clientHeight || 300;
      const camera = new THREE.PerspectiveCamera(45, w / h, 0.01, 100);
      camera.position.set(3, 2.5, 5);

      const renderer = new THREE.WebGLRenderer({ antialias: true });
      renderer.setPixelRatio(window.devicePixelRatio);
      renderer.setSize(w, h);
      renderer.shadowMap.enabled = true;
      renderer.shadowMap.type = THREE.PCFSoftShadowMap;
      mount.appendChild(renderer.domElement);
      rendererRef.current = renderer;

      const controls = new OrbitControls(camera, renderer.domElement);
      controls.enableDamping = true;
      controls.dampingFactor = 0.08;
      controls.minDistance = 0.3;
      controls.maxDistance = 50;

      // Lighting
      scene.add(new THREE.AmbientLight(0xffffff, 0.4));
      const sun = new THREE.DirectionalLight(0xfff5e0, 1.2);
      sun.position.set(4, 8, 5);
      sun.castShadow = true;
      sun.shadow.mapSize.set(1024, 1024);
      sun.shadow.camera.near = 0.1;
      sun.shadow.camera.far = 80;
      sun.shadow.camera.left = -20;
      sun.shadow.camera.right = 20;
      sun.shadow.camera.top = 20;
      sun.shadow.camera.bottom = -20;
      scene.add(sun);
      const fill = new THREE.DirectionalLight(0x8ab4c0, 0.3);
      fill.position.set(-3, 2, -4);
      scene.add(fill);

      // Ground plane (checkered)
      const checkerCanvas = document.createElement("canvas");
      checkerCanvas.width = 256;
      checkerCanvas.height = 256;
      const ctx = checkerCanvas.getContext("2d")!;
      for (let r = 0; r < 8; r++)
        for (let c = 0; c < 8; c++) {
          ctx.fillStyle = (r + c) % 2 === 0 ? "#2a3d44" : "#243540";
          ctx.fillRect(c * 32, r * 32, 32, 32);
        }
      const tex = new THREE.CanvasTexture(checkerCanvas);
      tex.wrapS = tex.wrapT = THREE.RepeatWrapping;
      tex.repeat.set(8, 8);
      const ground = new THREE.Mesh(
        new THREE.PlaneGeometry(80, 80),
        new THREE.MeshLambertMaterial({ map: tex })
      );
      ground.rotation.x = -Math.PI / 2;
      ground.position.y = -0.001;
      ground.receiveShadow = true;
      scene.add(ground);

      // --- WASM simulation handle ---
      let handle;
      try {
        handle = sim_init(genomeBytes);
      } catch (e) {
        console.error("CreatureViewer: sim_init failed:", e);
        return;
      }

      const bodyCount = sim_body_count(handle);
      console.log(`CreatureViewer: bodyCount=${bodyCount}`);

      const bodyMaterial = new THREE.MeshLambertMaterial({ color: 0xeae5d9 });
      const meshes: THREE.Mesh[] = [];
      for (let i = 0; i < bodyCount; i++) {
        const mesh = new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), bodyMaterial);
        mesh.castShadow = true;
        mesh.receiveShadow = true;
        scene.add(mesh);
        meshes.push(mesh);
      }
      meshesRef.current = meshes;

      // Apply frame 0 (initial state before any stepping)
      const initialTransforms = sim_transforms(handle);
      applyTransforms(initialTransforms, meshes);

      // Auto-fit camera to the creature's initial bounding box
      if (meshes.length > 0) {
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
          camera.position.set(
            center.x + dist,
            center.y + dist * 0.6,
            center.z + dist
          );
          camera.lookAt(center);
          camera.updateProjectionMatrix();
          controls.update();
        }
      }

      // Resize observer
      const resizeObs = new ResizeObserver(() => {
        const rw = mount.clientWidth;
        const rh = mount.clientHeight;
        if (rw === 0 || rh === 0) return;
        camera.aspect = rw / rh;
        camera.updateProjectionMatrix();
        renderer.setSize(rw, rh);
      });
      resizeObs.observe(mount);

      // Render the initial frame while pre-computing
      renderer.render(scene, camera);

      // --- Pre-compute all frames asynchronously ---
      // Store frame 0 (initial) first
      const allFrames: Float64Array[] = [initialTransforms.slice()];
      let firstNanFrame: number | null = null;

      // Process frames in batches to avoid blocking the UI
      const BATCH_SIZE = 30;
      for (let start = 0; start < TOTAL_FRAMES; start += BATCH_SIZE) {
        if (cancelled) return;

        // Yield to the event loop between batches
        await new Promise<void>((r) => setTimeout(r, 0));

        const end = Math.min(start + BATCH_SIZE, TOTAL_FRAMES);
        for (let i = start; i < end; i++) {
          const t = sim_step_accurate(handle);
          const hasNaN = someNotFinite(t);
          if (hasNaN && firstNanFrame === null) {
            firstNanFrame = i + 1;
          }
          allFrames.push(hasNaN ? allFrames[allFrames.length - 1] : t.slice());
        }

        setProgress((end) / TOTAL_FRAMES);
        // Render current progress
        renderer.render(scene, camera);
      }

      if (cancelled) return;

      framesRef.current = allFrames;
      totalFramesRef.current = allFrames.length;
      setTotalFrames(allFrames.length);
      setIsComputing(false);
      setNanFrame(firstNanFrame);
      setHasNan(firstNanFrame !== null);

      if (firstNanFrame !== null) {
        console.log(`CreatureViewer: NaN in accurate sim at frame ${firstNanFrame}`);
      }

      // --- Playback animation loop ---
      const animate = () => {
        animIdRef.current = requestAnimationFrame(animate);

        if (isPlayingRef.current) {
          currentFrameRef.current =
            (currentFrameRef.current + 1) % totalFramesRef.current;
          setCurrentFrame(currentFrameRef.current);
        }

        const frameData = framesRef.current[currentFrameRef.current];
        if (frameData) {
          applyTransforms(frameData, meshes);
        }

        controls.update();
        renderer.render(scene, camera);
      };
      animIdRef.current = requestAnimationFrame(animate);
    })();

    return () => {
      cancelled = true;
      cancelAnimationFrame(animIdRef.current);
      if (rendererRef.current) {
        rendererRef.current.dispose();
        rendererRef.current.domElement.remove();
        rendererRef.current = null;
      }
      framesRef.current = [];
      meshesRef.current = [];
    };
  }, [genomeBytes]);

  return (
    <div style={{ width: "100%", height: "100%", position: "relative" }}>
      <div ref={mountRef} style={{ width: "100%", height: "100%" }} />

      {/* Loading overlay */}
      {isComputing && (
        <div
          style={{
            position: "absolute",
            bottom: 0,
            left: 0,
            right: 0,
            padding: "8px 12px",
            background: "rgba(0,0,0,0.5)",
            display: "flex",
            alignItems: "center",
            gap: 8,
          }}
        >
          <div
            style={{
              flex: 1,
              height: 4,
              background: "#2a3d44",
              borderRadius: 2,
              overflow: "hidden",
            }}
          >
            <div
              style={{
                height: "100%",
                width: `${progress * 100}%`,
                background: "#4a9eca",
                borderRadius: 2,
                transition: "width 0.1s",
              }}
            />
          </div>
          <span style={{ color: "#aac", fontSize: 11, whiteSpace: "nowrap" }}>
            {Math.round(progress * 100)}%
          </span>
        </div>
      )}

      {/* Timeline controls */}
      {!isComputing && (
        <div
          style={{
            position: "absolute",
            bottom: 0,
            left: 0,
            right: 0,
            padding: "6px 10px",
            background: "rgba(0,0,0,0.6)",
            display: "flex",
            alignItems: "center",
            gap: 8,
          }}
        >
          {/* Play/Pause */}
          <button
            onClick={() => setIsPlaying((p) => !p)}
            style={{
              background: "none",
              border: "none",
              color: "#ccc",
              cursor: "pointer",
              fontSize: 14,
              padding: "0 4px",
              lineHeight: 1,
            }}
            title={isPlaying ? "Pause" : "Play"}
          >
            {isPlaying ? "⏸" : "▶"}
          </button>

          {/* Scrubber */}
          <input
            type="range"
            min={0}
            max={totalFrames - 1}
            value={currentFrame}
            onChange={(e) => {
              setIsPlaying(false);
              seekTo(Number(e.target.value));
            }}
            style={{ flex: 1, accentColor: "#4a9eca", cursor: "pointer" }}
          />

          {/* Frame counter */}
          <span
            style={{
              color: hasNan && nanFrame !== null && currentFrame >= nanFrame
                ? "#f87171"
                : "#aac",
              fontSize: 11,
              fontFamily: "monospace",
              whiteSpace: "nowrap",
            }}
          >
            {(currentFrame / 60).toFixed(1)}s
            {hasNan && nanFrame !== null && currentFrame >= nanFrame && " ⚠"}
          </span>
        </div>
      )}
    </div>
  );
}

function applyTransforms(data: Float64Array, meshes: THREE.Mesh[]) {
  const STRIDE = 10;
  for (let i = 0; i < meshes.length; i++) {
    const b = i * STRIDE;
    meshes[i].position.set(data[b], data[b + 1], data[b + 2]);
    // wasm: [w, x, y, z] → Three.js Quaternion(x, y, z, w)
    meshes[i].quaternion.set(data[b + 4], data[b + 5], data[b + 6], data[b + 3]);
    // half_extents × 2 = full box size
    meshes[i].scale.set(data[b + 7] * 2, data[b + 8] * 2, data[b + 9] * 2);
  }
}

function someNotFinite(arr: Float64Array): boolean {
  for (let i = 0; i < arr.length; i++) {
    if (!isFinite(arr[i])) return true;
  }
  return false;
}
