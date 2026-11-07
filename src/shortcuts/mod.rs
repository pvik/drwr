use egui::{Context, Key, KeyboardShortcut, Modifiers, Popup};

use crate::action::Action;
use crate::tool::Tool;

/// How far Ctrl+arrow moves the selected node, in points.
const STEP: f64 = 1.0;

/// `COMMAND` is Ctrl, or Cmd on macOS.
#[rustfmt::skip]
const SHORTCUTS: [(Modifiers, Key, Action); 15] = [
    (Modifiers::COMMAND, Key::N,          Action::New),
    (Modifiers::COMMAND, Key::O,          Action::Open),
    (Modifiers::COMMAND, Key::S,          Action::Save),
    (Modifiers::COMMAND, Key::Enter,      Action::EditLabel),
    (Modifiers::COMMAND, Key::ArrowLeft,  Action::MoveBy(-STEP, 0.0)),
    (Modifiers::COMMAND, Key::ArrowRight, Action::MoveBy(STEP, 0.0)),
    (Modifiers::COMMAND, Key::ArrowUp,    Action::MoveBy(0.0, -STEP)),
    (Modifiers::COMMAND, Key::ArrowDown,  Action::MoveBy(0.0, STEP)),
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
    ctx.input_mut(|input| {
        SHORTCUTS
            .into_iter()
            .flat_map(|(modifiers, key, action)| {
                // Counted, as a slow frame can get several key repeats.
                let presses = input.count_and_consume_key(modifiers, key);
                std::iter::repeat_n(action, presses)
            })
            .collect()
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

/// An open menu or modal dialog keeps the keys, e.g. Escape to close it, and
/// so does a text field being typed in.
fn blocked(ctx: &Context) -> bool {
    Popup::is_any_open(ctx)
        || ctx.memory(|mem| mem.top_modal_layer()).is_some()
        || ctx.text_edit_focused()
}

#[cfg(test)]
mod tests;
