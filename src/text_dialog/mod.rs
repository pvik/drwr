use egui::text::CCursorRange;
use egui::{Context, Id, Key, KeyboardShortcut, Modal, Modifiers, ScrollArea, TextEdit, Ui};

const FIELD_WIDTH: f32 = 320.0;
const FIELD_ROWS: usize = 6;
/// Beyond this, the field scrolls, so the buttons stay in the window.
const FIELD_MAX_HEIGHT: f32 = 300.0;

/// Ctrl+Enter, or Cmd+Enter on macOS, is OK; Enter starts a new line.
const OK: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Enter);

/// A modal dialog for editing text over several lines, for a target of type
/// `T`, which tells the caller what the text is for.
pub struct TextDialog<T> {
    dialog: Option<Dialog<T>>,
}

struct Dialog<T> {
    target: T,
    title: String,
    text: String,
    /// Whether the field has yet to take the focus.
    focus: bool,
}

impl<T> Default for TextDialog<T> {
    fn default() -> Self {
        TextDialog { dialog: None }
    }
}

impl<T> TextDialog<T> {
    pub fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    /// Opens the dialog on `text`, unless it's open already.
    pub fn open(&mut self, target: T, title: impl Into<String>, text: impl Into<String>) {
        if self.is_open() {
            return;
        }
        self.dialog = Some(Dialog {
            target,
            title: title.into(),
            text: text.into(),
            focus: true,
        });
    }

    /// Closes the dialog, dropping the text.
    pub fn close(&mut self) {
        self.dialog = None;
    }

    /// Shows the dialog while it's open. Returns its target and the text once
    /// OK is pressed.
    pub fn show(&mut self, ctx: &Context) -> Option<(T, String)> {
        let dialog = self.dialog.as_mut()?;
        let modal = Modal::new(Id::unique("text_dialog")).show(ctx, |ui| dialog.ui(ui));
        if modal.inner {
            let dialog = self.dialog.take()?;
            return Some((dialog.target, dialog.text));
        }
        if modal.should_close() {
            self.dialog = None;
        }
        None
    }
}

impl<T> Dialog<T> {
    /// Returns whether the text was accepted.
    fn ui(&mut self, ui: &mut Ui) -> bool {
        ui.strong(&self.title);
        let mut ok = ui.input_mut(|input| input.consume_shortcut(&OK));
        let id = ui.make_persistent_id("text");
        let field = ScrollArea::vertical()
            .max_height(FIELD_MAX_HEIGHT)
            .show(ui, |ui| {
                TextEdit::multiline(&mut self.text)
                    .id(id)
                    .desired_width(FIELD_WIDTH)
                    .desired_rows(FIELD_ROWS)
                    .show(ui)
            });
        // Selected, so typing replaces it.
        if std::mem::take(&mut self.focus) {
            let mut output = field.inner;
            output.response.response.request_focus();
            let all = CCursorRange::select_all_str(&self.text);
            output.state.cursor.set_char_range(Some(all));
            output.state.store(ui.ctx(), id);
        }
        ui.separator();
        ui.horizontal(|ui| {
            let shortcut = ui.ctx().format_shortcut(&OK);
            ok |= ui.button("OK").on_hover_text(shortcut).clicked();
            if ui.button("Cancel").clicked() {
                ui.close();
            }
        });
        ok
    }
}

#[cfg(test)]
mod tests;
