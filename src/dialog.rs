use egui::{Context, Id, Modal, Ui};

/// Shows `dialog`, if open, as a modal that doesn't fade in, laid out by
/// `add_contents`, which returns whether it was accepted. Closes it when
/// cancelled, and returns it once accepted.
pub fn show<D>(
    ctx: &Context,
    id: &str,
    dialog: &mut Option<D>,
    add_contents: impl FnOnce(&mut D, &mut Ui) -> bool,
) -> Option<D> {
    let open = dialog.as_mut()?;
    let id = Id::unique(id);
    let modal = Modal::new(id)
        .area(Modal::default_area(id).fade_in(false))
        .show(ctx, |ui| add_contents(open, ui));
    if modal.inner {
        return dialog.take();
    }
    if modal.should_close() {
        *dialog = None;
    }
    None
}

/// Adds a separator, then OK, which shows `ok_hint` on hover, and Cancel,
/// which closes the dialog. Returns whether OK was clicked.
pub fn ok_cancel(ui: &mut Ui, ok_hint: Option<&str>) -> bool {
    ui.separator();
    ui.horizontal(|ui| {
        let mut ok = ui.button("OK");
        if let Some(hint) = ok_hint {
            ok = ok.on_hover_text(hint);
        }
        if ui.button("Cancel").clicked() {
            ui.close();
        }
        ok.clicked()
    })
    .inner
}
