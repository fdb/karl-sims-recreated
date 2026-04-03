mod gpu_types;
mod renderer;

use std::cell::RefCell;
use std::rc::Rc;

use glam::{Mat4, Vec3};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use gpu_types::{CameraUniform, InstanceRaw, SceneUniform};
use renderer::WgpuRenderer;

struct AppState {
    renderer: WgpuRenderer,
    time: f64,
}

thread_local! {
    static APP: RefCell<Option<AppState>> = RefCell::new(None);
}

#[wasm_bindgen(start)]
pub fn wasm_main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).ok();
    log::info!("karl-sims WASM module loaded");
}

#[wasm_bindgen]
pub async fn create_renderer(canvas_id: &str) {
    let document = web_sys::window()
        .expect("no window")
        .document()
        .expect("no document");

    let canvas = document
        .get_element_by_id(canvas_id)
        .expect("no canvas element found")
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .expect("element is not a canvas");

    let renderer = WgpuRenderer::new(canvas).await;

    // Set up scene uniform (constant for now)
    let scene_uniform = SceneUniform {
        light_dir: [0.4, 0.8, 0.3],
        fog_near: 8.0,
        fog_color: [0.18, 0.32, 0.38],
        fog_far: 40.0,
    };
    renderer.update_scene(&scene_uniform);

    APP.with(|a| {
        *a.borrow_mut() = Some(AppState {
            renderer,
            time: 0.0,
        });
    });

    start_render_loop();
}

#[wasm_bindgen]
pub fn renderer_resize(width: u32, height: u32) {
    APP.with(|a| {
        if let Some(ref mut state) = *a.borrow_mut() {
            state.renderer.resize(width, height);
        }
    });
}

fn tick(state: &mut AppState, dt: f64) {
    state.time += dt;
    let t = state.time as f32;

    // Auto-orbit camera
    let orbit_speed = 0.3_f32;
    let orbit_radius = 12.0_f32;
    let orbit_height = 5.0_f32;
    let angle = t * orbit_speed;
    let eye = Vec3::new(
        angle.cos() * orbit_radius,
        orbit_height,
        angle.sin() * orbit_radius,
    );
    let center = Vec3::ZERO;
    let up = Vec3::Y;

    let (width, height) = state.renderer.size();
    let aspect = width as f32 / height.max(1) as f32;
    let view = Mat4::look_at_rh(eye, center, up);
    let proj = Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, aspect, 0.1, 100.0);
    let view_proj = proj * view;

    let camera_uniform = CameraUniform {
        view_proj: view_proj.to_cols_array_2d(),
        camera_pos: eye.to_array(),
        _pad: 0.0,
    };
    state.renderer.update_camera(&camera_uniform);

    // Instances: one cream cube slowly rotating, one ground plane
    let cube_rotation = Mat4::from_rotation_y(t * 0.5);
    let cube_translation = Mat4::from_translation(Vec3::new(0.0, 1.0, 0.0));
    let cube_model = cube_translation * cube_rotation;

    let ground_model =
        Mat4::from_scale(Vec3::new(20.0, 0.05, 20.0));

    let instances = [
        InstanceRaw {
            model: cube_model.to_cols_array_2d(),
            color: [0.92, 0.90, 0.82], // cream
            flags: 0,
        },
        InstanceRaw {
            model: ground_model.to_cols_array_2d(),
            color: [0.5, 0.5, 0.5], // unused for ground, but needed
            flags: 1,
        },
    ];

    state.renderer.update_instances(&instances);
    state.renderer.render_frame();
}

fn start_render_loop() {
    let f: Rc<RefCell<Option<Closure<dyn FnMut(JsValue)>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    let last_time: Rc<RefCell<Option<f64>>> = Rc::new(RefCell::new(None));

    *g.borrow_mut() = Some(Closure::new(move |val: JsValue| {
        let timestamp = val.as_f64().unwrap_or(0.0);
        let timestamp_secs = timestamp / 1000.0;

        let dt = {
            let mut lt = last_time.borrow_mut();
            let dt = match *lt {
                Some(prev) => timestamp_secs - prev,
                None => 0.0,
            };
            *lt = Some(timestamp_secs);
            dt.min(0.1) // cap dt to avoid huge jumps
        };

        APP.with(|a| {
            if let Some(ref mut state) = *a.borrow_mut() {
                tick(state, dt);
            }
        });

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
