use egui::{Context, Id, Modal};

/// As wide as a prompt at most.
const MAX_WIDTH: f32 = 400.0;

/// What converting a draw.io file to GraphML couldn't carry over, for the
/// notes dialog to tell.
pub struct Notes {
    /// The draw.io file's name.
    file: String,
    /// A sentence each.
    notes: Vec<String>,
    /// Whether OK has yet to take the focus.
    focus: bool,
}

impl Notes {
    pub fn new(file: String, notes: Vec<String>) -> Self {
        Notes {
            file,
            notes,
            focus: true,
        }
    }
}

/// Shows `notes`, if any, in a modal dialog until OK, Enter or Escape closes
/// it.
pub fn show(ctx: &Context, notes: &mut Option<Notes>) {
    let Some(shown) = notes else {
        return;
    };
    // Each its own, as a reused area would show at the last one's size for a
    // frame.
    let id = Id::unique(("notes_dialog", &shown.file, &shown.notes));
    // Opaque from the first frame it's shown, unlike egui's fade-in.
    let area = Modal::default_area(id).fade_in(false);
    let modal = Modal::new(id).area(area).show(ctx, |ui| {
        ui.set_max_width(MAX_WIDTH);
        ui.strong("Opened with changes");
        ui.label(format!(
            "{} was converted to GraphML. Some things couldn't be carried over:",
            shown.file
        ));
        for note in &shown.notes {
            ui.horizontal_top(|ui| {
                ui.label("•");
                // So that it wraps beside the bullet.
                ui.vertical(|ui| ui.label(note));
            });
        }
        ui.separator();
        let ok = ui.button("OK");
        // So Enter closes it, though the page dialog may still have the focus.
        if std::mem::take(&mut shown.focus) || ui.memory(|memory| memory.focused().is_none()) {
            ok.request_focus();
        }
        ok.clicked()
    });
    if modal.inner || modal.should_close() {
        *notes = None;
    }
}
