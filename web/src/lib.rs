mod renderer;

use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use renderer::WgpuRenderer;

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

    RENDERER.with(|r| {
        *r.borrow_mut() = Some(renderer);
    });

    start_render_loop();
}

#[wasm_bindgen]
pub fn renderer_resize(width: u32, height: u32) {
    RENDERER.with(|r| {
        if let Some(ref mut renderer) = *r.borrow_mut() {
            renderer.resize(width, height);
        }
    });
}

fn start_render_loop() {
    let f: Rc<RefCell<Option<Closure<dyn FnMut()>>>> = Rc::new(RefCell::new(None));
    let g = f.clone();

    *g.borrow_mut() = Some(Closure::new(move || {
        RENDERER.with(|r| {
            if let Some(ref renderer) = *r.borrow() {
                renderer.render_frame();
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
