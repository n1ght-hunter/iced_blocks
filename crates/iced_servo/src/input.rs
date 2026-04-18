//! Translate iced events into `servo::InputEvent`s and forward them to
//! the webview. Cross-platform by construction — iced's event delivery
//! goes through winit (or the host windowing backend) and arrives at
//! `Program::update` in logical pixels regardless of OS.

use euclid::Point2D;
use iced::advanced::input_method;
use iced::keyboard::{self, key as ice_key};
use iced::mouse;
use iced::touch;
use iced::window;
use iced::{Event, Point, Rectangle};
use servo::{
    CSSPixel, Code as ServoCode, CompositionEvent, CompositionState, ImeEvent, InputEvent,
    Key as ServoKey, KeyState, KeyboardEvent, Location as ServoLocation,
    Modifiers as ServoModifiers, MouseButton, MouseButtonAction, MouseButtonEvent, MouseMoveEvent,
    NamedKey as ServoNamedKey, TouchEvent, TouchEventType, TouchId, WebView, WebViewPoint,
    WheelDelta, WheelEvent, WheelMode,
};

use crate::controller::ServoWebViewController;

/// Translate one iced event and forward it to Servo. Returns `true` if
/// any Servo `notify_input_event` was called (so the widget's `update`
/// can request a redraw).
///
/// `focused` is the widget's "logical focus" flag from click-to-focus;
/// keyboard events are gated on it so typing into iced widgets (e.g.
/// the app's own address bar) doesn't leak into the webview.
pub(crate) fn translate_event(
    event: &Event,
    bounds: Rectangle,
    cursor: mouse::Cursor,
    focused: bool,
    controller: &ServoWebViewController,
) -> bool {
    let webview = controller.webview();
    let scale = controller.scale_factor();

    match event {
        Event::Mouse(mouse::Event::CursorMoved { .. }) => {
            let Some(local) = cursor.position_in(bounds) else {
                return false;
            };
            webview.notify_input_event(InputEvent::MouseMove(MouseMoveEvent::new(web_point(
                local, scale,
            ))));
            true
        }
        Event::Mouse(mouse::Event::CursorLeft) => {
            webview.notify_input_event(InputEvent::MouseLeftViewport(
                servo::MouseLeftViewportEvent::default(),
            ));
            true
        }
        Event::Mouse(mouse::Event::ButtonPressed(btn)) => {
            // Intercept the extra mouse buttons and drive browser history
            // directly — Servo dispatches them as DOM mouse events but
            // does not auto-navigate, so without this the physical back /
            // forward buttons on a mouse do nothing.
            if matches!(btn, mouse::Button::Back) {
                controller.go_back();
                return true;
            }
            if matches!(btn, mouse::Button::Forward) {
                controller.go_forward();
                return true;
            }
            let Some(local) = cursor.position_in(bounds) else {
                return false;
            };
            webview.notify_input_event(InputEvent::MouseButton(MouseButtonEvent::new(
                MouseButtonAction::Down,
                iced_button_to_servo(*btn),
                web_point(local, scale),
            )));
            true
        }
        Event::Mouse(mouse::Event::ButtonReleased(btn)) => {
            if matches!(btn, mouse::Button::Back | mouse::Button::Forward) {
                return true;
            }
            // Deliberately forward button-up even when the cursor has
            // left the widget bounds, so drag-to-click-outside behaviors
            // (text selection release, drag-and-drop cancel) cleanly
            // terminate. Use the cursor's best-known logical position.
            let local = cursor
                .position_in(bounds)
                .or_else(|| {
                    cursor
                        .position()
                        .map(|p| Point::new(p.x - bounds.x, p.y - bounds.y))
                })
                .unwrap_or(Point::ORIGIN);
            webview.notify_input_event(InputEvent::MouseButton(MouseButtonEvent::new(
                MouseButtonAction::Up,
                iced_button_to_servo(*btn),
                web_point(local, scale),
            )));
            true
        }
        Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
            let Some(local) = cursor.position_in(bounds) else {
                return false;
            };
            let (dx, dy, mode) = iced_wheel_delta(*delta);
            webview.notify_input_event(InputEvent::Wheel(WheelEvent::new(
                WheelDelta {
                    x: dx,
                    y: dy,
                    z: 0.0,
                    mode,
                },
                web_point(local, scale),
            )));
            true
        }
        Event::Keyboard(kb) if focused => translate_keyboard(kb, webview),
        Event::Touch(t) => translate_touch(t, bounds, webview),
        Event::InputMethod(ime) if focused => translate_input_method(ime, webview),
        Event::Window(window::Event::Focused) => {
            webview.focus();
            true
        }
        Event::Window(window::Event::Unfocused) => {
            webview.blur();
            true
        }
        _ => false,
    }
}

/// iced delivers touch points in window-logical pixels; we re-base to
/// widget-local and hand them to Servo as CSS/page pixels (the same
/// coordinate system the mouse path uses, so hit-testing stays
/// consistent between mouse and touch on hybrid devices).
fn translate_touch(event: &touch::Event, bounds: Rectangle, webview: &WebView) -> bool {
    let (event_type, id, position) = match event {
        touch::Event::FingerPressed { id, position } => (TouchEventType::Down, *id, *position),
        touch::Event::FingerMoved { id, position } => (TouchEventType::Move, *id, *position),
        touch::Event::FingerLifted { id, position } => (TouchEventType::Up, *id, *position),
        touch::Event::FingerLost { id, position } => (TouchEventType::Cancel, *id, *position),
    };
    // Drop touches that start outside the widget. Moves/lifts that leave
    // the widget are still forwarded so drags that finish outside the
    // bounds clean up correctly, matching the mouse button-release path.
    let inside = position.x >= bounds.x
        && position.y >= bounds.y
        && position.x <= bounds.x + bounds.width
        && position.y <= bounds.y + bounds.height;
    if matches!(event_type, TouchEventType::Down) && !inside {
        return false;
    }
    let local = Point::new(position.x - bounds.x, position.y - bounds.y);
    // `touch::Finger` wraps a u64, Servo expects an i32 — just truncate.
    // Collisions would require an app to have > 2 billion active touches,
    // which isn't a failure mode we care about.
    let touch_id = TouchId(id.0 as i32);
    webview.notify_input_event(InputEvent::Touch(TouchEvent::new(
        event_type,
        touch_id,
        web_point(local, 1.0),
    )));
    true
}

/// iced's IME events map cleanly onto Servo's `CompositionState` /
/// `ImeEvent::Dismissed`. iced only fires these while the widget
/// receiving them is an active IME target, and we gate further on the
/// webview's own click-to-focus state so typing into the address bar's
/// IME candidate window doesn't leak into the page.
fn translate_input_method(event: &input_method::Event, webview: &WebView) -> bool {
    let servo_event = match event {
        input_method::Event::Opened => ImeEvent::Composition(CompositionEvent {
            state: CompositionState::Start,
            data: String::new(),
        }),
        input_method::Event::Preedit(text, _range) => ImeEvent::Composition(CompositionEvent {
            state: CompositionState::Update,
            data: text.clone(),
        }),
        input_method::Event::Commit(text) => ImeEvent::Composition(CompositionEvent {
            state: CompositionState::End,
            data: text.clone(),
        }),
        input_method::Event::Closed => ImeEvent::Dismissed,
    };
    webview.notify_input_event(InputEvent::Ime(servo_event));
    true
}

fn translate_keyboard(event: &keyboard::Event, webview: &WebView) -> bool {
    let (state, key, modified_key, physical_key, location, modifiers, repeat) = match event {
        keyboard::Event::KeyPressed {
            key,
            modified_key,
            physical_key,
            location,
            modifiers,
            ..
        } => (
            KeyState::Down,
            key,
            modified_key,
            physical_key,
            location,
            modifiers,
            false, // iced 0.14 doesn't expose repeat state
        ),
        keyboard::Event::KeyReleased {
            key,
            modified_key,
            physical_key,
            location,
            modifiers,
            ..
        } => (
            KeyState::Up,
            key,
            modified_key,
            physical_key,
            location,
            modifiers,
            false,
        ),
        keyboard::Event::ModifiersChanged(_) => return false,
    };

    let servo_key = iced_key_to_servo_key(modified_key, key);
    let servo_code = iced_physical_to_servo_code(physical_key);
    let servo_location = iced_location_to_servo(*location);
    let servo_modifiers = iced_modifiers_to_servo(*modifiers);

    webview.notify_input_event(InputEvent::Keyboard(KeyboardEvent::new_without_event(
        state,
        servo_key,
        servo_code,
        servo_location,
        servo_modifiers,
        repeat,
        false,
    )));
    true
}

/// Map iced `Key` → Servo (`keyboard_types`) `Key`. Named variants share
/// W3C UI Events names between both libraries, so we translate by
/// `Debug`-formatting iced's enum and parsing it on the keyboard-types
/// side. Printable keys go through as `Key::Character(String)`.
///
/// `keyboard_types::Key` has no `Unidentified` variant — fall back to
/// `Key::Named(NamedKey::Unidentified)`, which is the W3C-spec way to
/// represent an unknown key.
fn iced_key_to_servo_key(modified: &ice_key::Key, base: &ice_key::Key) -> ServoKey {
    // Prefer the modified key (shift/alt applied) so typed characters
    // match what the user produced. Fall back to the base key for
    // Named variants.
    match modified {
        ice_key::Key::Character(s) => ServoKey::Character(s.to_string()),
        ice_key::Key::Named(named) => named_to_servo(*named),
        ice_key::Key::Unidentified => match base {
            ice_key::Key::Character(s) => ServoKey::Character(s.to_string()),
            ice_key::Key::Named(named) => named_to_servo(*named),
            ice_key::Key::Unidentified => ServoKey::Named(ServoNamedKey::Unidentified),
        },
    }
}

fn named_to_servo(named: ice_key::Named) -> ServoKey {
    // W3C spec: Space is Key::Character(" "), not a named key.
    // iced reports it as Named::Space, so catch it here.
    if named == ice_key::Named::Space {
        return ServoKey::Character(" ".into());
    }
    let debug_name = format!("{named:?}");
    debug_name
        .parse::<ServoNamedKey>()
        .map(ServoKey::Named)
        .unwrap_or(ServoKey::Named(ServoNamedKey::Unidentified))
}

fn iced_physical_to_servo_code(physical: &ice_key::Physical) -> ServoCode {
    match physical {
        ice_key::Physical::Code(code) => {
            let debug_name = format!("{code:?}");
            debug_name
                .parse::<ServoCode>()
                .unwrap_or(ServoCode::Unidentified)
        }
        ice_key::Physical::Unidentified(_) => ServoCode::Unidentified,
    }
}

fn iced_location_to_servo(loc: keyboard::Location) -> ServoLocation {
    match loc {
        keyboard::Location::Standard => ServoLocation::Standard,
        keyboard::Location::Left => ServoLocation::Left,
        keyboard::Location::Right => ServoLocation::Right,
        keyboard::Location::Numpad => ServoLocation::Numpad,
    }
}

fn iced_modifiers_to_servo(mods: keyboard::Modifiers) -> ServoModifiers {
    let mut out = ServoModifiers::empty();
    if mods.shift() {
        out |= ServoModifiers::SHIFT;
    }
    if mods.control() {
        out |= ServoModifiers::CONTROL;
    }
    if mods.alt() {
        out |= ServoModifiers::ALT;
    }
    if mods.logo() {
        out |= ServoModifiers::META;
    }
    out
}

fn web_point(local: Point, _scale: f32) -> WebViewPoint {
    // Send CSS (page) pixels directly. iced gives us logical pixels and
    // Servo's `CSSPixel` unit is logical pixels — skipping the device
    // round-trip avoids rounding drift between iced's layout and Servo's
    // hit testing (which was causing "Empty hit test result" warnings
    // after navigation/resize).
    WebViewPoint::from(Point2D::<f32, CSSPixel>::new(local.x, local.y))
}

fn iced_button_to_servo(button: mouse::Button) -> MouseButton {
    match button {
        mouse::Button::Left => MouseButton::Left,
        mouse::Button::Right => MouseButton::Right,
        mouse::Button::Middle => MouseButton::Middle,
        mouse::Button::Back => MouseButton::Back,
        mouse::Button::Forward => MouseButton::Forward,
        mouse::Button::Other(n) => MouseButton::Other(n),
    }
}

fn iced_wheel_delta(delta: mouse::ScrollDelta) -> (f64, f64, WheelMode) {
    match delta {
        mouse::ScrollDelta::Lines { x, y } => {
            // Matches what `winit_minimal.rs` does for WM_MOUSEWHEEL
            // lines in the canonical servo example: 76 px per line.
            (x as f64 * 76.0, y as f64 * 76.0, WheelMode::DeltaPixel)
        }
        mouse::ScrollDelta::Pixels { x, y } => (x as f64, y as f64, WheelMode::DeltaPixel),
    }
}

/// Helper used by callers that want to consume `notify_input_event`
/// directly (e.g. synthetic events).
#[allow(dead_code)]
pub(crate) fn forward_raw(webview: &WebView, event: InputEvent) {
    let _ = webview.notify_input_event(event);
}

/// Map a Servo `Cursor` value to the closest iced `mouse::Interaction`
/// variant. iced's `mouse::Interaction` is what gets turned into a
/// winit `CursorIcon` by `iced_winit::conversion::mouse_interaction`,
/// so returning the right value here is enough to update the OS cursor
/// cross-platform — no Win32/NSCursor/X11 code needed.
pub(crate) fn cursor_to_interaction(cursor: servo::Cursor) -> mouse::Interaction {
    use mouse::Interaction::*;
    use servo::Cursor;
    match cursor {
        Cursor::None => Hidden,
        Cursor::Default => None,
        Cursor::Pointer => Pointer,
        Cursor::Text | Cursor::VerticalText => Text,
        Cursor::Wait => Wait,
        Cursor::Progress => Progress,
        Cursor::Crosshair => Crosshair,
        Cursor::Move | Cursor::AllScroll => Move,
        Cursor::Grab => Grab,
        Cursor::Grabbing => Grabbing,
        Cursor::NotAllowed => NotAllowed,
        Cursor::NoDrop => NoDrop,
        Cursor::Help => Help,
        Cursor::ZoomIn => ZoomIn,
        Cursor::ZoomOut => ZoomOut,
        Cursor::Cell => Cell,
        Cursor::ContextMenu => ContextMenu,
        Cursor::Alias => Alias,
        Cursor::Copy => Copy,
        Cursor::ColResize => ResizingColumn,
        Cursor::RowResize => ResizingRow,
        Cursor::EResize | Cursor::WResize | Cursor::EwResize => ResizingHorizontally,
        Cursor::NResize | Cursor::SResize | Cursor::NsResize => ResizingVertically,
        Cursor::NeResize | Cursor::SwResize | Cursor::NeswResize => ResizingDiagonallyUp,
        Cursor::NwResize | Cursor::SeResize | Cursor::NwseResize => ResizingDiagonallyDown,
    }
}
