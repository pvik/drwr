//! Drawings as SVG.

use std::collections::HashMap;
use std::fmt::Write as _;

use egui::{Align, Pos2, Rect};
use graphml::yed::{self, Color, FontStyle};

use crate::canvas::{Drawing, Mark, Outline, Pen, Text};
use crate::fonts::{self, Fonts};
use crate::line_type;

use super::outline::Outliner;

/// `drawing` as an SVG image of `page`, in graph points, over `background`
/// unless that's `None`. Its text is drawn in `fonts`, as the outlines of
/// the glyphs if `outlines` says so.
pub fn write(
    drawing: &Drawing,
    page: Rect,
    background: Option<Color>,
    outlines: bool,
    fonts: &Fonts,
) -> String {
    let mut svg = Svg {
        out: String::new(),
        fonts,
        outliner: outlines.then(|| Outliner::new(fonts)),
        families: HashMap::new(),
    };
    let (x, y) = (num(page.min.x), num(page.min.y));
    let (width, height) = (num(page.width()), num(page.height()));
    svg.out
        .push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    let _ = writeln!(
        svg.out,
        r#"<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="{width}" height="{height}" viewBox="{x} {y} {width} {height}">"#
    );
    if let Some(color) = background {
        let fill = paint("fill", color);
        let rect = format!(r#"x="{x}" y="{y}" width="{width}" height="{height}""#);
        let _ = writeln!(svg.out, "<rect {rect}{fill}/>");
    }
    for mark in &drawing.marks {
        svg.mark(mark);
    }
    svg.out.push_str("</svg>\n");
    svg.out
}

struct Svg<'a> {
    out: String,
    fonts: &'a Fonts,
    /// Draws text as outlines, if it's drawn so.
    outliner: Option<Outliner<'a>>,
    /// The `font-family` of each yEd family and style.
    families: HashMap<(String, FontStyle), String>,
}

impl Svg<'_> {
    fn mark(&mut self, mark: &Mark) {
        match mark {
            Mark::Node {
                rect,
                outline,
                fill,
                border,
            } => self.node(*rect, *outline, *fill, border.as_ref()),
            Mark::Line { points, pen } => {
                let d = path_data(points, false);
                let _ = writeln!(self.out, r#"<path d="{d}" fill="none"{}/>"#, stroke(pen));
            }
            Mark::Polygon { points, color } => {
                let d = path_data(points, true);
                let _ = writeln!(self.out, r#"<path d="{d}"{}/>"#, paint("fill", *color));
            }
            Mark::Text(text) => self.text(text),
        }
    }

    fn node(&mut self, rect: Rect, outline: Outline, fill: Option<Color>, border: Option<&Pen>) {
        let stroke = border.map(stroke).unwrap_or_default();
        let fill = fill.map_or_else(|| r#" fill="none""#.to_owned(), |fill| paint("fill", fill));
        let [x, y, width, height] = [rect.min.x, rect.min.y, rect.width(), rect.height()].map(num);
        if rect.width() == 0.0 || rect.height() == 0.0 {
            // A line, which only the border draws, as on the canvas.
            if border.is_some() {
                let (right, bottom) = (num(rect.max.x), num(rect.max.y));
                let d = format!("M{x} {y}H{right}V{bottom}H{x}Z");
                let _ = writeln!(self.out, r#"<path d="{d}" fill="none"{stroke}/>"#);
            }
            return;
        }
        let _ = match outline {
            Outline::Rect(radius) if radius > 0.0 => {
                // Both, so that each is capped at half its side, like Java.
                let radius = num(radius);
                writeln!(
                    self.out,
                    r#"<rect x="{x}" y="{y}" width="{width}" height="{height}" rx="{radius}" ry="{radius}"{fill}{stroke}/>"#
                )
            }
            Outline::Rect(_) => writeln!(
                self.out,
                r#"<rect x="{x}" y="{y}" width="{width}" height="{height}"{fill}{stroke}/>"#
            ),
            Outline::Ellipse => {
                let [cx, cy] = [rect.center().x, rect.center().y].map(num);
                let [rx, ry] = [rect.width() / 2.0, rect.height() / 2.0].map(num);
                writeln!(
                    self.out,
                    r#"<ellipse cx="{cx}" cy="{cy}" rx="{rx}" ry="{ry}"{fill}{stroke}/>"#
                )
            }
        };
    }

    fn text(&mut self, text: &Text) {
        let fill = paint("fill", text.color);
        if let Some(outliner) = &mut self.outliner {
            let d = outliner.outline(text);
            if !d.is_empty() {
                let _ = writeln!(self.out, r#"<path d="{d}"{fill}/>"#);
            }
            return;
        }
        let font = &text.font;
        let family = self.family(font);
        let mut attributes = format!(
            r#"font-family="{}" font-size="{}""#,
            escape(&family),
            font.size.max(1)
        );
        if matches!(font.style, FontStyle::Bold | FontStyle::BoldItalic) {
            attributes.push_str(r#" font-weight="bold""#);
        }
        if matches!(font.style, FontStyle::Italic | FontStyle::BoldItalic) {
            attributes.push_str(r#" font-style="italic""#);
        }
        match text.align {
            Align::Min => {}
            Align::Center => attributes.push_str(r#" text-anchor="middle""#),
            Align::Max => attributes.push_str(r#" text-anchor="end""#),
        }
        let _ = write!(
            self.out,
            r#"<text {attributes}{fill} xml:space="preserve">"#
        );
        for row in &text.rows {
            let [x, y] = [row.x, row.baseline].map(num);
            let row = escape(&row.text);
            let _ = write!(self.out, r#"<tspan x="{x}" y="{y}">{row}</tspan>"#);
        }
        self.out.push_str("</text>\n");
    }

    /// The `font-family` of text in `font`: the family it's drawn in on the
    /// canvas, after the one yEd asks for if that's a real one but missing,
    /// then a generic family.
    fn family(&mut self, font: &yed::Font) -> String {
        let key = (font.family.clone(), font.style.clone());
        let fonts = self.fonts;
        let family = self.families.entry(key).or_insert_with(|| {
            let drawn = fonts.face(&font.family, &font.style).family;
            let mut families = Vec::new();
            if !fonts::is_logical(&font.family) && !font.family.eq_ignore_ascii_case(&drawn) {
                families.push(quote(&font.family));
            }
            families.push(quote(&drawn));
            families.push(fonts::css_generic(&font.family).to_owned());
            families.join(", ")
        });
        family.clone()
    }
}

/// Path data through `points`, closed if `closed`.
fn path_data(points: &[Pos2], closed: bool) -> String {
    let mut d = String::new();
    for (index, point) in points.iter().enumerate() {
        let command = if index == 0 { 'M' } else { 'L' };
        let _ = write!(d, "{command}{} {}", num(point.x), num(point.y));
    }
    if closed {
        d.push('Z');
    }
    d
}

/// The attributes that stroke with `pen`.
fn stroke(pen: &Pen) -> String {
    let mut attributes = paint("stroke", pen.color);
    let _ = write!(attributes, r#" stroke-width="{}""#, num(pen.width));
    if let Some(pattern) = line_type::pattern(&pen.line_type) {
        let unit = pen.width.max(1.0);
        let lengths: Vec<_> = pattern
            .iter()
            .flatten()
            .map(|length| num(length * unit))
            .collect();
        let _ = write!(attributes, r#" stroke-dasharray="{}""#, lengths.join(" "));
    }
    attributes
}

/// The attributes that set `property`, e.g. "fill", to `color`.
fn paint(property: &str, color: Color) -> String {
    let Color { r, g, b, a } = color;
    let mut attributes = format!(r##" {property}="#{r:02X}{g:02X}{b:02X}""##);
    if a < u8::MAX {
        let opacity = num(f32::from(a) / 255.0);
        let _ = write!(attributes, r#" {property}-opacity="{opacity}""#);
    }
    attributes
}

/// A number with at most 2 decimals, which are a hundredth of a point.
pub(super) fn num(value: f32) -> String {
    // Adding zero turns -0 into 0.
    let text = format!("{:.2}", (value * 100.0).round() / 100.0 + 0.0);
    text.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// A font family's name quoted for CSS.
fn quote(family: &str) -> String {
    format!("'{}'", family.replace('\\', "\\\\").replace('\'', "\\'"))
}

/// `text` as XML text or attribute values, without the characters XML can't
/// have.
pub(super) fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\t' | '\n' | '\r' => escaped.push(c),
            c if c < ' ' || c == '\u{FFFE}' || c == '\u{FFFF}' => {}
            c => escaped.push(c),
        }
    }
    escaped
}
