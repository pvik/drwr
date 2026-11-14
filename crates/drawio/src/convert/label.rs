//! Labels: their text, font and place.

use graphml::yed::{self, Alignment, Color, Font, FontStyle, Label, LabelPosition, Point};

use super::{Converter, fade};
use crate::{model::Cell, style::Style, text};

/// draw.io's `fontStyle` bits.
const BOLD: u32 = 1;
const ITALIC: u32 = 2;
const UNDERLINE: u32 = 4;
const STRIKETHROUGH: u32 = 8;

impl Converter<'_> {
    /// The label of `cell`, unless it has none, yet to be placed.
    pub(super) fn label(&mut self, cell: &Cell, style: &Style) -> Option<Label> {
        if style.flag("noLabel") {
            return None;
        }
        let placeholders = cell.properties.get("placeholders");
        let value = match placeholders.is_some_and(|p| p == "1") {
            true => self.replace_placeholders(cell),
            false => cell.value.clone(),
        };
        let text = match style.is("html", "1") {
            true => text::from_html(&value, style.parsed("nl2Br") != Some(0.0)),
            false => text::from_plain(&value),
        };
        if text.text.trim().is_empty() {
            return None;
        }
        let bits = style.number("fontStyle", 0.0) as u32;
        if text.lost_formatting || bits & (UNDERLINE | STRIKETHROUGH) != 0 {
            self.notes.formatting += 1;
        }
        if style.parsed("horizontal") == Some(0.0) {
            self.notes.upwards += 1;
        }
        let mut label = Label::new(text.text);
        let bold = text.bold || bits & BOLD != 0;
        let italic = text.italic || bits & ITALIC != 0;
        label.font = Font {
            family: font_family(style),
            // draw.io's size for a style without one.
            size: style.number("fontSize", 11.0).round().max(1.0) as u32,
            style: match (bold, italic) {
                (true, true) => FontStyle::BoldItalic,
                (true, false) => FontStyle::Bold,
                (false, true) => FontStyle::Italic,
                (false, false) => FontStyle::Plain,
            },
        };
        let color = style.color("fontColor", Color::BLACK);
        // Only textOpacity: draw.io's opacity is the shape's.
        let alpha = style.number("textOpacity", 100.0) / 100.0;
        label.text_color = fade(color.unwrap_or(Color::BLACK), alpha);
        label.background = style.color("labelBackgroundColor", Color::WHITE);
        label.border = style.color("labelBorderColor", Color::BLACK);
        label.alignment = match style.get("align") {
            Some("left") => Alignment::Left,
            Some("right") => Alignment::Right,
            _ => Alignment::Center,
        };
        Some(label)
    }

    /// The value of `cell` with its `%name%` placeholders replaced as draw.io
    /// does: by the cell's id or size, or by a property of it or of the cells
    /// it's in. Others, such as `%date%`, stay as written.
    fn replace_placeholders(&self, cell: &Cell) -> String {
        let text = cell.value.as_str();
        let mut replaced = String::with_capacity(text.len());
        // How much of `text` is in `replaced`, and where to look on.
        let (mut done, mut at) = (0, 0);
        while let Some(start) = text[at..].find('%').map(|i| at + i) {
            let Some((name, end)) = placeholder_at(text, start) else {
                at = start + 1;
                continue;
            };
            at = end;
            let value = match name {
                "label" | "tooltip" => continue,
                // `%%name%` writes `%name%`.
                _ if start > done && text[..start].ends_with('%') => {
                    Some(text[start + 1..end].to_owned())
                }
                "id" => Some(cell.id.clone()),
                "width" if cell.vertex => Some(cell.geometry.width.to_string()),
                "height" if cell.vertex => Some(cell.geometry.height.to_string()),
                _ => self.property(cell, name),
            };
            replaced.push_str(&text[done..start]);
            replaced.push_str(value.as_deref().unwrap_or(&text[start..end]));
            done = end;
        }
        replaced.push_str(&text[done..]);
        replaced
    }

    /// The property `name` of `cell`, or else of the nearest cell it's in.
    fn property(&self, cell: &Cell, name: &str) -> Option<String> {
        let mut cell = cell;
        // Bounded, as a broken file's parents may run in a circle.
        for _ in 0..self.cells.len() {
            if let Some(value) = cell.properties.get(name) {
                return Some(value.clone());
            }
            cell = &self.cells[*self.index.get(cell.parent.as_deref()?)?];
        }
        None
    }
}

/// The name of the placeholder at `start` of `text` and where it ends, as
/// draw.io's pattern for them matches.
fn placeholder_at(text: &str, start: usize) -> Option<(&str, usize)> {
    let rest = &text[start + 1..];
    let len = match rest.strip_prefix("date{") {
        Some(format) => {
            let close = format.find(['{', '}', '\n', '\r', '\u{2028}', '\u{2029}'])?;
            format[close..]
                .starts_with("}%")
                .then_some("date{".len() + close + 1)?
        }
        None => {
            let len = rest.find(['%', '{', '}', '"', '\'', '=', ';'])?;
            (len > 0 && rest[len..].starts_with('%')).then_some(len)?
        }
    };
    Some((&rest[..len], start + 1 + len + 1))
}

/// The first font family in `fontFamily`, which may be a CSS list.
fn font_family(style: &Style) -> String {
    // draw.io's family for a style without one is "Arial,Helvetica".
    let families = style.get("fontFamily").unwrap_or("Arial");
    let first = families.split(',').next().unwrap_or_default();
    let first = first.trim().trim_matches(['"', '\'']);
    match first.is_empty() {
        true => "Arial".to_owned(),
        false => first.to_owned(),
    }
}

/// The yEd position closest to where draw.io puts a vertex's label.
pub(super) fn position(style: &Style) -> LabelPosition {
    use LabelPosition::*;
    let outside_x = style.get("labelPosition").unwrap_or("center");
    let outside_y = style.get("verticalLabelPosition").unwrap_or("middle");
    match (outside_x, outside_y) {
        ("center", "middle") => {}
        ("center", "top") => return North,
        ("center", "bottom") => return South,
        ("left", "middle") => return West,
        ("right", "middle") => return East,
        ("left", "top") => return NorthWest,
        ("right", "top") => return NorthEast,
        ("left", "bottom") => return SouthWest,
        ("right", "bottom") => return SouthEast,
        _ => {}
    }
    match (
        style.get("align").unwrap_or("center"),
        style.get("verticalAlign").unwrap_or("middle"),
    ) {
        ("left", "top") => TopLeft,
        ("right", "top") => TopRight,
        ("left", "bottom") => BottomLeft,
        ("right", "bottom") => BottomRight,
        ("left", _) => Left,
        ("right", _) => Right,
        (_, "top") => Top,
        (_, "bottom") => Bottom,
        _ => Center,
    }
}

/// Where draw.io anchors what's `x` along a line, from -1 at its start to 1
/// at its end (and past them), `y` off it, then moved by `offset`.
pub(super) fn point_on(route: &yed::Route, x: f64, y: f64, offset: Option<Point>) -> Point {
    let points = &route.points;
    let lengths: Vec<f64> = points.windows(2).map(|w| distance(w[0], w[1])).collect();
    let mut left = (x / 2.0 + 0.5) * lengths.iter().sum::<f64>();
    // The segment it's on; at a bend, the one after it, as in draw.io.
    let mut i = 0;
    while i + 1 < lengths.len() && left >= lengths[i] {
        left -= lengths[i];
        i += 1;
    }
    let (a, b, length) = (points[i], points[i + 1], lengths[i]);
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let (t, nx, ny) = match length > 0.0 {
        true => (left / length, dy / length, dx / length),
        false => (0.0, 0.0, 0.0),
    };
    let offset = offset.unwrap_or_default();
    Point {
        x: a.x + dx * t + nx * y + offset.x,
        y: a.y + dy * t - ny * y + offset.y,
    }
}

/// Places an edge label at `anchor` on `route`: centered on it, unless it's
/// aligned otherwise.
pub(super) fn place(mut label: Label, route: &yed::Route, anchor: Point, style: &Style) -> Label {
    let (width, height) = estimate_size(&label);
    let mut center = anchor;
    match style.get("align") {
        Some("left") => center.x += width / 2.0,
        Some("right") => center.x -= width / 2.0,
        _ => {}
    }
    match style.get("verticalAlign") {
        Some("top") => center.y += height / 2.0,
        Some("bottom") => center.y -= height / 2.0,
        _ => {}
    }
    label.place_on(route, center, width, height);
    label
}

fn distance(a: Point, b: Point) -> f64 {
    (b.x - a.x).hypot(b.y - a.y)
}

/// A label's rough size, before a renderer measures it.
fn estimate_size(label: &Label) -> (f64, f64) {
    let size = f64::from(label.font.size);
    let lines = label.text.lines().count().max(1);
    let widest = label
        .text
        .lines()
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0);
    (
        widest as f64 * size * 0.55 + 4.0,
        lines as f64 * size * 1.2 + 4.0,
    )
}
