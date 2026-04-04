# M1: Visual Proof of Life — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Open `localhost:5173`, see animated rectangular solids connected by joints, rendered in the Karl Sims underwater style, driven by simple physics.

**Architecture:** Rust workspace with `core` (physics types + simulation, compiles to native + WASM) and `web` (wgpu renderer + WASM bindings). React+TypeScript frontend hosts the canvas and provides scene selection UI. Physics uses simple semi-implicit Euler integration with forward kinematics — just enough to animate jointed creatures. Featherstone's algorithm comes in M2.

**Tech Stack:** Rust 1.90, glam (scalar-math), wgpu 24, wasm-pack, React 19, Vite, TypeScript

---

## File Structure

```
karl-sims-recreated/
├── Cargo.toml                          # workspace root
├── core/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                      # crate root, re-exports
│       ├── body.rs                     # RigidBody: dimensions, mass, inertia
│       ├── joint.rs                    # Joint: revolute, angle, velocity
│       ├── world.rs                    # World: bodies + joints + step() + FK
│       └── scene.rs                    # Hard-coded test scene builders
├── web/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                      # WASM entry, #[wasm_bindgen] API
│       ├── renderer.rs                 # WgpuRenderer: init, resize, render_frame
│       ├── camera.rs                   # OrbitCamera: azimuth, elevation, distance
│       ├── gpu_types.rs                # Vertex, InstanceRaw, CameraUniform, etc.
│       └── shader.wgsl                 # Vertex + fragment shader (Karl Sims look)
├── frontend/
│   ├── package.json
│   ├── tsconfig.json
│   ├── tsconfig.app.json
│   ├── vite.config.ts
│   ├── index.html
│   └── src/
│       ├── main.tsx                    # React root
│       ├── App.tsx                     # Layout + scene selector + controls
│       ├── App.css                     # Minimal styling
│       └── wasm.ts                     # WASM init + typed wrapper
└── server/
    ├── Cargo.toml                      # placeholder
    └── src/
        └── lib.rs                      # empty
```

---

## Task 1: Rust Workspace + Crate Scaffolds

**Files:**
- Create: `Cargo.toml` (workspace root)
- Create: `core/Cargo.toml`, `core/src/lib.rs`
- Create: `web/Cargo.toml`, `web/src/lib.rs`
- Create: `server/Cargo.toml`, `server/src/lib.rs`

- [ ] **Step 1: Create workspace Cargo.toml**

```toml
# Cargo.toml
[workspace]
resolver = "2"
members = ["core", "server", "web"]

[workspace.package]
edition = "2024"
```

- [ ] **Step 2: Create core crate**

```toml
# core/Cargo.toml
[package]
name = "karl-sims-core"
version = "0.1.0"
edition.workspace = true

[dependencies]
glam = { version = "0.30", features = ["scalar-math"] }
```

```rust
// core/src/lib.rs
pub mod body;
pub mod joint;
pub mod world;
pub mod scene;
```

Create empty module files:

```rust
// core/src/body.rs
// RigidBody types — implemented in Task 7
```

```rust
// core/src/joint.rs
// Joint types — implemented in Task 7
```

```rust
// core/src/world.rs
// World simulation — implemented in Task 8
```

```rust
// core/src/scene.rs
// Test scene builders — implemented in Task 9
```

- [ ] **Step 3: Create web crate**

```toml
# web/Cargo.toml
[package]
name = "karl-sims-web"
version = "0.1.0"
edition.workspace = true

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
karl-sims-core = { path = "../core" }
wgpu = "24"
bytemuck = { version = "1", features = ["derive"] }
glam = { version = "0.30", features = ["scalar-math"] }
log = "0.4"
wasm-bindgen = "0.2"
wasm-bindgen-futures = "0.4"
js-sys = "0.3"
web-sys = { version = "0.3", features = [
    "Document", "Window", "Element", "HtmlCanvasElement",
    "MouseEvent", "WheelEvent", "EventTarget",
    "AddEventListenerOptions",
] }
console_log = "1"
console_error_panic_hook = "0.1"
```

```rust
// web/src/lib.rs
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn wasm_main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).ok();
    log::info!("karl-sims WASM module loaded");
}
```

- [ ] **Step 4: Create server placeholder**

```toml
# server/Cargo.toml
[package]
name = "karl-sims-server"
version = "0.1.0"
edition.workspace = true

[dependencies]
karl-sims-core = { path = "../core" }
```

```rust
// server/src/lib.rs
// Server — implemented in M6
```

- [ ] **Step 5: Verify workspace builds**

Run: `cargo check`
Expected: compiles with no errors (warnings about unused modules are fine)

- [ ] **Step 6: Verify WASM target builds**

Run: `wasm-pack build web/ --target web --dev`
Expected: produces `web/pkg/` directory with `.wasm` and `.js` files

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml core/ web/ server/
git commit -m "feat: scaffold Rust workspace with core, web, and server crates"
```

---

## Task 2: React Frontend Skeleton

**Files:**
- Create: `frontend/package.json`, `frontend/vite.config.ts`, `frontend/tsconfig.json`, `frontend/tsconfig.app.json`
- Create: `frontend/index.html`, `frontend/src/main.tsx`, `frontend/src/App.tsx`, `frontend/src/App.css`

- [ ] **Step 1: Initialize frontend project**

```json
// frontend/package.json
{
  "name": "karl-sims-frontend",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "build:wasm": "cd ../web && wasm-pack build --target web --dev && cd ../frontend && npm install"
  },
  "dependencies": {
    "karl-sims-web": "file:../web/pkg",
    "react": "^19.0.0",
    "react-dom": "^19.0.0"
  },
  "devDependencies": {
    "@types/react": "^19.0.0",
    "@types/react-dom": "^19.0.0",
    "@vitejs/plugin-react": "^4.4.0",
    "typescript": "~5.7.0",
    "vite": "^6.0.0",
    "vite-plugin-wasm": "^3.4.0",
    "vite-plugin-top-level-await": "^1.5.0"
  }
}
```

- [ ] **Step 2: Create Vite config**

```typescript
// frontend/vite.config.ts
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

export default defineConfig({
  plugins: [react(), wasm(), topLevelAwait()],
  server: {
    fs: {
      allow: [".."],
    },
  },
});
```

- [ ] **Step 3: Create TypeScript config**

```json
// frontend/tsconfig.json
{
  "files": [],
  "references": [{ "path": "./tsconfig.app.json" }]
}
```

```json
// frontend/tsconfig.app.json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["ES2020", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "isolatedModules": true,
    "moduleDetection": "force",
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "noUncheckedSideEffectImports": true
  },
  "include": ["src"]
}
```

- [ ] **Step 4: Create HTML entry point and React app**

```html
<!-- frontend/index.html -->
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Evolving Virtual Creatures</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

```tsx
// frontend/src/main.tsx
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App.tsx";
import "./App.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>
);
```

```tsx
// frontend/src/App.tsx
import { useEffect, useRef } from "react";

export default function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    if (!canvasRef.current) return;
    // WASM renderer will attach to this canvas in Task 3
    console.log("Canvas ready:", canvasRef.current);
  }, []);

  return (
    <div className="app">
      <canvas ref={canvasRef} id="sim-canvas" width={960} height={640} />
    </div>
  );
}
```

```css
/* frontend/src/App.css */
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  background: #1a1a2e;
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 100vh;
  font-family: system-ui, sans-serif;
  color: #e0e0e0;
}

.app {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
}

#sim-canvas {
  border: 1px solid #333;
  border-radius: 4px;
}
```

- [ ] **Step 5: Install dependencies and verify dev server**

Run:
```bash
cd frontend && npm install && npm run dev -- --host 2>&1 | head -20
```
Expected: Vite dev server starts, page shows an empty canvas at `localhost:5173`. (The `karl-sims-web` dependency may warn about missing package — that's fine until we build the WASM.)

Note: if `karl-sims-web` install fails because `web/pkg` doesn't exist yet, run `npm run build:wasm` first, then `npm install`.

- [ ] **Step 6: Commit**

```bash
git add frontend/
git commit -m "feat: scaffold React frontend with Vite and canvas element"
```

---

## Task 3: WGPU Bootstrap in Browser

**Files:**
- Create: `web/src/renderer.rs`
- Modify: `web/src/lib.rs`
- Modify: `frontend/src/App.tsx`
- Create: `frontend/src/wasm.ts`

- [ ] **Step 1: Create the WASM wrapper for the frontend**

```typescript
// frontend/src/wasm.ts
import init, { create_renderer, renderer_resize } from "karl-sims-web";

let initialized = false;

export async function initWasm(): Promise<void> {
  if (initialized) return;
  await init();
  initialized = true;
}

export { create_renderer, renderer_resize };
```

- [ ] **Step 2: Create the WgpuRenderer struct**

```rust
// web/src/renderer.rs
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

pub struct WgpuRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: (u32, u32),
}

impl WgpuRenderer {
    pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        let width = canvas.width();
        let height = canvas.height();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });

        let surface_target = wgpu::SurfaceTarget::Canvas(canvas);
        let surface = instance
            .create_surface(surface_target)
            .map_err(|e| format!("Failed to create surface: {e}"))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .ok_or("No suitable GPU adapter found")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("karl-sims device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                    ..Default::default()
                },
                None,
            )
            .await
            .map_err(|e| format!("Failed to create device: {e}"))?;

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size: (width, height),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.size = (width, height);
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    pub fn render_frame(&self) -> Result<(), String> {
        let output = self
            .surface
            .get_current_texture()
            .map_err(|e| format!("Surface error: {e}"))?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.18,
                            g: 0.32,
                            b: 0.38,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }
}
```

- [ ] **Step 3: Wire WASM entry point to renderer**

```rust
// web/src/lib.rs
mod renderer;

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::renderer::WgpuRenderer;

thread_local! {
    static RENDERER: RefCell<Option<WgpuRenderer>> = RefCell::new(None);
}

#[wasm_bindgen(start)]
pub fn wasm_main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).ok();
    log::info!("karl-sims WASM module loaded");
}

#[wasm_bindgen]
pub async fn create_renderer(canvas_id: &str) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or("canvas not found")?
        .dyn_into::<HtmlCanvasElement>()?;

    let renderer = WgpuRenderer::new(canvas)
        .await
        .map_err(|e| JsValue::from_str(&e))?;

    RENDERER.with(|r| {
        *r.borrow_mut() = Some(renderer);
    });

    start_render_loop();
    Ok(())
}

#[wasm_bindgen]
pub fn renderer_resize(width: u32, height: u32) {
    RENDERER.with(|r| {
        if let Some(renderer) = r.borrow_mut().as_mut() {
            renderer.resize(width, height);
        }
    });
}

fn start_render_loop() {
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::new(move || {
        RENDERER.with(|r| {
            if let Some(renderer) = r.borrow().as_ref() {
                if let Err(e) = renderer.render_frame() {
                    log::error!("Render error: {}", e);
                }
            }
        });

        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    request_animation_frame(g.borrow().as_ref().unwrap());
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .unwrap();
}
```

- [ ] **Step 4: Update App.tsx to init WASM and create renderer**

```tsx
// frontend/src/App.tsx
import { useEffect, useRef } from "react";
import { initWasm, create_renderer } from "./wasm";

export default function App() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const initedRef = useRef(false);

  useEffect(() => {
    if (initedRef.current) return;
    initedRef.current = true;

    (async () => {
      await initWasm();
      await create_renderer("sim-canvas");
      console.log("Renderer created");
    })();
  }, []);

  return (
    <div className="app">
      <canvas ref={canvasRef} id="sim-canvas" width={960} height={640} />
    </div>
  );
}
```

- [ ] **Step 5: Build WASM and verify teal screen in browser**

Run:
```bash
cd web && wasm-pack build --target web --dev && cd ../frontend && npm install && npm run dev
```
Expected: Browser shows a teal-colored canvas (RGB roughly 0.18, 0.32, 0.38). Console shows "karl-sims WASM module loaded" and "Renderer created".

- [ ] **Step 6: Commit**

```bash
git add web/src/ frontend/src/
git commit -m "feat: wgpu renderer bootstrap — teal clear screen in browser"
```

---

## Task 4: Box Mesh + Karl Sims Lighting Shader

**Files:**
- Create: `web/src/gpu_types.rs`
- Create: `web/src/shader.wgsl`
- Modify: `web/src/renderer.rs`
- Modify: `web/src/lib.rs`

- [ ] **Step 1: Define GPU vertex and instance types**

```rust
// web/src/gpu_types.rs
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

impl Vertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct InstanceRaw {
    pub model: [[f32; 4]; 4],
    pub color: [f32; 3],
    pub flags: u32,
}

impl InstanceRaw {
    const ATTRIBS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
        2 => Float32x4,   // model col 0
        3 => Float32x4,   // model col 1
        4 => Float32x4,   // model col 2
        5 => Float32x4,   // model col 3
        6 => Float32x3,   // color
        7 => Uint32,       // flags
    ];

    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub _pad: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct SceneUniform {
    pub light_dir: [f32; 3],
    pub fog_near: f32,
    pub fog_color: [f32; 3],
    pub fog_far: f32,
}

/// Unit cube centered at origin: 36 vertices (6 faces x 2 triangles x 3 verts)
pub fn cube_vertices() -> Vec<Vertex> {
    let mut verts = Vec::with_capacity(36);
    let faces: [([f32; 3], [[f32; 3]; 4]); 6] = [
        // normal,      4 corners (CCW when viewed from outside)
        ([0.0, 0.0, 1.0],  [[-0.5,-0.5, 0.5],[ 0.5,-0.5, 0.5],[ 0.5, 0.5, 0.5],[-0.5, 0.5, 0.5]]), // +Z
        ([0.0, 0.0,-1.0],  [[ 0.5,-0.5,-0.5],[-0.5,-0.5,-0.5],[-0.5, 0.5,-0.5],[ 0.5, 0.5,-0.5]]), // -Z
        ([1.0, 0.0, 0.0],  [[ 0.5,-0.5, 0.5],[ 0.5,-0.5,-0.5],[ 0.5, 0.5,-0.5],[ 0.5, 0.5, 0.5]]), // +X
        ([-1.0,0.0, 0.0],  [[-0.5,-0.5,-0.5],[-0.5,-0.5, 0.5],[-0.5, 0.5, 0.5],[-0.5, 0.5,-0.5]]), // -X
        ([0.0, 1.0, 0.0],  [[-0.5, 0.5, 0.5],[ 0.5, 0.5, 0.5],[ 0.5, 0.5,-0.5],[-0.5, 0.5,-0.5]]), // +Y
        ([0.0,-1.0, 0.0],  [[-0.5,-0.5,-0.5],[ 0.5,-0.5,-0.5],[ 0.5,-0.5, 0.5],[-0.5,-0.5, 0.5]]), // -Y
    ];
    for (normal, corners) in &faces {
        // Two triangles per face: 0-1-2, 0-2-3
        for &idx in &[0usize, 1, 2, 0, 2, 3] {
            verts.push(Vertex {
                position: corners[idx],
                normal: *normal,
            });
        }
    }
    verts
}
```

- [ ] **Step 2: Write the WGSL shader**

```wgsl
// web/src/shader.wgsl

struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
}

struct SceneUniform {
    light_dir: vec3<f32>,
    fog_near: f32,
    fog_color: vec3<f32>,
    fog_far: f32,
}

@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(0) @binding(1) var<uniform> scene: SceneUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct InstanceInput {
    @location(2) model_0: vec4<f32>,
    @location(3) model_1: vec4<f32>,
    @location(4) model_2: vec4<f32>,
    @location(5) model_3: vec4<f32>,
    @location(6) color: vec3<f32>,
    @location(7) flags: u32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec3<f32>,
    @location(1) @interpolate(flat) world_normal: vec3<f32>,
    @location(2) @interpolate(flat) base_color: vec3<f32>,
    @location(3) @interpolate(flat) flags: u32,
}

@vertex
fn vs_main(vert: VertexInput, inst: InstanceInput) -> VertexOutput {
    let model = mat4x4<f32>(
        inst.model_0, inst.model_1, inst.model_2, inst.model_3,
    );
    let world_pos = model * vec4<f32>(vert.position, 1.0);
    // Extract rotation (upper 3x3) for normal transform — assumes uniform scale per axis
    let world_normal = normalize((model * vec4<f32>(vert.normal, 0.0)).xyz);

    var out: VertexOutput;
    out.clip_position = camera.view_proj * world_pos;
    out.world_position = world_pos.xyz;
    out.world_normal = world_normal;
    out.base_color = inst.color;
    out.flags = inst.flags;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var color: vec3<f32>;
    let is_ground = (in.flags & 1u) != 0u;

    if is_ground {
        // Checkered ground plane in muted blue-gray tones
        let scale = 2.0;
        let cx = i32(floor(in.world_position.x / scale));
        let cz = i32(floor(in.world_position.z / scale));
        let checker = ((cx + cz) % 2 + 2) % 2;
        let light_tile = vec3<f32>(0.48, 0.55, 0.58);
        let dark_tile = vec3<f32>(0.35, 0.42, 0.46);
        let tile_color = select(dark_tile, light_tile, checker == 0);

        // Apply subtle lighting to ground too
        let ndotl = max(dot(in.world_normal, normalize(scene.light_dir)), 0.0);
        color = tile_color * (0.6 + 0.4 * ndotl);
    } else {
        // Creature box: Karl Sims style flat shading
        // Lit faces: cream/white. Shadow faces: olive/yellow-green tint.
        let light = normalize(scene.light_dir);
        let ndotl = dot(in.world_normal, light);

        // Ambient: olive-green tint (shadow color)
        let ambient = vec3<f32>(0.38, 0.40, 0.34);
        // Diffuse: cream/warm white
        let diffuse = in.base_color * max(ndotl, 0.0) * 0.65;

        color = ambient + diffuse;
    }

    // Depth fog: fade to underwater teal
    let dist = length(in.world_position - camera.camera_pos);
    let fog_t = clamp((dist - scene.fog_near) / (scene.fog_far - scene.fog_near), 0.0, 1.0);
    color = mix(color, scene.fog_color, fog_t);

    return vec4<f32>(color, 1.0);
}
```

- [ ] **Step 3: Add render pipeline, vertex buffer, and uniform buffers to renderer**

Replace the full contents of `web/src/renderer.rs`:

```rust
// web/src/renderer.rs
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use wgpu::util::DeviceExt;

use crate::gpu_types::*;

pub struct WgpuRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: (u32, u32),
    pipeline: wgpu::RenderPipeline,
    depth_view: wgpu::TextureView,
    vertex_buf: wgpu::Buffer,
    vertex_count: u32,
    instance_buf: wgpu::Buffer,
    instance_count: u32,
    camera_buf: wgpu::Buffer,
    scene_buf: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl WgpuRenderer {
    pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        let width = canvas.width();
        let height = canvas.height();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..Default::default()
        });

        let surface_target = wgpu::SurfaceTarget::Canvas(canvas);
        let surface = instance
            .create_surface(surface_target)
            .map_err(|e| format!("Surface: {e}"))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                power_preference: wgpu::PowerPreference::HighPerformance,
                ..Default::default()
            })
            .await
            .ok_or("No GPU adapter")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("karl-sims"),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                    ..Default::default()
                },
                None,
            )
            .await
            .map_err(|e| format!("Device: {e}"))?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Depth buffer
        let depth_view = Self::create_depth_view(&device, width, height);

        // Vertex buffer: unit cube
        let cube_verts = cube_vertices();
        let vertex_count = cube_verts.len() as u32;
        let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cube Vertices"),
            contents: bytemuck::cast_slice(&cube_verts),
            usage: wgpu::BufferUsages::VERTEX,
        });

        // Instance buffer: start with capacity for 64 instances
        let instance_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Instances"),
            size: (64 * std::mem::size_of::<InstanceRaw>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Uniform buffers
        let camera_uniform = CameraUniform {
            view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 5.0, 10.0],
            _pad: 0.0,
        };
        let camera_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera"),
            contents: bytemuck::bytes_of(&camera_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let scene_uniform = SceneUniform {
            light_dir: [0.4, 0.8, 0.3],
            fog_near: 8.0,
            fog_color: [0.18, 0.32, 0.38],
            fog_far: 40.0,
        };
        let scene_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Scene"),
            contents: bytemuck::bytes_of(&scene_uniform),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Bind group
        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Scene Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Scene Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: scene_buf.as_entire_binding(),
                },
            ],
        });

        // Shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Karl Sims Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        // Pipeline
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Main Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::layout(), InstanceRaw::layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Ok(Self {
            surface,
            device,
            queue,
            config,
            size: (width, height),
            pipeline,
            depth_view,
            vertex_buf,
            vertex_count,
            instance_buf,
            instance_count: 0,
            camera_buf,
            scene_buf,
            bind_group,
        })
    }

    fn create_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        depth_texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.size = (width, height);
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
            self.depth_view = Self::create_depth_view(&self.device, width, height);
        }
    }

    pub fn update_camera(&self, uniform: &CameraUniform) {
        self.queue
            .write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(uniform));
    }

    pub fn update_scene(&self, uniform: &SceneUniform) {
        self.queue
            .write_buffer(&self.scene_buf, 0, bytemuck::bytes_of(uniform));
    }

    pub fn update_instances(&mut self, instances: &[InstanceRaw]) {
        let data = bytemuck::cast_slice(instances);
        let needed = data.len() as u64;
        if needed > self.instance_buf.size() {
            self.instance_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Instances"),
                size: needed,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
        }
        self.queue.write_buffer(&self.instance_buf, 0, data);
        self.instance_count = instances.len() as u32;
    }

    pub fn render_frame(&self) -> Result<(), String> {
        let output = self
            .surface
            .get_current_texture()
            .map_err(|e| format!("Surface: {e}"))?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Frame"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Main Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.18,
                            g: 0.32,
                            b: 0.38,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buf.slice(..));
            pass.set_vertex_buffer(1, self.instance_buf.slice(..));
            pass.draw(0..self.vertex_count, 0..self.instance_count);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn size(&self) -> (u32, u32) {
        self.size
    }
}
```

- [ ] **Step 4: Update lib.rs to render a test cube**

```rust
// web/src/lib.rs
mod camera;
mod gpu_types;
mod renderer;

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crate::gpu_types::*;
use crate::renderer::WgpuRenderer;

struct AppState {
    renderer: WgpuRenderer,
    time: f64,
}

thread_local! {
    static STATE: RefCell<Option<AppState>> = RefCell::new(None);
}

#[wasm_bindgen(start)]
pub fn wasm_main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).ok();
    log::info!("karl-sims WASM module loaded");
}

#[wasm_bindgen]
pub async fn create_renderer(canvas_id: &str) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or("canvas not found")?
        .dyn_into::<HtmlCanvasElement>()?;

    let renderer = WgpuRenderer::new(canvas)
        .await
        .map_err(|e| JsValue::from_str(&e))?;

    // Set up initial scene uniform
    renderer.update_scene(&SceneUniform {
        light_dir: [0.4, 0.8, 0.3],
        fog_near: 8.0,
        fog_color: [0.18, 0.32, 0.38],
        fog_far: 40.0,
    });

    STATE.with(|s| {
        *s.borrow_mut() = Some(AppState {
            renderer,
            time: 0.0,
        });
    });

    start_render_loop();
    Ok(())
}

#[wasm_bindgen]
pub fn renderer_resize(width: u32, height: u32) {
    STATE.with(|s| {
        if let Some(state) = s.borrow_mut().as_mut() {
            state.renderer.resize(width, height);
        }
    });
}

fn tick(dt: f64) {
    STATE.with(|s| {
        let mut borrow = s.borrow_mut();
        let state = match borrow.as_mut() {
            Some(s) => s,
            None => return,
        };
        state.time += dt;

        let (w, h) = state.renderer.size();
        let aspect = w as f32 / h as f32;

        // Orbit camera: slowly rotate around origin
        let angle = state.time as f32 * 0.3;
        let dist = 12.0f32;
        let eye = glam::Vec3::new(angle.cos() * dist, 5.0, angle.sin() * dist);
        let target = glam::Vec3::new(0.0, 0.5, 0.0);
        let view = glam::Mat4::look_at_rh(eye, target, glam::Vec3::Y);
        let proj = glam::Mat4::perspective_rh(45.0f32.to_radians(), aspect, 0.1, 100.0);

        state.renderer.update_camera(&CameraUniform {
            view_proj: (proj * view).to_cols_array_2d(),
            camera_pos: eye.to_array(),
            _pad: 0.0,
        });

        // Test instances: one cream cube + ground plane
        let cube_model = glam::Mat4::from_translation(glam::Vec3::new(0.0, 1.0, 0.0));
        let ground_model = glam::Mat4::from_scale(glam::Vec3::new(20.0, 0.05, 20.0));

        state.renderer.update_instances(&[
            InstanceRaw {
                model: cube_model.to_cols_array_2d(),
                color: [0.92, 0.90, 0.85],
                flags: 0,
            },
            InstanceRaw {
                model: ground_model.to_cols_array_2d(),
                color: [0.45, 0.52, 0.56],
                flags: 1,
            },
        ]);

        if let Err(e) = state.renderer.render_frame() {
            log::error!("Render: {e}");
        }
    });
}

fn start_render_loop() {
    let last_time: Rc<RefCell<Option<f64>>> = Rc::new(RefCell::new(None));
    let f: Rc<RefCell<Option<Closure<dyn FnMut(JsValue)>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();
    let lt = last_time.clone();

    *g.borrow_mut() = Some(Closure::new(move |timestamp: JsValue| {
        let now = timestamp.as_f64().unwrap_or(0.0) / 1000.0; // ms → s
        let dt = {
            let mut lt_ref = lt.borrow_mut();
            let dt = lt_ref.map_or(0.016, |prev| (now - prev).min(0.1));
            *lt_ref = Some(now);
            dt
        };
        tick(dt);
        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    request_animation_frame(g.borrow().as_ref().unwrap());
}

fn request_animation_frame(f: &Closure<dyn FnMut(JsValue)>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .unwrap();
}
```

- [ ] **Step 5: Create placeholder camera module**

```rust
// web/src/camera.rs
// OrbitCamera — interactive camera implemented in Task 6
```

- [ ] **Step 6: Build and verify a lit cube + checkered ground in the browser**

Run:
```bash
cd web && wasm-pack build --target web --dev && cd ../frontend && npm install && npm run dev
```
Expected: Browser shows a cream-colored cube sitting on a checkered blue-gray ground plane. The camera slowly orbits. Depth fog fades distant parts of the ground plane to teal.

- [ ] **Step 7: Commit**

```bash
git add web/src/ frontend/src/
git commit -m "feat: box mesh with Karl Sims flat shading and checkered ground"
```

---

## Task 5: Orbit Camera with Mouse Controls

**Files:**
- Modify: `web/src/camera.rs`
- Modify: `web/src/lib.rs`

- [ ] **Step 1: Implement OrbitCamera**

```rust
// web/src/camera.rs
pub struct OrbitCamera {
    pub azimuth: f32,
    pub elevation: f32,
    pub distance: f32,
    pub target: glam::Vec3,
    // Interaction state
    dragging: bool,
    last_mouse: (f32, f32),
}

impl OrbitCamera {
    pub fn new() -> Self {
        Self {
            azimuth: 0.3,
            elevation: 0.4,
            distance: 14.0,
            target: glam::Vec3::new(0.0, 0.5, 0.0),
            dragging: false,
            last_mouse: (0.0, 0.0),
        }
    }

    pub fn eye(&self) -> glam::Vec3 {
        let y = self.elevation.sin() * self.distance;
        let xz_dist = self.elevation.cos() * self.distance;
        let x = self.azimuth.cos() * xz_dist;
        let z = self.azimuth.sin() * xz_dist;
        self.target + glam::Vec3::new(x, y, z)
    }

    pub fn view_matrix(&self) -> glam::Mat4 {
        glam::Mat4::look_at_rh(self.eye(), self.target, glam::Vec3::Y)
    }

    pub fn projection_matrix(&self, aspect: f32) -> glam::Mat4 {
        glam::Mat4::perspective_rh(45.0f32.to_radians(), aspect, 0.1, 100.0)
    }

    pub fn on_mouse_down(&mut self, x: f32, y: f32) {
        self.dragging = true;
        self.last_mouse = (x, y);
    }

    pub fn on_mouse_up(&mut self) {
        self.dragging = false;
    }

    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        if !self.dragging {
            return;
        }
        let dx = x - self.last_mouse.0;
        let dy = y - self.last_mouse.1;
        self.last_mouse = (x, y);

        self.azimuth -= dx * 0.005;
        self.elevation = (self.elevation + dy * 0.005).clamp(0.05, 1.5);
    }

    pub fn on_wheel(&mut self, delta: f32) {
        self.distance = (self.distance + delta * 0.01).clamp(3.0, 50.0);
    }
}
```

- [ ] **Step 2: Wire mouse events to camera in lib.rs**

Add to `web/src/lib.rs`, after the `STATE` thread-local, add a camera thread-local and event setup:

Add `camera: crate::camera::OrbitCamera` field to `AppState`:

```rust
// In AppState struct, add:
struct AppState {
    renderer: WgpuRenderer,
    camera: crate::camera::OrbitCamera,
    time: f64,
}
```

Update `create_renderer` — after creating the renderer, initialize camera and attach mouse events:

```rust
// In create_renderer, replace the STATE.with block:
    let camera = crate::camera::OrbitCamera::new();

    STATE.with(|s| {
        *s.borrow_mut() = Some(AppState {
            renderer,
            camera,
            time: 0.0,
        });
    });

    setup_mouse_events(canvas_id)?;
    start_render_loop();
    Ok(())
```

Add the mouse event setup function:

```rust
fn setup_mouse_events(canvas_id: &str) -> Result<(), JsValue> {
    let document = web_sys::window().unwrap().document().unwrap();
    let canvas = document
        .get_element_by_id(canvas_id)
        .unwrap()
        .dyn_into::<web_sys::EventTarget>()?;

    // Mouse down
    let on_down = Closure::<dyn FnMut(_)>::new(move |e: web_sys::MouseEvent| {
        STATE.with(|s| {
            if let Some(state) = s.borrow_mut().as_mut() {
                state.camera.on_mouse_down(e.client_x() as f32, e.client_y() as f32);
            }
        });
    });
    canvas.add_event_listener_with_callback("mousedown", on_down.as_ref().unchecked_ref())?;
    on_down.forget();

    // Mouse up (on window to catch releases outside canvas)
    let window = web_sys::window().unwrap();
    let on_up = Closure::<dyn FnMut(_)>::new(move |_e: web_sys::MouseEvent| {
        STATE.with(|s| {
            if let Some(state) = s.borrow_mut().as_mut() {
                state.camera.on_mouse_up();
            }
        });
    });
    window.add_event_listener_with_callback("mouseup", on_up.as_ref().unchecked_ref())?;
    on_up.forget();

    // Mouse move
    let on_move = Closure::<dyn FnMut(_)>::new(move |e: web_sys::MouseEvent| {
        STATE.with(|s| {
            if let Some(state) = s.borrow_mut().as_mut() {
                state.camera.on_mouse_move(e.client_x() as f32, e.client_y() as f32);
            }
        });
    });
    canvas.add_event_listener_with_callback("mousemove", on_move.as_ref().unchecked_ref())?;
    on_move.forget();

    // Wheel
    let on_wheel = Closure::<dyn FnMut(_)>::new(move |e: web_sys::WheelEvent| {
        e.prevent_default();
        STATE.with(|s| {
            if let Some(state) = s.borrow_mut().as_mut() {
                state.camera.on_wheel(e.delta_y() as f32);
            }
        });
    });
    let mut opts = web_sys::AddEventListenerOptions::new();
    opts.passive(false);
    canvas.add_event_listener_with_callback_and_add_event_listener_options(
        "wheel",
        on_wheel.as_ref().unchecked_ref(),
        &opts,
    )?;
    on_wheel.forget();

    Ok(())
}
```

- [ ] **Step 3: Update tick() to use the orbit camera instead of auto-rotate**

Replace the camera section of `tick()`:

```rust
fn tick(dt: f64) {
    STATE.with(|s| {
        let mut borrow = s.borrow_mut();
        let state = match borrow.as_mut() {
            Some(s) => s,
            None => return,
        };
        state.time += dt;

        let (w, h) = state.renderer.size();
        let aspect = w as f32 / h as f32;

        let eye = state.camera.eye();
        let view = state.camera.view_matrix();
        let proj = state.camera.projection_matrix(aspect);

        state.renderer.update_camera(&CameraUniform {
            view_proj: (proj * view).to_cols_array_2d(),
            camera_pos: eye.to_array(),
            _pad: 0.0,
        });

        // Test instances: one cream cube + ground plane
        let t = state.time as f32;
        let cube_rot = glam::Mat4::from_rotation_y(t * 0.5);
        let cube_model =
            glam::Mat4::from_translation(glam::Vec3::new(0.0, 1.0, 0.0)) * cube_rot;
        let ground_model = glam::Mat4::from_scale(glam::Vec3::new(20.0, 0.05, 20.0));

        state.renderer.update_instances(&[
            InstanceRaw {
                model: cube_model.to_cols_array_2d(),
                color: [0.92, 0.90, 0.85],
                flags: 0,
            },
            InstanceRaw {
                model: ground_model.to_cols_array_2d(),
                color: [0.45, 0.52, 0.56],
                flags: 1,
            },
        ]);

        if let Err(e) = state.renderer.render_frame() {
            log::error!("Render: {e}");
        }
    });
}
```

- [ ] **Step 4: Build and verify mouse-controlled camera**

Run:
```bash
cd web && wasm-pack build --target web --dev && cd ../frontend && npm run dev
```
Expected: drag to orbit, scroll to zoom. Cube rotates slowly. Ground plane visible with checkered pattern, fading into fog.

- [ ] **Step 5: Commit**

```bash
git add web/src/
git commit -m "feat: orbit camera with mouse drag and scroll zoom"
```

---

## Task 6: Core — Rigid Body + Joint Types

**Files:**
- Modify: `core/src/body.rs`
- Modify: `core/src/joint.rs`
- Test: inline `#[cfg(test)]` modules

- [ ] **Step 1: Write tests for RigidBody**

```rust
// core/src/body.rs
use glam::DVec3;

#[derive(Debug, Clone)]
pub struct RigidBody {
    /// Half-extents of the rectangular solid (width/2, height/2, depth/2)
    pub half_extents: DVec3,
    /// Mass (computed from dimensions, uniform density = 1.0)
    pub mass: f64,
    /// Diagonal of the inertia tensor in local frame (Ixx, Iyy, Izz)
    pub inertia_diag: DVec3,
}

impl RigidBody {
    pub fn new(half_extents: DVec3) -> Self {
        // Full dimensions
        let w = half_extents.x * 2.0;
        let h = half_extents.y * 2.0;
        let d = half_extents.z * 2.0;
        let volume = w * h * d;
        let density = 1.0;
        let mass = density * volume;

        // Box inertia tensor (diagonal, about center of mass)
        let ixx = mass / 12.0 * (h * h + d * d);
        let iyy = mass / 12.0 * (w * w + d * d);
        let izz = mass / 12.0 * (w * w + h * h);

        Self {
            half_extents,
            mass,
            inertia_diag: DVec3::new(ixx, iyy, izz),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_cube_mass() {
        let body = RigidBody::new(DVec3::new(0.5, 0.5, 0.5));
        assert!((body.mass - 1.0).abs() < 1e-10);
    }

    #[test]
    fn unit_cube_inertia_is_symmetric() {
        let body = RigidBody::new(DVec3::new(0.5, 0.5, 0.5));
        assert!((body.inertia_diag.x - body.inertia_diag.y).abs() < 1e-10);
        assert!((body.inertia_diag.y - body.inertia_diag.z).abs() < 1e-10);
        // I = m/12 * (h^2 + d^2) = 1/12 * (1+1) = 1/6
        assert!((body.inertia_diag.x - 1.0 / 6.0).abs() < 1e-10);
    }

    #[test]
    fn rectangular_body_inertia() {
        // 2x1x1 box (half_extents 1, 0.5, 0.5), mass = 2
        let body = RigidBody::new(DVec3::new(1.0, 0.5, 0.5));
        assert!((body.mass - 2.0).abs() < 1e-10);
        // Ixx = 2/12*(1^2+1^2) = 2/6
        assert!((body.inertia_diag.x - 2.0 / 6.0).abs() < 1e-10);
        // Iyy = 2/12*(2^2+1^2) = 2/12*5 = 10/12
        assert!((body.inertia_diag.y - 10.0 / 12.0).abs() < 1e-10);
    }
}
```

- [ ] **Step 2: Run body tests**

Run: `cargo test -p karl-sims-core body`
Expected: 3 tests pass

- [ ] **Step 3: Write Joint type**

```rust
// core/src/joint.rs
use glam::DVec3;

/// Joint type determines degrees of freedom.
/// For M1, only Revolute is dynamically simulated.
/// Others are defined for the genotype representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JointType {
    Rigid,       // 0 DOF
    Revolute,    // 1 DOF: rotation about primary axis
    Twist,       // 1 DOF: rotation about attachment axis
    Universal,   // 2 DOF: rotation about two axes
    BendTwist,   // 2 DOF: bend + twist
    TwistBend,   // 2 DOF: twist + bend
    Spherical,   // 3 DOF: rotation about all axes
}

impl JointType {
    pub fn dof_count(&self) -> usize {
        match self {
            JointType::Rigid => 0,
            JointType::Revolute | JointType::Twist => 1,
            JointType::Universal | JointType::BendTwist | JointType::TwistBend => 2,
            JointType::Spherical => 3,
        }
    }
}

/// A joint connecting a parent body to a child body.
#[derive(Debug, Clone)]
pub struct Joint {
    pub parent_idx: usize,
    pub child_idx: usize,
    pub joint_type: JointType,

    /// Position on parent body surface where joint attaches (local coords)
    pub parent_anchor: DVec3,
    /// Position on child body where joint attaches (local coords, typically a face center)
    pub child_anchor: DVec3,
    /// Primary rotation axis (in parent-local frame)
    pub axis: DVec3,

    /// Joint angle state (radians). One per DOF, but for M1 we use only [0].
    pub angles: [f64; 3],
    /// Joint angular velocity (rad/s)
    pub velocities: [f64; 3],

    /// Joint limits
    pub angle_min: [f64; 3],
    pub angle_max: [f64; 3],
    pub limit_stiffness: f64,
    pub damping: f64,
}

impl Joint {
    pub fn revolute(parent_idx: usize, child_idx: usize, parent_anchor: DVec3, child_anchor: DVec3, axis: DVec3) -> Self {
        Self {
            parent_idx,
            child_idx,
            joint_type: JointType::Revolute,
            parent_anchor,
            child_anchor,
            axis: axis.normalize(),
            angles: [0.0; 3],
            velocities: [0.0; 3],
            angle_min: [-1.5; 3],
            angle_max: [1.5; 3],
            limit_stiffness: 20.0,
            damping: 0.5,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joint_type_dof_count() {
        assert_eq!(JointType::Rigid.dof_count(), 0);
        assert_eq!(JointType::Revolute.dof_count(), 1);
        assert_eq!(JointType::Universal.dof_count(), 2);
        assert_eq!(JointType::Spherical.dof_count(), 3);
    }

    #[test]
    fn revolute_joint_defaults() {
        let j = Joint::revolute(0, 1, DVec3::ZERO, DVec3::ZERO, DVec3::Y);
        assert_eq!(j.joint_type, JointType::Revolute);
        assert!((j.axis - DVec3::Y).length() < 1e-10);
        assert_eq!(j.angles[0], 0.0);
    }
}
```

- [ ] **Step 4: Run joint tests**

Run: `cargo test -p karl-sims-core joint`
Expected: 2 tests pass

- [ ] **Step 5: Commit**

```bash
git add core/src/
git commit -m "feat: rigid body and joint types with inertia computation"
```

---

## Task 7: Core — World + Simulation Step

**Files:**
- Modify: `core/src/world.rs`

- [ ] **Step 1: Write World struct with forward kinematics**

```rust
// core/src/world.rs
use glam::{DAffine3, DMat3, DQuat, DVec3};

use crate::body::RigidBody;
use crate::joint::Joint;

#[derive(Debug, Clone)]
pub struct World {
    pub bodies: Vec<RigidBody>,
    pub joints: Vec<Joint>,
    /// World-space transforms for each body, computed by forward_kinematics()
    pub transforms: Vec<DAffine3>,
    /// Applied torque per joint (one per DOF, using [0] for revolute)
    pub torques: Vec<[f64; 3]>,
    pub root: usize,
    pub time: f64,
}

impl World {
    pub fn new() -> Self {
        Self {
            bodies: Vec::new(),
            joints: Vec::new(),
            transforms: Vec::new(),
            torques: Vec::new(),
            root: 0,
            time: 0.0,
        }
    }

    pub fn add_body(&mut self, half_extents: DVec3) -> usize {
        let idx = self.bodies.len();
        self.bodies.push(RigidBody::new(half_extents));
        self.transforms.push(DAffine3::IDENTITY);
        idx
    }

    pub fn add_joint(&mut self, joint: Joint) -> usize {
        let idx = self.joints.len();
        self.torques.push([0.0; 3]);
        self.joints.push(joint);
        idx
    }

    pub fn set_root_transform(&mut self, transform: DAffine3) {
        if !self.bodies.is_empty() {
            self.transforms[self.root] = transform;
        }
    }

    /// Compute world transforms for all bodies from joint angles.
    /// Assumes joints are ordered parent-first (parent always has lower body index than child).
    pub fn forward_kinematics(&mut self) {
        for joint in &self.joints {
            let parent_transform = self.transforms[joint.parent_idx];

            // Joint rotation quaternion from angle × axis
            let rotation = DQuat::from_axis_angle(joint.axis, joint.angles[0]);

            // Child transform:
            // 1. Start at parent
            // 2. Translate to parent anchor point
            // 3. Apply joint rotation
            // 4. Translate by negative child anchor (child anchor moves to joint origin)
            let joint_pos = parent_transform.transform_point3(joint.parent_anchor);
            let parent_rot = DQuat::from_mat3(&DMat3::from_cols(
                parent_transform.matrix3.col(0),
                parent_transform.matrix3.col(1),
                parent_transform.matrix3.col(2),
            ));
            let world_rotation = parent_rot * rotation;
            let child_offset = world_rotation * (-joint.child_anchor);

            self.transforms[joint.child_idx] = DAffine3 {
                matrix3: DMat3::from_quat(world_rotation),
                translation: joint_pos + child_offset,
            };
        }
    }

    /// Advance simulation by dt seconds using semi-implicit Euler.
    pub fn step(&mut self, dt: f64) {
        for (i, joint) in self.joints.iter_mut().enumerate() {
            let dof = joint.joint_type.dof_count();
            if dof == 0 {
                continue;
            }

            // For M1, we simulate DOF 0 only (revolute)
            let torque = self.torques[i][0];

            // Simple effective inertia estimate: use child body's inertia about joint axis
            let child = &self.bodies[joint.child_idx];
            // Project inertia onto rotation axis: I_axis = axis^T * I * axis
            let ax = joint.axis;
            let i_axis = ax.x * ax.x * child.inertia_diag.x
                + ax.y * ax.y * child.inertia_diag.y
                + ax.z * ax.z * child.inertia_diag.z;
            let i_eff = i_axis.max(0.001); // prevent division by zero

            // Joint limit spring torque
            let angle = joint.angles[0];
            let mut limit_torque = 0.0;
            if angle < joint.angle_min[0] {
                limit_torque = joint.limit_stiffness * (joint.angle_min[0] - angle);
            } else if angle > joint.angle_max[0] {
                limit_torque = joint.limit_stiffness * (joint.angle_max[0] - angle);
            }

            // Semi-implicit Euler: update velocity first, then position
            let total_torque = torque + limit_torque - joint.damping * joint.velocities[0];
            joint.velocities[0] += total_torque / i_eff * dt;
            joint.angles[0] += joint.velocities[0] * dt;
        }

        self.forward_kinematics();
        self.time += dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_body_world() -> World {
        let mut w = World::new();
        let _root = w.add_body(DVec3::new(0.5, 0.5, 0.5));
        let _child = w.add_body(DVec3::new(0.5, 0.25, 0.25));
        w.add_joint(Joint::revolute(
            0,
            1,
            DVec3::new(0.5, 0.0, 0.0),   // right face of parent
            DVec3::new(-0.5, 0.0, 0.0),   // left face of child
            DVec3::Z,                       // rotate around Z axis
        ));
        w.set_root_transform(DAffine3::IDENTITY);
        w.forward_kinematics();
        w
    }

    #[test]
    fn fk_zero_angle_child_adjacent() {
        let w = two_body_world();
        // Child should be positioned to the right of parent
        let child_pos = w.transforms[1].translation;
        // Parent right edge at x=0.5, child left edge at -0.5 from child center
        // So child center at x = 0.5 + 0.5 = 1.0
        assert!((child_pos.x - 1.0).abs() < 1e-10, "got {child_pos}");
        assert!(child_pos.y.abs() < 1e-10);
        assert!(child_pos.z.abs() < 1e-10);
    }

    #[test]
    fn fk_90_degree_rotates_child() {
        let mut w = two_body_world();
        w.joints[0].angles[0] = std::f64::consts::FRAC_PI_2; // 90 degrees
        w.forward_kinematics();

        let child_pos = w.transforms[1].translation;
        // After 90° rotation around Z, the child's +X (length direction) points in +Y
        // Joint is at parent's right face (0.5, 0, 0)
        // Child anchor (-0.5, 0, 0) rotated 90° around Z becomes (0, -0.5, 0)
        // Child center = joint_pos + world_rot * (-child_anchor)
        //              = (0.5, 0, 0) + rot90z * (0.5, 0, 0)
        //              = (0.5, 0, 0) + (0, 0.5, 0)
        //              = (0.5, 0.5, 0)
        assert!((child_pos.x - 0.5).abs() < 1e-10, "x: got {}", child_pos.x);
        assert!((child_pos.y - 0.5).abs() < 1e-10, "y: got {}", child_pos.y);
    }

    #[test]
    fn step_with_torque_changes_angle() {
        let mut w = two_body_world();
        w.torques[0][0] = 1.0; // apply torque

        let angle_before = w.joints[0].angles[0];
        for _ in 0..100 {
            w.step(1.0 / 60.0);
        }
        let angle_after = w.joints[0].angles[0];

        assert!(
            angle_after > angle_before,
            "angle should increase: {angle_before} -> {angle_after}"
        );
    }

    #[test]
    fn joint_limits_prevent_excessive_rotation() {
        let mut w = two_body_world();
        w.joints[0].angle_max[0] = 1.0;
        w.torques[0][0] = 10.0; // strong torque

        for _ in 0..1000 {
            w.step(1.0 / 60.0);
        }

        let angle = w.joints[0].angles[0];
        // Should settle near the limit, not go wildly past it
        assert!(
            angle < 2.0,
            "angle should be bounded near limit: got {angle}"
        );
    }

    #[test]
    fn damping_reduces_velocity() {
        let mut w = two_body_world();
        w.joints[0].velocities[0] = 5.0; // initial velocity, no torque

        for _ in 0..300 {
            w.step(1.0 / 60.0);
        }

        let vel = w.joints[0].velocities[0];
        assert!(vel.abs() < 0.1, "velocity should decay: got {vel}");
    }
}
```

- [ ] **Step 2: Run world tests**

Run: `cargo test -p karl-sims-core world`
Expected: 5 tests pass

- [ ] **Step 3: Commit**

```bash
git add core/src/world.rs
git commit -m "feat: world simulation with forward kinematics and semi-implicit Euler"
```

---

## Task 8: Core — Test Scene Builders

**Files:**
- Modify: `core/src/scene.rs`

- [ ] **Step 1: Implement test scene builders**

```rust
// core/src/scene.rs
use glam::{DAffine3, DVec3};

use crate::joint::Joint;
use crate::world::World;

/// Scene 1: A single box, no joints. Useful for verifying rendering.
pub fn single_box() -> World {
    let mut world = World::new();
    world.add_body(DVec3::new(0.6, 0.4, 0.5));
    world.set_root_transform(DAffine3::from_translation(DVec3::new(0.0, 1.0, 0.0)));
    world.forward_kinematics();
    world
}

/// Scene 2: Two boxes connected by a revolute joint.
/// Parent is stationary, child swings on a hinge.
pub fn hinged_pair() -> World {
    let mut world = World::new();
    let _parent = world.add_body(DVec3::new(0.5, 0.5, 0.5));
    let _child = world.add_body(DVec3::new(0.6, 0.2, 0.3));
    world.add_joint(Joint::revolute(
        0,
        1,
        DVec3::new(0.5, 0.0, 0.0),  // right face of parent
        DVec3::new(-0.6, 0.0, 0.0), // left face of child
        DVec3::Z,                     // hinge around Z
    ));
    world.set_root_transform(DAffine3::from_translation(DVec3::new(0.0, 2.0, 0.0)));
    world.forward_kinematics();
    world
}

/// Scene 3: A central body with 4 flipper-like appendages.
/// Each flipper is driven by a sinusoidal torque with a phase offset,
/// producing a paddling motion.
pub fn starfish() -> World {
    let mut world = World::new();

    // Central body
    let _center = world.add_body(DVec3::new(0.5, 0.3, 0.5));

    // Four flippers, attached to +X, -X, +Z, -Z faces
    let flipper_half = DVec3::new(0.5, 0.08, 0.25);
    let anchors = [
        (DVec3::new(0.5, 0.0, 0.0), DVec3::new(-0.5, 0.0, 0.0), DVec3::Z),   // +X face
        (DVec3::new(-0.5, 0.0, 0.0), DVec3::new(0.5, 0.0, 0.0), DVec3::Z),    // -X face
        (DVec3::new(0.0, 0.0, 0.5), DVec3::new(0.0, 0.0, -0.5), DVec3::X),    // +Z face
        (DVec3::new(0.0, 0.0, -0.5), DVec3::new(0.0, 0.0, 0.5), DVec3::X),    // -Z face
    ];

    for (parent_anchor, child_anchor, axis) in anchors {
        let child = world.add_body(flipper_half);
        let mut joint = Joint::revolute(0, child, parent_anchor, child_anchor, axis);
        joint.angle_min[0] = -0.8;
        joint.angle_max[0] = 0.8;
        joint.damping = 0.3;
        world.add_joint(joint);
    }

    world.set_root_transform(DAffine3::from_translation(DVec3::new(0.0, 1.5, 0.0)));
    world.forward_kinematics();
    world
}

/// Apply sinusoidal driving torques for the starfish scene.
/// Each flipper gets a phase-offset sine wave.
pub fn starfish_torques(world: &mut World) {
    let t = world.time;
    let phases = [0.0, std::f64::consts::PI, std::f64::consts::FRAC_PI_2, std::f64::consts::PI * 1.5];
    let amplitude = 2.0;
    let frequency = 3.0;

    for (i, &phase) in phases.iter().enumerate() {
        if i < world.torques.len() {
            world.torques[i][0] = amplitude * (frequency * t + phase).sin();
        }
    }
}

/// Apply sinusoidal torque for the hinged pair scene.
pub fn hinged_pair_torque(world: &mut World) {
    if !world.torques.is_empty() {
        world.torques[0][0] = 3.0 * (2.0 * world.time).sin();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_box_has_one_body_no_joints() {
        let w = single_box();
        assert_eq!(w.bodies.len(), 1);
        assert_eq!(w.joints.len(), 0);
    }

    #[test]
    fn hinged_pair_has_two_bodies_one_joint() {
        let w = hinged_pair();
        assert_eq!(w.bodies.len(), 2);
        assert_eq!(w.joints.len(), 1);
    }

    #[test]
    fn starfish_has_five_bodies_four_joints() {
        let w = starfish();
        assert_eq!(w.bodies.len(), 5);
        assert_eq!(w.joints.len(), 4);
    }

    #[test]
    fn starfish_paddling_motion() {
        let mut w = starfish();
        // Run 2 seconds of simulation with torques
        for _ in 0..120 {
            starfish_torques(&mut w);
            w.step(1.0 / 60.0);
        }
        // All flippers should have non-zero angles (they're being driven)
        for joint in &w.joints {
            assert!(
                joint.angles[0].abs() > 0.01,
                "flipper should be moving: angle = {}",
                joint.angles[0]
            );
        }
    }
}
```

- [ ] **Step 2: Run scene tests**

Run: `cargo test -p karl-sims-core scene`
Expected: 4 tests pass

- [ ] **Step 3: Commit**

```bash
git add core/src/scene.rs
git commit -m "feat: test scenes — single box, hinged pair, starfish with paddling"
```

---

## Task 9: Physics-Renderer Bridge + Animation

**Files:**
- Modify: `web/src/lib.rs`

This task replaces the hardcoded test cube in `tick()` with actual physics simulation driving the rendered instances.

- [ ] **Step 1: Add scene management to AppState and update tick()**

Replace `web/src/lib.rs` entirely:

```rust
// web/src/lib.rs
mod camera;
mod gpu_types;
mod renderer;

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use karl_sims_core::scene;
use karl_sims_core::world::World;

use crate::camera::OrbitCamera;
use crate::gpu_types::*;
use crate::renderer::WgpuRenderer;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SceneId {
    SingleBox,
    HingedPair,
    Starfish,
}

struct AppState {
    renderer: WgpuRenderer,
    camera: OrbitCamera,
    world: World,
    scene_id: SceneId,
    paused: bool,
}

thread_local! {
    static STATE: RefCell<Option<AppState>> = RefCell::new(None);
}

#[wasm_bindgen(start)]
pub fn wasm_main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).ok();
    log::info!("karl-sims WASM module loaded");
}

#[wasm_bindgen]
pub async fn create_renderer(canvas_id: &str) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let document = window.document().ok_or("no document")?;
    let canvas = document
        .get_element_by_id(canvas_id)
        .ok_or("canvas not found")?
        .dyn_into::<HtmlCanvasElement>()?;

    let renderer = WgpuRenderer::new(canvas)
        .await
        .map_err(|e| JsValue::from_str(&e))?;

    renderer.update_scene(&SceneUniform {
        light_dir: [0.4, 0.8, 0.3],
        fog_near: 8.0,
        fog_color: [0.18, 0.32, 0.38],
        fog_far: 40.0,
    });

    let world = scene::starfish();

    STATE.with(|s| {
        *s.borrow_mut() = Some(AppState {
            renderer,
            camera: OrbitCamera::new(),
            world,
            scene_id: SceneId::Starfish,
            paused: false,
        });
    });

    setup_mouse_events(canvas_id)?;
    start_render_loop();
    Ok(())
}

#[wasm_bindgen]
pub fn set_scene(name: &str) {
    STATE.with(|s| {
        if let Some(state) = s.borrow_mut().as_mut() {
            let (world, id) = match name {
                "single_box" => (scene::single_box(), SceneId::SingleBox),
                "hinged_pair" => (scene::hinged_pair(), SceneId::HingedPair),
                _ => (scene::starfish(), SceneId::Starfish),
            };
            state.world = world;
            state.scene_id = id;
        }
    });
}

#[wasm_bindgen]
pub fn set_paused(paused: bool) {
    STATE.with(|s| {
        if let Some(state) = s.borrow_mut().as_mut() {
            state.paused = paused;
        }
    });
}

#[wasm_bindgen]
pub fn reset_scene() {
    STATE.with(|s| {
        if let Some(state) = s.borrow_mut().as_mut() {
            let world = match state.scene_id {
                SceneId::SingleBox => scene::single_box(),
                SceneId::HingedPair => scene::hinged_pair(),
                SceneId::Starfish => scene::starfish(),
            };
            state.world = world;
        }
    });
}

#[wasm_bindgen]
pub fn renderer_resize(width: u32, height: u32) {
    STATE.with(|s| {
        if let Some(state) = s.borrow_mut().as_mut() {
            state.renderer.resize(width, height);
        }
    });
}

fn tick(dt: f64) {
    STATE.with(|s| {
        let mut borrow = s.borrow_mut();
        let state = match borrow.as_mut() {
            Some(s) => s,
            None => return,
        };

        // Physics step
        if !state.paused {
            let physics_dt = 1.0 / 60.0;
            // Apply scene-specific torques
            match state.scene_id {
                SceneId::SingleBox => {}
                SceneId::HingedPair => scene::hinged_pair_torque(&mut state.world),
                SceneId::Starfish => scene::starfish_torques(&mut state.world),
            }
            state.world.step(physics_dt);
        }

        // Camera
        let (w, h) = state.renderer.size();
        let aspect = w as f32 / h as f32;
        let eye = state.camera.eye();
        let view = state.camera.view_matrix();
        let proj = state.camera.projection_matrix(aspect);

        state.renderer.update_camera(&CameraUniform {
            view_proj: (proj * view).to_cols_array_2d(),
            camera_pos: eye.to_array(),
            _pad: 0.0,
        });

        // Build instance list from physics bodies
        let cream = [0.92f32, 0.90, 0.85];
        let mut instances: Vec<InstanceRaw> = Vec::with_capacity(state.world.bodies.len() + 1);

        for (i, body) in state.world.bodies.iter().enumerate() {
            let t = &state.world.transforms[i];
            let scale = glam::DVec3::new(
                body.half_extents.x * 2.0,
                body.half_extents.y * 2.0,
                body.half_extents.z * 2.0,
            );
            // Construct f32 model matrix: translation * rotation * scale
            let model_f64 = glam::DAffine3 {
                matrix3: glam::DMat3::from_cols(
                    t.matrix3.col(0) * scale.x,
                    t.matrix3.col(1) * scale.y,
                    t.matrix3.col(2) * scale.z,
                ),
                translation: t.translation,
            };
            let model = glam::Mat4::from(glam::Affine3A::from(model_f64));

            instances.push(InstanceRaw {
                model: model.to_cols_array_2d(),
                color: cream,
                flags: 0,
            });
        }

        // Ground plane
        let ground = glam::Mat4::from_scale(glam::Vec3::new(30.0, 0.05, 30.0));
        instances.push(InstanceRaw {
            model: ground.to_cols_array_2d(),
            color: [0.45, 0.52, 0.56],
            flags: 1,
        });

        state.renderer.update_instances(&instances);

        if let Err(e) = state.renderer.render_frame() {
            log::error!("Render: {e}");
        }
    });
}

fn setup_mouse_events(canvas_id: &str) -> Result<(), JsValue> {
    let document = web_sys::window().unwrap().document().unwrap();
    let canvas = document
        .get_element_by_id(canvas_id)
        .unwrap()
        .dyn_into::<web_sys::EventTarget>()?;

    let on_down = Closure::<dyn FnMut(_)>::new(move |e: web_sys::MouseEvent| {
        STATE.with(|s| {
            if let Some(state) = s.borrow_mut().as_mut() {
                state.camera.on_mouse_down(e.client_x() as f32, e.client_y() as f32);
            }
        });
    });
    canvas.add_event_listener_with_callback("mousedown", on_down.as_ref().unchecked_ref())?;
    on_down.forget();

    let window = web_sys::window().unwrap();
    let on_up = Closure::<dyn FnMut(_)>::new(move |_e: web_sys::MouseEvent| {
        STATE.with(|s| {
            if let Some(state) = s.borrow_mut().as_mut() {
                state.camera.on_mouse_up();
            }
        });
    });
    window.add_event_listener_with_callback("mouseup", on_up.as_ref().unchecked_ref())?;
    on_up.forget();

    let on_move = Closure::<dyn FnMut(_)>::new(move |e: web_sys::MouseEvent| {
        STATE.with(|s| {
            if let Some(state) = s.borrow_mut().as_mut() {
                state.camera.on_mouse_move(e.client_x() as f32, e.client_y() as f32);
            }
        });
    });
    canvas.add_event_listener_with_callback("mousemove", on_move.as_ref().unchecked_ref())?;
    on_move.forget();

    let on_wheel = Closure::<dyn FnMut(_)>::new(move |e: web_sys::WheelEvent| {
        e.prevent_default();
        STATE.with(|s| {
            if let Some(state) = s.borrow_mut().as_mut() {
                state.camera.on_wheel(e.delta_y() as f32);
            }
        });
    });
    let mut opts = web_sys::AddEventListenerOptions::new();
    opts.passive(false);
    canvas.add_event_listener_with_callback_and_add_event_listener_options(
        "wheel",
        on_wheel.as_ref().unchecked_ref(),
        &opts,
    )?;
    on_wheel.forget();

    Ok(())
}

fn start_render_loop() {
    let last_time: Rc<RefCell<Option<f64>>> = Rc::new(RefCell::new(None));
    let f: Rc<RefCell<Option<Closure<dyn FnMut(JsValue)>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();
    let lt = last_time.clone();

    *g.borrow_mut() = Some(Closure::new(move |timestamp: JsValue| {
        let now = timestamp.as_f64().unwrap_or(0.0) / 1000.0;
        let dt = {
            let mut lt_ref = lt.borrow_mut();
            let dt = lt_ref.map_or(0.016, |prev| (now - prev).min(0.1));
            *lt_ref = Some(now);
            dt
        };
        tick(dt);
        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    request_animation_frame(g.borrow().as_ref().unwrap());
}

fn request_animation_frame(f: &Closure<dyn FnMut(JsValue)>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .unwrap();
}
```

- [ ] **Step 2: Update wasm.ts exports**

```typescript
// frontend/src/wasm.ts
import init, {
  create_renderer,
  renderer_resize,
  set_scene,
  set_paused,
  reset_scene,
} from "karl-sims-web";

let initialized = false;

export async function initWasm(): Promise<void> {
  if (initialized) return;
  await init();
  initialized = true;
}

export { create_renderer, renderer_resize, set_scene, set_paused, reset_scene };
```

- [ ] **Step 3: Build and verify animated starfish in browser**

Run:
```bash
cd web && wasm-pack build --target web --dev && cd ../frontend && npm install && npm run dev
```
Expected: Browser shows a starfish-like creature with 4 flippers paddling on a checkered ground. Camera is controllable with mouse drag/scroll. Cream-colored boxes with olive shadow tones, teal fog in background.

- [ ] **Step 4: Commit**

```bash
git add web/src/ frontend/src/
git commit -m "feat: physics-renderer bridge — animated creatures driven by simulation"
```

---

## Task 10: React UI — Scene Selector + Play/Pause/Reset

**Files:**
- Modify: `frontend/src/App.tsx`
- Modify: `frontend/src/App.css`

- [ ] **Step 1: Add scene selector and controls to App.tsx**

```tsx
// frontend/src/App.tsx
import { useCallback, useEffect, useRef, useState } from "react";
import {
  initWasm,
  create_renderer,
  set_scene,
  set_paused,
  reset_scene,
} from "./wasm";

const SCENES = [
  { id: "starfish", label: "Starfish (4 flippers)" },
  { id: "hinged_pair", label: "Hinged Pair" },
  { id: "single_box", label: "Single Box" },
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
      setReady(true);
    })();
  }, []);

  const handleSceneChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      const scene = e.target.value;
      setCurrentScene(scene);
      set_scene(scene);
    },
    []
  );

  const handlePause = useCallback(() => {
    setPaused((p) => {
      set_paused(!p);
      return !p;
    });
  }, []);

  const handleReset = useCallback(() => {
    reset_scene();
    setPaused(false);
    set_paused(false);
  }, []);

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

        <button onClick={handlePause} disabled={!ready}>
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
```

- [ ] **Step 2: Style the controls**

```css
/* frontend/src/App.css */
* {
  margin: 0;
  padding: 0;
  box-sizing: border-box;
}

body {
  background: #1a1a2e;
  display: flex;
  justify-content: center;
  align-items: center;
  min-height: 100vh;
  font-family: system-ui, sans-serif;
  color: #e0e0e0;
}

.app {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 20px;
}

h1 {
  font-size: 1.4rem;
  font-weight: 500;
  color: #b0c4ce;
}

.controls {
  display: flex;
  gap: 8px;
  align-items: center;
}

.controls select,
.controls button {
  padding: 6px 14px;
  border: 1px solid #444;
  border-radius: 4px;
  background: #2a2a3e;
  color: #e0e0e0;
  font-size: 0.9rem;
  cursor: pointer;
}

.controls select:hover,
.controls button:hover {
  background: #3a3a4e;
}

.controls button:disabled,
.controls select:disabled {
  opacity: 0.5;
  cursor: default;
}

#sim-canvas {
  border: 1px solid #333;
  border-radius: 4px;
}

.hint {
  font-size: 0.8rem;
  color: #667;
}
```

- [ ] **Step 3: Build and verify full UI**

Run:
```bash
cd web && wasm-pack build --target web --dev && cd ../frontend && npm install && npm run dev
```
Expected: Title, scene dropdown (Starfish/Hinged Pair/Single Box), Play/Pause button, Reset button. Switching scenes changes the creature. Pause stops physics. Reset returns to initial state.

- [ ] **Step 4: Run all core tests one final time**

Run: `cargo test -p karl-sims-core`
Expected: All tests pass (body: 3, joint: 2, world: 5, scene: 4 = 14 tests total)

- [ ] **Step 5: Commit**

```bash
git add frontend/src/
git commit -m "feat: React UI with scene selector, play/pause, and reset controls"
```

---

## Self-Review Checklist

**Spec coverage:**
- [x] Rust workspace with core + web crates
- [x] Core: rigid body with mass/inertia from box dimensions
- [x] Core: revolute joint with angle, velocity, limits, damping
- [x] Core: simple Euler integration (M2 upgrades to Featherstone)
- [x] Core: forward kinematics
- [x] No collision detection (spec says M1 has none)
- [x] Web: wgpu WebGPU renderer
- [x] Karl Sims visual style: flat shading, cream boxes, olive shadow, teal fog, checkered ground
- [x] Orbit camera with mouse controls
- [x] Test scenes: single box, hinged pair, starfish
- [x] React frontend with scene selector, play/pause/reset
- [x] Compiles to both native (cargo test) and WASM (wasm-pack)

**Not in M1 scope (deferred):**
- Featherstone's algorithm → M2
- RK4-Fehlberg adaptive integration → M3
- Water drag / collision → M3
- Genotype/phenotype/brain → M4
- All other joint types (simulated) → M2
- Server/SQLite → M6

**Placeholder scan:** No TBDs or TODOs. All code is complete.

**Type consistency:** `InstanceRaw`, `CameraUniform`, `SceneUniform` used consistently across gpu_types.rs, renderer.rs, and lib.rs. `World`, `Joint`, `RigidBody` used consistently across core modules. `set_scene`/`set_paused`/`reset_scene` match between wasm.ts and lib.rs exports.
