//! Generic placeholder widget for embedding a native OS child surface inside
//! an Iced layout.
//!
//! The widget reserves layout space, reports its current bounds to a
//! [`BoundsSink`] whenever they change, and asks the sink to refocus the
//! parent window when the user clicks outside the reserved area. Backends
//! (webview engines, video players, native map views, GL canvases, …) supply
//! their own [`BoundsSink`] implementation to reposition their underlying
//! native surface.

use std::{cell::Cell, rc::Rc};

use iced::{
    Element, Event, Length, Rectangle, Size,
    advanced::{
        Clipboard, Layout, Shell, Widget,
        layout::{self, Node},
        renderer,
        widget::Tree,
    },
    mouse,
};

/// Receives layout updates from a [`NativeSurfacePlaceholder`].
///
/// Implementations are expected to reposition their underlying native child
/// surface to the reported bounds and to return keyboard focus to the parent
/// window when [`refocus_parent`](Self::refocus_parent) is called.
pub trait BoundsSink: 'static {
    /// Called whenever the placeholder's layout bounds change.
    fn apply(&self, bounds: Rectangle);

    /// Called when the user clicks outside the placeholder; the sink should
    /// transfer keyboard focus back to the parent window.
    fn refocus_parent(&self);
}

#[derive(Default)]
struct State {
    // `Cell` so `draw` (which takes `&Tree`) can update this without an event.
    last_bounds: Cell<Option<Rectangle>>,
}

/// Placeholder widget that reserves layout space for a native child surface.
pub struct NativeSurfacePlaceholder<Message> {
    width: Length,
    height: Length,
    sink: Option<Rc<dyn BoundsSink>>,
    _message: std::marker::PhantomData<Message>,
}

impl<Message> NativeSurfacePlaceholder<Message> {
    /// Create a placeholder with no bounds sink attached. Without a sink the
    /// widget still reserves layout space but does not report updates.
    pub fn new() -> Self {
        Self {
            width: Length::Fill,
            height: Length::Fill,
            sink: None,
            _message: std::marker::PhantomData,
        }
    }

    /// Set the placeholder width.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Set the placeholder height.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Attach a [`BoundsSink`] that will receive layout and focus updates.
    pub fn bounds_sink(mut self, sink: Rc<dyn BoundsSink>) -> Self {
        self.sink = Some(sink);
        self
    }
}

impl<Message> Default for NativeSurfacePlaceholder<Message> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for NativeSurfacePlaceholder<Message>
where
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        iced::advanced::widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        iced::advanced::widget::tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(self.width, self.height)
    }

    fn layout(&mut self, _tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) -> Node {
        Node::new(limits.resolve(self.width, self.height, Size::ZERO))
    }

    fn draw(
        &self,
        tree: &Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        // `draw` runs on every render frame, including the very first one
        // (before any user input event has had a chance to trigger `update`).
        // Push bounds from here so the attached sink sees the real layout
        // without waiting for a mouse move.
        let Some(sink) = &self.sink else {
            return;
        };
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        if state.last_bounds.get() != Some(bounds) {
            state.last_bounds.set(Some(bounds));
            sink.apply(bounds);
        }
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        _shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let Some(sink) = &self.sink else {
            return;
        };

        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();

        if state.last_bounds.get() != Some(bounds) {
            state.last_bounds.set(Some(bounds));
            sink.apply(bounds);
        }

        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event
            && !cursor.is_over(bounds)
        {
            sink.refocus_parent();
        }
    }
}

impl<'a, Message, Theme, Renderer> From<NativeSurfacePlaceholder<Message>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(placeholder: NativeSurfacePlaceholder<Message>) -> Self {
        Self::new(placeholder)
    }
}
