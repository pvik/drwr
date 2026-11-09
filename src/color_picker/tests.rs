use std::sync::{Arc, Mutex};

use egui::accesskit::{self, Toggled};
use egui::{FallbackRequest, FontInsert, Id, Modal, Shape};

use super::*;
use crate::testing::{Drive, Driver, center};

const ORANGE: Color = Color::rgb(0xFF, 0xCC, 0x00);
const RED: Color = Color::rgb(0xFF, 0x00, 0x00);

/// Drives a picker in an 800×600 window, one frame at a time.
struct Harness {
    driver: Driver,
    picker: ColorPicker<&'static str>,
    /// What the picker returned, in order.
    picked: Vec<(&'static str, Option<Color>)>,
}

impl Drive for Harness {
    fn driver(&mut self) -> &mut Driver {
        &mut self.driver
    }

    fn show(&mut self, ui: &mut Ui) {
        self.picked.extend(self.picker.show(ui.ctx()));
    }
}

impl Harness {
    fn new() -> Self {
        Harness {
            driver: Driver::default(),
            picker: ColorPicker::default(),
            picked: Vec::new(),
        }
    }

    /// Opens the picker on `color` for the fill.
    fn open(&mut self, color: Option<Color>) {
        self.picker.open("fill", "Fill color", color, true);
        // A new modal is laid out unseen first.
        self.frame(vec![]);
        self.frame(vec![]);
    }

    /// Opens the picker and picks `hex` with the keyboard.
    fn pick(&mut self, hex: &str) {
        self.open(Some(ORANGE));
        self.type_text(hex);
        self.press(Key::Enter);
    }

    /// Clicks the `index`th recent color `clicks` times in a row.
    fn click_recent(&mut self, index: usize, clicks: usize) {
        let at = center(self.recent_swatches()[index]);
        self.click_at(at, clicks);
    }

    fn hex(&self) -> &str {
        (self.driver.widgets.iter())
            // A number field being typed in is one too, labelled.
            .find(|widget| widget.role() == Role::TextInput && widget.label().is_none())
            .and_then(|widget| widget.value())
            .expect("the hex field is shown")
    }

    /// The value of the number field labelled `label`, e.g. "R".
    fn number(&self, label: &str) -> Option<f64> {
        self.driver.widget(label).numeric_value()
    }

    fn no_color(&self) -> bool {
        self.driver.widget("No color").toggled() == Some(Toggled::True)
    }

    /// The hex codes of the recent colors offered, in reading order.
    fn recent(&self) -> Vec<&str> {
        let swatches = self.recent_swatches().into_iter();
        swatches.map(|swatch| swatch.label().unwrap()).collect()
    }

    /// The swatches of the recent colors, in reading order: those below
    /// their heading, unlike the palette's.
    fn recent_swatches(&self) -> Vec<&accesskit::Node> {
        let widgets = &self.driver.widgets;
        let heading = (widgets.iter())
            .find(|widget| widget.value() == Some("Recent colors"))
            .and_then(|widget| widget.bounds())
            .expect("the recent colors are shown");
        let mut swatches: Vec<_> = (widgets.iter())
            .filter(|widget| widget.label().is_some_and(|label| label.starts_with('#')))
            .filter(|widget| {
                let bounds = widget.bounds().unwrap();
                bounds.x0 >= heading.x0 && bounds.y0 >= heading.y1
            })
            .collect();
        swatches.sort_by(|a, b| {
            let (a, b) = (a.bounds().unwrap(), b.bounds().unwrap());
            (a.y0, a.x0).partial_cmp(&(b.y0, b.x0)).unwrap()
        });
        swatches
    }
}

#[test]
fn opens_with_the_color_picked() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    assert!(!harness.no_color());
    assert_eq!(harness.hex(), "#FFCC00");

    harness.click("OK");
    assert_eq!(harness.picked, [("fill", Some(ORANGE))]);
    assert!(!harness.picker.is_open());
}

#[test]
fn typing_a_hex_code_replaces_the_color() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    // The hex field has the focus, with all its text selected.
    harness.type_text("ff0000");
    assert_eq!(harness.hex(), "ff0000");
    harness.press(Key::Enter);
    assert_eq!(harness.picked, [("fill", Some(RED))]);
}

#[test]
fn hex_codes_may_have_a_hash_and_transparency() {
    let mut harness = Harness::new();
    harness.pick("#12345678");
    let color = Color {
        r: 0x12,
        g: 0x34,
        b: 0x56,
        a: 0x78,
    };
    assert_eq!(harness.picked, [("fill", Some(color))]);
}

#[test]
fn invalid_hex_codes_are_ignored() {
    let mut harness = Harness::new();
    harness.pick("#FF00");
    assert_eq!(harness.picked, [("fill", Some(ORANGE))]);
}

#[test]
fn the_hex_field_follows_the_color_once_left() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    harness.type_text("#ff000");
    harness.click("No color");
    assert_eq!(harness.hex(), "");
    harness.click("No color");
    assert_eq!(harness.hex(), "#FFCC00");
}

#[test]
fn the_hex_field_reads_none_only_for_no_color() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    // Emptied, it keeps the color.
    harness.press(Key::Backspace);
    assert_eq!(harness.hex(), "");
    assert!(!harness.driver.painted("None"));
    harness.click("No color");
    assert!(harness.driver.painted("None"));
}

#[test]
fn cancel_and_escape_pick_nothing() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    harness.type_text("ff0000");
    harness.click("Cancel");
    assert!(!harness.picker.is_open());

    harness.open(Some(ORANGE));
    harness.press(Key::Escape);
    assert!(!harness.picker.is_open());

    assert_eq!(harness.picked, []);
    assert_eq!(harness.picker.recent, []);
}

#[test]
fn close_picks_nothing() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    harness.picker.close();
    harness.frame(vec![]);
    assert!(!harness.picker.is_open());
    assert_eq!(harness.picked, []);
}

#[test]
fn opening_an_open_picker_changes_nothing() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    harness
        .picker
        .open("border", "Border color", Some(RED), true);
    harness.click("OK");
    assert_eq!(harness.picked, [("fill", Some(ORANGE))]);
}

#[test]
fn colors_that_are_not_optional_cannot_be_none() {
    let mut harness = Harness::new();
    harness.picker.open("text", "Text color", None, false);
    harness.frame(vec![]);
    harness.frame(vec![]);
    let widgets = harness.driver.widgets.iter();
    let labels: Vec<_> = widgets.filter_map(|w| w.label()).collect();
    assert!(!labels.contains(&"No color"), "{labels:?}");
    assert_eq!(harness.hex(), "#FFFFFF");

    harness.click("OK");
    assert_eq!(harness.picked, [("text", Some(Color::WHITE))]);
}

#[test]
fn no_color_picks_none() {
    let mut harness = Harness::new();
    harness.open(None);
    assert!(harness.no_color());
    assert_eq!(harness.hex(), "");

    harness.click("OK");
    assert_eq!(harness.picked, [("fill", None)]);
    assert_eq!(harness.picker.recent, []);
}

#[test]
fn ticking_no_color_picks_none() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    harness.click("No color");
    harness.click("OK");
    assert_eq!(harness.picked, [("fill", None)]);
}

#[test]
fn unticking_no_color_picks_white() {
    let mut harness = Harness::new();
    harness.open(None);
    harness.click("No color");
    assert_eq!(harness.hex(), "#FFFFFF");
    harness.click("OK");
    assert_eq!(harness.picked, [("fill", Some(Color::WHITE))]);
}

#[test]
fn typing_a_color_unticks_no_color() {
    let mut harness = Harness::new();
    harness.open(None);
    harness.type_text("ff0000");
    assert!(!harness.no_color());
    harness.press(Key::Enter);
    assert_eq!(harness.picked, [("fill", Some(RED))]);
}

#[test]
fn picked_colors_are_offered_most_recent_first() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    assert_eq!(harness.recent(), Vec::<&str>::new());
    harness.press(Key::Escape);

    for hex in ["#FF0000", "#00FF00", "#0000FF", "#FF0000"] {
        harness.pick(hex);
    }
    harness.open(Some(ORANGE));
    assert_eq!(harness.recent(), ["#FF0000", "#0000FF", "#00FF00"]);
}

#[test]
fn only_the_last_ten_colors_are_offered() {
    let mut harness = Harness::new();
    for n in 1..=12 {
        harness.pick(&format!("#0000{n:02X}"));
    }
    harness.open(Some(ORANGE));
    let expected: Vec<_> = (3..=12).rev().map(|n| format!("#0000{n:02X}")).collect();
    assert_eq!(harness.recent(), expected);
}

#[test]
fn clicking_a_recent_color_picks_it() {
    let mut harness = Harness::new();
    harness.pick("#FF0000");
    harness.open(None);
    harness.click_recent(0, 1);
    assert!(!harness.no_color());
    assert_eq!(harness.hex(), "#FF0000");
    assert!(harness.picker.is_open());

    harness.click("OK");
    assert_eq!(harness.picked.last(), Some(&("fill", Some(RED))));
}

#[test]
fn double_clicking_a_recent_color_accepts_it() {
    let mut harness = Harness::new();
    harness.pick("#FF0000");
    harness.open(Some(ORANGE));
    harness.click_recent(0, 2);
    assert_eq!(harness.picked.last(), Some(&("fill", Some(RED))));
    assert!(!harness.picker.is_open());
}

#[test]
fn clicking_a_palette_color_picks_it() {
    let mut harness = Harness::new();
    harness.open(None);
    harness.click("#FF0000");
    assert!(!harness.no_color());
    assert_eq!(harness.hex(), "#FF0000");
    assert!(harness.picker.is_open());

    harness.click("OK");
    assert_eq!(harness.picked, [("fill", Some(RED))]);
}

#[test]
fn double_clicking_a_palette_color_accepts_it() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    harness.double_click("#FF0000");
    assert_eq!(harness.picked, [("fill", Some(RED))]);
    assert!(!harness.picker.is_open());
}

#[test]
fn the_color_picked_is_marked_in_the_palette() {
    let marked = |harness: &Harness| -> Vec<String> {
        (harness.driver.widgets.iter())
            .filter(|widget| widget.toggled() == Some(Toggled::True))
            .filter_map(|widget| widget.label().map(str::to_owned))
            .collect()
    };
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    assert_eq!(marked(&harness), ["#FFCC00"]);
    harness.type_text("ff0000");
    assert_eq!(marked(&harness), ["#FF0000"]);
    harness.press(Key::Escape);
    harness.open(Some(Color::rgb(0xFF, 0x00, 0x01)));
    assert_eq!(marked(&harness), Vec::<String>::new());
}

#[test]
fn typing_red_green_blue_and_alpha_sets_them() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    for (field, value) in [("R", "18"), ("G", "52"), ("B", "86"), ("A", "120")] {
        harness.click(field);
        harness.type_text(value);
    }
    harness.press(Key::Enter);
    let color = Color {
        r: 0x12,
        g: 0x34,
        b: 0x56,
        a: 0x78,
    };
    assert_eq!(harness.picked, [("fill", Some(color))]);
}

#[test]
fn the_number_fields_follow_the_hex_code() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    let rgba = |harness: &Harness| ["R", "G", "B", "A"].map(|field| harness.number(field));
    assert_eq!(rgba(&harness), [255.0, 204.0, 0.0, 255.0].map(Some));
    harness.type_text("#12345678");
    assert_eq!(rgba(&harness), [18.0, 52.0, 86.0, 120.0].map(Some));
}

#[test]
fn the_number_fields_are_disabled_for_no_color() {
    let mut harness = Harness::new();
    harness.open(None);
    assert!(harness.driver.widget("R").is_disabled());
    harness.click("No color");
    assert!(!harness.driver.widget("R").is_disabled());
}

#[test]
fn transparent_colors_keep_their_red_green_and_blue() {
    let clear = Color {
        a: 0,
        ..Color::rgb(0x12, 0x34, 0x56)
    };
    let mut harness = Harness::new();
    harness.open(Some(clear));
    assert_eq!(harness.hex(), "#12345600");
    harness.click("OK");

    harness.open(Some(ORANGE));
    harness.click("A");
    harness.type_text("0");
    harness.press(Key::Enter);
    let clear_orange = Color { a: 0, ..ORANGE };
    assert_eq!(
        harness.picked,
        [("fill", Some(clear)), ("fill", Some(clear_orange))]
    );
}

/// Finding a glyph among the system's fonts can take most of a second.
#[test]
fn the_dialog_needs_no_system_fonts() {
    let asked = Arc::new(Mutex::new(Vec::new()));
    let mut harness = Harness::new();
    let record = asked.clone();
    harness.driver.ctx.add_font_provider(Arc::new(
        move |request: &FallbackRequest<'_>| -> Option<FontInsert> {
            record.lock().unwrap().push(request.cluster.to_owned());
            None
        },
    ));
    harness.pick("#12345678");
    harness.open(None);
    assert_eq!(*asked.lock().unwrap(), Vec::<String>::new());
}

#[test]
fn the_palette_is_offered_without_recent_colors() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    let swatches = (harness.driver.widgets.iter())
        .filter(|widget| widget.label().is_some_and(|label| label.starts_with('#')))
        .count();
    assert_eq!(swatches, PALETTE.len());
    assert!(harness.recent().is_empty());
}

#[test]
fn hex_codes_read_like_yed_writes_them() {
    assert_eq!(parse_hex("#FFCC00"), Some(ORANGE));
    assert_eq!(parse_hex(" ffcc00 "), Some(ORANGE));
    assert_eq!(parse_hex("#ffcc0080").map(|color| color.a), Some(0x80));
    assert_eq!(parse_hex(""), None);
    assert_eq!(parse_hex("#FC0"), None);
    assert_eq!(parse_hex("##FFCC00"), None);
}

#[test]
fn enter_on_a_focused_button_presses_only_that_button() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    harness.type_text("ff0000");
    // From the hex field through R, G, B, A and OK to Cancel.
    for _ in 0..6 {
        harness.press(Key::Tab);
    }
    let focused = harness.driver.focused.as_ref();
    assert_eq!(focused.and_then(|widget| widget.label()), Some("Cancel"));
    harness.press(Key::Enter);
    assert!(!harness.picker.is_open());
    assert_eq!(harness.picked, []);
}

#[test]
fn the_dialog_keeps_still() {
    let layout = |harness: &Harness| {
        ["OK", "No color", "A"].map(|label| harness.driver.widget(label).bounds())
    };
    let mut harness = Harness::new();
    // Black has the narrowest R, G and B values.
    harness.open(Some(Color::BLACK));
    let before = layout(&harness);
    for _ in 0..5 {
        harness.frame(vec![]);
    }
    assert_eq!(layout(&harness), before);
    harness.type_text("ffffff");
    assert_eq!(layout(&harness), before);
}

#[test]
fn the_dialog_is_opaque_on_its_first_visible_frame() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    // As dark as the backdrop gets, so not fading in.
    let backdrop = Modal::new(Id::NULL).backdrop_color;
    let shaded = (harness.driver.shapes.iter())
        .any(|shape| matches!(shape, Shape::Rect(rect) if rect.fill == backdrop));
    assert!(shaded);
}
