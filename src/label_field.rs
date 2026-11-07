use egui::{Event, EventFilter, Key, KeyboardShortcut, Modifiers, Response, TextEdit, Ui};

/// How typing a label in a field ended.
pub enum End {
    Apply,
    Cancel,
}

/// A field for a label's text, one line high until Shift+Enter adds more.
/// See [`end`] for the keys that leave it.
pub fn text_edit(text: &mut String) -> TextEdit<'_> {
    TextEdit::multiline(text)
        .desired_rows(1)
        .return_key(KeyboardShortcut::new(Modifiers::SHIFT, Key::Enter))
        // Escape reaches `end` instead of just dropping the focus.
        .event_filter(EventFilter {
            horizontal_arrows: true,
            vertical_arrows: true,
            escape: true,
            ..EventFilter::default()
        })
}

/// Checks the focused `field` for Enter, which applies the label typed, or
/// Escape, which cancels it. Either drops the focus.
pub fn end(ui: &Ui, field: &Response) -> Option<End> {
    if !field.has_focus() {
        return None;
    }
    let end = ui.input(|input| {
        input.events.iter().find_map(|event| match event {
            Event::Key {
                key: Key::Enter,
                pressed: true,
                modifiers,
                ..
            } if !modifiers.shift => Some(End::Apply),
            Event::Key {
                key: Key::Escape,
                pressed: true,
                ..
            } => Some(End::Cancel),
            _ => None,
        })
    })?;
    field.surrender_focus();
    Some(end)
}
