//! Photoshop-style tool-flyout button for [Iced].
//!
//! [`ToolFlyout`] shows a single-icon button with a small corner indicator.
//! Left-clicking activates the currently-selected tool; **right-clicking** or
//! **long-pressing** (default 400 ms) opens a popup flyout listing every tool
//! variant. Picking a variant swaps the button's icon and makes it the new
//! activation target.
//!
//! The widget is uncontrolled: it remembers the selected variant across
//! renders. Attach `.on_select(...)` if you need to mirror the selection in
//! application state.
//!
//! [Iced]: https://github.com/iced-rs/iced

mod overlay;
mod style;
mod widget;

pub use style::{Catalog, Status, Style, StyleFn, default};
pub use widget::{Id, ToolFlyout, tool_flyout};

use iced::Element;
use iced::Task;

/// Produces a [`Task`] that programmatically selects the variant matching
/// `value` in the [`ToolFlyout`] identified by `id`.
pub fn select<T: PartialEq + Send + 'static, M: Send + 'static>(
    id: impl Into<Id>,
    value: T,
) -> Task<M> {
    iced::advanced::widget::operate(widget::select_operation(id.into(), value)).discard()
}

/// A single variant inside a [`ToolFlyout`].
///
/// Construct with [`tool_item`]; chain [`ToolItem::label`] and
/// [`ToolItem::shortcut`] to add text metadata displayed in the flyout.
pub struct ToolItem<'a, T, Message, Theme, Renderer> {
    pub(crate) value: T,
    pub(crate) icon: Element<'a, Message, Theme, Renderer>,
    pub(crate) label: Option<String>,
    pub(crate) shortcut: Option<String>,
}

impl<'a, T, Message, Theme, Renderer> ToolItem<'a, T, Message, Theme, Renderer> {
    /// Sets the human-readable label shown in the flyout row.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Sets a keyboard-shortcut hint rendered right-aligned in the flyout row.
    ///
    /// Purely cosmetic — the widget does not bind the shortcut itself.
    pub fn shortcut(mut self, shortcut: impl Into<String>) -> Self {
        self.shortcut = Some(shortcut.into());
        self
    }
}

/// Builds a [`ToolItem`] with the given identifier and icon element.
pub fn tool_item<'a, T, Message, Theme, Renderer>(
    value: T,
    icon: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> ToolItem<'a, T, Message, Theme, Renderer> {
    ToolItem {
        value,
        icon: icon.into(),
        label: None,
        shortcut: None,
    }
}
