use egui::accesskit::{self, Toggled};
use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, pos2, vec2};

use super::*;

const ORANGE: Color = Color::rgb(0xFF, 0xCC, 0x00);
const RED: Color = Color::rgb(0xFF, 0x00, 0x00);

/// Drives a picker in an 800×600 window, one frame at a time.
struct Harness {
    ctx: Context,
    picker: ColorPicker<&'static str>,
    /// What the picker returned, in order.
    picked: Vec<(&'static str, Option<Color>)>,
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
            picker: ColorPicker::default(),
            picked: Vec::new(),
            time: 0.0,
            widgets: Vec::new(),
        }
    }

    /// Opens the picker on `color` for the fill.
    fn open(&mut self, color: Option<Color>) {
        self.picker.open("fill", "Fill color", color);
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

    fn frame(&mut self, events: Vec<Event>) {
        self.time += 0.1;
        let input = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            time: Some(self.time),
            events,
            ..RawInput::default()
        };
        let mut output = self.ctx.run_ui(input, |ui| {
            self.picked.extend(self.picker.show(ui.ctx()));
        });
        let update = output.platform_output.accesskit_update.take();
        self.widgets = update.map_or_else(Vec::new, |update| {
            update.nodes.into_iter().map(|(_, node)| node).collect()
        });
        output.drop_without_applying_deltas();
    }

    fn widget(&self, label: &str) -> &accesskit::Node {
        self.widgets
            .iter()
            .find(|widget| widget.label() == Some(label))
            .unwrap_or_else(|| panic!("no widget is labelled {label:?}"))
    }

    /// Clicks the widget labelled `label` `clicks` times in a row.
    fn click(&mut self, label: &str, clicks: usize) {
        let bounds = self.widget(label).bounds().expect("widgets have bounds");
        let at = pos2(
            ((bounds.x0 + bounds.x1) / 2.0) as f32,
            ((bounds.y0 + bounds.y1) / 2.0) as f32,
        );
        // Too long since any earlier click to make this a double click.
        self.time += 1.0;
        self.frame(vec![Event::PointerMoved(at)]);
        for _ in 0..clicks {
            for pressed in [true, false] {
                self.frame(vec![Event::PointerButton {
                    pos: at,
                    button: PointerButton::Primary,
                    pressed,
                    modifiers: Modifiers::NONE,
                }]);
            }
        }
        self.repaint();
    }

    fn press(&mut self, key: Key) {
        self.frame(vec![Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        }]);
        self.repaint();
    }

    fn type_text(&mut self, text: &str) {
        self.frame(vec![Event::Text(text.to_owned())]);
        self.repaint();
    }

    /// Widgets shown before a change catch up in the frame egui repaints
    /// right after any input.
    fn repaint(&mut self) {
        self.frame(vec![]);
    }

    fn hex(&self) -> &str {
        self.widgets
            .iter()
            .find(|widget| widget.role() == Role::TextInput)
            .and_then(|widget| widget.value())
            .expect("the hex field is shown")
    }

    fn no_color(&self) -> bool {
        self.widget("No color").toggled() == Some(Toggled::True)
    }

    /// The hex codes of the recent colors offered, in reading order.
    fn recent(&self) -> Vec<&str> {
        let mut swatches: Vec<_> = self
            .widgets
            .iter()
            .filter(|widget| widget.label().is_some_and(|label| label.starts_with('#')))
            .map(|widget| (widget.bounds().unwrap(), widget.label().unwrap()))
            .collect();
        swatches.sort_by(|(a, _), (b, _)| (a.y0, a.x0).partial_cmp(&(b.y0, b.x0)).unwrap());
        swatches.into_iter().map(|(_, label)| label).collect()
    }
}

#[test]
fn opens_with_the_color_picked() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    assert!(!harness.no_color());
    assert_eq!(harness.hex(), "#FFCC00");

    harness.click("OK", 1);
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
    harness.click("No color", 1);
    assert_eq!(harness.hex(), "");
    harness.click("No color", 1);
    assert_eq!(harness.hex(), "#FFCC00");
}

#[test]
fn cancel_and_escape_pick_nothing() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    harness.type_text("ff0000");
    harness.click("Cancel", 1);
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
    harness.picker.open("border", "Border color", Some(RED));
    harness.click("OK", 1);
    assert_eq!(harness.picked, [("fill", Some(ORANGE))]);
}

#[test]
fn no_color_picks_none() {
    let mut harness = Harness::new();
    harness.open(None);
    assert!(harness.no_color());
    assert_eq!(harness.hex(), "");

    harness.click("OK", 1);
    assert_eq!(harness.picked, [("fill", None)]);
    assert_eq!(harness.picker.recent, []);
}

#[test]
fn ticking_no_color_picks_none() {
    let mut harness = Harness::new();
    harness.open(Some(ORANGE));
    harness.click("No color", 1);
    harness.click("OK", 1);
    assert_eq!(harness.picked, [("fill", None)]);
}

#[test]
fn unticking_no_color_picks_white() {
    let mut harness = Harness::new();
    harness.open(None);
    harness.click("No color", 1);
    assert_eq!(harness.hex(), "#FFFFFF");
    harness.click("OK", 1);
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
    harness.click("#FF0000", 1);
    assert!(!harness.no_color());
    assert_eq!(harness.hex(), "#FF0000");
    assert!(harness.picker.is_open());

    harness.click("OK", 1);
    assert_eq!(harness.picked.last(), Some(&("fill", Some(RED))));
}

#[test]
fn double_clicking_a_recent_color_accepts_it() {
    let mut harness = Harness::new();
    harness.pick("#FF0000");
    harness.open(Some(ORANGE));
    harness.click("#FF0000", 2);
    assert_eq!(harness.picked.last(), Some(&("fill", Some(RED))));
    assert!(!harness.picker.is_open());
}

#[test]
fn colors_survive_the_trip_through_the_picker() {
    for value in 0..=255 {
        let colors = [
            Color::rgb(value, value, value),
            Color::rgb(value, 0x80, 0x40),
            Color::rgb(0x20, value, 0xC0),
            Color::rgb(0xFF, 0xEE, value),
            Color {
                a: value,
                ..Color::rgb(0x10, 0x20, 0x30)
            },
            Color {
                a: 0,
                ..Color::rgb(value, 0xA0, 0x05)
            },
        ];
        for original in colors {
            assert_eq!(color(hsva(original)), original);
        }
    }
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
    // From the hex field to OK, then Cancel.
    harness.press(Key::Tab);
    harness.press(Key::Tab);
    harness.press(Key::Enter);
    assert!(!harness.picker.is_open());
    assert_eq!(harness.picked, []);
}

#[test]
fn the_dialog_keeps_still() {
    let layout = |harness: &Harness| ["OK", "No color"].map(|label| harness.widget(label).bounds());
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
