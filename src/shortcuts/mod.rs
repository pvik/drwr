use egui::{Context, Event, Key, KeyboardShortcut, Modifiers, Popup};

use crate::action::Action;
use crate::tool::Tool;

/// How far Ctrl+arrow moves the selected node, in points.
const MOVE_STEP: f64 = 1.0;
/// How far an arrow pans the view, in screen points: as far as egui scrolls
/// for a wheel notch.
const PAN_STEP: f32 = 40.0;

const SHIFT_COMMAND: Modifiers = Modifiers::SHIFT.plus(Modifiers::COMMAND);

/// `COMMAND` is Ctrl, or Cmd on macOS.
#[rustfmt::skip]
const SHORTCUTS: [(Modifiers, Key, Action); 28] = [
    (Modifiers::COMMAND, Key::N,          Action::New),
    (Modifiers::COMMAND, Key::O,          Action::Open),
    // Before Ctrl+S, which egui also matches with Shift held.
    (SHIFT_COMMAND,      Key::S,          Action::SaveAs),
    (Modifiers::COMMAND, Key::S,          Action::Save),
    (Modifiers::COMMAND, Key::X,          Action::Cut),
    (Modifiers::COMMAND, Key::C,          Action::Copy),
    (Modifiers::COMMAND, Key::V,          Action::Paste),
    (Modifiers::COMMAND, Key::Enter,      Action::EditLabel),
    (Modifiers::COMMAND, Key::Plus,       Action::ZoomIn),
    // Like browsers, as + takes Shift on many keyboards.
    (Modifiers::COMMAND, Key::Equals,     Action::ZoomIn),
    (Modifiers::COMMAND, Key::Minus,      Action::ZoomOut),
    (Modifiers::COMMAND, Key::Num0,       Action::SetZoom(1.0)),
    (Modifiers::COMMAND, Key::ArrowLeft,  Action::MoveBy(-MOVE_STEP, 0.0)),
    (Modifiers::COMMAND, Key::ArrowRight, Action::MoveBy(MOVE_STEP, 0.0)),
    (Modifiers::COMMAND, Key::ArrowUp,    Action::MoveBy(0.0, -MOVE_STEP)),
    (Modifiers::COMMAND, Key::ArrowDown,  Action::MoveBy(0.0, MOVE_STEP)),
    (Modifiers::NONE,    Key::ArrowLeft,  Action::PanBy(-PAN_STEP, 0.0)),
    (Modifiers::NONE,    Key::ArrowRight, Action::PanBy(PAN_STEP, 0.0)),
    (Modifiers::NONE,    Key::ArrowUp,    Action::PanBy(0.0, -PAN_STEP)),
    (Modifiers::NONE,    Key::ArrowDown,  Action::PanBy(0.0, PAN_STEP)),
    (Modifiers::NONE,    Key::Delete,     Action::Delete),
    (Modifiers::NONE,    Key::Escape,     Action::SelectTool(Tool::Cursor)),
    (Modifiers::NONE,    Key::B,          Action::SelectTool(Tool::Box)),
    (Modifiers::NONE,    Key::C,          Action::SelectTool(Tool::Circle)),
    (Modifiers::NONE,    Key::E,          Action::SelectTool(Tool::Ellipse)),
    (Modifiers::NONE,    Key::L,          Action::SelectTool(Tool::Line)),
    (Modifiers::NONE,    Key::A,          Action::SelectTool(Tool::Arrow)),
    (Modifiers::NONE,    Key::T,          Action::SelectTool(Tool::TextBox)),
];

/// Takes this frame's presses of shortcuts, returning an action per press.
/// `typing` is whether a text field had the focus at the end of the last frame.
pub fn pressed(ctx: &Context, typing: bool) -> Vec<Action> {
    // Egui drops a field's focus on Escape before this frame runs, but the
    // field still needs the Escape, to cancel what was typed.
    if typing || blocked(ctx) {
        return Vec::new();
    }
    // A widget with the focus keeps the keys that move the focus on.
    let focused = ctx.memory(|memory| memory.focused().is_some());
    ctx.input_mut(|input| {
        let mut actions: Vec<_> = SHORTCUTS
            .into_iter()
            .filter(|&(modifiers, key, _)| !(focused && moves_focus(modifiers, key)))
            .flat_map(|(modifiers, key, action)| {
                // Counted, as a slow frame can get several key repeats.
                let presses = input.count_and_consume_key(modifiers, key);
                std::iter::repeat_n(action, presses)
            })
            .collect();
        // egui-winit sends Ctrl+X, Ctrl+C and Ctrl+V as these, not as keys.
        input.events.retain(|event| {
            actions.push(match event {
                Event::Cut => Action::Cut,
                Event::Copy => Action::Copy,
                Event::Paste(text) => Action::PasteText(text.clone()),
                _ => return true,
            });
            false
        });
        actions
    })
}

/// Takes this frame's press of Alt+F, which opens the File menu.
pub fn file_menu_pressed(ctx: &Context) -> bool {
    !blocked(ctx) && ctx.input_mut(|input| input.consume_key(Modifiers::ALT, Key::F))
}

/// How `action`'s shortcut reads, e.g. "Ctrl+O", if it has one.
pub fn text(ctx: &Context, action: &Action) -> Option<String> {
    let (modifiers, key, _) = SHORTCUTS.into_iter().find(|(.., a)| a == action)?;
    Some(ctx.format_shortcut(&KeyboardShortcut::new(modifiers, key)))
}

/// Whether egui moves the focus between widgets with the key: a plain arrow.
fn moves_focus(modifiers: Modifiers, key: Key) -> bool {
    let arrow = matches!(
        key,
        Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown
    );
    arrow && modifiers.is_none()
}

/// An open menu or modal dialog keeps the keys, e.g. Escape to close it, and
/// so does a text field being typed in.
fn blocked(ctx: &Context) -> bool {
    Popup::is_any_open(ctx)
        || ctx.memory(|mem| mem.top_modal_layer()).is_some()
        || ctx.text_edit_focused()
}

#[cfg(test)]
mod tests;
