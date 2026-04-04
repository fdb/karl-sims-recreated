import { useEffect, useRef } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { initWasm, sim_init, sim_step, sim_body_count } from "../wasm";

interface Props {
  genomeBytes: Uint8Array;
}

export default function CreatureViewer({ genomeBytes }: Props) {
  const mountRef = useRef<HTMLDivElement>(null);
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

      // --- WASM simulation ---
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

      // Get first frame of transforms
      const firstTransforms = sim_step(handle);
      applyTransforms(firstTransforms, meshes);

      // Auto-fit camera to the creature's bounding box
      if (meshes.length > 0) {
        const box = new THREE.Box3();
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
          console.log(
            `CreatureViewer: center=(${center.x.toFixed(2)}, ${center.y.toFixed(2)}, ${center.z.toFixed(2)}) maxDim=${maxDim.toFixed(2)}`
          );
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

      // Animation loop
      let animId: number;
      let frameCount = 0;
      const animate = () => {
        animId = requestAnimationFrame(animate);
        const transforms = sim_step(handle);

        // Debug: log first 5 frames
        if (frameCount < 5) {
          const p0 = `(${transforms[0].toFixed(3)}, ${transforms[1].toFixed(3)}, ${transforms[2].toFixed(3)})`;
          const hasNaN = transforms.some((v) => !isFinite(v));
          console.log(`CreatureViewer frame ${frameCount}: body[0]=${p0} NaN=${hasNaN}`);
          frameCount++;
        }

        // NaN guard: skip update if physics has diverged
        if (transforms.some((v) => !isFinite(v))) {
          console.warn("CreatureViewer: NaN/Inf in transforms, freezing on last valid frame");
          controls.update();
          renderer.render(scene, camera);
          return;
        }

        applyTransforms(transforms, meshes);
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
  }, [genomeBytes]);

  return <div ref={mountRef} style={{ width: "100%", height: "100%" }} />;
}

function applyTransforms(data: number[], meshes: THREE.Mesh[]) {
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
