use iced::widget::{button, column, container, row, scrollable, slider, text, toggler};
use iced::{Background, Border, Color, Element, Length, Padding, Task, alignment};
use iced_tool_flyout::{Id, Style, tool_flyout, tool_item};

fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("iced_tool_flyout — playground")
        .window_size((900.0, 700.0))
        .run()
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum Shape {
    #[default]
    Rectangle,
    Ellipse,
    Line,
    Polygon,
}

impl Shape {
    const ALL: [Shape; 4] = [
        Shape::Rectangle,
        Shape::Ellipse,
        Shape::Line,
        Shape::Polygon,
    ];

    fn glyph(self) -> &'static str {
        match self {
            Shape::Rectangle => "▭",
            Shape::Ellipse => "◯",
            Shape::Line => "╱",
            Shape::Polygon => "⬠",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Shape::Rectangle => "Rectangle",
            Shape::Ellipse => "Ellipse",
            Shape::Line => "Line",
            Shape::Polygon => "Polygon",
        }
    }
}

const FLYOUT_ID: &str = "tools";

struct App {
    last_activated: Option<Shape>,
    current: Shape,
    style: StyleEditor,
}

impl Default for App {
    fn default() -> Self {
        Self {
            last_activated: None,
            current: Shape::Rectangle,
            style: StyleEditor::default(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Rgb {
    r: f32,
    g: f32,
    b: f32,
}

impl Rgb {
    fn new(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b }
    }

    fn to_color(self) -> Color {
        Color::from_rgb(self.r, self.g, self.b)
    }
}

struct StyleEditor {
    has_background: bool,
    background: Rgb,
    text_color: Rgb,
    indicator_color: Rgb,
    border_radius: f32,
    border_width: f32,
    flyout_background: Rgb,
    flyout_border_radius: f32,
    flyout_border_width: f32,
    flyout_text_color: Rgb,
    flyout_shortcut_color: Rgb,
    flyout_highlight: Rgb,
    flyout_highlight_text: Rgb,
}

impl Default for StyleEditor {
    fn default() -> Self {
        Self {
            has_background: false,
            background: Rgb::new(0.9, 0.9, 0.9),
            text_color: Rgb::new(0.0, 0.0, 0.0),
            indicator_color: Rgb::new(0.0, 0.0, 0.0),
            border_radius: 4.0,
            border_width: 1.0,
            flyout_background: Rgb::new(1.0, 1.0, 1.0),
            flyout_border_radius: 6.0,
            flyout_border_width: 1.0,
            flyout_text_color: Rgb::new(0.0, 0.0, 0.0),
            flyout_shortcut_color: Rgb::new(0.45, 0.45, 0.45),
            flyout_highlight: Rgb::new(0.2, 0.4, 0.8),
            flyout_highlight_text: Rgb::new(1.0, 1.0, 1.0),
        }
    }
}

fn darken(c: Color, amount: f32) -> Color {
    Color::from_rgb(
        (c.r - amount).max(0.0),
        (c.g - amount).max(0.0),
        (c.b - amount).max(0.0),
    )
}

#[derive(Debug, Clone)]
enum Message {
    Activated(Shape),
    Selected(Shape),
    SelectVia(Shape),

    HasBackground(bool),
    BackgroundR(f32),
    BackgroundG(f32),
    BackgroundB(f32),
    TextR(f32),
    TextG(f32),
    TextB(f32),
    IndicatorR(f32),
    IndicatorG(f32),
    IndicatorB(f32),
    BorderRadius(f32),
    BorderWidth(f32),
    FlyoutBgR(f32),
    FlyoutBgG(f32),
    FlyoutBgB(f32),
    FlyoutBorderRadius(f32),
    FlyoutBorderWidth(f32),
    FlyoutTextR(f32),
    FlyoutTextG(f32),
    FlyoutTextB(f32),
    FlyoutShortcutR(f32),
    FlyoutShortcutG(f32),
    FlyoutShortcutB(f32),
    FlyoutHighlightR(f32),
    FlyoutHighlightG(f32),
    FlyoutHighlightB(f32),
    FlyoutHighlightTextR(f32),
    FlyoutHighlightTextG(f32),
    FlyoutHighlightTextB(f32),
}

impl App {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Activated(s) => self.last_activated = Some(s),
            Message::Selected(s) => self.current = s,
            Message::SelectVia(s) => {
                return iced_tool_flyout::select(Id::new(FLYOUT_ID), s);
            }

            Message::HasBackground(v) => self.style.has_background = v,
            Message::BackgroundR(v) => self.style.background.r = v,
            Message::BackgroundG(v) => self.style.background.g = v,
            Message::BackgroundB(v) => self.style.background.b = v,
            Message::TextR(v) => self.style.text_color.r = v,
            Message::TextG(v) => self.style.text_color.g = v,
            Message::TextB(v) => self.style.text_color.b = v,
            Message::IndicatorR(v) => self.style.indicator_color.r = v,
            Message::IndicatorG(v) => self.style.indicator_color.g = v,
            Message::IndicatorB(v) => self.style.indicator_color.b = v,
            Message::BorderRadius(v) => self.style.border_radius = v,
            Message::BorderWidth(v) => self.style.border_width = v,
            Message::FlyoutBgR(v) => self.style.flyout_background.r = v,
            Message::FlyoutBgG(v) => self.style.flyout_background.g = v,
            Message::FlyoutBgB(v) => self.style.flyout_background.b = v,
            Message::FlyoutBorderRadius(v) => self.style.flyout_border_radius = v,
            Message::FlyoutBorderWidth(v) => self.style.flyout_border_width = v,
            Message::FlyoutTextR(v) => self.style.flyout_text_color.r = v,
            Message::FlyoutTextG(v) => self.style.flyout_text_color.g = v,
            Message::FlyoutTextB(v) => self.style.flyout_text_color.b = v,
            Message::FlyoutShortcutR(v) => self.style.flyout_shortcut_color.r = v,
            Message::FlyoutShortcutG(v) => self.style.flyout_shortcut_color.g = v,
            Message::FlyoutShortcutB(v) => self.style.flyout_shortcut_color.b = v,
            Message::FlyoutHighlightR(v) => self.style.flyout_highlight.r = v,
            Message::FlyoutHighlightG(v) => self.style.flyout_highlight.g = v,
            Message::FlyoutHighlightB(v) => self.style.flyout_highlight.b = v,
            Message::FlyoutHighlightTextR(v) => self.style.flyout_highlight_text.r = v,
            Message::FlyoutHighlightTextG(v) => self.style.flyout_highlight_text.g = v,
            Message::FlyoutHighlightTextB(v) => self.style.flyout_highlight_text.b = v,
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let icon = |s: Shape| text(s.glyph()).size(22);

        let editor = &self.style;
        let style_snapshot = StyleEditorSnapshot {
            has_background: editor.has_background,
            background: editor.background,
            text_color: editor.text_color,
            indicator_color: editor.indicator_color,
            border_radius: editor.border_radius,
            border_width: editor.border_width,
            flyout_background: editor.flyout_background,
            flyout_border_radius: editor.flyout_border_radius,
            flyout_border_width: editor.flyout_border_width,
            flyout_text_color: editor.flyout_text_color,
            flyout_shortcut_color: editor.flyout_shortcut_color,
            flyout_highlight: editor.flyout_highlight,
            flyout_highlight_text: editor.flyout_highlight_text,
        };

        let flyout = tool_flyout(
            vec![
                tool_item(Shape::Rectangle, icon(Shape::Rectangle))
                    .label("Rectangle")
                    .shortcut("R"),
                tool_item(Shape::Ellipse, icon(Shape::Ellipse))
                    .label("Ellipse")
                    .shortcut("E"),
                tool_item(Shape::Line, icon(Shape::Line))
                    .label("Line")
                    .shortcut("L"),
                tool_item(Shape::Polygon, icon(Shape::Polygon))
                    .label("Polygon")
                    .shortcut("P"),
            ],
            Message::Activated,
        )
        .id(Id::new(FLYOUT_ID))
        .default_selected(Shape::Rectangle)
        .on_select(Message::Selected)
        .style(move |_theme, status| style_snapshot.build(status));

        let select_buttons = row(Shape::ALL.map(|s| {
            button(text(s.name()).size(12))
                .on_press(Message::SelectVia(s))
                .padding(Padding::from([4, 8]))
                .into()
        }))
        .spacing(4);

        let status = match self.last_activated {
            Some(s) => format!("Activated: {}", s.name()),
            None => "Left-click to activate, right-click for flyout".into(),
        };
        let selected_text = format!("Selected: {}", self.current.name());

        let left = container(
            column![
                text("Tool Flyout").size(18),
                row![flyout].padding(8),
                text("Programmatic select (operation):").size(13),
                select_buttons,
                divider(),
                text(status).size(13),
                text(selected_text).size(13),
            ]
            .spacing(10)
            .padding(16),
        )
        .width(Length::FillPortion(2));

        let right = container(scrollable(
            column![
                text("Style Editor").size(18),
                divider(),
                // Button
                text("Button").size(15),
                toggler(self.style.has_background)
                    .label("Has background")
                    .on_toggle(Message::HasBackground)
                    .text_size(13),
                color_sliders(
                    "Background",
                    self.style.background,
                    Message::BackgroundR,
                    Message::BackgroundG,
                    Message::BackgroundB,
                ),
                color_sliders(
                    "Text color",
                    self.style.text_color,
                    Message::TextR,
                    Message::TextG,
                    Message::TextB,
                ),
                color_sliders(
                    "Indicator",
                    self.style.indicator_color,
                    Message::IndicatorR,
                    Message::IndicatorG,
                    Message::IndicatorB,
                ),
                labeled_slider(
                    "Border radius",
                    self.style.border_radius,
                    0.0,
                    20.0,
                    Message::BorderRadius
                ),
                labeled_slider(
                    "Border width",
                    self.style.border_width,
                    0.0,
                    5.0,
                    Message::BorderWidth
                ),
                divider(),
                // Flyout
                text("Flyout panel").size(15),
                color_sliders(
                    "Background",
                    self.style.flyout_background,
                    Message::FlyoutBgR,
                    Message::FlyoutBgG,
                    Message::FlyoutBgB,
                ),
                labeled_slider(
                    "Border radius",
                    self.style.flyout_border_radius,
                    0.0,
                    20.0,
                    Message::FlyoutBorderRadius
                ),
                labeled_slider(
                    "Border width",
                    self.style.flyout_border_width,
                    0.0,
                    5.0,
                    Message::FlyoutBorderWidth
                ),
                color_sliders(
                    "Text color",
                    self.style.flyout_text_color,
                    Message::FlyoutTextR,
                    Message::FlyoutTextG,
                    Message::FlyoutTextB,
                ),
                color_sliders(
                    "Shortcut color",
                    self.style.flyout_shortcut_color,
                    Message::FlyoutShortcutR,
                    Message::FlyoutShortcutG,
                    Message::FlyoutShortcutB,
                ),
                color_sliders(
                    "Highlight",
                    self.style.flyout_highlight,
                    Message::FlyoutHighlightR,
                    Message::FlyoutHighlightG,
                    Message::FlyoutHighlightB,
                ),
                color_sliders(
                    "Highlight text",
                    self.style.flyout_highlight_text,
                    Message::FlyoutHighlightTextR,
                    Message::FlyoutHighlightTextG,
                    Message::FlyoutHighlightTextB,
                ),
            ]
            .spacing(8)
            .padding(16),
        ))
        .width(Length::FillPortion(3));

        row![left, right].height(Length::Fill).into()
    }
}

struct StyleEditorSnapshot {
    has_background: bool,
    background: Rgb,
    text_color: Rgb,
    indicator_color: Rgb,
    border_radius: f32,
    border_width: f32,
    flyout_background: Rgb,
    flyout_border_radius: f32,
    flyout_border_width: f32,
    flyout_text_color: Rgb,
    flyout_shortcut_color: Rgb,
    flyout_highlight: Rgb,
    flyout_highlight_text: Rgb,
}

impl StyleEditorSnapshot {
    fn build(&self, status: iced_tool_flyout::Status) -> Style {
        let bg_color = self.background.to_color();
        let background = if self.has_background {
            Some(Background::Color(match status {
                iced_tool_flyout::Status::Idle => bg_color,
                iced_tool_flyout::Status::Hovered => darken(bg_color, 0.05),
                iced_tool_flyout::Status::Pressed => darken(bg_color, 0.15),
                iced_tool_flyout::Status::Open => darken(bg_color, 0.05),
            }))
        } else {
            match status {
                iced_tool_flyout::Status::Idle => None,
                _ => Some(Background::Color(darken(bg_color, 0.05))),
            }
        };

        Style {
            background,
            text_color: self.text_color.to_color(),
            indicator_color: self.indicator_color.to_color(),
            border: Border {
                color: match status {
                    iced_tool_flyout::Status::Idle => Color::TRANSPARENT,
                    _ => self.indicator_color.to_color(),
                },
                width: self.border_width,
                radius: self.border_radius.into(),
            },
            flyout_background: Background::Color(self.flyout_background.to_color()),
            flyout_border: Border {
                color: darken(self.flyout_background.to_color(), 0.2),
                width: self.flyout_border_width,
                radius: self.flyout_border_radius.into(),
            },
            flyout_text_color: self.flyout_text_color.to_color(),
            flyout_shortcut_color: self.flyout_shortcut_color.to_color(),
            flyout_highlight: Background::Color(self.flyout_highlight.to_color()),
            flyout_highlight_text: self.flyout_highlight_text.to_color(),
        }
    }
}

fn color_sliders<'a>(
    label: &str,
    color: Rgb,
    on_r: fn(f32) -> Message,
    on_g: fn(f32) -> Message,
    on_b: fn(f32) -> Message,
) -> Element<'a, Message> {
    let swatch = container(text(""))
        .width(16)
        .height(16)
        .style(move |_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(color.to_color())),
            border: Border {
                color: Color::from_rgb(0.6, 0.6, 0.6),
                width: 1.0,
                radius: 2.0.into(),
            },
            ..Default::default()
        });

    column![
        row![swatch, text(label.to_string()).size(13),]
            .spacing(6)
            .align_y(alignment::Vertical::Center),
        row![
            text("R").size(11).width(14),
            slider(0.0..=1.0, color.r, on_r).step(0.01),
        ]
        .spacing(4)
        .align_y(alignment::Vertical::Center),
        row![
            text("G").size(11).width(14),
            slider(0.0..=1.0, color.g, on_g).step(0.01),
        ]
        .spacing(4)
        .align_y(alignment::Vertical::Center),
        row![
            text("B").size(11).width(14),
            slider(0.0..=1.0, color.b, on_b).step(0.01),
        ]
        .spacing(4)
        .align_y(alignment::Vertical::Center),
    ]
    .spacing(2)
    .into()
}

fn divider<'a>() -> Element<'a, Message> {
    container(text(""))
        .height(1)
        .width(Length::Fill)
        .style(|_theme: &iced::Theme| container::Style {
            background: Some(Background::Color(Color::from_rgb(0.8, 0.8, 0.8))),
            ..Default::default()
        })
        .into()
}

fn labeled_slider<'a>(
    label: &str,
    value: f32,
    min: f32,
    max: f32,
    on_change: fn(f32) -> Message,
) -> Element<'a, Message> {
    row![
        text(format!("{label}: {value:.1}")).size(13).width(140),
        slider(min..=max, value, on_change).step(0.5),
    ]
    .spacing(8)
    .align_y(alignment::Vertical::Center)
    .into()
}
