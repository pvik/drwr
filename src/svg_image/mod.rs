//! The SVG drawings yEd's SVG nodes show, parsed once and drawn at any size.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use egui::{ColorImage, Rect, Vec2, pos2, vec2};
use resvg::{tiny_skia, usvg};

/// How many pixels the longer side of a drawing's mask has.
const MASK_SIZE: f32 = 256.0;
/// How much of a mask pixel must be drawn on to count, out of 255, so that
/// lines much thinner than a pixel count.
const MASK_THRESHOLD: u8 = 8;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// An SVG drawing. Like yEd, a node shows what it draws stretched to fill the
/// node, leaving out the empty margins of its canvas.
pub struct SvgImage {
    tree: usvg::Tree,
    text: String,
    /// What it draws covers, in its own units, which are points.
    bounds: Rect,
    mask: Mask,
    id: u64,
}

/// Which pixels of a drawing's bounds it draws on.
struct Mask {
    width: usize,
    height: usize,
    alpha: Vec<u8>,
}

impl SvgImage {
    /// Parses `text`, failing if it isn't SVG or draws nothing.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut options = usvg::Options::default();
        // Only embedded images, as a yEd file keeps no others.
        options.image_href_resolver.resolve_string = Box::new(|_, _| None);
        let tree = usvg::Tree::from_str(text, &options).map_err(|err| err.to_string())?;
        let bounds = drawn_bounds(tree.root());
        if !bounds.is_positive() {
            return Err("It draws nothing.".to_owned());
        }
        let mask = Mask::of(&tree, bounds);
        Ok(SvgImage {
            tree,
            text: text.to_owned(),
            bounds,
            mask,
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        })
    }

    /// The SVG as written.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// What it draws covers, in its own units.
    pub fn bounds(&self) -> Rect {
        self.bounds
    }

    /// The size of its canvas, from its `width` and `height`.
    pub fn canvas_size(&self) -> Vec2 {
        let size = self.tree.size();
        vec2(size.width(), size.height())
    }

    /// Tells drawings apart, unlike pointers, which can be reused.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Whether it draws at `at` in a node it fills, as fractions of the node's
    /// width and height from its top-left corner.
    pub fn draws_at(&self, at: Vec2) -> bool {
        let Mask {
            width,
            height,
            alpha,
        } = &self.mask;
        if !(0.0..=1.0).contains(&at.x) || !(0.0..=1.0).contains(&at.y) {
            return false;
        }
        let x = ((at.x * *width as f32) as usize).min(width - 1);
        let y = ((at.y * *height as f32) as usize).min(height - 1);
        alpha[y * width + x] >= MASK_THRESHOLD
    }

    /// The first point it draws at on the way from `from` to `to`, both given
    /// as for [`Self::draws_at`].
    pub fn first_drawn(&self, from: Vec2, to: Vec2) -> Option<Vec2> {
        let mask = vec2(self.mask.width as f32, self.mask.height as f32);
        // Every half a mask pixel, so none is skipped.
        let steps = (((to - from) * mask).length() * 2.0).ceil().max(1.0);
        let at = |t: f32| from + (to - from) * t;
        let hit = (0..=steps as u32)
            .map(|step| step as f32 / steps)
            .find(|&t| self.draws_at(at(t)))?;
        let (mut missed, mut hit) = ((hit - 1.0 / steps).max(0.0), hit);
        for _ in 0..8 {
            let middle = (missed + hit) / 2.0;
            if self.draws_at(at(middle)) {
                hit = middle;
            } else {
                missed = middle;
            }
        }
        Some(at(hit))
    }

    /// What it draws, stretched to `width` by `height` pixels.
    pub fn image(&self, width: u32, height: u32) -> Option<ColorImage> {
        let pixmap = render(&self.tree, self.bounds, width, height)?;
        let size = [pixmap.width() as usize, pixmap.height() as usize];
        Some(ColorImage::from_rgba_premultiplied(size, pixmap.data()))
    }
}

impl Mask {
    fn of(tree: &usvg::Tree, bounds: Rect) -> Self {
        let scale = MASK_SIZE / bounds.width().max(bounds.height());
        let side = |length: f32| (length * scale).ceil().max(1.0) as u32;
        let (width, height) = (side(bounds.width()), side(bounds.height()));
        let alpha = render(tree, bounds, width, height)
            .map(|pixmap| pixmap.pixels().iter().map(|pixel| pixel.alpha()).collect())
            .unwrap_or_else(|| vec![0; (width * height) as usize]);
        Mask {
            width: width as usize,
            height: height as usize,
            alpha,
        }
    }
}

/// What `group` draws covers, like yEd's visible bounds: with the width of
/// strokes, but without shapes left unpainted, such as icons' spacers.
fn drawn_bounds(group: &usvg::Group) -> Rect {
    let mut bounds = Rect::NOTHING;
    if group.opacity().get() == 0.0 {
        return bounds;
    }
    let rect = |rect: tiny_skia::Rect| {
        Rect::from_min_max(
            pos2(rect.left(), rect.top()),
            pos2(rect.right(), rect.bottom()),
        )
    };
    for node in group.children() {
        let drawn = match node {
            usvg::Node::Group(group) => drawn_bounds(group),
            usvg::Node::Path(path) => {
                let painted = path.fill().is_some_and(|fill| fill.opacity().get() > 0.0)
                    || (path.stroke()).is_some_and(|stroke| stroke.opacity().get() > 0.0);
                match path.is_visible() && painted {
                    true => rect(path.abs_stroke_bounding_box()),
                    false => Rect::NOTHING,
                }
            }
            usvg::Node::Image(image) if image.is_visible() => rect(image.abs_bounding_box()),
            usvg::Node::Image(_) => Rect::NOTHING,
            usvg::Node::Text(text) => rect(text.abs_stroke_bounding_box()),
        };
        bounds = bounds.union(drawn);
    }
    bounds
}

/// The part of `tree` within `bounds`, stretched to `width` by `height`
/// pixels.
fn render(tree: &usvg::Tree, bounds: Rect, width: u32, height: u32) -> Option<tiny_skia::Pixmap> {
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    let scale = vec2(width as f32, height as f32) / bounds.size();
    let transform = tiny_skia::Transform::from_row(
        scale.x,
        0.0,
        0.0,
        scale.y,
        -bounds.min.x * scale.x,
        -bounds.min.y * scale.y,
    );
    resvg::render(tree, transform, &mut pixmap.as_mut());
    Some(pixmap)
}

impl fmt::Debug for SvgImage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SvgImage")
            .field("bounds", &self.bounds)
            .field("text_len", &self.text.len())
            .finish_non_exhaustive()
    }
}

/// Drawings parsed from the same text are the same.
impl PartialEq for SvgImage {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

#[cfg(test)]
mod tests;
