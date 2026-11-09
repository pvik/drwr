//! The canvas's grid lines, spaced to suit the zoom.

use egui::emath::TSTransform;
use egui::{Color32, Mesh, Painter, Rangef, Rect};

/// How far apart the lines can be, in graph points. Each spacing is a whole
/// multiple of the one before, so the lines of the finer spacings fade in
/// between those of the coarser as the view zooms in.
const SPACINGS: [f64; 7] = [1.0, 5.0, 10.0, 50.0, 100.0, 500.0, 1000.0];
/// Lines closer together on screen, in points, aren't drawn.
const MIN_GAP: f32 = 5.0;
/// Lines this far apart on screen are drawn as dark as [`FAINT`], fading out
/// to nothing at [`MIN_GAP`]...
const FAINT_GAP: f32 = 10.0;
/// ...and lines this far apart, or more, as dark as [`DARK`].
const DARK_GAP: f32 = 100.0;
/// How dark lines are drawn, from 0 (white) to 1 (black).
const FAINT: f32 = 0.065;
const DARK: f32 = 0.17;

/// Paints grid lines across the canvas at `rect`, which `to_screen` takes
/// graph coordinates to. They're a pixel wide at any zoom.
pub fn paint(painter: &Painter, rect: Rect, to_screen: TSTransform) {
    let pixels_per_point = painter.pixels_per_point();
    // The pixel egui would put a 1 px border at `at` on.
    let pixel = |at: f32| {
        let start = (at * pixels_per_point).floor();
        Rangef::new(start, start + 1.0) / pixels_per_point
    };
    let (origin, zoom) = (to_screen.translation, to_screen.scaling);
    let across = lines(rect.x_range(), origin.x, zoom)
        .into_iter()
        .map(|(x, darkness)| (darkness, Rect::from_x_y_ranges(pixel(x), rect.y_range())));
    let down = lines(rect.y_range(), origin.y, zoom)
        .into_iter()
        .map(|(y, darkness)| (darkness, Rect::from_x_y_ranges(rect.x_range(), pixel(y))));
    let mut lines: Vec<_> = across.chain(down).collect();
    // The darker over the fainter, where they cross.
    lines.sort_by(|(a, _), (b, _)| a.total_cmp(b));
    let mut mesh = Mesh::default();
    for (darkness, line) in lines {
        let gray = (255.0 * (1.0 - darkness)).round() as u8;
        // Those just fading in show nothing yet.
        if gray < 255 {
            mesh.add_colored_rect(line, Color32::from_gray(gray));
        }
    }
    painter.add(mesh);
}

/// Where lines cross `range` on screen along an axis, on which graph point 0
/// is at `origin` and `zoom` screen points make a graph point, with how dark
/// each is drawn: the darker, the wider the spacing it's a multiple of shows.
fn lines(range: Rangef, origin: f32, zoom: f32) -> Vec<(f32, f32)> {
    let gap = |spacing: f64| spacing as f32 * zoom;
    let Some(finest) = SPACINGS.iter().position(|&s| gap(s) >= MIN_GAP) else {
        return Vec::new();
    };
    let spacing = SPACINGS[finest];
    let step = f64::from(gap(spacing));
    let first = (f64::from(range.min - origin) / step).ceil() as i64;
    let last = (f64::from(range.max - origin) / step).floor() as i64;
    (first..=last)
        .map(|line| {
            // The widest spacing it's a multiple of, which the finest always is.
            let widest = (SPACINGS[finest..].iter().rev())
                .find(|&&wide| line % (wide / spacing).round() as i64 == 0)
                .unwrap_or(&spacing);
            let at = f64::from(origin) + line as f64 * step;
            (at as f32, darkness(gap(*widest)))
        })
        .collect()
}

/// How dark lines `gap` points apart on screen are drawn.
fn darkness(gap: f32) -> f32 {
    let fraction = |from: f32, to: f32| ((gap - from) / (to - from)).clamp(0.0, 1.0);
    if gap < FAINT_GAP {
        FAINT * fraction(MIN_GAP, FAINT_GAP)
    } else {
        FAINT + (DARK - FAINT) * fraction(FAINT_GAP, DARK_GAP)
    }
}

#[cfg(test)]
mod tests;
