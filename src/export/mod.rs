//! Drawings as PNG or SVG files.

mod outline;
mod svg;

use std::fmt;

use egui::Rect;
use graphml::yed::Color;
use resvg::{tiny_skia, usvg};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::canvas::Drawing;
use crate::fonts::Fonts;

/// The most pixels a PNG may have, which take 400 MB to draw.
pub const MAX_PIXELS: u64 = 100_000_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Format {
    #[default]
    Png,
    Svg,
}

impl Format {
    pub const ALL: [Format; 2] = [Format::Png, Format::Svg];

    pub fn name(self) -> &'static str {
        match self {
            Format::Png => "PNG",
            Format::Svg => "SVG",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Format::Png => "png",
            Format::Svg => "svg",
        }
    }
}

/// Written as its extension. One this version doesn't know reads as PNG.
impl Serialize for Format {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.extension())
    }
}

impl<'de> Deserialize<'de> for Format {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let extension = String::deserialize(deserializer)?;
        let format = Format::ALL.into_iter().find(|f| f.extension() == extension);
        Ok(format.unwrap_or_default())
    }
}

/// What's behind an exported drawing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Background {
    #[default]
    White,
    /// The canvas's background color.
    Canvas,
    Transparent,
}

impl Background {
    pub const ALL: [Background; 3] = [
        Background::White,
        Background::Canvas,
        Background::Transparent,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Background::White => "White",
            Background::Canvas => "Canvas color",
            Background::Transparent => "Transparent",
        }
    }

    fn keyword(self) -> &'static str {
        match self {
            Background::White => "white",
            Background::Canvas => "canvas",
            Background::Transparent => "transparent",
        }
    }

    /// Its color, with `canvas` the canvas's, or `None` if it's transparent.
    pub fn color(self, canvas: Color) -> Option<Color> {
        match self {
            Background::White => Some(Color::WHITE),
            Background::Canvas => Some(canvas),
            Background::Transparent => None,
        }
    }
}

/// Written as its keyword. One this version doesn't know reads as white.
impl Serialize for Background {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.keyword())
    }
}

/// Also read from the `transparent = true` or `false` older versions wrote.
impl<'de> Deserialize<'de> for Background {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Read;

        impl Visitor<'_> for Read {
            type Value = Background;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a background or whether it's transparent")
            }

            fn visit_str<E: de::Error>(self, keyword: &str) -> Result<Background, E> {
                let known = Background::ALL.into_iter().find(|b| b.keyword() == keyword);
                Ok(known.unwrap_or_default())
            }

            fn visit_bool<E: de::Error>(self, transparent: bool) -> Result<Background, E> {
                Ok(if transparent {
                    Background::Transparent
                } else {
                    Background::White
                })
            }
        }

        deserializer.deserialize_any(Read)
    }
}

/// How drawings are exported.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub format: Format,
    /// Around the drawing, in points.
    pub margin: u32,
    #[serde(alias = "transparent")]
    pub background: Background,
    /// A PNG's pixels per hundred points.
    pub scale: u32,
    /// Whether an SVG has its text as the outlines of the glyphs, rather than
    /// as text.
    pub outlines: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            format: Format::Png,
            margin: 10,
            background: Background::White,
            scale: 100,
            outlines: false,
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Empty,
    TooLarge,
    Render(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Empty => f.write_str("There's nothing to export."),
            Error::TooLarge => write!(
                f,
                "The image would be too large: the most is {} megapixels.",
                MAX_PIXELS / 1_000_000
            ),
            Error::Render(err) => f.write_str(err),
        }
    }
}

/// What's exported of a drawing that covers `bounds`, in graph points:
/// those with `margin` around them, if they cover anything.
pub fn page(bounds: Rect, margin: u32) -> Option<Rect> {
    bounds.is_finite().then(|| bounds.expand(margin as f32))
}

/// How many pixels wide and tall a PNG of `page` is, at `scale` pixels per
/// hundred points.
pub fn pixels(page: Rect, scale: u32) -> [u32; 2] {
    let size = page.size() * (scale as f32 / 100.0);
    // Saturating, for absurd sizes.
    [size.x, size.y].map(|side| side.ceil().max(1.0) as u32)
}

/// Whether a PNG `pixels` big has more than [`MAX_PIXELS`].
pub fn too_large([width, height]: [u32; 2]) -> bool {
    u64::from(width) * u64::from(height) > MAX_PIXELS
}

/// The file `drawing` exports to with `settings`, `canvas` being the canvas's
/// background color, its text drawn in `fonts`.
pub fn export(
    drawing: &Drawing,
    settings: &Settings,
    canvas: Color,
    fonts: &Fonts,
) -> Result<Vec<u8>, Error> {
    let page = page(drawing.bounds, settings.margin).ok_or(Error::Empty)?;
    let background = settings.background.color(canvas);
    match settings.format {
        Format::Svg => {
            let svg = svg::write(drawing, page, background, settings.outlines, fonts);
            Ok(svg.into_bytes())
        }
        Format::Png => {
            let pixels = pixels(page, settings.scale);
            if too_large(pixels) {
                return Err(Error::TooLarge);
            }
            // Outlined, as resvg is built without text.
            let svg = svg::write(drawing, page, background, true, fonts);
            png(&svg, pixels)
        }
    }
}

/// The SVG image `svg` drawn as a PNG `pixels` big.
fn png(svg: &str, [width, height]: [u32; 2]) -> Result<Vec<u8>, Error> {
    let render = |err: &dyn fmt::Display| Error::Render(err.to_string());
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).map_err(|err| render(&err))?;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or(Error::TooLarge)?;
    // Stretched over the last pixels, which are partly past the page.
    let size = tree.size();
    let scale = |pixels: u32, points: f32| pixels as f32 / points;
    let transform =
        tiny_skia::Transform::from_scale(scale(width, size.width()), scale(height, size.height()));
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    pixmap.encode_png().map_err(|err| render(&err))
}

#[cfg(test)]
mod tests;
