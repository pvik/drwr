use std::sync::{Arc, Mutex};

use egui::accesskit::{Node, Role, Toggled};
use egui::{FallbackRequest, FontInsert, Key, Modifiers, pos2, vec2};

use super::*;
use crate::testing::{Drive, Driver, center};

/// 100×50 points at the origin.
const EVERYTHING: Rect = Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 50.0));
const SELECTED: Rect = Rect::from_min_max(pos2(10.0, 10.0), pos2(30.0, 30.0));

/// Drives an export dialog in an 800×600 window, one frame at a time.
struct Harness {
    driver: Driver,
    dialog: ExportDialog,
    /// What the dialog was accepted with, in order.
    accepted: Vec<Accepted>,
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
    /// Opens a dialog on `settings`, over a drawing that covers
    /// [`EVERYTHING`] and has [`SELECTED`] selected, if `selection`.
    fn open(settings: Settings, selection: bool) -> Self {
        Harness::open_on(settings, EVERYTHING, selection.then_some(SELECTED))
    }

    fn open_on(settings: Settings, everything: Rect, selected: Option<Rect>) -> Self {
        Harness::open_in(Driver::default(), settings, everything, selected)
    }

    fn open_in(
        driver: Driver,
        settings: Settings,
        everything: Rect,
        selected: Option<Rect>,
    ) -> Self {
        let mut harness = Harness {
            driver,
            dialog: ExportDialog::default(),
            accepted: Vec::new(),
        };
        harness.dialog.open(settings, everything, selected);
        // A new modal is laid out unseen first.
        harness.frame(vec![]);
        harness.frame(vec![]);
        harness
    }

    fn checked(&self, label: &str) -> bool {
        self.driver.widget(label).toggled() == Some(Toggled::True)
    }

    fn enabled(&self, label: &str) -> bool {
        !self.driver.widget(label).is_disabled()
    }

    fn shown(&self, label: &str) -> bool {
        (self.driver.widgets.iter()).any(|widget| widget.label() == Some(label))
    }

    /// The number fields, top down.
    fn fields(&self) -> Vec<&Node> {
        let mut fields: Vec<_> = (self.driver.widgets.iter())
            .filter(|widget| widget.role() == Role::SpinButton)
            .collect();
        fields.sort_by(|a, b| center(a).y.total_cmp(&center(b).y));
        fields
    }

    /// Types `value` in the `index`th number field, top down.
    fn set_field(&mut self, index: usize, value: &str) {
        let at = center(self.fields()[index]);
        self.click_at(at, 1);
        self.press_with(Modifiers::COMMAND, Key::A);
        self.type_text(value);
        self.press(Key::Enter);
    }
}

#[test]
fn entire_document_is_chosen_and_selected_area_only_needs_a_selection() {
    let harness = Harness::open(Settings::default(), false);
    assert!(harness.checked("Entire document"));
    assert!(!harness.checked("Selected area only"));
    assert!(!harness.enabled("Selected area only"));

    let mut harness = Harness::open(Settings::default(), true);
    assert!(harness.checked("Entire document"));
    assert!(harness.enabled("Selected area only"));
    harness.click("Selected area only");
    assert!(harness.checked("Selected area only"));
    assert!(!harness.checked("Entire document"));
    harness.click("Export…");
    assert_eq!(
        harness.accepted,
        [Accepted {
            settings: Settings::default(),
            selected_only: true,
        }]
    );
    assert!(!harness.dialog.is_open());
}

#[test]
fn the_options_shown_are_the_formats() {
    let mut harness = Harness::open(Settings::default(), false);
    assert!(harness.checked("PNG"));
    assert!(harness.driver.painted("PNG options"));
    // Scale and margin.
    assert_eq!(harness.fields().len(), 2);
    assert!(harness.shown("Transparent background"));
    assert!(!harness.shown("Text as outlines"));

    harness.click("SVG");
    assert!(harness.checked("SVG"));
    assert!(harness.driver.painted("SVG options"));
    // Margin only.
    assert_eq!(harness.fields().len(), 1);
    assert!(harness.shown("Transparent background"));
    assert!(harness.shown("Text as outlines"));
    assert!(!harness.driver.painted("120 × 70 px"));
}

#[test]
fn fields_are_named_by_the_labels_beside_them() {
    let harness = Harness::open(Settings::default(), false);
    for field in harness.fields() {
        // Not by their units.
        assert_eq!(field.label(), None);
        assert_eq!(field.labelled_by().len(), 1);
    }
}

#[test]
fn the_size_in_pixels_follows_the_area_scale_and_margin() {
    let mut harness = Harness::open(Settings::default(), true);
    // 100×50 with a margin of 10 all round.
    assert!(harness.driver.painted("120 × 70 px"));
    harness.click("Selected area only");
    assert!(harness.driver.painted("40 × 40 px"));
    harness.set_field(0, "250");
    assert!(harness.driver.painted("100 × 100 px"));
    harness.set_field(1, "0");
    assert!(harness.driver.painted("50 × 50 px"));
    harness.click("Export…");
    let settings = &harness.accepted[0].settings;
    assert_eq!((settings.scale, settings.margin), (250, 0));
}

#[test]
fn export_returns_the_options_set() {
    let mut harness = Harness::open(Settings::default(), false);
    harness.click("SVG");
    harness.click("Transparent background");
    harness.click("Text as outlines");
    harness.set_field(0, "4");
    harness.click("Export…");
    assert_eq!(
        harness.accepted,
        [Accepted {
            settings: Settings {
                format: Format::Svg,
                margin: 4,
                transparent: true,
                scale: 100,
                outlines: true,
            },
            selected_only: false,
        }]
    );
}

#[test]
fn it_opens_on_the_settings_given() {
    let settings = Settings {
        format: Format::Svg,
        margin: 5,
        transparent: true,
        scale: 300,
        outlines: true,
    };
    let mut harness = Harness::open(settings.clone(), false);
    assert!(harness.checked("SVG"));
    assert!(harness.checked("Transparent background"));
    assert!(harness.checked("Text as outlines"));
    assert_eq!(harness.fields()[0].numeric_value(), Some(5.0));
    harness.click("PNG");
    assert_eq!(harness.fields()[0].numeric_value(), Some(300.0));
    // 100×50 with a margin of 5, tripled.
    assert!(harness.driver.painted("330 × 180 px"));
}

#[test]
fn enter_exports() {
    let mut harness = Harness::open(Settings::default(), false);
    harness.press(Key::Enter);
    assert_eq!(harness.accepted.len(), 1);
}

#[test]
fn cancel_and_escape_close_without_exporting() {
    let mut harness = Harness::open(Settings::default(), false);
    harness.click("Cancel");
    assert!(!harness.dialog.is_open());
    assert!(harness.accepted.is_empty());
    let mut harness = Harness::open(Settings::default(), false);
    harness.press(Key::Escape);
    assert!(!harness.dialog.is_open());
    assert!(harness.accepted.is_empty());
}

#[test]
fn nothing_to_export_leaves_export_disabled() {
    let mut harness = Harness::open_on(Settings::default(), Rect::NOTHING, None);
    assert!(harness.driver.painted("Nothing to export"));
    assert!(!harness.enabled("Export…"));
    harness.press(Key::Enter);
    harness.click("Export…");
    assert!(harness.accepted.is_empty());
}

#[test]
fn pngs_over_the_limit_cant_be_exported() {
    let huge = Rect::from_min_size(pos2(0.0, 0.0), vec2(20_000.0, 10_000.0));
    let mut harness = Harness::open_on(Settings::default(), huge, None);
    assert!(
        harness
            .driver
            .painted("Too large: the most is 100 megapixels")
    );
    assert!(!harness.enabled("Export…"));
    harness.click("SVG");
    assert!(harness.enabled("Export…"));
    harness.click("PNG");
    harness.set_field(0, "50");
    assert!(harness.driver.painted("10010 × 5010 px"));
    assert!(harness.enabled("Export…"));
}

#[test]
fn opening_it_again_while_open_keeps_it() {
    let mut harness = Harness::open(Settings::default(), true);
    harness.click("Selected area only");
    harness.dialog.open(Settings::default(), EVERYTHING, None);
    harness.repaint();
    assert!(harness.checked("Selected area only"));
}

/// Finding a glyph among the system's fonts can take most of a second.
#[test]
fn the_dialog_needs_no_system_fonts() {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let driver = Driver::default();
    let record = asked.clone();
    driver.ctx.add_font_provider(Arc::new(
        move |request: &FallbackRequest<'_>| -> Option<FontInsert> {
            record.lock().unwrap().push(request.cluster.to_owned());
            None
        },
    ));
    // Too large, to show that too.
    let huge = Rect::from_min_size(pos2(0.0, 0.0), vec2(20_000.0, 10_000.0));
    let mut harness = Harness::open_in(driver, Settings::default(), huge, Some(SELECTED));
    assert!(harness.driver.painted("20020 × 10020 px"));
    harness.click("SVG");
    assert_eq!(*asked.lock().unwrap(), Vec::<String>::new());
}

#[test]
fn the_dialog_shows_where_it_stays() {
    let mut harness = Harness {
        driver: Driver::default(),
        dialog: ExportDialog::default(),
        accepted: Vec::new(),
    };
    harness
        .dialog
        .open(Settings::default(), EVERYTHING, Some(SELECTED));
    // Laid out unseen first.
    harness.frame(vec![]);
    let mut shown = Vec::new();
    for _ in 0..3 {
        harness.frame(vec![]);
        shown.push(center(harness.driver.widget("Export…")));
    }
    assert!(shown.iter().all(|&at| at == shown[0]), "{shown:?}");
}
