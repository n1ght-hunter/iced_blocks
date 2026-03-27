//! Iced shader program for animated 3D player skin rendering.

use iced::{
    Event, Point, Rectangle, mouse,
    widget::shader::{self, Action, Shader},
};

use crate::{
    animation::{AnimationMode, AnimationState},
    camera::OrbitCamera,
    model::{
        ArmVariant, BodyPartMesh,
        geometry::{animated_vertices, player_parts},
    },
    primitive::SkinPrimitive,
};

pub struct SkinProgram {
    skin_rgba: Option<Vec<u8>>,
    skin_generation: u64,
    pub arm_variant: ArmVariant,
    pub animation_mode: AnimationMode,
    parts: Vec<BodyPartMesh>,
}

#[derive(Default)]
pub struct SkinState {
    camera: OrbitCamera,
    animation: AnimationState,
    is_dragging: bool,
    last_cursor: Option<Point>,
    last_skin_generation: u64,
}

impl Default for SkinProgram {
    fn default() -> Self {
        Self::new(ArmVariant::default(), AnimationMode::default())
    }
}

impl SkinProgram {
    pub fn new(arm_variant: ArmVariant, animation_mode: AnimationMode) -> Self {
        Self {
            skin_rgba: None,
            skin_generation: 0,
            arm_variant,
            animation_mode,
            parts: player_parts(arm_variant),
        }
    }

    pub fn set_arm_variant(&mut self, variant: ArmVariant) {
        if self.arm_variant != variant {
            self.arm_variant = variant;
            self.parts = player_parts(variant);
        }
    }

    pub fn set_skin(&mut self, rgba: impl Into<Vec<u8>>) {
        self.skin_generation += 1;
        self.skin_rgba = Some(rgba.into());
    }

    pub fn view<Message>(&self) -> Shader<Message, &Self> {
        Shader::new(self)
    }
}

impl<Message> shader::Program<Message> for SkinProgram {
    type State = SkinState;
    type Primitive = SkinPrimitive;

    fn update(
        &self,
        state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        state.animation.mode = self.animation_mode;
        state.animation.tick();

        let cursor_pos = cursor.position().unwrap_or(Point::ORIGIN);
        let in_bounds = cursor.is_over(bounds);

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if in_bounds => {
                state.is_dragging = true;
                state.last_cursor = Some(cursor_pos);
                return Some(Action::capture());
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if state.is_dragging =>
            {
                state.is_dragging = false;
                state.last_cursor = None;
                return Some(Action::capture());
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.is_dragging => {
                if let Some(last) = state.last_cursor {
                    let dx = cursor_pos.x - last.x;
                    let dy = cursor_pos.y - last.y;
                    state.camera.rotate(dx, dy);
                }
                state.last_cursor = Some(cursor_pos);
                return Some(Action::capture());
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if in_bounds => {
                let scroll = match *delta {
                    mouse::ScrollDelta::Lines { y, .. } => y,
                    mouse::ScrollDelta::Pixels { y, .. } => y / 50.0,
                };
                state.camera.zoom(scroll);
                return Some(Action::capture());
            }
            _ => {}
        }

        Some(Action::request_redraw())
    }

    fn draw(
        &self,
        state: &Self::State,
        _cursor: mouse::Cursor,
        bounds: Rectangle,
    ) -> Self::Primitive {
        let aspect = bounds.width / bounds.height.max(1.0);
        let view_proj = state.camera.view_projection(aspect);
        let rotations = state.animation.limb_rotations();
        let vertices = animated_vertices(&self.parts, &rotations);

        // Only pass skin data when it actually changed
        let skin_changed = state.last_skin_generation != self.skin_generation;
        let skin_data = if skin_changed {
            self.skin_rgba.clone()
        } else {
            None
        };

        SkinPrimitive::new(vertices, view_proj, skin_data, self.skin_generation)
    }

    fn mouse_interaction(
        &self,
        state: &Self::State,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if state.is_dragging {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
}
