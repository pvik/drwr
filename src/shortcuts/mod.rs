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
const SHORTCUTS: [(Modifiers, Key, Action); 34] = [
    (Modifiers::COMMAND, Key::N,          Action::New),
    (Modifiers::COMMAND, Key::O,          Action::Open),
    // Before Ctrl+S, which egui also matches with Shift held.
    (SHIFT_COMMAND,      Key::S,          Action::SaveAs),
    (Modifiers::COMMAND, Key::S,          Action::Save),
    (Modifiers::COMMAND, Key::E,          Action::Export),
    (Modifiers::COMMAND, Key::Q,          Action::Quit),
    // The menu shows Redo's first shortcut.
    (Modifiers::COMMAND, Key::Y,          Action::Redo),
    // Before Ctrl+Z, which egui also matches with Shift held.
    (SHIFT_COMMAND,      Key::Z,          Action::Redo),
    (Modifiers::COMMAND, Key::Z,          Action::Undo),
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
    (Modifiers::NONE,    Key::PageUp,     Action::Raise),
    (Modifiers::NONE,    Key::PageDown,   Action::Lower),
    (Modifiers::NONE,    Key::Escape,     Action::SelectTool(Tool::Cursor)),
    (Modifiers::NONE,    Key::B,          Action::SelectTool(Tool::Box)),
    (Modifiers::NONE,    Key::C,          Action::SelectTool(Tool::Circle)),
    (Modifiers::NONE,    Key::E,          Action::SelectTool(Tool::Ellipse)),
    (Modifiers::NONE,    Key::L,          Action::SelectTool(Tool::Line)),
    (Modifiers::NONE,    Key::A,          Action::SelectTool(Tool::Arrow)),
];

/// Takes this frame's presses of shortcuts, returning an action per press.
/// While typing, which `typing` says a text field was at the end of the last
/// frame, only Save, Save As and Quit, as text fields have no use for them.
pub fn pressed(ctx: &Context, typing: bool) -> Vec<Action> {
    if menu_or_dialog_open(ctx) {
        return Vec::new();
    }
    // Egui drops a field's focus on Escape before this frame runs, but the
    // field still needs the Escape, to cancel what was typed.
    let typing = typing || ctx.text_edit_focused();
    // A widget with the focus keeps the keys that move the focus on.
    let focused = ctx.memory(|memory| memory.focused().is_some());
    ctx.input_mut(|input| {
        let mut actions: Vec<_> = SHORTCUTS
            .into_iter()
            .filter(|(.., action)| {
                !typing || matches!(action, Action::Save | Action::SaveAs | Action::Quit)
            })
            .filter(|&(modifiers, key, _)| !(focused && moves_focus(modifiers, key)))
            .flat_map(|(modifiers, key, action)| {
                // Counted, as a slow frame can get several key repeats.
                let presses = input.count_and_consume_key(modifiers, key);
                std::iter::repeat_n(action, presses)
            })
            .collect();
        if typing {
            return actions;
        }
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
    let blocked = menu_or_dialog_open(ctx) || ctx.text_edit_focused();
    !blocked && ctx.input_mut(|input| input.consume_key(Modifiers::ALT, Key::F))
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

/// An open menu or modal dialog keeps the keys, e.g. Escape to close it.
fn menu_or_dialog_open(ctx: &Context) -> bool {
    Popup::is_any_open(ctx) || ctx.memory(|mem| mem.top_modal_layer()).is_some()
}

#[cfg(test)]
mod tests;
