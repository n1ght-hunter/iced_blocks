//! The floating popup listing every tool variant.

use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay::Overlay;
use iced::advanced::renderer;
use iced::advanced::text::{self, Text};
use iced::advanced::widget::Tree;
use iced::advanced::{Clipboard, Shell};
use iced::alignment;
use iced::{Border, Event, Pixels, Point, Rectangle, Size, keyboard, mouse};

use crate::ToolItem;
use crate::style::{Catalog, Status};
use crate::widget::State;

pub(crate) const ICON_BOX: f32 = 24.0;
pub(crate) const ROW_HEIGHT: f32 = 30.0;
pub(crate) const H_PAD: f32 = 8.0;
pub(crate) const GAP: f32 = 10.0;
pub(crate) const LABEL_W: f32 = 140.0;
pub(crate) const SHORTCUT_W: f32 = 28.0;

pub(crate) fn row_width() -> f32 {
    H_PAD + ICON_BOX + GAP + LABEL_W + GAP + SHORTCUT_W + H_PAD
}

pub(crate) struct FlyoutOverlay<'a, 'b, T, Message, Theme, Renderer>
where
    T: Clone + PartialEq,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    pub state: &'a mut State,
    pub trees: &'a mut [Tree],
    pub items: &'a mut [ToolItem<'b, T, Message, Theme, Renderer>],
    pub on_select: Option<&'a dyn Fn(T) -> Message>,
    pub anchor: Rectangle,
    pub class: &'a <Theme as Catalog>::Class<'b>,
    pub text_size: Option<Pixels>,
}

impl<'a, 'b, T, Message, Theme, Renderer> Overlay<Message, Theme, Renderer>
    for FlyoutOverlay<'a, 'b, T, Message, Theme, Renderer>
where
    T: Clone + PartialEq,
    Theme: Catalog,
    Renderer: text::Renderer,
{
    fn layout(&mut self, renderer: &Renderer, bounds: Size) -> layout::Node {
        let overlay_w = row_width();
        let icon_limits = layout::Limits::new(Size::ZERO, Size::new(ICON_BOX, ICON_BOX));

        let mut row_nodes = Vec::with_capacity(self.items.len());
        for (i, (item, tree)) in self.items.iter_mut().zip(self.trees.iter_mut()).enumerate() {
            let icon_node = item
                .icon
                .as_widget_mut()
                .layout(tree, renderer, &icon_limits);
            let icon_size = icon_node.size();
            let icon_x = H_PAD + (ICON_BOX - icon_size.width).max(0.0) * 0.5;
            let icon_y = (ROW_HEIGHT - icon_size.height).max(0.0) * 0.5;
            let icon_node = icon_node.move_to((icon_x, icon_y));

            let row_node =
                layout::Node::with_children(Size::new(overlay_w, ROW_HEIGHT), vec![icon_node])
                    .move_to((0.0, i as f32 * ROW_HEIGHT));
            row_nodes.push(row_node);
        }

        let total_h = self.items.len() as f32 * ROW_HEIGHT;

        let mut x = self.anchor.x;
        let mut y = self.anchor.y + self.anchor.height;
        if y + total_h > bounds.height {
            y = (self.anchor.y - total_h).max(0.0);
        }
        if x + overlay_w > bounds.width {
            x = (bounds.width - overlay_w).max(0.0);
        }

        layout::Node::with_children(Size::new(overlay_w, total_h), row_nodes).move_to((x, y))
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        let bounds = layout.bounds();
        let style = <Theme as Catalog>::style(theme, self.class, Status::Open);

        renderer.fill_quad(
            renderer::Quad {
                bounds,
                border: style.flyout_border,
                ..renderer::Quad::default()
            },
            style.flyout_background,
        );

        let font = renderer.default_font();
        let text_size = self.text_size.unwrap_or_else(|| renderer.default_size());

        for (i, row_layout) in layout.children().enumerate() {
            let row_bounds = row_layout.bounds();
            let hovered = self.state.hovered_item == Some(i);

            let (label_color, shortcut_color, icon_text_color) = if hovered {
                renderer.fill_quad(
                    renderer::Quad {
                        bounds: row_bounds,
                        border: Border::default(),
                        ..renderer::Quad::default()
                    },
                    style.flyout_highlight,
                );
                (
                    style.flyout_highlight_text,
                    style.flyout_highlight_text,
                    style.flyout_highlight_text,
                )
            } else {
                (
                    style.flyout_text_color,
                    style.flyout_shortcut_color,
                    style.flyout_text_color,
                )
            };

            if let Some(icon_layout) = row_layout.children().next()
                && let Some(item) = self.items.get(i)
            {
                item.icon.as_widget().draw(
                    &self.trees[i],
                    renderer,
                    theme,
                    &renderer::Style {
                        text_color: icon_text_color,
                    },
                    icon_layout,
                    cursor,
                    &bounds,
                );
            }

            let item = &self.items[i];

            if let Some(label) = &item.label {
                let label_x = row_bounds.x + H_PAD + ICON_BOX + GAP;
                renderer.fill_text(
                    Text {
                        content: label.clone(),
                        bounds: Size::new(LABEL_W, row_bounds.height),
                        size: text_size,
                        line_height: text::LineHeight::default(),
                        font,
                        align_x: text::Alignment::Default,
                        align_y: alignment::Vertical::Center,
                        shaping: text::Shaping::default(),
                        wrapping: text::Wrapping::default(),
                    },
                    Point::new(label_x, row_bounds.center_y()),
                    label_color,
                    bounds,
                );
            }

            if let Some(shortcut) = &item.shortcut {
                let sx = row_bounds.x + row_bounds.width - H_PAD;
                renderer.fill_text(
                    Text {
                        content: shortcut.clone(),
                        bounds: Size::new(SHORTCUT_W, row_bounds.height),
                        size: text_size,
                        line_height: text::LineHeight::default(),
                        font,
                        align_x: text::Alignment::Right,
                        align_y: alignment::Vertical::Center,
                        shaping: text::Shaping::default(),
                        wrapping: text::Wrapping::default(),
                    },
                    Point::new(sx, row_bounds.center_y()),
                    shortcut_color,
                    bounds,
                );
            }
        }
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let bounds = layout.bounds();

        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let mut new_hover = None;
                for (i, row_layout) in layout.children().enumerate() {
                    if cursor.is_over(row_layout.bounds()) {
                        new_hover = Some(i);
                        break;
                    }
                }
                if self.state.hovered_item != new_hover {
                    self.state.hovered_item = new_hover;
                    shell.request_redraw();
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(
                mouse::Button::Left | mouse::Button::Right,
            )) => {
                let mut picked: Option<usize> = None;
                for (i, row_layout) in layout.children().enumerate() {
                    if cursor.is_over(row_layout.bounds()) {
                        picked = Some(i);
                        break;
                    }
                }
                if let Some(i) = picked {
                    self.state.selected = i;
                    self.state.is_open = false;
                    self.state.hovered_item = None;
                    if let Some(f) = self.on_select
                        && let Some(item) = self.items.get(i)
                    {
                        shell.publish(f(item.value.clone()));
                    }
                    shell.capture_event();
                    shell.request_redraw();
                } else if cursor.is_over(bounds) {
                    // Click inside the overlay but not on a row — swallow so
                    // the flyout doesn't immediately close on gutter clicks.
                    shell.capture_event();
                }
                // Click outside: let it bubble; the widget's update() sees
                // `is_open == true` and closes.
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) => {
                if matches!(key, keyboard::Key::Named(keyboard::key::Named::Escape)) {
                    self.state.is_open = false;
                    self.state.hovered_item = None;
                    shell.capture_event();
                    shell.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
    ) -> mouse::Interaction {
        for row_layout in layout.children() {
            if cursor.is_over(row_layout.bounds()) {
                return mouse::Interaction::Pointer;
            }
        }
        mouse::Interaction::default()
    }
}
