//! Custom iced `Widget` that drives a [`ServoWebViewController`] and draws
//! its latest frame through a [`ServoTexturePrimitive`]. Implemented
//! directly against `iced::advanced::Widget` rather than wrapping
//! `iced::widget::shader::Shader` / `Program` — the extra indirection
//! bought us nothing since we always have exactly one program type.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{self, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::mouse;
use iced::{Element, Event, Length, Rectangle, Size};
use iced_wgpu::primitive;

use crate::controller::ServoWebViewController;
use crate::primitive::ServoTexturePrimitive;

/// Per-widget-instance state tracked by iced between events + draws.
/// Currently just the "logically focused" flag used to gate keyboard
/// forwarding; set on a left-click inside the bounds, cleared on a
/// left-click outside.
#[derive(Default)]
pub struct ServoWidgetState {
    focused: bool,
}

/// Custom iced widget that renders a Servo webview. Clone-cheap
/// (`ServoWebViewController` is `Rc` inside) but not typically cloned —
/// constructed fresh by [`shader`] each `view()` call.
pub struct ServoWidget {
    controller: ServoWebViewController,
    width: Length,
    height: Length,
}

impl ServoWidget {
    pub fn new(controller: ServoWebViewController) -> Self {
        Self {
            controller,
            width: Length::Fill,
            height: Length::Fill,
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
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for ServoWidget
where
    Renderer: primitive::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<ServoWidgetState>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(ServoWidgetState::default())
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
        let state = tree.state.downcast_mut::<ServoWidgetState>();

        // Click-to-focus: a left-click inside grabs keyboard focus; a
        // left-click anywhere else releases it.
        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            state.focused = cursor.is_over(bounds);
        }

        if crate::input::translate_event(event, bounds, cursor, state.focused, &self.controller) {
            shell.request_redraw();
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
        let bounds = layout.bounds();
        if cursor.is_over(bounds) {
            crate::input::cursor_to_interaction(self.controller.current_cursor())
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
            ServoTexturePrimitive {
                frame_slot: self.controller.frame_slot(),
                size_request: self.controller.size_request_slot(),
                logical_bounds: bounds.size(),
            },
        );
    }
}

impl<'a, Message, Theme, Renderer> From<ServoWidget> for Element<'a, Message, Theme, Renderer>
where
    Renderer: primitive::Renderer,
{
    fn from(widget: ServoWidget) -> Self {
        Element::new(widget)
    }
}

/// Build a [`ServoWidget`] bound to the given controller. Defaults to
/// `Length::Fill` on both axes — override via `.width(...)` /
/// `.height(...)` on the returned widget if you need a fixed size.
pub fn shader(controller: &ServoWebViewController) -> ServoWidget {
    ServoWidget::new(controller.clone())
}
