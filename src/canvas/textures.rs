//! The drawings of SVG nodes as textures, at the sizes they're shown at.

use std::cell::RefCell;
use std::collections::HashMap;

use egui::{Context, TextureHandle, TextureId, TextureOptions, Vec2};

use crate::svg_image::SvgImage;

/// The most pixels a drawing is drawn with, so a node zoomed far in can't
/// take all memory; it's scaled up past that.
const MAX_PIXELS: f32 = 8e6;

#[derive(Default)]
pub struct Textures {
    /// By drawing id and size in pixels.
    cache: RefCell<HashMap<(u64, [u32; 2]), Texture>>,
}

struct Texture {
    /// `None` if the drawing can't be drawn at its size.
    handle: Option<TextureHandle>,
    /// Whether it was shown since the last sweep.
    shown: bool,
}

impl Textures {
    /// `image` drawn `size` pixels wide and high, at most as big as `ctx`
    /// allows, as a texture.
    pub fn get(&self, ctx: &Context, image: &SvgImage, size: Vec2) -> Option<TextureId> {
        let max_side = ctx.input(|input| input.max_texture_side) as f32;
        let pixels = pixels(size, max_side)?;
        let mut cache = self.cache.borrow_mut();
        let texture = cache.entry((image.id(), pixels)).or_insert_with(|| {
            let drawn = image.image(pixels[0], pixels[1]);
            let name = format!("svg_image_{}", image.id());
            let handle = drawn.map(|drawn| ctx.load_texture(name, drawn, TextureOptions::LINEAR));
            Texture {
                handle,
                shown: false,
            }
        });
        texture.shown = true;
        texture.handle.as_ref().map(TextureHandle::id)
    }

    /// Drops the textures not shown since the last sweep.
    pub fn sweep(&mut self) {
        let cache = self.cache.get_mut();
        cache.retain(|_, texture| std::mem::take(&mut texture.shown));
    }
}

/// How many pixels wide and high a drawing `size` pixels big is drawn, within
/// limits; `None` if it has no area.
fn pixels(size: Vec2, max_side: f32) -> Option<[u32; 2]> {
    if !(size.x > 0.0 && size.y > 0.0 && size.is_finite()) {
        return None;
    }
    let fit = (max_side / size.max_elem()).min((MAX_PIXELS / (size.x * size.y)).sqrt());
    let size = (size * fit.min(1.0)).round().max(Vec2::splat(1.0));
    Some([size.x as u32, size.y as u32])
}

#[cfg(test)]
pub fn sizes(textures: &Textures) -> Vec<[u32; 2]> {
    let cache = textures.cache.borrow();
    let mut sizes: Vec<_> = cache.keys().map(|(_, size)| *size).collect();
    sizes.sort_unstable();
    sizes
}
