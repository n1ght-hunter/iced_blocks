//! Generic iced [`Widget`] that renders a [`FrameSource`] through a
//! persistent wgpu texture.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{self, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::mouse;
use iced::{Element, Event, Length, Rectangle, Size};
use iced_wgpu::primitive;

use crate::primitive::FramePrimitive;
use crate::{Alignment, ContentFit, FilterMode, FrameSource};

#[derive(Default)]
struct FrameWidgetState {
    focused: bool,
}

/// Generic iced widget that renders any [`FrameSource`] through a wgpu
/// textured quad.
pub struct FrameWidget<S> {
    source: S,
    width: Length,
    height: Length,
    content_fit: ContentFit,
    alignment: Alignment,
    filter: FilterMode,
}

impl<S: FrameSource> FrameWidget<S> {
    pub fn new(source: S) -> Self {
        Self {
            source,
            width: Length::Fill,
            height: Length::Fill,
            content_fit: ContentFit::default(),
            alignment: Alignment::default(),
            filter: FilterMode::default(),
        }
    }

    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    pub fn content_fit(mut self, fit: ContentFit) -> Self {
        self.content_fit = fit;
        self
    }

    pub fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn filter(mut self, filter: FilterMode) -> Self {
        self.filter = filter;
        self
    }
}

impl<Message, Theme, Renderer, S> Widget<Message, Theme, Renderer> for FrameWidget<S>
where
    Renderer: primitive::Renderer,
    S: FrameSource,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<FrameWidgetState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(FrameWidgetState::default())
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        layout::atomic(limits, self.width, self.height)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        let state = tree.state.downcast_mut::<FrameWidgetState>();

        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            state.focused = cursor.is_over(bounds);
        }

        let consumed = self
            .source
            .handle_event(event, bounds, cursor, state.focused);

        if consumed {
            shell.request_redraw();
            // Capture keyboard and IME events when focused so other
            // widgets (e.g. a text input in the URL bar) don't also
            // process them.
            if matches!(event, Event::Keyboard(_) | Event::InputMethod(_)) && state.focused {
                shell.capture_event();
            }
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        if cursor.is_over(layout.bounds()) {
            self.source.cursor()
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        _tree: &widget::Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        renderer.draw_primitive(
            bounds,
            FramePrimitive {
                frame_slot: self.source.frame_slot(),
                size_request: self.source.size_request_slot(),
                logical_bounds: bounds.size(),
                content_fit: self.content_fit,
                alignment: self.alignment,
                filter: self.filter,
            },
        );
    }
}

impl<'a, Message, Theme, Renderer, S> From<FrameWidget<S>> for Element<'a, Message, Theme, Renderer>
where
    Renderer: primitive::Renderer,
    S: FrameSource,
{
    fn from(widget: FrameWidget<S>) -> Self {
        Element::new(widget)
    }
}

/// Build a [`FrameWidget`] with default settings (`Fill`, `Center`,
/// `Linear`).
pub fn frame<S: FrameSource>(source: &S) -> FrameWidget<S> {
    FrameWidget::new(source.clone())
}
