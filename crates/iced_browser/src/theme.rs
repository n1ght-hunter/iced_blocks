use iced::theme::Palette;
use iced::{Color, Theme};

pub fn browser_theme() -> Theme {
    Theme::custom(
        String::from("Browser Dark"),
        Palette {
            background: Color::from_rgb(0.10, 0.10, 0.13),
            text: Color::from_rgb(0.90, 0.90, 0.92),
            primary: Color::from_rgb(0.35, 0.55, 0.95),
            success: Color::from_rgb(0.30, 0.75, 0.45),
            warning: Color::from_rgb(0.95, 0.75, 0.20),
            danger: Color::from_rgb(0.90, 0.30, 0.30),
        },
    )
}
