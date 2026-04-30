use iced::{
    Element, Event, Length, Point, Size,
    advanced::{Widget, layout, mouse, widget},
};

use crate::{
    AnimationMode, ArmVariant,
    animation::AnimationState,
    camera::OrbitCamera,
    model::{
        BodyPartMesh,
        geometry::{animated_vertices, player_parts},
    },
    primitive::SkinPrimitive,
    source::Source,
};

pub fn skin_view(source: &Source) -> MCSkinView<'_> {
    MCSkinView::new(source)
}

pub struct MCSkinView<'a> {
    source: &'a Source,
    width: Length,
    height: Length,
    id: Option<widget::Id>,
    arm_variant: ArmVariant,
    animation_mode: AnimationMode,
}

impl<'a> MCSkinView<'a> {
    pub fn new(source: &'a Source) -> Self {
        Self {
            source,
            width: Length::Shrink,
            height: Length::Shrink,
            arm_variant: ArmVariant::default(),
            animation_mode: AnimationMode::default(),
            id: None,
        }
    }

    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    pub fn arm_variant(mut self, variant: ArmVariant) -> Self {
        self.arm_variant = variant;
        self
    }

    pub fn animation_mode(mut self, mode: AnimationMode) -> Self {
        self.animation_mode = mode;
        self
    }
}

#[derive(Default)]
pub struct SkinState {
    camera: OrbitCamera,
    animation: AnimationState,
    is_dragging: bool,
    last_cursor: Option<Point>,
    arm_variant: ArmVariant,
    parts: Vec<BodyPartMesh>,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for MCSkinView<'_>
where
    Renderer: iced_wgpu::primitive::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(
        &mut self,
        _: &mut iced::advanced::widget::Tree,
        _: &Renderer,
        limits: &iced::advanced::layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, self.width, self.height)
    }

    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<SkinState>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(SkinState {
            arm_variant: self.arm_variant,
            parts: player_parts(self.arm_variant),
            ..Default::default()
        })
    }

    fn mouse_interaction(
        &self,
        tree: &widget::Tree,
        layout: layout::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        _viewport: &iced::Rectangle,
        _renderer: &Renderer,
    ) -> iced::advanced::mouse::Interaction {
        let state = tree.state.downcast_ref::<SkinState>();
        let bounds = layout.bounds();

        if state.is_dragging {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(bounds) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }

    fn update(
        &mut self,
        tree: &mut widget::Tree,
        event: &iced::Event,
        layout: layout::Layout<'_>,
        cursor: iced::advanced::mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn iced::advanced::Clipboard,
        shell: &mut iced::advanced::Shell<'_, Message>,
        _viewport: &iced::Rectangle,
    ) {
        let state = tree.state.downcast_mut::<SkinState>();
        let bounds = layout.bounds();

        // perhaps should be somewhere else
        if state.arm_variant != self.arm_variant {
            state.arm_variant = self.arm_variant;
            state.parts = player_parts(self.arm_variant);
        }

        state.animation.mode = self.animation_mode;
        state.animation.tick();

        let cursor_pos = cursor.position().unwrap_or(Point::ORIGIN);
        let in_bounds = cursor.is_over(bounds);

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if in_bounds => {
                state.is_dragging = true;
                state.last_cursor = Some(cursor_pos);
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                if state.is_dragging =>
            {
                state.is_dragging = false;
                state.last_cursor = None;
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if state.is_dragging => {
                if let Some(last) = state.last_cursor {
                    let dx = cursor_pos.x - last.x;
                    let dy = cursor_pos.y - last.y;
                    state.camera.rotate(dx, dy);
                }
                state.last_cursor = Some(cursor_pos);
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if in_bounds => {
                let scroll = match *delta {
                    mouse::ScrollDelta::Lines { y, .. } => y,
                    mouse::ScrollDelta::Pixels { y, .. } => y / 50.0,
                };
                state.camera.zoom(scroll);
                shell.capture_event();
            }
            _ => {}
        }
        shell.request_redraw_at(iced::time::Instant::now() + std::time::Duration::from_millis(16));
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut Renderer,
        _: &Theme,
        _: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        _: iced::advanced::mouse::Cursor,
        _: &iced::Rectangle,
    ) {
        let state = tree.state.downcast_ref::<SkinState>();
        let bounds = layout.bounds();

        let aspect = bounds.width / bounds.height.max(1.0);
        let view_proj = state.camera.view_projection(aspect);
        let rotations = state.animation.limb_rotations();
        let vertices = animated_vertices(&state.parts, &rotations);

        renderer.draw_primitive(
            bounds,
            SkinPrimitive::new(vertices, view_proj, self.source.clone()),
        );
    }

    fn operate(
        &mut self,
        _tree: &mut widget::Tree,
        _layout: layout::Layout<'_>,
        _renderer: &Renderer,
        _operation: &mut dyn widget::Operation,
    ) {
    }
}

impl<'a, Message, Theme, Renderer> From<MCSkinView<'a>> for Element<'a, Message, Theme, Renderer>
where
    Renderer: iced_wgpu::primitive::Renderer,
{
    fn from(view: MCSkinView<'a>) -> Self {
        Element::new(view)
    }
}
