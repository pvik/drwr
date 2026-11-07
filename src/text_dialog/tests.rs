use egui::accesskit::{self, Role};
use egui::{Event, PointerButton, Pos2, RawInput, Rect, pos2, vec2};

use super::*;

/// Drives a dialog in an 800×600 window, one frame at a time.
struct Harness {
    ctx: Context,
    dialog: TextDialog<&'static str>,
    /// What the dialog returned, in order.
    accepted: Vec<(&'static str, String)>,
    time: f64,
    /// The widgets the last frame showed.
    widgets: Vec<accesskit::Node>,
}

impl Harness {
    fn new() -> Self {
        let ctx = Context::default();
        ctx.enable_accesskit();
        Harness {
            ctx,
            dialog: TextDialog::default(),
            accepted: Vec::new(),
            time: 0.0,
            widgets: Vec::new(),
        }
    }

    /// Opens the dialog on `text` for a label.
    fn open(&mut self, text: &str) {
        self.dialog.open("label", "Label", text);
        // A new modal is laid out unseen first.
        self.frame(vec![]);
        self.frame(vec![]);
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.time += 0.1;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            time: Some(self.time),
            events,
            ..RawInput::default()
        };
        let mut output = self.ctx.run_ui(input, |ui| {
            self.accepted.extend(self.dialog.show(ui.ctx()));
        });
        let update = output.platform_output.accesskit_update.take();
        self.widgets = update.map_or_else(Vec::new, |update| {
            update.nodes.into_iter().map(|(_, node)| node).collect()
        });
        output.drop_without_applying_deltas();
    }

    fn click(&mut self, label: &str) {
        let widget = self
            .widgets
            .iter()
            .find(|widget| widget.label() == Some(label))
            .unwrap_or_else(|| panic!("no widget is labelled {label:?}"));
        let bounds = widget.bounds().expect("widgets have bounds");
        let at = pos2(
            ((bounds.x0 + bounds.x1) / 2.0) as f32,
            ((bounds.y0 + bounds.y1) / 2.0) as f32,
        );
        self.frame(vec![Event::PointerMoved(at)]);
        for pressed in [true, false] {
            self.frame(vec![Event::PointerButton {
                pos: at,
                button: PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            }]);
        }
        self.frame(vec![]);
    }

    fn press(&mut self, modifiers: Modifiers, key: Key) {
        self.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }]);
        self.frame(vec![]);
    }

    fn type_text(&mut self, text: &str) {
        self.frame(vec![Event::Text(text.to_owned())]);
        self.frame(vec![]);
    }

    fn text(&self) -> &str {
        self.widgets
            .iter()
            .find(|widget| widget.role() == Role::MultilineTextInput)
            .and_then(|widget| widget.value())
            .expect("the text field is shown")
    }
}

#[test]
fn opens_with_the_text_selected() {
    let mut harness = Harness::new();
    harness.open("Start");
    assert_eq!(harness.text(), "Start");
    harness.type_text("Stop");
    harness.click("OK");
    assert_eq!(harness.accepted, [("label", "Stop".to_owned())]);
    assert!(!harness.dialog.is_open());
}

#[test]
fn enter_starts_a_new_line() {
    let mut harness = Harness::new();
    harness.open("");
    harness.type_text("One");
    harness.press(Modifiers::NONE, Key::Enter);
    harness.type_text("Two");
    assert!(harness.dialog.is_open());
    harness.click("OK");
    assert_eq!(harness.accepted, [("label", "One\nTwo".to_owned())]);
}

#[test]
fn ctrl_enter_is_ok() {
    let mut harness = Harness::new();
    harness.open("Start");
    harness.type_text("Stop");
    harness.press(Modifiers::COMMAND, Key::Enter);
    assert_eq!(harness.accepted, [("label", "Stop".to_owned())]);
    assert!(!harness.dialog.is_open());
}

#[test]
fn cancel_and_escape_drop_the_text() {
    let mut harness = Harness::new();
    harness.open("Start");
    harness.type_text("Stop");
    harness.click("Cancel");
    assert!(!harness.dialog.is_open());

    harness.open("Start");
    harness.type_text("Stop");
    harness.press(Modifiers::NONE, Key::Escape);
    assert!(!harness.dialog.is_open());
    assert_eq!(harness.accepted, []);
}

#[test]
fn close_drops_the_text() {
    let mut harness = Harness::new();
    harness.open("Start");
    harness.dialog.close();
    harness.frame(vec![]);
    assert!(!harness.dialog.is_open());
    assert_eq!(harness.accepted, []);
}

#[test]
fn opening_an_open_dialog_changes_nothing() {
    let mut harness = Harness::new();
    harness.open("Start");
    harness.dialog.open("other", "Other", "Other");
    harness.click("OK");
    assert_eq!(harness.accepted, [("label", "Start".to_owned())]);
}
