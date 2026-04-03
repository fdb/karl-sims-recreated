mod camera;
mod gpu_types;
mod renderer;

use std::cell::RefCell;
use std::rc::Rc;

use glam::{Affine3A, DAffine3, DMat3, DVec3, Mat4, Vec3};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use karl_sims_core::creature::Creature;
use karl_sims_core::scene;
use karl_sims_core::world::World;

use camera::OrbitCamera;
use gpu_types::{CameraUniform, InstanceRaw, SceneUniform};
use renderer::WgpuRenderer;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SceneId {
    SingleBox,
    HingedPair,
    Starfish,
    UniversalJoint,
    SphericalJoint,
    TripleChain,
    SwimmingStarfish,
    RandomCreature,
}

struct AppState {
    renderer: WgpuRenderer,
    camera: OrbitCamera,
    world: World,
    creature: Option<Creature>,
    scene_id: SceneId,
    paused: bool,
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

    let renderer = WgpuRenderer::new(canvas.clone()).await;

    // Set up scene uniform (constant for now)
    let scene_uniform = SceneUniform {
        light_dir: [0.4, 0.8, 0.3],
        fog_near: 8.0,
        fog_color: [0.18, 0.32, 0.38],
        fog_far: 40.0,
    };
    renderer.update_scene(&scene_uniform);

    let world = scene::starfish();

    APP.with(|a| {
        *a.borrow_mut() = Some(AppState {
            renderer,
            camera: OrbitCamera::new(),
            world,
            creature: None,
            scene_id: SceneId::Starfish,
            paused: false,
        });
    });

    setup_mouse_events(&canvas);
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

fn build_world(scene_id: SceneId) -> World {
    match scene_id {
        SceneId::SingleBox => scene::single_box(),
        SceneId::HingedPair => scene::hinged_pair(),
        SceneId::Starfish => scene::starfish(),
        SceneId::UniversalJoint => scene::universal_joint_demo(),
        SceneId::SphericalJoint => scene::spherical_joint_demo(),
        SceneId::TripleChain => scene::triple_chain(),
        SceneId::SwimmingStarfish => scene::swimming_starfish(),
        SceneId::RandomCreature => World::new(), // handled via Creature path
    }
}

#[wasm_bindgen]
pub fn set_scene(name: &str) {
    APP.with(|a| {
        if let Some(ref mut state) = *a.borrow_mut() {
            if name == "random_creature" {
                let creature = scene::random_creature(42);
                state.world = World::new();
                state.creature = Some(creature);
                state.scene_id = SceneId::RandomCreature;
                return;
            }
            let scene_id = match name {
                "single_box" => SceneId::SingleBox,
                "hinged_pair" => SceneId::HingedPair,
                "universal" => SceneId::UniversalJoint,
                "spherical" => SceneId::SphericalJoint,
                "triple_chain" => SceneId::TripleChain,
                "swimming_starfish" => SceneId::SwimmingStarfish,
                _ => SceneId::Starfish,
            };
            state.scene_id = scene_id;
            state.creature = None;
            state.world = build_world(scene_id);
        }
    });
}

#[wasm_bindgen]
pub fn set_paused(paused: bool) {
    APP.with(|a| {
        if let Some(ref mut state) = *a.borrow_mut() {
            state.paused = paused;
        }
    });
}

#[wasm_bindgen]
pub fn reset_scene() {
    APP.with(|a| {
        if let Some(ref mut state) = *a.borrow_mut() {
            if state.scene_id == SceneId::RandomCreature {
                state.creature = Some(scene::random_creature(42));
            } else {
                state.creature = None;
                state.world = build_world(state.scene_id);
            }
        }
    });
}

fn build_instances(world: &World) -> Vec<InstanceRaw> {
    let cream = [0.92f32, 0.90, 0.85];
    let mut instances = Vec::with_capacity(world.bodies.len() + 1);

    for (i, body) in world.bodies.iter().enumerate() {
        let t = &world.transforms[i];
        let scale = DVec3::new(
            body.half_extents.x * 2.0,
            body.half_extents.y * 2.0,
            body.half_extents.z * 2.0,
        );
        let model_f64 = DAffine3 {
            matrix3: DMat3::from_cols(
                t.matrix3.col(0) * scale.x,
                t.matrix3.col(1) * scale.y,
                t.matrix3.col(2) * scale.z,
            ),
            translation: t.translation,
        };
        let model = Mat4::from(Affine3A::from_cols_array(&{
            let m = model_f64.to_cols_array();
            let mut out = [0.0f32; 12];
            for i in 0..12 {
                out[i] = m[i] as f32;
            }
            out
        }));
        instances.push(InstanceRaw {
            model: model.to_cols_array_2d(),
            color: cream,
            flags: 0,
        });
    }

    // Ground plane
    let ground = Mat4::from_scale(Vec3::new(30.0, 0.05, 30.0));
    instances.push(InstanceRaw {
        model: ground.to_cols_array_2d(),
        color: [0.45, 0.52, 0.56],
        flags: 1,
    });

    instances
}

fn tick(state: &mut AppState, _dt: f64) {
    // 1. Physics step
    if !state.paused {
        if let Some(ref mut creature) = state.creature {
            creature.step(1.0 / 60.0);
        } else {
            match state.scene_id {
                SceneId::SingleBox => {}
                SceneId::HingedPair => scene::hinged_pair_torque(&mut state.world),
                SceneId::Starfish => scene::starfish_torques(&mut state.world),
                SceneId::UniversalJoint => scene::universal_joint_torque(&mut state.world),
                SceneId::SphericalJoint => scene::spherical_joint_torque(&mut state.world),
                SceneId::TripleChain => scene::triple_chain_torque(&mut state.world),
                SceneId::SwimmingStarfish => scene::swimming_starfish_torques(&mut state.world),
                SceneId::RandomCreature => {} // handled above
            }
            state.world.step(1.0 / 60.0);
        }
    }

    // 2. Camera uniform
    let eye = state.camera.eye();
    let view = state.camera.view_matrix();
    let (width, height) = state.renderer.size();
    let aspect = width as f32 / height.max(1) as f32;
    let proj = state.camera.projection_matrix(aspect);
    let view_proj = proj * view;

    let camera_uniform = CameraUniform {
        view_proj: view_proj.to_cols_array_2d(),
        camera_pos: eye.to_array(),
        _pad: 0.0,
    };
    state.renderer.update_camera(&camera_uniform);

    // 3. Build instances from the appropriate world
    let render_world = if let Some(ref creature) = state.creature {
        &creature.world
    } else {
        &state.world
    };
    let instances = build_instances(render_world);

    // 4. Update and render
    state.renderer.update_instances(&instances);
    state.renderer.render_frame();
}

fn setup_mouse_events(canvas: &web_sys::HtmlCanvasElement) {
    let target: &web_sys::EventTarget = canvas.unchecked_ref();

    // mousedown on canvas
    let on_mousedown = Closure::<dyn FnMut(_)>::new(move |e: web_sys::MouseEvent| {
        APP.with(|a| {
            if let Some(ref mut state) = *a.borrow_mut() {
                state.camera.on_mouse_down(e.client_x() as f32, e.client_y() as f32);
            }
        });
    });
    target
        .add_event_listener_with_callback("mousedown", on_mousedown.as_ref().unchecked_ref())
        .unwrap();
    on_mousedown.forget();

    // mouseup on window (catch releases outside canvas)
    let on_mouseup = Closure::<dyn FnMut(_)>::new(move |_e: web_sys::MouseEvent| {
        APP.with(|a| {
            if let Some(ref mut state) = *a.borrow_mut() {
                state.camera.on_mouse_up();
            }
        });
    });
    let window = web_sys::window().unwrap();
    let window_target: &web_sys::EventTarget = window.unchecked_ref();
    window_target
        .add_event_listener_with_callback("mouseup", on_mouseup.as_ref().unchecked_ref())
        .unwrap();
    on_mouseup.forget();

    // mousemove on canvas
    let on_mousemove = Closure::<dyn FnMut(_)>::new(move |e: web_sys::MouseEvent| {
        APP.with(|a| {
            if let Some(ref mut state) = *a.borrow_mut() {
                state.camera.on_mouse_move(e.client_x() as f32, e.client_y() as f32);
            }
        });
    });
    target
        .add_event_listener_with_callback("mousemove", on_mousemove.as_ref().unchecked_ref())
        .unwrap();
    on_mousemove.forget();

    // wheel on canvas with passive: false
    let on_wheel = Closure::<dyn FnMut(_)>::new(move |e: web_sys::WheelEvent| {
        e.prevent_default();
        APP.with(|a| {
            if let Some(ref mut state) = *a.borrow_mut() {
                state.camera.on_wheel(e.delta_y() as f32);
            }
        });
    });
    let opts = web_sys::AddEventListenerOptions::new();
    opts.set_passive(false);
    target
        .add_event_listener_with_callback_and_add_event_listener_options(
            "wheel",
            on_wheel.as_ref().unchecked_ref(),
            &opts,
        )
        .unwrap();
    on_wheel.forget();
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
