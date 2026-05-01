use iced::{
    Background, Element, Event, Length, Point, Size,
    advanced::{Widget, layout, mouse, renderer::Quad, widget},
    window,
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
    skin::Skin,
    style::{Catalog, Style, StyleFn},
};

/// Builds an [`MCSkinView`] widget rendering the given [`Skin`].
pub fn skin_view<Theme: Catalog>(skin: &Skin) -> MCSkinView<'_, Theme> {
    MCSkinView::new(skin)
}

/// Iced widget that renders an animated 3D Minecraft player skin with an
/// orbit camera controlled by mouse drag and scroll.
pub struct MCSkinView<'a, Theme = iced::Theme>
where
    Theme: Catalog,
{
    skin: &'a Skin,
    width: Length,
    height: Length,
    id: Option<widget::Id>,
    arm_variant: ArmVariant,
    animation_mode: AnimationMode,
    class: <Theme as Catalog>::Class<'a>,
}

impl<'a, Theme> MCSkinView<'a, Theme>
where
    Theme: Catalog,
{
    pub fn new(skin: &'a Skin) -> Self {
        Self {
            skin,
            width: Length::Shrink,
            height: Length::Shrink,
            arm_variant: ArmVariant::default(),
            animation_mode: AnimationMode::default(),
            id: None,
            class: <Theme as Catalog>::default(),
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

    /// Sets the styling function used to resolve the widget's [`Style`].
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        <Theme as Catalog>::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the [`Catalog::Class`] used to resolve the widget's [`Style`].
    pub fn class(mut self, class: impl Into<<Theme as Catalog>::Class<'a>>) -> Self {
        self.class = class.into();
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

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for MCSkinView<'_, Theme>
where
    Theme: Catalog,
    Renderer: iced_wgpu::primitive::Renderer + iced::advanced::Renderer,
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

        let cursor_pos = cursor.position().unwrap_or(Point::ORIGIN);
        let in_bounds = cursor.is_over(bounds);

        match event {
            Event::Window(window::Event::RedrawRequested(now)) => {
                state.animation.tick();
                shell.request_redraw_at(*now + std::time::Duration::from_millis(16));
            }
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
                shell.request_redraw();
                shell.capture_event();
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if in_bounds => {
                let scroll = match *delta {
                    mouse::ScrollDelta::Lines { y, .. } => y,
                    mouse::ScrollDelta::Pixels { y, .. } => y / 50.0,
                };
                state.camera.zoom(scroll);
                shell.request_redraw();
                shell.capture_event();
            }
            _ => {}
        }
    }

    fn draw(
        &self,
        tree: &iced::advanced::widget::Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        _: iced::advanced::mouse::Cursor,
        _: &iced::Rectangle,
    ) {
        let state = tree.state.downcast_ref::<SkinState>();
        let bounds = layout.bounds();

        let style = <Theme as Catalog>::style(theme, &self.class);
        if !matches!(style.background, Background::Color(c) if c.a == 0.0) {
            renderer.fill_quad(
                Quad {
                    bounds,
                    ..Default::default()
                },
                style.background,
            );
        }

        let aspect = bounds.width / bounds.height.max(1.0);
        let view_proj = state.camera.view_projection(aspect);
        let rotations = state.animation.limb_rotations();
        let vertices = animated_vertices(&state.parts, &rotations);

        renderer.draw_primitive(
            bounds,
            SkinPrimitive::new(vertices, view_proj, self.skin.clone()),
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

impl<'a, Message, Theme, Renderer> From<MCSkinView<'a, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Theme: Catalog + 'a,
    Renderer: iced_wgpu::primitive::Renderer + iced::advanced::Renderer,
{
    fn from(view: MCSkinView<'a, Theme>) -> Self {
        Element::new(view)
    }
}
