use egui::accesskit::{self, Role, Toggled};
use egui::{Event, Popup, Rect, UiBuilder, pos2};

use super::*;
use crate::testing::{Drive, Driver, center};

/// Drives a picker 200 points wide in an 800×600 window, one frame at a time.
struct Harness {
    driver: Driver,
    picker: FontPicker,
    current: String,
    families: Vec<String>,
    /// What the picker returned, in order.
    picked: Vec<String>,
    /// How many times it asked for the families.
    listed: usize,
}

impl Drive for Harness {
    fn driver(&mut self) -> &mut Driver {
        &mut self.driver
    }

    fn show(&mut self, ui: &mut Ui) {
        let rect = Rect::from_min_size(pos2(20.0, 20.0), vec2(200.0, 20.0));
        ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
            let families = || {
                self.listed += 1;
                self.families.clone()
            };
            let picked = self.picker.show(ui, "font", &self.current, families);
            self.picked.extend(picked);
        });
    }
}

impl Harness {
    fn new(current: &str, families: &[&str]) -> Self {
        let mut harness = Harness {
            driver: Driver::default(),
            picker: FontPicker::default(),
            current: current.to_owned(),
            families: families.iter().map(|&family| family.to_owned()).collect(),
            picked: Vec::new(),
            listed: 0,
        };
        harness.frame(vec![]);
        harness
    }

    /// With families "Family 0" to "Family 99".
    fn with_many(current: &str) -> Self {
        let families: Vec<String> = (0..100).map(|n| format!("Family {n}")).collect();
        let families: Vec<&str> = families.iter().map(String::as_str).collect();
        Harness::new(current, &families)
    }

    /// Clicks the dropdown, then waits for its list to show.
    fn open(&mut self) {
        self.click_where(|widget| widget.role() == Role::ComboBox);
        // A new popup is laid out unseen first.
        self.frame(vec![]);
        self.frame(vec![]);
    }

    fn click_family(&mut self, family: &str) {
        self.click_where(|widget| widget.role() == Role::Button && widget.label() == Some(family));
    }

    /// Clicks the first widget `find` accepts.
    fn click_where(&mut self, find: impl Fn(&accesskit::Node) -> bool) {
        let widget = self.driver.widgets.iter().find(|widget| find(widget));
        let at = center(widget.expect("the widget is shown"));
        self.click_at(at, 1);
    }

    fn is_open(&self) -> bool {
        Popup::is_any_open(&self.driver.ctx)
    }

    /// The families listed in view, top to bottom.
    fn listed(&self) -> Vec<&str> {
        let mut rows: Vec<_> = self.rows().collect();
        rows.sort_by(|a, b| a.bounds().unwrap().y0.total_cmp(&b.bounds().unwrap().y0));
        rows.into_iter().filter_map(|row| row.label()).collect()
    }

    fn highlighted(&self) -> Option<&str> {
        let mut highlighted = self
            .rows()
            .filter(|row| row.toggled() == Some(Toggled::True));
        highlighted.next().and_then(|row| row.label())
    }

    fn rows(&self) -> impl Iterator<Item = &accesskit::Node> {
        (self.driver.widgets.iter()).filter(|widget| widget.role() == Role::Button)
    }

    fn shows(&self, text: &str) -> bool {
        (self.driver.widgets.iter()).any(|widget| widget.value() == Some(text))
    }
}

const FAMILIES: [&str; 7] = [
    "Dialog",
    "DialogInput",
    "Monospaced",
    "Noto Sans",
    "Noto Sans Mono",
    "Noto Serif",
    "Serif",
];

#[test]
fn opens_with_the_current_family_highlighted_and_the_field_focused() {
    let mut harness = Harness::new("Noto Sans", &FAMILIES);
    assert!(harness.listed().is_empty());
    harness.open();
    assert_eq!(harness.listed(), FAMILIES);
    assert_eq!(harness.highlighted(), Some("Noto Sans"));
    assert!(harness.driver.ctx.text_edit_focused());
}

#[test]
fn typing_narrows_the_families_to_those_with_every_word() {
    let mut harness = Harness::new("Noto Sans", &FAMILIES);
    harness.open();
    harness.type_text("MONO");
    assert_eq!(harness.listed(), ["Monospaced", "Noto Sans Mono"]);
    assert_eq!(harness.highlighted(), Some("Monospaced"));

    harness.press(Key::Backspace);
    harness.type_text(" noto");
    assert_eq!(harness.listed(), ["Noto Sans Mono"]);
    assert!(harness.is_open());
    assert_eq!(harness.picked, Vec::<String>::new());
}

#[test]
fn enter_picks_the_highlighted_family() {
    let mut harness = Harness::new("Noto Sans", &FAMILIES);
    harness.open();
    harness.type_text("mono");
    harness.press(Key::ArrowDown);
    assert_eq!(harness.highlighted(), Some("Noto Sans Mono"));
    harness.press(Key::Enter);
    assert_eq!(harness.picked, ["Noto Sans Mono"]);
    assert!(!harness.is_open());
}

#[test]
fn arrows_move_the_highlight_within_the_families_listed() {
    let mut harness = Harness::new("Dialog", &FAMILIES);
    harness.open();
    harness.press(Key::ArrowUp);
    assert_eq!(harness.highlighted(), Some("Dialog"));
    for _ in 0..10 {
        harness.press(Key::ArrowDown);
    }
    assert_eq!(harness.highlighted(), Some("Serif"));
    harness.press(Key::ArrowUp);
    assert_eq!(harness.highlighted(), Some("Noto Serif"));
    // The arrows moved no cursor through the field.
    harness.type_text("x");
    assert!(harness.shows("x"));
}

#[test]
fn every_arrow_press_counts_even_in_one_frame() {
    let mut harness = Harness::new("Dialog", &FAMILIES);
    harness.open();
    let down = Event::Key {
        key: Key::ArrowDown,
        physical_key: None,
        pressed: true,
        repeat: true,
        modifiers: Modifiers::NONE,
    };
    harness.frame(vec![down.clone(), down.clone(), down]);
    assert_eq!(harness.highlighted(), Some("Noto Sans"));
}

#[test]
fn clicking_a_family_picks_it() {
    let mut harness = Harness::new("Noto Sans", &FAMILIES);
    harness.open();
    harness.click_family("Serif");
    assert_eq!(harness.picked, ["Serif"]);
    assert!(!harness.is_open());
}

#[test]
fn clicking_the_field_keeps_the_list_open() {
    let mut harness = Harness::new("Noto Sans", &FAMILIES);
    harness.open();
    harness.click_where(|widget| widget.role() == Role::TextInput);
    assert!(harness.is_open());
}

#[test]
fn escape_closes_the_list_without_picking() {
    let mut harness = Harness::new("Noto Sans", &FAMILIES);
    harness.open();
    harness.type_text("mono");
    harness.press(Key::Escape);
    assert!(!harness.is_open());
    assert_eq!(harness.picked, Vec::<String>::new());

    // Reopened afresh.
    harness.open();
    assert_eq!(harness.listed(), FAMILIES);
    assert_eq!(harness.highlighted(), Some("Noto Sans"));
}

#[test]
fn picking_the_current_family_closes_the_list_without_picking() {
    let mut harness = Harness::new("Noto Sans", &FAMILIES);
    harness.open();
    harness.press(Key::Enter);
    assert!(!harness.is_open());
    assert_eq!(harness.picked, Vec::<String>::new());
}

#[test]
fn enter_picks_nothing_when_nothing_matches() {
    let mut harness = Harness::new("Noto Sans", &FAMILIES);
    harness.open();
    harness.type_text("wingdings");
    assert!(harness.listed().is_empty());
    assert!(harness.shows("No matches"));
    harness.press(Key::Enter);
    assert!(harness.is_open());
    assert_eq!(harness.picked, Vec::<String>::new());
}

#[test]
fn the_current_family_is_highlighted_whatever_its_case() {
    let mut harness = Harness::new("noto sans", &FAMILIES);
    harness.open();
    assert_eq!(harness.highlighted(), Some("Noto Sans"));
}

#[test]
fn a_current_family_not_offered_is_shown_but_not_highlighted() {
    let mut harness = Harness::new("Noto Mono", &FAMILIES);
    assert!(harness.shows("Noto Mono"));
    harness.open();
    assert_eq!(harness.highlighted(), None);
    harness.press(Key::ArrowDown);
    assert_eq!(harness.highlighted(), Some("Dialog"));
}

#[test]
fn the_families_are_listed_once_per_opening() {
    let mut harness = Harness::new("Noto Sans", &FAMILIES);
    harness.frame(vec![]);
    assert_eq!(harness.listed, 0);
    harness.open();
    harness.type_text("mono");
    assert_eq!(harness.listed, 1);
    harness.press(Key::Escape);
    harness.open();
    assert_eq!(harness.listed, 2);
}

#[test]
fn the_list_opens_with_the_current_family_in_the_middle() {
    let mut harness = Harness::with_many("Family 80");
    harness.open();
    let listed = harness.listed();
    for family in ["Family 75", "Family 80", "Family 85"] {
        assert!(listed.contains(&family), "{listed:?}");
    }
}

#[test]
fn moving_the_highlight_scrolls_only_as_far_as_it_takes() {
    let mut harness = Harness::with_many("Family 80");
    harness.open();
    let before: Vec<String> = harness.listed().iter().map(|&f| f.to_owned()).collect();
    for _ in 0..3 {
        harness.press(Key::ArrowDown);
    }
    assert_eq!(harness.highlighted(), Some("Family 83"));
    assert_eq!(harness.listed(), before);

    for _ in 0..43 {
        harness.press(Key::ArrowUp);
    }
    assert_eq!(harness.highlighted(), Some("Family 40"));
    // At the top, the rest below it.
    let listed = harness.listed();
    assert!(listed.contains(&"Family 50"), "{listed:?}");
    assert!(!listed.contains(&"Family 38"), "{listed:?}");
}

#[test]
fn narrowing_a_scrolled_list_shows_the_matches_at_once() {
    let mut harness = Harness::with_many("Family 80");
    harness.open();
    harness.frame(vec![Event::Text("family 1".to_owned())]);
    let listed = harness.listed();
    assert_eq!(harness.highlighted(), Some("Family 1"));
    assert!(listed.starts_with(&["Family 1", "Family 10"]), "{listed:?}");
}
