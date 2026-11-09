/// What the user chose to do with the unsaved changes.
#[derive(Clone, Copy, Debug)]
pub enum Answer {
    Save,
    DontSave,
    Cancel,
}

/// Asks `question` in a modal dialog with Save, Don't Save and Cancel buttons,
/// where Escape cancels. Returns the answer once given.
pub fn show(ui: &mut egui::Ui, question: &str) -> Option<Answer> {
    let mut answer = None;
    let modal = egui::Modal::new(egui::Id::unique("unsaved_dialog")).show(ui.ctx(), |ui| {
        ui.set_max_width(400.0);
        ui.label(question);
        ui.horizontal(|ui| {
            let save = ui.button("Save");
            // So Enter saves, and Tab moves on to the other buttons.
            if ui.memory(|memory| memory.focused().is_none()) {
                save.request_focus();
            }
            for (response, chosen) in [
                (save, Answer::Save),
                (ui.button("Don't Save"), Answer::DontSave),
                (ui.button("Cancel"), Answer::Cancel),
            ] {
                if response.clicked() {
                    answer = Some(chosen);
                }
            }
        });
    });
    answer.or(modal.should_close().then_some(Answer::Cancel))
}
