use egui::accesskit::Role;

use super::*;
use crate::testing::{Drive, Driver};

/// Drives a dialog in an 800×600 window, one frame at a time.
struct Harness {
    driver: Driver,
    dialog: TextDialog<&'static str>,
    /// What the dialog returned, in order.
    accepted: Vec<(&'static str, String)>,
}

impl Drive for Harness {
    fn driver(&mut self) -> &mut Driver {
        &mut self.driver
    }

    fn show(&mut self, ui: &mut Ui) {
        self.accepted.extend(self.dialog.show(ui.ctx()));
    }
}

impl Harness {
    fn new() -> Self {
        Harness {
            driver: Driver::default(),
            dialog: TextDialog::default(),
            accepted: Vec::new(),
        }
    }

    /// Opens the dialog on `text` for a label.
    fn open(&mut self, text: &str) {
        self.dialog.open("label", "Label", text);
        // A new modal is laid out unseen first.
        self.frame(vec![]);
        self.frame(vec![]);
    }

    fn text(&self) -> &str {
        (self.driver.widgets.iter())
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
    harness.press(Key::Enter);
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
    harness.press_with(Modifiers::COMMAND, Key::Enter);
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
    harness.press(Key::Escape);
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

#[test]
fn long_texts_scroll_leaving_the_buttons_in_the_window() {
    let mut harness = Harness::new();
    harness.open(&["Line"; 100].join("\n"));
    let ok = |harness: &Harness| harness.driver.widget("OK").bounds().unwrap();
    let shown = ok(&harness);
    for _ in 0..10 {
        harness.frame(vec![]);
    }
    assert_eq!(ok(&harness), shown);
    assert!(shown.y1 <= 600.0, "{shown:?}");
}
