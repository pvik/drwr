use egui::color_picker::{self, Alpha};
use egui::ecolor::{Hsva, linear_f32_from_gamma_u8, linear_f32_from_linear_u8};
use egui::text::CCursorRange;
use egui::{
    Color32, Context, Id, Key, Modal, Painter, Rect, Response, Role, Sense, Stroke, StrokeKind,
    TextEdit, Ui, Vec2, WidgetInfo,
};
use graphml::yed::Color;

/// How many of the last colors picked the dialog offers.
const RECENT: usize = 10;

/// The width of the saturation-value square and the sliders.
const PICKER_WIDTH: f32 = 200.0;

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
    no_color: bool,
    /// Unlike a `Color`, keeps its hue while it's gray.
    color: Hsva,
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
    /// unless it's open already.
    pub fn open(&mut self, target: T, title: impl Into<String>, color: Option<Color>) {
        if self.is_open() {
            return;
        }
        self.dialog = Some(Dialog {
            target,
            title: title.into(),
            no_color: color.is_none(),
            color: hsva(color.unwrap_or(Color::WHITE)),
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
        (!self.no_color).then(|| color(self.color))
    }

    fn pick(&mut self, picked: Color) {
        self.color = hsva(picked);
        self.no_color = false;
    }

    /// Returns whether the color picked was accepted.
    fn ui(&mut self, ui: &mut Ui, recent: &[Color]) -> bool {
        let mut ok = false;
        ui.strong(&self.title);
        // Side by side, so the dialog fits in a small window.
        ui.horizontal_top(|ui| {
            ui.add_enabled_ui(!self.no_color, |ui| {
                let field_width = field_width(ui);
                let spacing = ui.spacing_mut();
                spacing.slider_width = PICKER_WIDTH;
                // The minimum width of its R, G, B and A fields.
                spacing.interact_size.x = field_width;
                color_picker::color_picker_hsva_2d(ui, &mut self.color, Alpha::OnlyBlend);
            });
            // Not a separator: that takes all the height there is, and the
            // modal then grows a little every frame.
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.checkbox(&mut self.no_color, "No color");
                self.hex_field(ui);
                ui.label("Recent colors");
                ok = self.recent_colors(ui, recent);
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

    /// Shows the recent colors in two rows, picking the one clicked. Returns
    /// whether one was double-clicked.
    fn recent_colors(&mut self, ui: &mut Ui, recent: &[Color]) -> bool {
        if recent.is_empty() {
            ui.weak("None yet");
        }
        let mut double_clicked = false;
        for row in recent.chunks(RECENT / 2) {
            ui.horizontal(|ui| {
                for &color in row {
                    let swatch = swatch_button(ui, color);
                    if swatch.clicked() {
                        self.pick(color);
                    }
                    double_clicked |= swatch.double_clicked();
                }
            });
        }
        double_clicked
    }

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
                .hint_text("None")
                .desired_width(80.0);
            if invalid {
                field = field.text_color(ui.visuals().error_fg_color);
            }
            let mut output = field.show(ui);
            if output.response.response.changed()
                && let Some(color) = parse_hex(&self.hex)
            {
                self.pick(color);
            }
            // Selected, so typing replaces it.
            if std::mem::take(&mut self.focus_hex) {
                output.response.response.request_focus();
                let all = CCursorRange::select_all_str(&self.hex);
                output.state.cursor.set_char_range(Some(all));
                output.state.store(ui.ctx(), id);
            }
        });
    }
}

/// Wide enough for any value of the picker's R, G, B and A fields, so the
/// controls beside it stay put as the color changes.
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

/// A button showing `color`, labelled with its hex code.
fn swatch_button(ui: &mut Ui, color: Color) -> Response {
    let size = Vec2::splat(ui.spacing().interact_size.y);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click());
    let hex = color.to_string();
    response.widget_info(|| WidgetInfo::labeled(Role::Button, ui.is_enabled(), &hex));
    let outline = ui.style().interact(&response).fg_stroke;
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

/// `Hsva::from_srgba_unmultiplied` premultiplies on the way, which loses the
/// red, green and blue of transparent colors.
fn hsva(color: Color) -> Hsva {
    Hsva::from_rgba_unmultiplied(
        linear_f32_from_gamma_u8(color.r),
        linear_f32_from_gamma_u8(color.g),
        linear_f32_from_gamma_u8(color.b),
        linear_f32_from_linear_u8(color.a),
    )
}

fn color(hsva: Hsva) -> Color {
    let [r, g, b, a] = hsva.to_srgba_unmultiplied();
    Color { r, g, b, a }
}

/// Reads `#RRGGBB` or `#RRGGBBAA`, with or without the `#`.
fn parse_hex(text: &str) -> Option<Color> {
    let text = text.trim();
    Color::from_hex(&format!("#{}", text.strip_prefix('#').unwrap_or(text)))
}

#[cfg(test)]
mod tests;
