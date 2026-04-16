//! Demonstrates `iced_frame`'s content fit and alignment modes by
//! rendering a checkerboard pattern. The source only regenerates the
//! checkerboard when the widget requests a new size.

use std::sync::{Arc, Mutex};

use iced::widget::{button, column, container, pick_list, row, text};
use iced::{Element, Length, Task, mouse};

use iced_frame::{
    Alignment as FrameAlignment, ContentFit, FilterMode, Frame, FrameSource, FrameWidget,
    SizeRequestSlot,
};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("iced_frame demo")
        .run()
}

const CELL: u32 = 32;

fn checkerboard(width: u32, height: u32) -> Vec<u8> {
    let mut data = vec![0u8; (width * height * 4) as usize];
    for (i, pixel) in data.chunks_exact_mut(4).enumerate() {
        let x = (i as u32) % width;
        let y = (i as u32) / width;
        let dark = ((x / CELL) + (y / CELL)).is_multiple_of(2);
        let (r, g, b) = if dark { (60, 60, 80) } else { (200, 180, 255) };
        pixel[0] = r;
        pixel[1] = g;
        pixel[2] = b;
        pixel[3] = 255;
    }
    data
}

#[derive(Clone)]
struct CheckerboardSource {
    frame_slot: Arc<Mutex<Option<Frame>>>,
    size_request: SizeRequestSlot,
    current_size: Arc<Mutex<(u32, u32)>>,
}

const INITIAL_W: u32 = 200;
const INITIAL_H: u32 = 150;

impl CheckerboardSource {
    fn new() -> Self {
        let w = INITIAL_W;
        let h = INITIAL_H;
        Self {
            frame_slot: Arc::new(Mutex::new(Some(Frame::new(checkerboard(w, h), w, h)))),
            size_request: SizeRequestSlot::new(),
            current_size: Arc::new(Mutex::new((w, h))),
        }
    }
}

impl FrameSource for CheckerboardSource {
    fn frame_slot(&self) -> Arc<Mutex<Option<Frame>>> {
        Arc::clone(&self.frame_slot)
    }

    fn size_request_slot(&self) -> SizeRequestSlot {
        self.size_request.clone()
    }

    fn cursor(&self) -> mouse::Interaction {
        mouse::Interaction::default()
    }

    fn handle_event(
        &self,
        _event: &iced::Event,
        _bounds: iced::Rectangle,
        _cursor: mouse::Cursor,
        _focused: bool,
    ) -> bool {
        false
    }
}

struct App {
    source: CheckerboardSource,
    fit: ContentFit,
    alignment: FrameAlignment,
    filter: FilterMode,
}

#[derive(Debug, Clone)]
enum Message {
    Fit(ContentFit),
    Align(FrameAlignment),
    Filter(FilterMode),
    Resize,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        (
            Self {
                source: CheckerboardSource::new(),
                fit: ContentFit::Fill,
                alignment: FrameAlignment::Center,
                filter: FilterMode::Linear,
            },
            Task::none(),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Fit(f) => self.fit = f,
            Message::Align(a) => self.alignment = a,
            Message::Filter(f) => self.filter = f,
            Message::Resize => {
                if let Some((size, _)) = self.source.size_request.size() {
                    let (w, h) = (size.width.max(1), size.height.max(1));
                    let mut current = self.source.current_size.lock().unwrap();
                    *current = (w, h);
                    *self.source.frame_slot.lock().unwrap() =
                        Some(Frame::new(checkerboard(w, h), w, h));
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let (fw, fh) = *self.source.current_size.lock().unwrap();
        let scale = self
            .source
            .size_request
            .size()
            .map(|(_, s)| s)
            .unwrap_or(1.0);

        let controls = row![
            text("Fit:").size(14),
            pick_list(
                &[
                    ContentFit::Fill,
                    ContentFit::Contain,
                    ContentFit::Cover,
                    ContentFit::FitWidth,
                    ContentFit::FitHeight,
                    ContentFit::None,
                ][..],
                Some(self.fit),
                Message::Fit,
            ),
            text("Align:").size(14),
            pick_list(
                &[
                    FrameAlignment::TopLeft,
                    FrameAlignment::TopCenter,
                    FrameAlignment::TopRight,
                    FrameAlignment::CenterLeft,
                    FrameAlignment::Center,
                    FrameAlignment::CenterRight,
                    FrameAlignment::BottomLeft,
                    FrameAlignment::BottomCenter,
                    FrameAlignment::BottomRight,
                ][..],
                Some(self.alignment),
                Message::Align,
            ),
            text("Filter:").size(14),
            pick_list(
                &[FilterMode::Linear, FilterMode::Nearest][..],
                Some(self.filter),
                Message::Filter,
            ),
            button(text("Resize to fit").size(14))
                .on_press(Message::Resize)
                .padding(4),
        ]
        .spacing(8)
        .padding(8)
        .wrap();

        let widget = FrameWidget::new(self.source.clone())
            .content_fit(self.fit)
            .alignment(self.alignment)
            .filter(self.filter)
            .width(Length::Fill)
            .height(Length::Fill);

        column![
            controls,
            text(format!(
                "Frame: {fw}x{fh}  Scale: {scale:.2}  Mode: {} / {} / {}",
                self.fit, self.alignment, self.filter
            ))
            .size(12),
            container(widget).width(Length::Fill).height(Length::Fill),
        ]
        .into()
    }
}
