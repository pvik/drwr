use std::collections::HashMap;
use std::path::PathBuf;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use resvg::tiny_skia::{self, ColorU8, Pixmap};
use resvg::usvg;

use super::*;

/// The icon, a tile filling its square canvas.
const SVG: &str = include_str!("../../assets/app_icon/drwr.svg");

/// How many pixels across drwr.png is.
const PNG_SIZE: u32 = 256;

/// The Windows icon's sizes: small and large icons at 100% to 200% scaling,
/// and Explorer's biggest.
const ICO_SIZES: [u32; 8] = [16, 20, 24, 32, 40, 48, 64, 256];

/// The macOS icon's images, as `(type, pixels across)`. The 16 and 32 pixel
/// ones are RLE-compressed RGB plus an 8-bit mask, which every macOS reads;
/// the others are PNGs, two of each size for normal and Retina screens.
const ICNS_IMAGES: [(&[u8; 4], u32); 12] = [
    (b"is32", 16),
    (b"s8mk", 16),
    (b"il32", 32),
    (b"l8mk", 32),
    (b"ic11", 32),
    (b"ic12", 64),
    (b"ic07", 128),
    (b"ic13", 256),
    (b"ic08", 256),
    (b"ic14", 512),
    (b"ic09", 512),
    (b"ic10", 1024),
];

fn dir() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/app_icon"))
}

/// The icon files, as `(name, contents)`.
fn files() -> [(&'static str, Vec<u8>); 3] {
    [
        ("drwr.png", png(&render(SVG, PNG_SIZE))),
        ("drwr.ico", ico()),
        ("drwr.icns", icns()),
    ]
}

/// Redraws the icon files from drwr.svg.
#[test]
#[ignore = "writes assets/app_icon/: run it after editing drwr.svg"]
fn write_files() {
    for (name, contents) in files() {
        std::fs::write(dir().join(name), contents).unwrap();
    }
}

#[test]
fn files_match_the_svg() {
    let read = |name: &str| std::fs::read(dir().join(name)).unwrap();

    assert_drawn("drwr.png", &read("drwr.png"), &render(SVG, PNG_SIZE));

    let ico = read("drwr.ico");
    let images = ico_images(&ico);
    let sizes: Vec<u32> = images.iter().map(|&(size, _)| size).collect();
    assert_eq!(sizes, ICO_SIZES);
    let &(size, largest) = images.last().unwrap();
    assert_drawn("drwr.ico", largest, &render(SVG, size));

    let icns = read("drwr.icns");
    let images = icns_images(&icns);
    let types: Vec<[u8; 4]> = images.iter().map(|&(kind, _)| kind).collect();
    assert_eq!(types, ICNS_IMAGES.map(|(kind, _)| *kind));
    let &(_, ic08) = images.iter().find(|(kind, _)| kind == b"ic08").unwrap();
    assert_drawn("drwr.icns", ic08, &render(&mac_svg(), 256));
}

#[test]
fn window_icon_is_the_tile() {
    let icon = window_icon();
    if cfg!(target_os = "macos") {
        assert!(icon.is_empty());
        return;
    }
    assert_eq!([icon.width, icon.height], [PNG_SIZE; 2]);
    let pixel = |x: usize, y: usize| &icon.rgba[4 * (y * PNG_SIZE as usize + x)..][..4];
    // Past the tile's rounded corner, then inside the box.
    assert_eq!(pixel(1, 1)[3], 0);
    assert_eq!(pixel(95, 86), [0xad, 0xd8, 0xe6, 0xff]);
}

/// Asserts that the PNG `png`, from the file `name`, is `expected` give or
/// take rounding, which may differ by processor.
fn assert_drawn(name: &str, png: &[u8], expected: &Pixmap) {
    let actual = Pixmap::decode_png(png).unwrap();
    let size = |pixmap: &Pixmap| [pixmap.width(), pixmap.height()];
    assert_eq!(size(&actual), size(expected), "{name}");
    let differs = |(a, b): (&u8, &u8)| a.abs_diff(*b) > 8;
    assert!(
        !actual.data().iter().zip(expected.data()).any(differs),
        "{name} isn't drawn from drwr.svg; redraw it with `cargo test app_icon -- --ignored`"
    );
}

/// The images in the .ico file `ico`, as `(pixels across, data)`.
fn ico_images(ico: &[u8]) -> Vec<(u32, &[u8])> {
    assert_eq!(ico[..4], [0, 0, 1, 0]);
    let count = u16::from_le_bytes([ico[4], ico[5]]);
    (0..usize::from(count))
        .map(|i| {
            let entry = &ico[6 + 16 * i..][..16];
            let size = if entry[0] == 0 { 256 } else { entry[0].into() };
            let len = u32::from_le_bytes(entry[8..12].try_into().unwrap()) as usize;
            let offset = u32::from_le_bytes(entry[12..].try_into().unwrap()) as usize;
            (size, &ico[offset..offset + len])
        })
        .collect()
}

/// The images in the .icns file `icns`, as `(type, data)`.
fn icns_images(icns: &[u8]) -> Vec<([u8; 4], &[u8])> {
    let len = |bytes: &[u8]| u32::from_be_bytes(bytes[4..8].try_into().unwrap()) as usize;
    assert_eq!(icns[..4], *b"icns");
    assert_eq!(len(icns), icns.len());
    let mut images = Vec::new();
    let mut rest = &icns[8..];
    while !rest.is_empty() {
        let (image, after) = rest.split_at(len(rest));
        images.push((image[..4].try_into().unwrap(), &image[8..]));
        rest = after;
    }
    images
}

/// The square SVG image `svg`, drawn `size` pixels wide and high.
fn render(svg: &str, size: u32) -> Pixmap {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).unwrap();
    let mut pixmap = Pixmap::new(size, size).unwrap();
    let scale = size as f32 / tree.size().width();
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap
}

/// The pixels of `pixmap` as RGBA without premultiplied alpha.
fn rgba(pixmap: &Pixmap) -> Vec<u8> {
    let pixels = pixmap.pixels().iter().map(|pixel| pixel.demultiply());
    pixels
        .flat_map(|color| [color.red(), color.green(), color.blue(), color.alpha()])
        .collect()
}

fn png(pixmap: &Pixmap) -> Vec<u8> {
    let mut data = Vec::new();
    let mut encoder = png::Encoder::new(&mut data, pixmap.width(), pixmap.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.set_compression(png::Compression::Best);
    encoder.set_adaptive_filter(png::AdaptiveFilterType::Adaptive);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&rgba(pixmap)).unwrap();
    writer.finish().unwrap();
    data
}

/// A Windows icon. Images below 256 pixels are bitmaps, which every reader
/// takes.
fn ico() -> Vec<u8> {
    let images: Vec<Vec<u8>> = ICO_SIZES
        .iter()
        .map(|&size| {
            let encode = if size < 256 { bmp } else { png };
            encode(&render(SVG, size))
        })
        .collect();
    // Type 1 is an icon.
    let mut ico = vec![0, 0, 1, 0];
    ico.extend((images.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * images.len();
    for (&size, image) in ICO_SIZES.iter().zip(&images) {
        // Width and height, where 256 wraps around to 0, no palette, then
        // reserved; 1 color plane of 32 bits per pixel.
        ico.extend([size as u8, size as u8, 0, 0, 1, 0, 32, 0]);
        ico.extend((image.len() as u32).to_le_bytes());
        ico.extend((offset as u32).to_le_bytes());
        offset += image.len();
    }
    ico.extend(images.concat());
    ico
}

/// `pixmap` as an icon's bitmap: a header, the rows bottom-up in BGRA, then
/// a 1-bit mask of the transparent pixels, which the format needs even with
/// alpha.
fn bmp(pixmap: &Pixmap) -> Vec<u8> {
    let (width, height) = (pixmap.width(), pixmap.height());
    let mask_row_len = width.div_ceil(32) * 4;
    // BITMAPINFOHEADER, whose height counts the mask's rows too.
    let mut bmp = Vec::new();
    for field in [40, width, 2 * height] {
        bmp.extend(field.to_le_bytes());
    }
    // 1 color plane of 32 bits per pixel, uncompressed.
    bmp.extend([1, 0, 32, 0, 0, 0, 0, 0]);
    bmp.extend((height * (4 * width + mask_row_len)).to_le_bytes());
    // No resolution or palette.
    bmp.extend([0; 16]);
    let rows = || pixmap.pixels().chunks(width as usize).rev();
    for row in rows() {
        for pixel in row {
            let color = pixel.demultiply();
            bmp.extend([color.blue(), color.green(), color.red(), color.alpha()]);
        }
    }
    for row in rows() {
        let mut mask = vec![0; mask_row_len as usize];
        for (x, pixel) in row.iter().enumerate() {
            if pixel.alpha() == 0 {
                mask[x / 8] |= 0x80 >> (x % 8);
            }
        }
        bmp.extend(mask);
    }
    bmp
}

/// A macOS icon, which Finder and the Dock show.
fn icns() -> Vec<u8> {
    let svg = mac_svg();
    let mut pixmaps = HashMap::new();
    // The file's length goes after its type.
    let mut icns = b"icns\0\0\0\0".to_vec();
    for (kind, size) in ICNS_IMAGES {
        let pixmap = pixmaps.entry(size).or_insert_with(|| render(&svg, size));
        let image = match kind {
            b"is32" | b"il32" => rle_rgb(pixmap),
            b"s8mk" | b"l8mk" => pixmap.pixels().iter().map(|pixel| pixel.alpha()).collect(),
            _ => png(pixmap),
        };
        icns.extend(kind);
        icns.extend((8 + image.len() as u32).to_be_bytes());
        icns.extend(image);
    }
    let len = icns.len() as u32;
    icns[4..8].copy_from_slice(&len.to_be_bytes());
    icns
}

/// The red, green and blue planes of `pixmap` one after another, each
/// RLE-compressed for is32 and il32 images. All runs here are literal: a
/// byte n, then n + 1 bytes.
fn rle_rgb(pixmap: &Pixmap) -> Vec<u8> {
    let colors: Vec<ColorU8> = pixmap.pixels().iter().map(|p| p.demultiply()).collect();
    let mut rle = Vec::new();
    for channel in [ColorU8::red, ColorU8::green, ColorU8::blue] {
        let plane: Vec<u8> = colors.iter().copied().map(channel).collect();
        for run in plane.chunks(128) {
            rle.push(run.len() as u8 - 1);
            rle.extend(run);
        }
    }
    rle
}

/// The icon on Apple's grid for macOS 11 and later: the tile 824 of 1024
/// pixels across, in the middle, casting a shadow.
fn mac_svg() -> String {
    let icon = STANDARD.encode(SVG);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <filter id="shadow"><feDropShadow dx="0" dy="10" stdDeviation="10" flood-opacity=".3"/></filter>
  <image x="100" y="100" width="824" height="824" filter="url(#shadow)" href="data:image/svg+xml;base64,{icon}"/>
</svg>"#
    )
}
