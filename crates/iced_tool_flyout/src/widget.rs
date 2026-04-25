//! The [`ToolFlyout`] widget: a Photoshop-style tool-group button.

use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::text;
use iced::advanced::widget::{self, Operation, Tree, tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::time::{Duration, Instant};
use iced::{
    Background, Border, Color, Element, Event, Length, Padding, Pixels, Rectangle, Size, Vector,
    mouse, window,
};

use std::any::Any;

use crate::ToolItem;
use crate::overlay::FlyoutOverlay;
use crate::style::{Catalog, Status, Style, StyleFn};

/// The identifier of a [`ToolFlyout`].
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Id(widget::Id);

impl Id {
    /// Creates a custom [`Id`].
    pub fn new(id: &'static str) -> Self {
        Self(widget::Id::new(id))
    }

    /// Creates a unique [`Id`].
    pub fn unique() -> Self {
        Self(widget::Id::unique())
    }
}

impl From<Id> for widget::Id {
    fn from(id: Id) -> Self {
        id.0
    }
}

const DEFAULT_LONG_PRESS_MS: u64 = 400;
const DEFAULT_MIN_SIZE: f32 = 28.0;
const INDICATOR_SIZE: f32 = 5.0;
const INDICATOR_INSET: f32 = 2.0;

/// A Photoshop-style tool-group button.
///
/// Displays the icon of the currently-selected tool variant. A left-click
/// activates that variant; a **right-click** or a **long-press** opens a
/// popup listing every variant, from which the user can pick a different
/// active tool. Build one with [`tool_flyout`].
pub struct ToolFlyout<'a, T, Message, Theme = iced::Theme, Renderer = iced::Renderer>
where
    T: Clone + PartialEq,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    id: Option<Id>,
    pub(crate) items: Vec<ToolItem<'a, T, Message, Theme, Renderer>>,
    on_activate: Box<dyn Fn(T) -> Message + 'a>,
    pub(crate) on_select: Option<Box<dyn Fn(T) -> Message + 'a>>,
    default_value: Option<T>,
    long_press: Duration,
    width: Length,
    height: Length,
    padding: Padding,
    pub(crate) text_size: Option<Pixels>,
    pub(crate) class: <Theme as Catalog>::Class<'a>,
}

/// Widget state persisted in the state tree across renders.
#[derive(Default)]
pub(crate) struct State {
    pub selected: usize,
    pub is_open: bool,
    pub pressed_at: Option<Instant>,
    pub hovered_item: Option<usize>,
    pub last_status: Option<Status>,
    pub initialized: bool,
    pub pending_select: Option<Box<dyn Any + Send>>,
}

/// Builds a [`ToolFlyout`] from a list of variants and an activation callback.
pub fn tool_flyout<'a, T, Message, Theme, Renderer>(
    items: Vec<ToolItem<'a, T, Message, Theme, Renderer>>,
    on_activate: impl Fn(T) -> Message + 'a,
) -> ToolFlyout<'a, T, Message, Theme, Renderer>
where
    T: Clone + PartialEq + 'a,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    ToolFlyout::new(items, on_activate)
}

impl<'a, T, Message, Theme, Renderer> ToolFlyout<'a, T, Message, Theme, Renderer>
where
    T: Clone + PartialEq + 'a,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    /// Creates a new [`ToolFlyout`].
    pub fn new(
        items: Vec<ToolItem<'a, T, Message, Theme, Renderer>>,
        on_activate: impl Fn(T) -> Message + 'a,
    ) -> Self {
        Self {
            id: None,
            items,
            on_activate: Box::new(on_activate),
            on_select: None,
            default_value: None,
            long_press: Duration::from_millis(DEFAULT_LONG_PRESS_MS),
            width: Length::Shrink,
            height: Length::Shrink,
            padding: Padding::new(6.0),
            text_size: None,
            class: <Theme as Catalog>::default(),
        }
    }

    /// Sets the [`Id`] of the widget, required for programmatic [`select`](crate::select).
    pub fn id(mut self, id: impl Into<Id>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the initial variant that will be selected when the widget is
    /// first rendered. If omitted, the first item is used.
    pub fn default_selected(mut self, value: T) -> Self {
        self.default_value = Some(value);
        self
    }

    /// Sets how long the user must hold the button before the flyout opens.
    pub fn long_press(mut self, duration: Duration) -> Self {
        self.long_press = duration;
        self
    }

    /// Sets the width of the button.
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the button.
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the padding around the selected icon inside the button.
    pub fn padding(mut self, padding: impl Into<Padding>) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the text size used to render labels in the flyout rows.
    pub fn text_size(mut self, size: impl Into<Pixels>) -> Self {
        self.text_size = Some(size.into());
        self
    }

    /// Sets the callback fired when the user picks a new variant from the
    /// flyout. The variant swap itself is handled internally — this is only
    /// to mirror the selection into application state.
    pub fn on_select(mut self, f: impl Fn(T) -> Message + 'a) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }

    /// Sets the style of the widget.
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        <Theme as Catalog>::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class directly.
    pub fn class(mut self, class: impl Into<<Theme as Catalog>::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    fn default_index(&self) -> usize {
        self.default_value
            .as_ref()
            .and_then(|v| self.items.iter().position(|it| &it.value == v))
            .unwrap_or(0)
    }
}

impl<'a, T, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for ToolFlyout<'a, T, Message, Theme, Renderer>
where
    T: Clone + PartialEq + 'static,
    Message: 'a,
    Theme: Catalog + 'a,
    Renderer: text::Renderer + 'a,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn children(&self) -> Vec<Tree> {
        self.items.iter().map(|it| Tree::new(&it.icon)).collect()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children_custom(
            &self.items,
            |child_tree, item| child_tree.diff(item.icon.as_widget()),
            |item| Tree::new(&item.icon),
        );
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let selected = {
            let state = tree.state.downcast_mut::<State>();
            if !state.initialized {
                state.selected = self.default_index().min(self.items.len().saturating_sub(1));
                state.initialized = true;
            }
            if let Some(pending) = state.pending_select.take()
                && let Some(value) = pending.downcast_ref::<T>()
                && let Some(idx) = self.items.iter().position(|it| &it.value == value)
            {
                state.selected = idx;
            }
            state.selected.min(self.items.len().saturating_sub(1))
        };

        let width = self.width;
        let height = self.height;
        let padding = self.padding;

        // Reserve at least DEFAULT_MIN_SIZE × DEFAULT_MIN_SIZE when the caller
        // leaves width/height as Shrink and the icon itself is small.
        layout::padded(limits, width, height, padding, |sub_limits| {
            let node = self.items[selected].icon.as_widget_mut().layout(
                &mut tree.children[selected],
                renderer,
                sub_limits,
            );
            let icon_size = node.size();
            let min_w = (DEFAULT_MIN_SIZE - padding.x()).max(0.0);
            let min_h = (DEFAULT_MIN_SIZE - padding.y()).max(0.0);
            let pad_w = (min_w - icon_size.width).max(0.0);
            let pad_h = (min_h - icon_size.height).max(0.0);
            if pad_w > 0.0 || pad_h > 0.0 {
                let moved = node.move_to((pad_w * 0.5, pad_h * 0.5));
                layout::Node::with_children(
                    Size::new(icon_size.width + pad_w, icon_size.height + pad_h),
                    vec![moved],
                )
            } else {
                node
            }
        })
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
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();
        let over = cursor.is_over(bounds);

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if state.is_open {
                    // Event bubbled past the overlay — the click was outside
                    // the flyout. Close without selecting.
                    state.is_open = false;
                    state.pressed_at = None;
                    shell.request_redraw();
                    shell.capture_event();
                } else if over {
                    let now = Instant::now();
                    state.pressed_at = Some(now);
                    shell.request_redraw_at(now + self.long_press);
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                if let Some(pressed_at) = state.pressed_at {
                    state.pressed_at = None;
                    let elapsed = Instant::now().saturating_duration_since(pressed_at);
                    if elapsed < self.long_press && over && !state.is_open {
                        let idx = state.selected.min(self.items.len().saturating_sub(1));
                        if let Some(item) = self.items.get(idx) {
                            shell.publish((self.on_activate)(item.value.clone()));
                        }
                    }
                    shell.request_redraw();
                    shell.capture_event();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right)) => {
                if state.is_open {
                    state.is_open = false;
                    shell.request_redraw();
                    shell.capture_event();
                } else if over && !self.items.is_empty() {
                    state.pressed_at = None;
                    state.is_open = true;
                    state.hovered_item =
                        Some(state.selected.min(self.items.len().saturating_sub(1)));
                    shell.request_redraw();
                    shell.capture_event();
                }
            }
            Event::Window(window::Event::RedrawRequested(now)) => {
                // Trip the flyout once the long-press threshold is crossed.
                if let Some(pressed_at) = state.pressed_at {
                    if now.saturating_duration_since(pressed_at) >= self.long_press {
                        state.pressed_at = None;
                        state.is_open = true;
                        state.hovered_item =
                            Some(state.selected.min(self.items.len().saturating_sub(1)));
                        shell.request_redraw();
                    } else {
                        shell.request_redraw_at(pressed_at + self.long_press);
                    }
                }
                state.last_status = Some(current_status(state, over));
            }
            _ => {}
        }

        let cur = current_status(state, over);
        if state.last_status.is_some_and(|last| last != cur) {
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
        if cursor.is_over(layout.bounds()) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _defaults: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let over = cursor.is_over(bounds);
        let status = current_status(state, over);
        let style = <Theme as Catalog>::style(theme, &self.class, status);

        if style.background.is_some() || style.border.width > 0.0 {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: style.border,
                    ..renderer::Quad::default()
                },
                style
                    .background
                    .unwrap_or(Background::Color(Color::TRANSPARENT)),
            );
        }

        let selected = state.selected.min(self.items.len().saturating_sub(1));
        if let Some(icon_layout) = layout.children().next()
            && let Some(item) = self.items.get(selected)
        {
            item.icon.as_widget().draw(
                &tree.children[selected],
                renderer,
                theme,
                &renderer::Style {
                    text_color: style.text_color,
                },
                icon_layout,
                cursor,
                viewport,
            );
        }

        let tri = INDICATOR_SIZE;
        let ix = bounds.x + bounds.width - tri - INDICATOR_INSET;
        let iy = bounds.y + bounds.height - tri - INDICATOR_INSET;
        renderer.fill_quad(
            renderer::Quad {
                bounds: Rectangle {
                    x: ix,
                    y: iy,
                    width: tri,
                    height: tri,
                },
                border: Border {
                    radius: 1.0.into(),
                    ..Border::default()
                },
                ..renderer::Quad::default()
            },
            Background::Color(style.indicator_color),
        );
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<State>();
        operation.custom(self.id.as_ref().map(|id| &id.0), layout.bounds(), state);

        let selected = state.selected.min(self.items.len().saturating_sub(1));
        if let Some(icon_layout) = layout.children().next()
            && let Some(item) = self.items.get_mut(selected)
        {
            item.icon.as_widget_mut().operate(
                &mut tree.children[selected],
                icon_layout,
                renderer,
                operation,
            );
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'_>,
        _renderer: &Renderer,
        _viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let Tree {
            state: tree_state,
            children: tree_children,
            ..
        } = tree;
        let state = tree_state.downcast_mut::<State>();
        if !state.is_open || self.items.is_empty() {
            return None;
        }
        let anchor = Rectangle {
            x: layout.bounds().x + translation.x,
            y: layout.bounds().y + translation.y,
            width: layout.bounds().width,
            height: layout.bounds().height,
        };
        Some(overlay::Element::new(Box::new(FlyoutOverlay {
            state,
            trees: tree_children.as_mut_slice(),
            items: self.items.as_mut_slice(),
            on_select: self.on_select.as_deref(),
            anchor,
            class: &self.class,
            text_size: self.text_size,
        })))
    }
}

fn current_status(state: &State, hovered: bool) -> Status {
    if state.is_open {
        Status::Open
    } else if state.pressed_at.is_some() {
        Status::Pressed
    } else if hovered {
        Status::Hovered
    } else {
        Status::Idle
    }
}

pub(crate) fn select_operation<T: PartialEq + Send + 'static>(
    target: Id,
    value: T,
) -> impl Operation {
    struct SelectOp<T> {
        target: widget::Id,
        value: Option<T>,
    }

    impl<T: PartialEq + Send + 'static> Operation for SelectOp<T> {
        fn custom(&mut self, id: Option<&widget::Id>, _bounds: Rectangle, state: &mut dyn Any) {
            if id == Some(&self.target)
                && let Some(state) = state.downcast_mut::<State>()
                && let Some(value) = self.value.take()
            {
                state.pending_select = Some(Box::new(value));
            }
        }

        fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation)) {
            operate(self);
        }
    }

    SelectOp {
        target: target.into(),
        value: Some(value),
    }
}

impl<'a, T, Message, Theme, Renderer> From<ToolFlyout<'a, T, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    T: Clone + PartialEq + 'static,
    Message: 'a,
    Theme: Catalog + 'a,
    Renderer: text::Renderer + 'a,
{
    fn from(widget: ToolFlyout<'a, T, Message, Theme, Renderer>) -> Self {
        Self::new(widget)
    }
}
