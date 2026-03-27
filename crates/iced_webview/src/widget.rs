//! Placeholder widget that reserves layout space for the webview and
//! drives bounds updates and focus management through shared state.

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

use crate::controller::BoundsSender;

#[derive(Default)]
struct State {
    last_bounds: Option<Rectangle>,
}

pub struct WebViewPlaceholder<Message> {
    width: Length,
    height: Length,
    bounds_tx: Option<BoundsSender>,
    _message: std::marker::PhantomData<Message>,
}

impl<Message> WebViewPlaceholder<Message> {
    pub fn new() -> Self {
        Self {
            width: Length::Fill,
            height: Length::Fill,
            bounds_tx: None,
            _message: std::marker::PhantomData,
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

    pub(crate) fn bounds_sender(mut self, sender: BoundsSender) -> Self {
        self.bounds_tx = Some(sender);
        self
    }
}

impl<Message> Default for WebViewPlaceholder<Message> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for WebViewPlaceholder<Message>
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
        _tree: &Tree,
        _renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
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
        let Some(tx) = &self.bounds_tx else {
            return;
        };

        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        if state.last_bounds.as_ref() != Some(&bounds) {
            state.last_bounds = Some(bounds);
            tx.apply(bounds);
        }

        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event
            && !cursor.is_over(bounds)
        {
            tx.refocus_parent();
        }
    }
}

impl<'a, Message, Theme, Renderer> From<WebViewPlaceholder<Message>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    fn from(placeholder: WebViewPlaceholder<Message>) -> Self {
        Self::new(placeholder)
    }
}
