use iced::{Element, Length, widget::column};
use iced_mc_skin::{AnimationMode, ArmVariant, SkinProgram};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title("Skin Viewer")
        .run()
}

struct App {
    skin: SkinProgram,
}

#[derive(Debug, Clone)]
enum Message {}

impl App {
    fn new() -> (Self, iced::Task<Message>) {
        let mut skin = SkinProgram::new(ArmVariant::Classic, AnimationMode::Walk);

        let img = image::open(concat!(env!("CARGO_MANIFEST_DIR"), "/examples/steve.png"))
            .expect("failed to load steve.png")
            .into_rgba8();
        skin.set_skin(img.into_raw());

        (Self { skin }, iced::Task::none())
    }

    fn update(&mut self, _message: Message) -> iced::Task<Message> {
        iced::Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        column![self.skin.view().width(Length::Fill).height(Length::Fill)].into()
    }
}
