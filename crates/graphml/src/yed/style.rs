use std::fmt;

use super::codec::{Part, get_color, get_or, keyword_enum, set, set_color};
use crate::{Error, xml::Element};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Color { r, g, b, a: 255 }
    }

    /// Parses `#RRGGBB` or `#RRGGBBAA`, as yEd writes colors.
    pub fn from_hex(text: &str) -> Option<Self> {
        let hex = text.strip_prefix('#')?;
        if !matches!(hex.len(), 6 | 8) || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
        Some(Color {
            r: byte(0)?,
            g: byte(2)?,
            b: byte(4)?,
            a: if hex.len() == 8 { byte(6)? } else { 255 },
        })
    }
}

/// `#RRGGBB`, or `#RRGGBBAA` if the color isn't opaque.
impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02X}{:02X}{:02X}", self.r, self.g, self.b)?;
        if self.a != 255 {
            write!(f, "{:02X}", self.a)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Part for Rect {
    fn read(element: &Element) -> Result<Self, Error> {
        Ok(Rect {
            x: get_or(element, "x", 0.0)?,
            y: get_or(element, "y", 0.0)?,
            width: get_or(element, "width", 0.0)?,
            height: get_or(element, "height", 0.0)?,
        })
    }

    fn write(&self, element: &mut Element) {
        set(element, "x", &self.x);
        set(element, "y", &self.y);
        set(element, "width", &self.width);
        set(element, "height", &self.height);
    }
}

/// How a node's border or an edge's line is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Stroke {
    /// `None` draws no line.
    pub color: Option<Color>,
    pub line_type: LineType,
    pub width: f64,
}

impl Default for Stroke {
    fn default() -> Self {
        Stroke {
            color: Some(Color::BLACK),
            line_type: LineType::Line,
            width: 1.0,
        }
    }
}

impl Part for Stroke {
    fn read(element: &Element) -> Result<Self, Error> {
        Ok(Stroke {
            color: get_color(element, "hasColor", "color")?,
            line_type: get_or(element, "type", LineType::Line)?,
            width: get_or(element, "width", 1.0)?,
        })
    }

    fn write(&self, element: &mut Element) {
        set_color(element, "hasColor", "color", self.color);
        set(element, "type", &self.line_type);
        set(element, "width", &self.width);
    }
}

keyword_enum! {
    pub enum LineType {
        Line = "line",
        Dashed = "dashed",
        Dotted = "dotted",
        DashedDotted = "dashed_dotted",
    }
}
