//! Helpers shared by the tests of the UI modules.

use egui::accesskit;
use egui::{Event, FullOutput, Key, Modifiers, PointerButton, Pos2, pos2};

/// Ctrl as egui-winit reports it outside macOS.
pub const CTRL: Modifiers = Modifiers::CTRL.plus(Modifiers::COMMAND);

/// A press of `key` with `modifiers` held.
pub fn key(modifiers: Modifiers, key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// The primary button going down, or up, at `pos`.
pub fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// The widgets a frame showed, read from its accessibility update.
pub fn widgets(output: &mut FullOutput) -> Vec<accesskit::Node> {
    let update = output.platform_output.accesskit_update.take();
    update.map_or_else(Vec::new, |update| {
        update.nodes.into_iter().map(|(_, node)| node).collect()
    })
}

/// The middle of a widget.
pub fn center(widget: &accesskit::Node) -> Pos2 {
    let bounds = widget.bounds().expect("widgets have bounds");
    pos2(
        ((bounds.x0 + bounds.x1) / 2.0) as f32,
        ((bounds.y0 + bounds.y1) / 2.0) as f32,
    )
}
