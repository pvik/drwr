use std::path::Path;

use crate::prompt;

/// What the user chose to do with the unsaved changes.
#[derive(Clone, Copy, Debug)]
pub enum Answer {
    Save,
    DontSave,
    Cancel,
}

/// Asks whether to save the changes to `file`, or else to the diagram
/// `imported` from a draw.io file, or else to a new diagram. Enter saves and
/// Escape cancels. Returns the answer once given.
pub fn show(ctx: &egui::Context, file: Option<&Path>, imported: Option<&Path>) -> Option<Answer> {
    let question = match (file.and_then(Path::file_name), imported) {
        (Some(name), _) => format!("Save changes to {}?", name.to_string_lossy()),
        (None, Some(imported)) => {
            let name = imported.file_name().unwrap_or_default().to_string_lossy();
            format!("Save changes to the diagram imported from {name}?")
        }
        (None, None) => "Save changes to the new diagram?".to_owned(),
    };
    let answers = [
        ("Save", Answer::Save),
        ("Don't Save", Answer::DontSave),
        ("Cancel", Answer::Cancel),
    ];
    prompt::ask(ctx, &question, &answers)
}
