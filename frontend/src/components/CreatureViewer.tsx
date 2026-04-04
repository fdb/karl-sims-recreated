import { useEffect, useRef } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { initWasm, sim_init, sim_step, sim_body_count } from "../wasm";

interface Props {
  genomeBytes: Uint8Array;
}

export default function CreatureViewer({ genomeBytes }: Props) {
  const mountRef = useRef<HTMLDivElement>(null);
  // Keep refs so the cleanup closure can access latest values
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

      // --- Three.js scene setup ---
      const scene = new THREE.Scene();
      scene.background = new THREE.Color(0x1a2a30);
      scene.fog = new THREE.Fog(0x1a2a30, 12, 40);

      const camera = new THREE.PerspectiveCamera(
        45,
        mount.clientWidth / mount.clientHeight,
        0.01,
        100
      );
      camera.position.set(3, 2.5, 5);
      camera.lookAt(0, 0, 0);

      const renderer = new THREE.WebGLRenderer({ antialias: true });
      renderer.setPixelRatio(window.devicePixelRatio);
      renderer.setSize(mount.clientWidth, mount.clientHeight);
      renderer.shadowMap.enabled = true;
      renderer.shadowMap.type = THREE.PCFSoftShadowMap;
      mount.appendChild(renderer.domElement);

      // Orbit controls (handles mouse + touch)
      const controls = new OrbitControls(camera, renderer.domElement);
      controls.enableDamping = true;
      controls.dampingFactor = 0.08;
      controls.minDistance = 0.5;
      controls.maxDistance = 30;

      // Lighting
      const ambient = new THREE.AmbientLight(0xffffff, 0.4);
      scene.add(ambient);

      const sun = new THREE.DirectionalLight(0xfff5e0, 1.2);
      sun.position.set(4, 8, 5);
      sun.castShadow = true;
      sun.shadow.mapSize.set(1024, 1024);
      sun.shadow.camera.near = 0.1;
      sun.shadow.camera.far = 50;
      sun.shadow.camera.left = -10;
      sun.shadow.camera.right = 10;
      sun.shadow.camera.top = 10;
      sun.shadow.camera.bottom = -10;
      scene.add(sun);

      const fill = new THREE.DirectionalLight(0x8ab4c0, 0.3);
      fill.position.set(-3, 2, -4);
      scene.add(fill);

      // Ground plane (checkered texture)
      const groundSize = 40;
      const checkerCanvas = document.createElement("canvas");
      checkerCanvas.width = 256;
      checkerCanvas.height = 256;
      const ctx = checkerCanvas.getContext("2d")!;
      const tileCount = 8;
      const tileSize = 256 / tileCount;
      for (let row = 0; row < tileCount; row++) {
        for (let col = 0; col < tileCount; col++) {
          ctx.fillStyle = (row + col) % 2 === 0 ? "#2a3d44" : "#243540";
          ctx.fillRect(col * tileSize, row * tileSize, tileSize, tileSize);
        }
      }
      const checkerTex = new THREE.CanvasTexture(checkerCanvas);
      checkerTex.wrapS = THREE.RepeatWrapping;
      checkerTex.wrapT = THREE.RepeatWrapping;
      checkerTex.repeat.set(tileCount, tileCount);

      const ground = new THREE.Mesh(
        new THREE.PlaneGeometry(groundSize, groundSize),
        new THREE.MeshLambertMaterial({ map: checkerTex })
      );
      ground.rotation.x = -Math.PI / 2;
      ground.position.y = -0.001;
      ground.receiveShadow = true;
      scene.add(ground);

      // --- WASM simulation ---
      const handle = sim_init(genomeBytes);
      const bodyCount = sim_body_count(handle);

      // Create one box mesh per body part
      const bodyMaterial = new THREE.MeshLambertMaterial({ color: 0xeae5d9 });
      const meshes: THREE.Mesh[] = [];
      for (let i = 0; i < bodyCount; i++) {
        // Geometry sized to 1x1x1 — scaled per-frame via half_extents
        const geo = new THREE.BoxGeometry(1, 1, 1);
        const mesh = new THREE.Mesh(geo, bodyMaterial);
        mesh.castShadow = true;
        mesh.receiveShadow = true;
        scene.add(mesh);
        meshes.push(mesh);
      }

      // Apply initial transforms before first frame
      applyTransforms(sim_step(handle), meshes);

      // Resize observer
      const resizeObs = new ResizeObserver(() => {
        if (!mount) return;
        const w = mount.clientWidth;
        const h = mount.clientHeight;
        camera.aspect = w / h;
        camera.updateProjectionMatrix();
        renderer.setSize(w, h);
      });
      resizeObs.observe(mount);

      // Animation loop
      let animId: number;
      const animate = () => {
        animId = requestAnimationFrame(animate);
        const transforms = sim_step(handle);
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
    const base = i * STRIDE;
    const mesh = meshes[i];

    mesh.position.set(data[base], data[base + 1], data[base + 2]);
    // Quaternion: wasm returns [w, x, y, z], Three.js Quaternion is (x, y, z, w)
    mesh.quaternion.set(
      data[base + 4],
      data[base + 5],
      data[base + 6],
      data[base + 3]
    );
    // Scale box from unit cube to actual body dimensions (half_extents * 2)
    mesh.scale.set(
      data[base + 7] * 2,
      data[base + 8] * 2,
      data[base + 9] * 2
    );
  }
}
