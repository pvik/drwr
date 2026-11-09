use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use super::*;

/// yEd's sample: a circle of radius 4 on a 10 by 10 canvas.
const CIRCLE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><circle cx="5" cy="5" r="4"/></svg>"#;

fn svg(body: &str) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10">{body}</svg>"#)
}

#[test]
fn bounds_are_what_it_draws_not_its_canvas() {
    let circle = SvgImage::parse(CIRCLE).unwrap();
    assert_eq!(
        circle.bounds(),
        Rect::from_min_max(pos2(1.0, 1.0), pos2(9.0, 9.0))
    );
    assert_eq!(circle.canvas_size(), vec2(10.0, 10.0));
    // With the stroke, as yEd's visible bounds have it.
    let stroked = svg(r#"<rect x="2" y="2" width="6" height="4" stroke="red" stroke-width="2"/>"#);
    let stroked = SvgImage::parse(&stroked).unwrap();
    assert_eq!(
        stroked.bounds(),
        Rect::from_min_max(pos2(1.0, 1.0), pos2(9.0, 7.0))
    );
}

#[test]
fn only_svg_that_draws_something_is_read() {
    assert!(SvgImage::parse("not svg").is_err());
    assert!(SvgImage::parse("<html/>").is_err());
    let empty = SvgImage::parse(&svg("")).unwrap_err();
    assert_eq!(empty, "It draws nothing.");
    let unpainted = SvgImage::parse(&svg(r#"<rect width="5" height="5" fill="none"/>"#));
    assert!(unpainted.is_err());
}

#[test]
fn unpainted_shapes_are_left_out_of_the_bounds() {
    // Like Material icons, whose first path marks out their canvas.
    let icon = r#"<path d="M0 0h24v24H0z" fill="none"/><path d="M8 6h8v12H8z"/>"#;
    let icon = SvgImage::parse(&svg(icon)).unwrap();
    assert_eq!(
        icon.bounds(),
        Rect::from_min_max(pos2(8.0, 6.0), pos2(16.0, 18.0))
    );
    let faded = r#"<rect width="24" height="24" fill-opacity="0"/><g opacity="0"><rect width="30" height="30"/></g><rect x="1" y="2" width="3" height="4"/>"#;
    let faded = SvgImage::parse(&svg(faded)).unwrap();
    assert_eq!(
        faded.bounds(),
        Rect::from_min_max(pos2(1.0, 2.0), pos2(4.0, 6.0))
    );
}

#[test]
fn draws_at_says_where_it_draws_on_its_bounds() {
    let circle = SvgImage::parse(CIRCLE).unwrap();
    assert!(circle.draws_at(vec2(0.5, 0.5)));
    assert!(circle.draws_at(vec2(0.02, 0.5)));
    assert!(circle.draws_at(vec2(0.5, 0.98)));
    // The corners of its bounds are off the circle.
    assert!(!circle.draws_at(vec2(0.05, 0.05)));
    assert!(!circle.draws_at(vec2(0.95, 0.9)));
    assert!(!circle.draws_at(vec2(-0.01, 0.5)));
    assert!(!circle.draws_at(vec2(0.5, 1.01)));
    // A line much thinner than a mask pixel counts too.
    let line = r#"<path d="M0 0H1000V1000M0 500H1000" fill="none" stroke="black"/>"#;
    let line = svg(line);
    let line = SvgImage::parse(&line).unwrap();
    assert!(line.draws_at(vec2(0.3, 0.5)));
    assert!(!line.draws_at(vec2(0.3, 0.25)));
}

#[test]
fn images_stretch_what_it_draws_to_their_size() {
    let circle = SvgImage::parse(CIRCLE).unwrap();
    let image = circle.image(40, 20).unwrap();
    assert_eq!(image.size, [40, 20]);
    let alpha = |x: usize, y: usize| image.pixels[y * 40 + x].a();
    assert_eq!(alpha(20, 10), 255);
    // The circle touches all four sides, as an ellipse.
    assert!(alpha(0, 10) > 128 && alpha(39, 10) > 128);
    assert!(alpha(20, 0) > 128 && alpha(20, 19) > 128);
    assert_eq!(alpha(1, 1), 0);
    assert!(circle.image(0, 10).is_none());
}

#[test]
fn only_embedded_images_are_drawn() {
    let encoded = STANDARD.encode(CIRCLE);
    let embedded =
        format!(r#"<image width="10" height="10" href="data:image/svg+xml;base64,{encoded}"/>"#);
    assert!(SvgImage::parse(&svg(&embedded)).is_ok());
    let icon = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icons/box.svg");
    let linked = format!(r#"<image width="10" height="10" href="{icon}"/>"#);
    assert!(SvgImage::parse(&svg(&linked)).is_err());
}

#[test]
fn drawings_are_equal_when_their_text_is() {
    let (a, b) = (
        SvgImage::parse(CIRCLE).unwrap(),
        SvgImage::parse(CIRCLE).unwrap(),
    );
    assert_eq!(a, b);
    assert_ne!(a.id(), b.id());
    assert_ne!(
        a,
        SvgImage::parse(&svg(r#"<rect width="5" height="5"/>"#)).unwrap()
    );
}

#[test]
fn first_drawn_is_where_a_line_from_outside_meets_it() {
    // A ring, which a line to its middle meets at its outer edge.
    let ring = r#"<circle cx="50" cy="50" r="45" fill="none" stroke="black" stroke-width="10"/>"#;
    let ring = SvgImage::parse(&svg(ring)).unwrap();
    let met = ring.first_drawn(vec2(0.0, 0.5), vec2(0.5, 0.5)).unwrap();
    assert!((met - vec2(0.0, 0.5)).length() < 0.01, "{met:?}");
    let met = ring.first_drawn(vec2(0.5, 1.0), vec2(0.5, 0.5)).unwrap();
    assert!((met - vec2(0.5, 1.0)).length() < 0.01, "{met:?}");
    // Diagonally, past the empty corner.
    let met = ring.first_drawn(vec2(1.0, 0.0), vec2(0.5, 0.5)).unwrap();
    let corner = 0.5 + 0.5 / 2f32.sqrt();
    assert!(
        (met - vec2(corner, 1.0 - corner)).length() < 0.01,
        "{met:?}"
    );
    // Through the hole only.
    let hole = ring.first_drawn(vec2(0.5, 0.5), vec2(0.6, 0.5));
    assert_eq!(hole, None);
}
