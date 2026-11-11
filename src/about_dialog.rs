use egui::{Context, Id, Modal};

/// Shows the About dialog while `open`, with the version, the commit that
/// `build.rs` embeds and a link to the project page, until Close, Enter or
/// Escape closes it.
pub fn show(ctx: &Context, open: &mut bool) {
    if !*open {
        return;
    }
    let id = Id::unique("about_dialog");
    // Opaque from the first frame it's shown, unlike egui's fade-in.
    let area = Modal::default_area(id).fade_in(false);
    let modal = Modal::new(id).area(area).show(ctx, |ui| {
        ui.strong("About drwr");
        ui.label(concat!("Version ", env!("CARGO_PKG_VERSION")));
        ui.label(concat!("Commit ", env!("DRWR_GIT_SHORT_COMMIT")));
        ui.hyperlink(env!("CARGO_PKG_HOMEPAGE"));
        ui.separator();
        let close = ui.button("Close");
        // So Enter closes it, and Tab moves on to the link.
        if ui.memory(|memory| memory.focused().is_none()) {
            close.request_focus();
        }
        close.clicked()
    });
    if modal.inner || modal.should_close() {
        *open = false;
    }
}
