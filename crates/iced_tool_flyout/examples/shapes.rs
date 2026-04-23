use iced::widget::{column, container, row, text};
use iced::{Element, Length, alignment};
use iced_tool_flyout::{tool_flyout, tool_item};

fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("iced_tool_flyout — shapes")
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

#[derive(Default)]
struct App {
    last_activated: Option<Shape>,
    current: Shape,
}

#[derive(Debug, Clone)]
enum Message {
    Activated(Shape),
    Selected(Shape),
}

impl App {
    fn update(&mut self, message: Message) {
        match message {
            Message::Activated(shape) => {
                self.last_activated = Some(shape);
            }
            Message::Selected(shape) => {
                self.current = shape;
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let icon = |s: Shape| text(s.glyph()).size(22);

        let flyout = tool_flyout(
            vec![
                tool_item(Shape::Rectangle, icon(Shape::Rectangle))
                    .label(Shape::Rectangle.name())
                    .shortcut("R"),
                tool_item(Shape::Ellipse, icon(Shape::Ellipse))
                    .label(Shape::Ellipse.name())
                    .shortcut("E"),
                tool_item(Shape::Line, icon(Shape::Line))
                    .label(Shape::Line.name())
                    .shortcut("L"),
                tool_item(Shape::Polygon, icon(Shape::Polygon))
                    .label(Shape::Polygon.name())
                    .shortcut("P"),
            ],
            Message::Activated,
        )
        .default_selected(Shape::Rectangle)
        .on_select(Message::Selected);

        let status = match self.last_activated {
            Some(s) => format!("Last activated: {}", s.name()),
            None => "Left-click to activate. Right-click or long-press to open flyout.".into(),
        };

        let toolbar = row![flyout].spacing(8).padding(8);

        container(
            column![toolbar, text(status).size(14)]
                .spacing(16)
                .padding(16),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(alignment::Horizontal::Left)
        .align_y(alignment::Vertical::Top)
        .into()
    }
}
