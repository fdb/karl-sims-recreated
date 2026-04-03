use glam::{Mat4, Vec3};

pub struct OrbitCamera {
    pub azimuth: f32,
    pub elevation: f32,
    pub distance: f32,
    pub target: Vec3,
    dragging: bool,
    last_mouse: (f32, f32),
}

impl OrbitCamera {
    pub fn new() -> Self {
        Self {
            azimuth: 0.3,
            elevation: 0.4,
            distance: 14.0,
            target: Vec3::new(0.0, 0.5, 0.0),
            dragging: false,
            last_mouse: (0.0, 0.0),
        }
    }

    pub fn eye(&self) -> Vec3 {
        let x = self.distance * self.elevation.cos() * self.azimuth.sin();
        let y = self.distance * self.elevation.sin();
        let z = self.distance * self.elevation.cos() * self.azimuth.cos();
        self.target + Vec3::new(x, y, z)
    }

    pub fn view_matrix(&self) -> Mat4 {
        Mat4::look_at_rh(self.eye(), self.target, Vec3::Y)
    }

    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, aspect, 0.1, 100.0)
    }

    pub fn on_mouse_down(&mut self, x: f32, y: f32) {
        self.dragging = true;
        self.last_mouse = (x, y);
    }

    pub fn on_mouse_up(&mut self) {
        self.dragging = false;
    }

    pub fn on_mouse_move(&mut self, x: f32, y: f32) {
        if self.dragging {
            let dx = x - self.last_mouse.0;
            let dy = y - self.last_mouse.1;
            self.azimuth -= dx * 0.005;
            self.elevation += dy * 0.005;
            self.elevation = self.elevation.clamp(0.05, 1.5);
            self.last_mouse = (x, y);
        }
    }

    pub fn on_wheel(&mut self, delta: f32) {
        self.distance += delta * 0.01;
        self.distance = self.distance.clamp(3.0, 50.0);
    }
}
