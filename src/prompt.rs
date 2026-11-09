use egui::{Context, Id, Modal};

/// Asks `question` in a modal dialog with a button per answer, the first
/// focused so Enter picks it, while Escape picks the last. Returns the answer
/// once given.
pub fn ask<A: Copy>(ctx: &Context, question: &str, answers: &[(&str, A)]) -> Option<A> {
    // Each question its own, as a reused area would show at the last one's
    // size for a frame.
    let id = Id::unique(("prompt", question));
    // Opaque from the first frame it's shown, unlike egui's fade-in.
    let area = Modal::default_area(id).fade_in(false);
    let mut answer = None;
    let modal = Modal::new(id).area(area).show(ctx, |ui| {
        ui.set_max_width(400.0);
        ui.label(question);
        ui.horizontal(|ui| {
            for (index, &(label, value)) in answers.iter().enumerate() {
                let button = ui.button(label);
                // So Enter picks it, and Tab moves on to the others.
                if index == 0 && ui.memory(|memory| memory.focused().is_none()) {
                    button.request_focus();
                }
                if button.clicked() {
                    answer = Some(value);
                }
            }
        });
    });
    let last = answers.last().map(|&(_, value)| value);
    answer.or(last.filter(|_| modal.should_close()))
}
