//! Label text as the outlines of its glyphs, shaped as egui shapes it, which
//! looks the same everywhere, fonts or not.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::ops::Range;

use egui::Align;
use graphml::yed::{self, FontStyle};
use harfrust::{ShapeOptions, ShaperData, ShaperInstance, UnicodeBuffer, Variation};
use skrifa::instance::{Location, Size};
use skrifa::outline::{DrawSettings, OutlineGlyphCollection, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider as _};

use crate::canvas::Text;
use crate::fonts::{Face, Fonts};

use super::svg::num;

/// How wide egui draws a tab, in spaces.
const TAB_SPACES: f32 = 4.0;

/// Outlines label text in the faces the canvas draws it in, falling back on
/// others for the characters those lack.
pub struct Outliner<'a> {
    fonts: &'a Fonts,
    /// Those used so far; `None` for any that can't be read.
    faces: Vec<Option<Shaping>>,
    /// The index in `faces` of each yEd family and style's face.
    primary: HashMap<(String, FontStyle), usize>,
    /// The index in `faces` of the face drawing each character a primary face
    /// lacked, in a style, or `None` if none has it.
    fallback: HashMap<(char, FontStyle), Option<usize>>,
}

/// A face, ready to shape text in.
struct Shaping {
    face: Face,
    data: ShaperData,
    instance: ShaperInstance,
    location: Location,
}

/// A glyph of a row, placed from the row's start on its baseline, upward.
struct Placed {
    face: usize,
    glyph: GlyphId,
    x: f32,
    y: f32,
}

impl<'a> Outliner<'a> {
    pub fn new(fonts: &'a Fonts) -> Self {
        Outliner {
            fonts,
            faces: Vec::new(),
            primary: HashMap::new(),
            fallback: HashMap::new(),
        }
    }

    /// Path data of the outlines of `text`'s glyphs, with its rows placed as
    /// the canvas places them.
    pub fn outline(&mut self, text: &Text) -> String {
        let size = text.font.size.max(1) as f32;
        let primary = self.primary(&text.font);
        let mut d = String::new();
        for row in &text.rows {
            let (glyphs, width) = self.shape(&row.text, primary, &text.font.style, size);
            let start = match text.align {
                Align::Min => row.x,
                Align::Center => row.x - width / 2.0,
                Align::Max => row.x - width,
            };
            self.draw(&glyphs, start, row.baseline, size, &mut d);
        }
        d
    }

    /// The index in `faces` of the face the canvas draws `font` in.
    fn primary(&mut self, font: &yed::Font) -> usize {
        let key = (font.family.clone(), font.style.clone());
        if let Some(&index) = self.primary.get(&key) {
            return index;
        }
        let index = self.add(self.fonts.face(&font.family, &font.style));
        self.primary.insert(key, index);
        index
    }

    fn add(&mut self, face: Face) -> usize {
        self.faces.push(Shaping::new(face));
        self.faces.len() - 1
    }

    /// The index in `faces` of the face to draw `c` in, in text whose face is
    /// at `primary`: that, unless it lacks `c` and another has it.
    fn face_for(&mut self, c: char, primary: usize, style: &FontStyle) -> usize {
        let has = |shaping: &Option<Shaping>| shaping.as_ref().is_some_and(|s| s.face.has(c));
        if c.is_whitespace() || c.is_control() || has(&self.faces[primary]) {
            return primary;
        }
        let key = (c, style.clone());
        if let Some(&found) = self.fallback.get(&key) {
            return found.unwrap_or(primary);
        }
        // Those found already, e.g. for the text's other characters, first.
        let used = (0..self.faces.len()).find(|&index| {
            (self.faces[index].as_ref()).is_some_and(|shaping| has_outline(&shaping.face, c))
        });
        let found = used.or_else(|| {
            let face = self.fonts.fallback(c, style, |face| has_outline(face, c))?;
            Some(self.add(face))
        });
        self.fallback.insert(key, found);
        found.unwrap_or(primary)
    }

    /// The glyphs of a row's `text` at `size`, in the face at `primary` or in
    /// fallbacks, and how wide the row is.
    fn shape(
        &mut self,
        text: &str,
        primary: usize,
        style: &FontStyle,
        size: f32,
    ) -> (Vec<Placed>, f32) {
        // Runs of characters in the same face, shaped one by one.
        let mut runs: Vec<(usize, Range<usize>)> = Vec::new();
        for (at, c) in text.char_indices() {
            let face = self.face_for(c, primary, style);
            let end = at + c.len_utf8();
            match runs.last_mut() {
                Some((last, range)) if *last == face => range.end = end,
                _ => runs.push((face, at..end)),
            }
        }
        let (mut glyphs, mut x) = (Vec::new(), 0.0);
        for (face, range) in runs {
            let Some(shaping) = &self.faces[face] else {
                continue;
            };
            let Ok(font) = shaping.font() else {
                continue;
            };
            let run = &text[range];
            let shaper = (shaping.data.shaper(&font))
                .instance(Some(&shaping.instance))
                .build();
            let scale = size / shaper.units_per_em() as f32;
            let mut buffer = UnicodeBuffer::new();
            buffer.push_str(run);
            buffer.guess_segment_properties();
            let shaped = shaper.shape(buffer, ShapeOptions::new());
            for (info, position) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
                let c = run[info.cluster as usize..].chars().next();
                if c == Some('\t') {
                    x += TAB_SPACES * shaping.space(&font, size);
                    continue;
                }
                if !c.is_some_and(char::is_control) {
                    glyphs.push(Placed {
                        face,
                        glyph: GlyphId::new(info.glyph_id),
                        x: x + position.x_offset as f32 * scale,
                        y: position.y_offset as f32 * scale,
                    });
                }
                x += position.x_advance as f32 * scale;
            }
        }
        (glyphs, x)
    }

    /// Adds to `d` the outlines of `glyphs` at `size`, placed from `start` on
    /// `baseline`.
    fn draw(&self, glyphs: &[Placed], start: f32, baseline: f32, size: f32, d: &mut String) {
        let mut outlines: Option<(usize, OutlineGlyphCollection)> = None;
        for placed in glyphs {
            let Some(shaping) = &self.faces[placed.face] else {
                continue;
            };
            if outlines
                .as_ref()
                .is_none_or(|(face, _)| *face != placed.face)
            {
                let Ok(font) = shaping.font() else {
                    continue;
                };
                outlines = Some((placed.face, font.outline_glyphs()));
            }
            let Some(glyph) = outlines
                .as_ref()
                .and_then(|(_, glyphs)| glyphs.get(placed.glyph))
            else {
                continue;
            };
            let mut pen = SvgPen {
                d: &mut *d,
                x: start + placed.x,
                y: baseline - placed.y,
            };
            let settings = DrawSettings::unhinted(Size::new(size), &shaping.location);
            // A glyph that can't be drawn is left out, as egui leaves it.
            let _ = glyph.draw(settings, &mut pen);
        }
    }
}

impl Shaping {
    fn new(face: Face) -> Option<Self> {
        let font = FontRef::from_index((*face.data).as_ref(), face.index).ok()?;
        let data = ShaperData::new(&font);
        let variations = (face.variations.iter()).map(|&(tag, value)| Variation { tag, value });
        let instance = ShaperInstance::from_variations(&font, variations);
        let location = font.axes().location(face.variations.iter().copied());
        Some(Shaping {
            face,
            data,
            instance,
            location,
        })
    }

    fn font(&self) -> Result<FontRef<'_>, skrifa::raw::ReadError> {
        FontRef::from_index((*self.face.data).as_ref(), self.face.index)
    }

    /// How far a space advances at `size`, in `font`, which is its face's.
    fn space(&self, font: &FontRef, size: f32) -> f32 {
        let metrics = font.glyph_metrics(Size::new(size), &self.location);
        let space = font.charmap().map(' ');
        space
            .and_then(|glyph| metrics.advance_width(glyph))
            .unwrap_or_default()
    }
}

/// Whether `face` has an outline for `c`, which a color bitmap font lacks.
fn has_outline(face: &Face, c: char) -> bool {
    let Ok(font) = FontRef::from_index((*face.data).as_ref(), face.index) else {
        return false;
    };
    let glyph = font.charmap().map(c);
    glyph.is_some_and(|glyph| font.outline_glyphs().get(glyph).is_some())
}

/// Writes outlines as SVG path data, flipped upright from `x`, `y`.
struct SvgPen<'a> {
    d: &'a mut String,
    x: f32,
    y: f32,
}

impl SvgPen<'_> {
    fn point(&mut self, x: f32, y: f32) {
        let _ = write!(self.d, "{} {}", num(self.x + x), num(self.y - y));
    }
}

impl OutlinePen for SvgPen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.d.push('M');
        self.point(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.d.push('L');
        self.point(x, y);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.d.push('Q');
        self.point(cx0, cy0);
        self.d.push(' ');
        self.point(x, y);
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.d.push('C');
        self.point(cx0, cy0);
        self.d.push(' ');
        self.point(cx1, cy1);
        self.d.push(' ');
        self.point(x, y);
    }

    fn close(&mut self) {
        self.d.push('Z');
    }
}
