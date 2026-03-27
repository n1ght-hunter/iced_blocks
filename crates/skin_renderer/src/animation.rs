use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationMode {
    #[default]
    Idle,
    Walk,
}

impl std::fmt::Display for AnimationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "Idle"),
            Self::Walk => write!(f, "Walk"),
        }
    }
}

pub struct AnimationState {
    time: f32,
    pub mode: AnimationMode,
    last_tick: Instant,
}

impl Default for AnimationState {
    fn default() -> Self {
        Self {
            time: 0.0,
            mode: AnimationMode::Idle,
            last_tick: Instant::now(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct LimbRotations {
    pub head_pitch: f32,
    pub right_arm_pitch: f32,
    pub left_arm_pitch: f32,
    pub right_leg_pitch: f32,
    pub left_leg_pitch: f32,
}

impl AnimationState {
    pub fn tick(&mut self) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f32();
        self.last_tick = now;
        self.time += dt;
    }

    pub fn limb_rotations(&self) -> LimbRotations {
        match self.mode {
            AnimationMode::Idle => {
                let t = self.time * 1.5;
                LimbRotations {
                    head_pitch: t.cos() * 0.02,
                    right_arm_pitch: t.cos() * 0.06 + 0.06,
                    left_arm_pitch: -t.cos() * 0.06 - 0.06,
                    right_leg_pitch: 0.0,
                    left_leg_pitch: 0.0,
                }
            }
            AnimationMode::Walk => {
                let t = self.time * 8.0;
                LimbRotations {
                    head_pitch: 0.0,
                    right_arm_pitch: t.sin() * 0.7,
                    left_arm_pitch: -t.sin() * 0.7,
                    right_leg_pitch: -t.sin() * 0.5,
                    left_leg_pitch: t.sin() * 0.5,
                }
            }
        }
    }
}
