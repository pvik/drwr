use crate::prompt;

/// Shows `error` in a modal dialog until OK, Enter or Escape dismisses it.
pub fn show(ctx: &egui::Context, error: &mut Option<String>) {
    if let Some(message) = error
        && prompt::ask(ctx, message, &[("OK", ())]).is_some()
    {
        *error = None;
    }
}
