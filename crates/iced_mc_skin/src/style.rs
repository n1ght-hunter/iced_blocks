//! Theming for [`MCSkinView`](crate::widget::MCSkinView).

use iced::{Background, Color, Theme};

/// The style of an [`MCSkinView`](crate::widget::MCSkinView).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// Background painted behind the 3D skin model. Defaults to
    /// [`Color::TRANSPARENT`] — whatever sits behind the widget shows through.
    pub background: Background,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            background: Background::Color(Color::TRANSPARENT),
        }
    }
}

/// The theme catalog for an [`MCSkinView`](crate::widget::MCSkinView).
pub trait Catalog {
    /// The item class of this [`Catalog`].
    type Class<'a>;

    /// The default class produced by this [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// Resolves the [`Style`] of a class.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for an [`MCSkinView`](crate::widget::MCSkinView).
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

/// The default [`MCSkinView`](crate::widget::MCSkinView) style — a transparent
/// background, preserving the widget's original behavior.
pub fn default(_theme: &Theme) -> Style {
    Style::default()
}
