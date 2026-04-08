import { useEffect, useRef, useState, useCallback } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { initWasm, sim_init, sim_step_accurate, sim_body_count, sim_transforms, sim_light_position, sim_set_light_position } from "../wasm";
import { exportToMp4, downloadBlob } from "../utils/videoExport";

interface Props {
  genomeBytes: Uint8Array;
  environment?: "Water" | "Land";
  goal?: "SwimmingSpeed" | "LightFollowing";
  /** JSON string with physics solver config, passed to WASM sim_init. */
  physicsJson?: string;
  /** When set to true, triggers a video export of all 600 frames as WebM. */
  exportVideo?: boolean;
  /** Creature ID, used for the exported filename. */
  creatureId?: number;
}

const SIM_DURATION = 10.0; // seconds
const DT = 1.0 / 60.0;
const TOTAL_FRAMES = Math.round(SIM_DURATION / DT); // 600
const STRIDE = 10; // values per body: px py pz qw qx qy qz hx hy hz

export default function CreatureViewer({ genomeBytes, environment = "Water", goal = "SwimmingSpeed", physicsJson, exportVideo = false, creatureId }: Props) {
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
  const [isExporting, setIsExporting] = useState(false);
  const [exportProgress, setExportProgress] = useState(0);

  // Mutable refs for animation loop (avoids stale closures)
  const framesRef = useRef<Float64Array[]>([]);
  const currentFrameRef = useRef(0);
  const isPlayingRef = useRef(true);
  const totalFramesRef = useRef(TOTAL_FRAMES);
  const meshesRef = useRef<THREE.Mesh[]>([]);
  const lightPositionsRef = useRef<[number, number, number][]>([]);

  // Refs for export: camera, scene, renderer, controls — set inside the main useEffect
  const sceneRef = useRef<THREE.Scene | null>(null);
  const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);
  const controlsRef = useRef<OrbitControls | null>(null);
  const lightSphereRef = useRef<THREE.Mesh | null>(null);

  useEffect(() => {
    isPlayingRef.current = isPlaying;
  }, [isPlaying]);

  const seekTo = useCallback((frame: number) => {
    currentFrameRef.current = frame;
    setCurrentFrame(frame);
    if (meshesRef.current.length > 0 && framesRef.current[frame]) {
      applyTransforms(framesRef.current[frame], meshesRef.current);
    }
  }, []);

  const handleExport = useCallback(async () => {
    const renderer = rendererRef.current;
    const scene = sceneRef.current;
    const camera = cameraRef.current;
    const frames = framesRef.current;
    const meshes = meshesRef.current;
    const lightPositions = lightPositionsRef.current;
    const lightSphere = lightSphereRef.current;
    if (!renderer || !scene || !camera || frames.length === 0) return;

    setIsExporting(true);
    setExportProgress(0);
    const wasPlaying = isPlayingRef.current;
    isPlayingRef.current = false;
    setIsPlaying(false);

    try {
      const blob = await exportToMp4({
        canvas: renderer.domElement,
        totalFrames: frames.length,
        fps: 60,
        renderFrame: (i) => {
          applyTransforms(frames[i], meshes);
          if (lightSphere && lightPositions.length > 0) {
            const lp = lightPositions[Math.min(i, lightPositions.length - 1)];
            if (lp) lightSphere.position.set(lp[0], lp[1], lp[2]);
          }
          renderer.render(scene, camera);
        },
        onProgress: setExportProgress,
      });

      const filename = creatureId ? `creature-${creatureId}.mp4` : "creature.mp4";
      downloadBlob(blob, filename);
    } catch (e) {
      console.error("Video export failed:", e);
      alert(`Video export failed: ${e}`);
    } finally {
      setIsExporting(false);
      setExportProgress(0);
      isPlayingRef.current = wasPlaying;
      setIsPlaying(wasPlaying);
    }
  }, [creatureId]);

  useEffect(() => {
    const mount = mountRef.current;
    if (!mount) return;

    let cancelled = false;

    (async () => {
      await initWasm();
      if (cancelled) return;

      // --- Three.js scene ---
      const scene = new THREE.Scene();
      const bgColor = environment === "Water" ? 0x0d1e2e : 0x1a2a30;
      scene.background = new THREE.Color(bgColor);
      scene.fog = new THREE.Fog(bgColor, 30, 120);

      const w = mount.clientWidth || 400;
      const h = mount.clientHeight || 300;
      const camera = new THREE.PerspectiveCamera(45, w / h, 0.01, 100);
      camera.position.set(3, 2.5, 5);

      const renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });
      renderer.setPixelRatio(window.devicePixelRatio);
      renderer.setSize(w, h);
      renderer.shadowMap.enabled = true;
      // Three.js r180+ deprecated PCFSoftShadowMap (it now aliases to
      // PCFShadowMap internally anyway); use the non-deprecated constant.
      renderer.shadowMap.type = THREE.PCFShadowMap;
      mount.appendChild(renderer.domElement);
      rendererRef.current = renderer;

      const controls = new OrbitControls(camera, renderer.domElement);
      controls.enableDamping = true;
      controls.dampingFactor = 0.08;
      controls.minDistance = 0.3;
      controls.maxDistance = 50;

      // Store refs for export handler
      sceneRef.current = scene;
      cameraRef.current = camera;
      controlsRef.current = controls;

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

      // --- WASM simulation handle ---
      let handle;
      try {
        handle = sim_init(genomeBytes, environment, physicsJson);
      } catch (e) {
        console.error("CreatureViewer: sim_init failed:", e);
        return;
      }

      const bodyCount = sim_body_count(handle);

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

      // Light target indicator (only for LightFollowing)
      let lightSphere: THREE.Mesh | null = null;
      lightSphereRef.current = null;
      if (goal === "LightFollowing") {
        // Set initial light position on the simulation
        sim_set_light_position(handle, 5.0, 0.0, 0.0);

        const lightMat = new THREE.MeshBasicMaterial({
          color: 0xffee88,
          transparent: true,
          opacity: 0.8,
        });
        lightSphere = new THREE.Mesh(
          new THREE.SphereGeometry(0.3, 16, 16),
          lightMat,
        );
        lightSphere.position.set(5, 0, 0);
        scene.add(lightSphere);
        lightSphereRef.current = lightSphere;

        // Glow ring around the light
        const glowMat = new THREE.MeshBasicMaterial({
          color: 0xffee88,
          transparent: true,
          opacity: 0.2,
          side: THREE.DoubleSide,
        });
        const glow = new THREE.Mesh(
          new THREE.SphereGeometry(0.6, 16, 16),
          glowMat,
        );
        lightSphere.add(glow);
      }

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

      renderer.render(scene, camera);

      // --- Pre-compute all frames ---
      const allFrames: Float64Array[] = [initialTransforms.slice()];
      const allLightPositions: [number, number, number][] = [];
      let firstNanFrame: number | null = null;

      // Store initial light position
      if (goal === "LightFollowing") {
        const lp = sim_light_position(handle);
        allLightPositions.push([lp[0], lp[1], lp[2]]);
      }

      const REPOSITION_INTERVAL = 300; // 5 seconds at 60fps
      const BATCH_SIZE = 30;
      for (let start = 0; start < TOTAL_FRAMES; start += BATCH_SIZE) {
        if (cancelled) return;
        await new Promise<void>((r) => setTimeout(r, 0));

        const end = Math.min(start + BATCH_SIZE, TOTAL_FRAMES);
        for (let i = start; i < end; i++) {
          // Reposition light every 5 seconds (matching fitness evaluation logic)
          if (goal === "LightFollowing" && i > 0 && i % REPOSITION_INTERVAL === 0) {
            const angle = (i * 0.01) * 2.0;
            sim_set_light_position(handle, 5.0 * Math.cos(angle), 0.0, 5.0 * Math.sin(angle));
          }

          const t = sim_step_accurate(handle);
          const hasNaN = someNotFinite(t);
          if (hasNaN && firstNanFrame === null) firstNanFrame = i + 1;
          allFrames.push(hasNaN ? allFrames[allFrames.length - 1] : t.slice());

          if (goal === "LightFollowing") {
            const lp = sim_light_position(handle);
            allLightPositions.push([lp[0], lp[1], lp[2]]);
          }
        }

        setProgress(end / TOTAL_FRAMES);
        renderer.render(scene, camera);
      }

      if (cancelled) return;

      // --- Add floor/water reference plane (after frames are computed) ---
      if (environment === "Land") {
        // Ground plane is at y=0 in the physics engine
        const floorY = 0;
        const checker = buildCheckerTexture();
        const ground = new THREE.Mesh(
          new THREE.PlaneGeometry(80, 80),
          new THREE.MeshLambertMaterial({ map: checker })
        );
        ground.rotation.x = -Math.PI / 2;
        ground.position.y = floorY;
        ground.receiveShadow = true;
        scene.add(ground);

        // Grid lines for depth perception
        const grid = new THREE.GridHelper(80, 40, 0x4a5a50, 0x3a4a40);
        grid.position.y = floorY + 0.002;
        scene.add(grid);
      } else {
        // Water: translucent horizontal plane at y=0 as a sea-level reference.
        // The creature floats freely; the plane is just a visual horizon.
        const waterY = 0;
        const waterMat = new THREE.MeshLambertMaterial({
          color: 0x1a5080,
          transparent: true,
          opacity: 0.35,
          side: THREE.DoubleSide,
        });
        const water = new THREE.Mesh(new THREE.PlaneGeometry(80, 80), waterMat);
        water.rotation.x = -Math.PI / 2;
        water.position.y = waterY;
        scene.add(water);

        // Faint grid lines on the water surface
        const grid = new THREE.GridHelper(80, 40, 0x2a6090, 0x1a4060);
        (grid.material as THREE.LineBasicMaterial).transparent = true;
        (grid.material as THREE.LineBasicMaterial).opacity = 0.4;
        grid.position.y = waterY + 0.01;
        scene.add(grid);
      }

      framesRef.current = allFrames;
      lightPositionsRef.current = allLightPositions;
      totalFramesRef.current = allFrames.length;
      setTotalFrames(allFrames.length);
      setIsComputing(false);
      setNanFrame(firstNanFrame);
      setHasNan(firstNanFrame !== null);

      // --- Expose frame export API for Playwright/CLI video capture ---
      (window as any).__creatureExport = {
        totalFrames: allFrames.length,
        renderFrame(n: number): string {
          applyTransforms(allFrames[Math.min(n, allFrames.length - 1)], meshes);
          if (lightSphere && allLightPositions.length > 0) {
            const lp = allLightPositions[Math.min(n, allLightPositions.length - 1)];
            if (lp) lightSphere.position.set(lp[0], lp[1], lp[2]);
          }
          controls.update();
          renderer.render(scene, camera);
          return renderer.domElement.toDataURL("image/jpeg", 0.92);
        },
        renderFrameBatch(start: number, count: number): string[] {
          const results: string[] = [];
          const end = Math.min(start + count, allFrames.length);
          for (let i = start; i < end; i++) {
            applyTransforms(allFrames[i], meshes);
            if (lightSphere && allLightPositions.length > 0) {
              const lp = allLightPositions[Math.min(i, allLightPositions.length - 1)];
              if (lp) lightSphere.position.set(lp[0], lp[1], lp[2]);
            }
            controls.update();
            renderer.render(scene, camera);
            results.push(renderer.domElement.toDataURL("image/jpeg", 0.92));
          }
          return results;
        },
      };
      document.body.setAttribute("data-export-ready", "true");

      // --- Playback animation loop ---
      const animate = () => {
        animIdRef.current = requestAnimationFrame(animate);

        if (isPlayingRef.current) {
          currentFrameRef.current =
            (currentFrameRef.current + 1) % totalFramesRef.current;
          setCurrentFrame(currentFrameRef.current);
        }

        const frameData = framesRef.current[currentFrameRef.current];
        if (frameData) applyTransforms(frameData, meshes);

        // Update light indicator position
        if (lightSphere && lightPositionsRef.current.length > 0) {
          const lp = lightPositionsRef.current[
            Math.min(currentFrameRef.current, lightPositionsRef.current.length - 1)
          ];
          if (lp) lightSphere.position.set(lp[0], lp[1], lp[2]);
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
      lightPositionsRef.current = [];
      sceneRef.current = null;
      cameraRef.current = null;
      controlsRef.current = null;
      lightSphereRef.current = null;
      delete (window as any).__creatureExport;
      document.body.removeAttribute("data-export-ready");
    };
  }, [genomeBytes, environment, goal, physicsJson]);

  // --- Pause playback when export mode is active (capture is driven externally) ---
  useEffect(() => {
    if (exportVideo && !isComputing) {
      isPlayingRef.current = false;
    }
  }, [exportVideo, isComputing]);

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

      {/* Export progress overlay */}
      {isExporting && (
        <div
          style={{
            position: "absolute",
            inset: 0,
            background: "rgba(0,0,0,0.5)",
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            justifyContent: "center",
            gap: 12,
            zIndex: 20,
          }}
        >
          <span style={{ color: "#ccc", fontSize: 13 }}>
            Encoding MP4... {Math.round(exportProgress * 100)}%
          </span>
          <div style={{ width: "60%", height: 4, background: "#2a3d44", borderRadius: 2, overflow: "hidden" }}>
            <div style={{ height: "100%", width: `${exportProgress * 100}%`, background: "#4a9eca", borderRadius: 2, transition: "width 0.1s" }} />
          </div>
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
          <button
            onClick={() => setIsPlaying((p) => !p)}
            style={{
              background: "none",
              border: "none",
              color: "#ccc",
              cursor: "pointer",
              padding: "0 4px",
              display: "flex",
              alignItems: "center",
              justifyContent: "center",
            }}
            title={isPlaying ? "Pause" : "Play"}
            aria-label={isPlaying ? "Pause" : "Play"}
          >
            {isPlaying ? (
              // Pause: two vertical bars
              <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
                <rect x="3" y="2" width="3.5" height="12" rx="0.5" />
                <rect x="9.5" y="2" width="3.5" height="12" rx="0.5" />
              </svg>
            ) : (
              // Play: right-pointing triangle
              <svg width="14" height="14" viewBox="0 0 16 16" fill="currentColor">
                <path d="M4 2 L13 8 L4 14 Z" />
              </svg>
            )}
          </button>

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

          <span
            style={{
              color:
                hasNan && nanFrame !== null && currentFrame >= nanFrame
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

          {/* Export button */}
          <button
            onClick={handleExport}
            disabled={isExporting}
            style={{
              background: "none",
              border: "1px solid #556",
              borderRadius: 4,
              color: "#ccc",
              cursor: isExporting ? "default" : "pointer",
              padding: "2px 8px",
              fontSize: 11,
              whiteSpace: "nowrap",
              opacity: isExporting ? 0.5 : 1,
            }}
            title="Export MP4 from current camera angle"
          >
            Export .mp4
          </button>
        </div>
      )}
    </div>
  );
}

/** Build a high-contrast checkered canvas texture for the land floor. */
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
      // Alternating warm gray / dark warm gray — much more contrast than before
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
