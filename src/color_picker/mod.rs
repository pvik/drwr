use egui::color_picker;
use egui::{
    Color32, Context, DragValue, Id, Key, Modal, Painter, Rect, Response, Role, Sense, Stroke,
    StrokeKind, TextEdit, Ui, Vec2, WidgetInfo, vec2,
};
use graphml::yed::Color;

use crate::text_field;

/// How many of the last colors picked the dialog offers.
const RECENT: usize = 10;

/// The colors the dialog offers: grays, then a column per hue, light to
/// dark. Web-safe, like yEd's default fill, #FFCC00.
#[rustfmt::skip]
const PALETTE: [u32; 54] = [
    0xFFFFFF, 0xFFCCCC, 0xFFCC99, 0xFFFFCC, 0xCCFFCC, 0xCCFFFF, 0x99CCFF, 0xCCCCFF, 0xFFCCFF,
    0xCCCCCC, 0xFF9999, 0xFFCC66, 0xFFFF99, 0x99FF99, 0x99FFFF, 0x6699FF, 0x9999FF, 0xFF99FF,
    0x999999, 0xFF0000, 0xFF9900, 0xFFFF00, 0x00FF00, 0x00FFFF, 0x0000FF, 0x9900FF, 0xFF00FF,
    0x666666, 0xCC0000, 0xFF6600, 0xFFCC00, 0x00CC00, 0x00CCCC, 0x0000CC, 0x6600CC, 0xCC00CC,
    0x333333, 0x990000, 0xCC6600, 0xCC9900, 0x009900, 0x009999, 0x000099, 0x660099, 0x990099,
    0x000000, 0x660000, 0x993300, 0x996600, 0x006600, 0x006666, 0x000066, 0x330066, 0x660066,
];
const PALETTE_COLUMNS: usize = 9;

/// Between swatches, so that they read as one grid.
const SWATCH_GAP: f32 = 2.0;

/// A modal dialog for picking a color, or no color, for a target of type `T`,
/// which tells the caller what the color is for.
pub struct ColorPicker<T> {
    dialog: Option<Dialog<T>>,
    /// The last distinct colors picked, most recent first.
    recent: Vec<Color>,
}

struct Dialog<T> {
    target: T,
    title: String,
    /// Whether "No color" is offered.
    optional: bool,
    no_color: bool,
    color: Color,
    hex: String,
    /// Whether the hex field has yet to take the focus.
    focus_hex: bool,
}

impl<T> Default for ColorPicker<T> {
    fn default() -> Self {
        ColorPicker {
            dialog: None,
            recent: Vec::new(),
        }
    }
}

impl<T> ColorPicker<T> {
    pub fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    /// Opens the dialog with `color` picked, or "No color" if it's `None`,
    /// unless it's open already. Only an `optional` color can be "No color".
    pub fn open(
        &mut self,
        target: T,
        title: impl Into<String>,
        color: Option<Color>,
        optional: bool,
    ) {
        if self.is_open() {
            return;
        }
        self.dialog = Some(Dialog {
            target,
            title: title.into(),
            optional,
            no_color: optional && color.is_none(),
            color: color.unwrap_or(Color::WHITE),
            hex: String::new(),
            focus_hex: true,
        });
    }

    /// Closes the dialog without picking anything.
    pub fn close(&mut self) {
        self.dialog = None;
    }

    /// Shows the dialog while it's open. Returns its target and the color
    /// picked once OK is pressed.
    pub fn show(&mut self, ctx: &Context) -> Option<(T, Option<Color>)> {
        let dialog = self.dialog.as_mut()?;
        let recent = &self.recent;
        let modal = Modal::new(Id::unique("color_picker")).show(ctx, |ui| dialog.ui(ui, recent));
        if modal.inner {
            let dialog = self.dialog.take()?;
            let picked = dialog.picked();
            if let Some(color) = picked {
                self.remember(color);
            }
            return Some((dialog.target, picked));
        }
        if modal.should_close() {
            self.dialog = None;
        }
        None
    }

    fn remember(&mut self, color: Color) {
        self.recent.retain(|&recent| recent != color);
        self.recent.insert(0, color);
        self.recent.truncate(RECENT);
    }
}

impl<T> Dialog<T> {
    fn picked(&self) -> Option<Color> {
        (!self.no_color).then_some(self.color)
    }

    fn pick(&mut self, picked: Color) {
        self.color = picked;
        self.no_color = false;
    }

    /// Returns whether the color picked was accepted.
    fn ui(&mut self, ui: &mut Ui, recent: &[Color]) -> bool {
        let mut ok = false;
        ui.strong(&self.title);
        // Side by side, so the dialog fits in a small window.
        ui.horizontal_top(|ui| {
            // Not egui's HSV picker: its copy button's emoji isn't in egui's
            // fonts, and finding it among the system's froze the first open.
            ok = self.swatches(ui, &PALETTE.map(rgb), PALETTE_COLUMNS);
            // Not a separator: that takes all the height there is, and the
            // modal then grows a little every frame.
            ui.add_space(8.0);
            ui.vertical(|ui| {
                if self.optional {
                    ui.checkbox(&mut self.no_color, "No color");
                }
                self.hex_field(ui);
                ui.add_enabled_ui(!self.no_color, |ui| self.channel_fields(ui));
                ui.label("Recent colors");
                if recent.is_empty() {
                    ui.weak("None yet");
                }
                ok |= self.swatches(ui, recent, RECENT / 2);
            });
        });
        ui.separator();
        ui.horizontal(|ui| {
            ok |= ui.button("OK").clicked();
            if ui.button("Cancel").clicked() {
                ui.close();
            }
        });
        // A focused button takes Enter as a click instead.
        let enter = ui.input(|input| input.key_pressed(Key::Enter))
            && ui.memory(|memory| memory.focused().is_none());
        ok || enter
    }

    /// Shows `colors` as swatches, `columns` to a row, picking the one
    /// clicked. Returns whether one was double-clicked.
    fn swatches(&mut self, ui: &mut Ui, colors: &[Color], columns: usize) -> bool {
        let mut double_clicked = false;
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = Vec2::splat(SWATCH_GAP);
            for row in colors.chunks(columns) {
                ui.horizontal(|ui| {
                    for &color in row {
                        let swatch = swatch_button(ui, color, self.picked() == Some(color));
                        if swatch.clicked() {
                            self.pick(color);
                        }
                        double_clicked |= swatch.double_clicked();
                    }
                });
            }
        });
        double_clicked
    }

    /// Shows the hex field, with a sample of the color picked beside it.
    fn hex_field(&mut self, ui: &mut Ui) {
        let id = ui.make_persistent_id("hex");
        // Follows the color, except while it's being typed.
        if !ui.memory(|memory| memory.has_focus(id)) {
            self.hex = self
                .picked()
                .map_or_else(String::new, |color| color.to_string());
        }
        let invalid = !self.hex.is_empty() && parse_hex(&self.hex).is_none();
        ui.horizontal(|ui| {
            ui.label("Hex");
            let mut field = TextEdit::singleline(&mut self.hex)
                .id(id)
                .desired_width(80.0);
            if self.optional {
                field = field.hint_text("None");
            }
            if invalid {
                field = field.text_color(ui.visuals().error_fg_color);
            }
            let output = field.show(ui);
            if output.response.response.changed()
                && let Some(color) = parse_hex(&self.hex)
            {
                self.pick(color);
            }
            if std::mem::take(&mut self.focus_hex) {
                text_field::focus_all(ui.ctx(), output, &self.hex);
            }
            let height = ui.spacing().interact_size.y;
            let (rect, _) = ui.allocate_exact_size(vec2(2.0 * height, height), Sense::hover());
            let outline = ui.visuals().widgets.noninteractive.fg_stroke;
            paint_swatch(ui.painter(), rect, self.picked(), outline);
        });
    }

    /// Shows a field each for the red, green, blue and alpha of the color.
    fn channel_fields(&mut self, ui: &mut Ui) {
        let size = vec2(field_width(ui), ui.spacing().interact_size.y);
        let Color { r, g, b, a } = &mut self.color;
        ui.horizontal(|ui| {
            for (prefix, value) in [("R ", r), ("G ", g), ("B ", b), ("A ", a)] {
                ui.add(
                    DragValue::new(value)
                        .prefix(prefix)
                        .speed(0.5)
                        .min_size(size),
                );
            }
        });
    }
}

/// Wide enough for any value of the R, G, B and A fields, so the controls
/// beside them stay put as the color changes.
fn field_width(ui: &Ui) -> f32 {
    let font = ui.style().drag_value_text_style.resolve(ui.style());
    let width = |text: &str| {
        let text = text.to_owned();
        ui.painter()
            .layout_no_wrap(text, font.clone(), Color32::PLACEHOLDER)
            .size()
            .x
    };
    let widest = ["R 255", "G 255", "B 255", "A 255"]
        .map(width)
        .into_iter()
        .fold(0.0, f32::max);
    // A point to spare, since the fields round their letter and value apart.
    widest + 2.0 * ui.spacing().button_padding.x + 1.0
}

/// A button showing `color`, labelled with its hex code, and marked if it's
/// the `current` color.
fn swatch_button(ui: &mut Ui, color: Color, current: bool) -> Response {
    let size = Vec2::splat(ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hex = color.to_string();
    response.widget_info(|| WidgetInfo::selected(Role::Button, ui.is_enabled(), current, &hex));
    let mut outline = ui.style().interact_selectable(&response, current).fg_stroke;
    if current {
        outline.width = 2.0;
    }
    paint_swatch(ui.painter(), rect, Some(color), outline);
    response.on_hover_text(hex)
}

/// Paints `color` in `rect` with an outline, or strikes `rect` out if `None`.
pub fn paint_swatch(painter: &Painter, rect: Rect, color: Option<Color>, outline: Stroke) {
    match color {
        Some(color) => color_picker::show_color_at(painter, color32(color), rect),
        None => {
            let strike = Stroke::new(outline.width, Color32::RED);
            painter.line_segment([rect.left_bottom(), rect.right_top()], strike);
        }
    }
    painter.rect_stroke(rect, 0.0, outline, StrokeKind::Inside);
}

pub fn color32(color: Color) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r, color.g, color.b, color.a)
}

const fn rgb(hex: u32) -> Color {
    Color::rgb((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

/// Reads `#RRGGBB` or `#RRGGBBAA`, with or without the `#`.
fn parse_hex(text: &str) -> Option<Color> {
    let text = text.trim();
    Color::from_hex(&format!("#{}", text.strip_prefix('#').unwrap_or(text)))
}

#[cfg(test)]
mod tests;
