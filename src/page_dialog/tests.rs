use egui::accesskit::Toggled;

use super::*;
use crate::testing::{self, Drive, Driver};

/// Drives a dialog in an 800×600 window, one frame at a time.
struct Harness {
    driver: Driver,
    dialog: PageDialog<&'static str>,
    /// What the dialog returned, in order.
    picked: Vec<(&'static str, usize)>,
}

impl Drive for Harness {
    fn driver(&mut self) -> &mut Driver {
        &mut self.driver
    }

    fn show(&mut self, ui: &mut Ui) {
        self.picked.extend(self.dialog.show(ui.ctx()));
    }
}

impl Harness {
    /// Opens the dialog on pages A, B and C of plan.drawio.
    fn new() -> Self {
        Harness::with_pages(&["A", "B", "C"])
    }

    fn with_pages(pages: &[&str]) -> Self {
        let mut harness = Harness {
            driver: Driver::default(),
            dialog: PageDialog::default(),
            picked: Vec::new(),
        };
        harness.open(pages);
        harness
    }

    fn open(&mut self, pages: &[&str]) {
        let pages = pages.iter().map(|&page| page.to_owned()).collect();
        self.dialog.open("plan", "plan.drawio", pages);
        // A new modal is laid out unseen first.
        self.frame(vec![]);
        self.frame(vec![]);
    }

    /// The pages listed, top down, and whether each is selected.
    fn rows(&self) -> Vec<(&str, bool)> {
        let mut rows: Vec<_> = (self.driver.widgets.iter())
            .filter(|widget| widget.toggled().is_some())
            .collect();
        rows.sort_by(|a, b| testing::center(a).y.total_cmp(&testing::center(b).y));
        (rows.into_iter())
            .map(|row| (row.label().unwrap(), row.toggled() == Some(Toggled::True)))
            .collect()
    }

    fn focused(&self) -> Option<&str> {
        self.driver
            .focused
            .as_ref()
            .and_then(|widget| widget.label())
    }

    /// Where the widget labelled or reading `text` is.
    fn bounds(&self, text: &str) -> egui::accesskit::Rect {
        let mut widgets = self.driver.widgets.iter();
        let widget = widgets.find(|w| w.label() == Some(text) || w.value() == Some(text));
        widget
            .and_then(|widget| widget.bounds())
            .expect("it's shown")
    }
}

#[test]
fn lists_the_pages_with_the_first_selected_and_open_focused() {
    let mut harness = Harness::new();
    assert!(harness.driver.painted("Open page"));
    assert!(
        harness
            .driver
            .painted("plan.drawio has 3 pages. Pick the one to open:")
    );
    assert_eq!(harness.rows(), [("A", true), ("B", false), ("C", false)]);
    let open = testing::center(harness.driver.widget("Open"));
    assert!(testing::center(harness.driver.widget("C")).y < open.y);
    assert_eq!(harness.focused(), Some("Open"));

    harness.press(Key::Enter);
    assert_eq!(harness.picked, [("plan", 0)]);
    assert!(!harness.dialog.is_open());
}

#[test]
fn up_and_down_move_the_selection_and_enter_opens_it() {
    let mut harness = Harness::new();
    for key in [Key::ArrowDown, Key::ArrowDown, Key::ArrowDown] {
        harness.press(key);
    }
    assert_eq!(harness.rows(), [("A", false), ("B", false), ("C", true)]);
    harness.press(Key::ArrowUp);
    assert_eq!(harness.rows(), [("A", false), ("B", true), ("C", false)]);
    // The focus stays put.
    assert_eq!(harness.focused(), Some("Open"));
    assert_eq!(harness.picked, []);

    harness.press(Key::Enter);
    assert_eq!(harness.picked, [("plan", 1)]);
}

#[test]
fn several_presses_in_a_frame_all_count() {
    let mut harness = Harness::new();
    let down = testing::key(Modifiers::NONE, Key::ArrowDown);
    harness.frame(vec![down.clone(), down]);
    harness.repaint();
    assert_eq!(harness.rows(), [("A", false), ("B", false), ("C", true)]);
}

#[test]
fn a_click_selects_a_page_and_a_double_click_opens_it() {
    let mut harness = Harness::new();
    harness.click("C");
    assert_eq!(harness.rows(), [("A", false), ("B", false), ("C", true)]);
    assert!(harness.dialog.is_open());
    // Enter still opens it.
    harness.press(Key::Enter);
    assert_eq!(harness.picked, [("plan", 2)]);

    harness.open(&["A", "B", "C"]);
    harness.click("B");
    harness.click("Open");
    harness.open(&["A", "B", "C"]);
    harness.double_click("C");
    assert_eq!(harness.picked, [("plan", 2), ("plan", 1), ("plan", 2)]);
}

#[test]
fn cancel_and_escape_open_nothing() {
    let mut harness = Harness::new();
    harness.press(Key::ArrowDown);
    harness.click("Cancel");
    assert!(!harness.dialog.is_open());

    harness.open(&["A", "B", "C"]);
    harness.press(Key::Escape);
    assert!(!harness.dialog.is_open());
    assert_eq!(harness.picked, []);
}

#[test]
fn tab_moves_to_cancel_which_enter_then_picks() {
    let mut harness = Harness::new();
    harness.press(Key::Tab);
    assert_eq!(harness.focused(), Some("Cancel"));
    harness.press(Key::ArrowDown);
    assert_eq!(harness.rows()[1], ("B", true));
    assert_eq!(harness.focused(), Some("Cancel"));
    harness.press(Key::Tab);
    assert_eq!(harness.focused(), Some("Open"));

    harness.press(Key::Tab);
    harness.press(Key::Enter);
    assert!(!harness.dialog.is_open());
    assert_eq!(harness.picked, []);
}

#[test]
fn long_lists_scroll_to_the_selected_page_leaving_the_buttons_in_the_window() {
    let pages: Vec<String> = (1..=40).map(|page| format!("Page-{page}")).collect();
    let pages: Vec<&str> = pages.iter().map(String::as_str).collect();
    let mut harness = Harness::with_pages(&pages);
    let down = testing::key(Modifiers::NONE, Key::ArrowDown);
    harness.frame(vec![down; 39]);
    // The scroll shows the frame after.
    harness.repaint();
    assert_eq!(harness.rows()[39], ("Page-40", true));

    let intro = harness.bounds("plan.drawio has 40 pages. Pick the one to open:");
    let open = harness.bounds("Open");
    assert!(open.y1 <= 600.0, "{open:?}");
    let last = harness.bounds("Page-40");
    assert!(last.y0 > intro.y1 && last.y1 < open.y0, "{last:?}");
    // Scrolled out of view, above the list.
    assert!(harness.bounds("Page-1").y1 < intro.y1);
}

#[test]
fn opening_an_open_dialog_changes_nothing() {
    let mut harness = Harness::new();
    harness
        .dialog
        .open("other", "other.drawio", vec!["X".to_owned()]);
    harness.repaint();
    assert!(
        !harness
            .driver
            .painted("other.drawio has 1 pages. Pick the one to open:")
    );
    harness.press(Key::Enter);
    assert_eq!(harness.picked, [("plan", 0)]);
}
