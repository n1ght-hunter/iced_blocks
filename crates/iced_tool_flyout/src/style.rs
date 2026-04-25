//! Theming for [`ToolFlyout`](crate::ToolFlyout).

use iced::{Background, Border, Color, Theme};

/// The visual states a [`ToolFlyout`](crate::ToolFlyout) can be in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The button is idle.
    Idle,
    /// The button is hovered.
    Hovered,
    /// The button is being pressed.
    Pressed,
    /// The flyout is open.
    Open,
}

/// The style of a [`ToolFlyout`](crate::ToolFlyout).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Button background, or `None` for transparent.
    pub background: Option<Background>,
    /// Color of the icon tint hint and labels rendered inside the button.
    pub text_color: Color,
    /// Color of the small corner triangle indicator.
    pub indicator_color: Color,
    /// Button border.
    pub border: Border,
    /// Flyout panel background.
    pub flyout_background: Background,
    /// Flyout panel border.
    pub flyout_border: Border,
    /// Flyout row label color.
    pub flyout_text_color: Color,
    /// Flyout row shortcut-hint color.
    pub flyout_shortcut_color: Color,
    /// Background of the hovered flyout row.
    pub flyout_highlight: Background,
    /// Text color of the hovered flyout row.
    pub flyout_highlight_text: Color,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background: None,
            text_color: Color::BLACK,
            indicator_color: Color::BLACK,
            border: Border::default(),
            flyout_background: Background::Color(Color::WHITE),
            flyout_border: Border::default(),
            flyout_text_color: Color::BLACK,
            flyout_shortcut_color: Color::from_rgb(0.45, 0.45, 0.45),
            flyout_highlight: Background::Color(Color::from_rgb(0.9, 0.9, 0.9)),
            flyout_highlight_text: Color::BLACK,
        }
    }
}

/// The theme catalog for a [`ToolFlyout`](crate::ToolFlyout).
pub trait Catalog {
    /// The item class of this [`Catalog`].
    type Class<'a>;

    /// The default class produced by this [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves the [`Style`] of a class with the given [`Status`].
    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style;
}

/// A styling function for a [`ToolFlyout`](crate::ToolFlyout).
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

/// The default [`ToolFlyout`](crate::ToolFlyout) style derived from the
/// theme's extended palette.
pub fn default(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();
    let bg = palette.background;
    let primary = palette.primary;

    let mut style = Style {
        background: None,
        text_color: bg.base.text,
        indicator_color: bg.strong.color,
        border: Border {
            color: Color::TRANSPARENT,
            width: 1.0,
            radius: 4.0.into(),
        },
        flyout_background: Background::Color(bg.base.color),
        flyout_border: Border {
            color: bg.strong.color,
            width: 1.0,
            radius: 6.0.into(),
        },
        flyout_text_color: bg.base.text,
        flyout_shortcut_color: bg.weak.text,
        flyout_highlight: Background::Color(primary.weak.color),
        flyout_highlight_text: primary.weak.text,
    };

    match status {
        Status::Idle => {}
        Status::Hovered => {
            style.background = Some(Background::Color(bg.weak.color));
            style.border.color = bg.strong.color;
        }
        Status::Pressed => {
            style.background = Some(Background::Color(bg.strong.color));
            style.border.color = primary.strong.color;
        }
        Status::Open => {
            style.background = Some(Background::Color(bg.weak.color));
            style.border.color = primary.strong.color;
        }
    }

    style
}
