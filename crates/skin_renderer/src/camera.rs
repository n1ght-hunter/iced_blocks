use glam::{Mat4, Vec3};

pub struct OrbitCamera {
    pub theta: f32,
    pub phi: f32,
    pub distance: f32,
    pub target: Vec3,
}

impl Default for OrbitCamera {
    fn default() -> Self {
        Self {
            theta: 0.5,
            phi: 1.2,
            distance: 70.0,
            target: Vec3::new(0.0, 12.0, 0.0),
        }
    }
}

impl OrbitCamera {
    pub fn rotate(&mut self, dx: f32, dy: f32) {
        self.theta -= dx * 0.01;
        self.phi = (self.phi - dy * 0.01).clamp(0.1, std::f32::consts::PI - 0.1);
    }

    pub fn zoom(&mut self, delta: f32) {
        self.distance = (self.distance - delta * 2.0).clamp(15.0, 200.0);
    }

    pub fn eye_position(&self) -> Vec3 {
        let x = self.distance * self.phi.sin() * self.theta.sin();
        let y = self.distance * self.phi.cos();
        let z = self.distance * self.phi.sin() * self.theta.cos();
        self.target + Vec3::new(x, y, z)
    }

    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        let eye = self.eye_position();
        let view = Mat4::look_at_rh(eye, self.target, Vec3::Y);
        let proj = Mat4::perspective_rh(45.0_f32.to_radians(), aspect, 0.1, 200.0);
        proj * view
    }
}
