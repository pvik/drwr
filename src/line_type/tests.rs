use egui::{pos2, vec2};

use super::*;

/// The pieces of `strips` that dashes cover, as pairs of (x, y).
fn pieces(strips: &[[Pos2; 2]], pattern: &[[f32; 2]], unit: f32) -> Vec<[(f32, f32); 2]> {
    dashes(strips, pattern, unit)
        .into_iter()
        .map(|piece| xy(piece.line))
        .collect()
}

fn xy([from, to]: [Pos2; 2]) -> [(f32, f32); 2] {
    [(from.x, from.y), (to.x, to.y)]
}

#[test]
fn lines_alternate_dashes_and_gaps_from_their_start() {
    let line = [[pos2(0.0, 0.0), pos2(20.0, 0.0)]];
    assert_eq!(
        pieces(&line, &[[4.0, 2.0]], 1.0),
        [
            [(0.0, 0.0), (4.0, 0.0)],
            [(6.0, 0.0), (10.0, 0.0)],
            [(12.0, 0.0), (16.0, 0.0)],
            // Cut short where the line ends.
            [(18.0, 0.0), (20.0, 0.0)],
        ]
    );
}

#[test]
fn dashes_and_gaps_are_in_units() {
    let line = [[pos2(0.0, 0.0), pos2(20.0, 0.0)]];
    assert_eq!(
        pieces(&line, &[[4.0, 2.0]], 2.5),
        [[(0.0, 0.0), (10.0, 0.0)], [(15.0, 0.0), (20.0, 0.0)]]
    );
}

#[test]
fn patterns_can_alternate_dashes_and_dots() {
    let line = [[pos2(0.0, 25.0), pos2(0.0, 5.0)]];
    let pattern = [[4.0, 2.0], [1.0, 2.0]];
    assert_eq!(
        pieces(&line, &pattern, 1.0),
        [
            [(0.0, 25.0), (0.0, 21.0)],
            [(0.0, 19.0), (0.0, 18.0)],
            [(0.0, 16.0), (0.0, 12.0)],
            [(0.0, 10.0), (0.0, 9.0)],
            [(0.0, 7.0), (0.0, 5.0)],
        ]
    );
    let dots: Vec<bool> = dashes(&line, &pattern, 1.0)
        .iter()
        .map(|piece| piece.dot)
        .collect();
    assert_eq!(dots, [false, true, false, true, false]);
}

#[test]
fn dashes_cut_short_are_not_dots() {
    let line = [[pos2(0.0, 0.0), pos2(7.0, 0.0)]];
    let pieces = dashes(&line, &[[4.0, 2.0]], 1.0);
    assert_eq!(xy(pieces[1].line), [(6.0, 0.0), (7.0, 0.0)]);
    assert!(!pieces[1].dot);
}

#[test]
fn dashes_run_on_from_one_strip_to_the_next() {
    // Like the sides of an outline, which needn't meet.
    let strips = [
        [pos2(0.0, 0.0), pos2(5.0, 0.0)],
        [pos2(6.0, 1.0), pos2(6.0, 11.0)],
    ];
    let dashes: Vec<Vec<_>> = dashes(&strips, &[[8.0, 2.0]], 1.0)
        .chunk_by(|_, next| next.runs_on)
        .map(|dash| dash.iter().map(|piece| xy(piece.line)).collect())
        .collect();
    // The first dash is in two pieces, one on each strip.
    assert_eq!(
        dashes,
        [
            vec![[(0.0, 0.0), (5.0, 0.0)], [(6.0, 1.0), (6.0, 4.0)]],
            vec![[(6.0, 6.0), (6.0, 11.0)]],
        ]
    );
}

#[test]
fn pieces_run_the_way_their_strips_do_however_short() {
    // The dash ends a hair past the corner.
    let strips = [
        [pos2(0.0, 0.0), pos2(3.0, 4.0)],
        [pos2(3.0, 4.0), pos2(3.0, 100.0)],
    ];
    let directions: Vec<Vec2> = dashes(&strips, &[[5.000_001, 100.0]], 1.0)
        .iter()
        .map(|piece| piece.direction)
        .collect();
    assert_eq!(directions, [vec2(0.6, 0.8), vec2(0.0, 1.0)]);
}

#[test]
fn dashes_ending_with_a_strip_leave_none_of_the_next() {
    let strips = [
        [pos2(0.0, 0.0), pos2(4.0, 0.0)],
        [pos2(4.0, 0.0), pos2(4.0, 10.0)],
    ];
    assert_eq!(
        pieces(&strips, &[[4.0, 2.0]], 1.0),
        [
            [(0.0, 0.0), (4.0, 0.0)],
            [(4.0, 2.0), (4.0, 6.0)],
            [(4.0, 8.0), (4.0, 10.0)],
        ]
    );
}

#[test]
fn lines_of_no_length_have_no_dashes() {
    let point = pos2(5.0, 5.0);
    assert!(dashes(&[[point, point]], &[[1.0, 1.0]], 1.0).is_empty());
    assert!(dashes(&[], &[[1.0, 1.0]], 1.0).is_empty());
}

#[test]
fn lines_too_long_to_dash_are_solid() {
    for end in [1e9, f32::INFINITY, f32::NAN] {
        let line = [[pos2(0.0, 0.0), pos2(end, 0.0)]];
        assert_eq!(dashes(&line, &[[1.0, 1.0]], 1.0).len(), 1, "{end}");
    }
}

#[test]
fn dashes_round_corners_are_mitered_in_one_piece() {
    // Right, then down.
    let strips = [
        [pos2(0.0, 0.0), pos2(10.0, 0.0)],
        [pos2(10.0, 0.0), pos2(10.0, 10.0)],
    ];
    let dash = dashes(&strips, &[[30.0, 2.0]], 1.0);
    let mut mesh = Mesh::default();
    add_curved_dash(&mut mesh, &dash, 1.0, 1.0, Color32::RED);

    let vertices: Vec<_> = mesh
        .vertices
        .iter()
        .map(|vertex| ((vertex.pos.x, vertex.pos.y), vertex.color))
        .collect();
    // Four across each end and the corner, mitered there, and faded out at
    // the sides.
    let (red, clear) = (Color32::RED, Color32::TRANSPARENT);
    assert_eq!(
        vertices[4..8],
        [
            ((11.5, -1.5), clear),
            ((10.5, -0.5), red),
            ((9.5, 0.5), red),
            ((8.5, 1.5), clear),
        ]
    );
    assert_eq!(vertices.len(), 12);
    assert_eq!(mesh.indices.len(), 2 * 6 * 3);
}

#[test]
fn ellipses_are_drawn_as_polygons_clockwise_from_the_right() {
    let (center, radius) = (pos2(100.0, 50.0), vec2(75.0, 40.0));
    let points = ellipse_points(Rect::from_center_size(center, 2.0 * radius), 1.0);
    assert_eq!(points.len(), 32);
    assert_eq!(points[0], pos2(175.0, 50.0));
    assert_eq!(points[8], pos2(100.0, 90.0));
    assert_eq!(points[16], pos2(25.0, 50.0));
    assert_eq!(points[24], pos2(100.0, 10.0));
    for pair in points[..9].windows(2) {
        assert!(pair[1].x < pair[0].x && pair[1].y > pair[0].y, "{pair:?}");
    }
    for point in &points {
        let offset = (*point - center) / radius;
        assert!((offset.length() - 1.0).abs() < 1e-5, "{point:?}");
    }
}

/// An ellipse of `radius` around zero.
fn ellipse(radius: Vec2) -> Rect {
    Rect::from_center_size(Pos2::ZERO, 2.0 * radius)
}

#[test]
fn bigger_ellipses_get_more_corners() {
    assert_eq!(ellipse_points(ellipse(vec2(10.0, 10.0)), 1.0).len(), 32);
    let big = ellipse(vec2(400.0, 100.0));
    assert_eq!(ellipse_points(big, 1.0).len(), 100);
    assert_eq!(ellipse_points(big, 2.0).len(), 200);
}

#[test]
fn ellipses_of_absurd_sizes_get_a_bounded_number_of_corners() {
    let huge = ellipse(Vec2::splat(1e10));
    assert_eq!(
        ellipse_points(huge, 2.0).len(),
        4 * MAX_QUARTER_POINTS as usize
    );
    let endless = Rect::from_min_size(Pos2::ZERO, vec2(f32::INFINITY, 10.0));
    assert!(ellipse_points(endless, 1.0).is_empty());
}

#[test]
fn flat_or_inverted_ellipses_have_no_corners() {
    for size in [vec2(0.0, 10.0), vec2(10.0, 0.0), vec2(-10.0, 10.0)] {
        let rect = Rect::from_min_size(Pos2::ZERO, size);
        assert!(ellipse_points(rect, 1.0).is_empty(), "{size:?}");
    }
}

#[test]
fn the_side_panel_names_the_types_it_offers() {
    let names = ALL.map(|line_type| name(&line_type).to_owned());
    assert_eq!(names, ["Solid", "Dashed", "Dotted", "Dash-dotted"]);
    assert_eq!(name(&LineType::Other("wavy".to_owned())), "wavy");
}

#[test]
fn unknown_types_are_drawn_solid() {
    assert_eq!(pattern(&LineType::Line), None);
    assert_eq!(pattern(&LineType::Other("wavy".to_owned())), None);
}

#[test]
fn dots_are_square_and_a_width_apart() {
    assert_eq!(pattern(&LineType::Dotted), Some(&[[1.0, 1.0]][..]));
}

/// The cross products of the edges meeting at each corner of `points`,
/// which a convex polygon running counterclockwise on screen has negative.
fn turns(points: &[Pos2]) -> Vec<f32> {
    let n = points.len();
    (0..n)
        .map(|i| {
            let (a, b, c) = (points[i], points[(i + 1) % n], points[(i + 2) % n]);
            (b - a).x * (c - b).y - (b - a).y * (c - b).x
        })
        .collect()
}

#[test]
fn rounded_rects_run_counterclockwise_from_the_top_of_the_left_side() {
    // On pixel centers, where egui puts a solid outline a pixel wide.
    let rect = Rect::from_min_max(pos2(0.5, 0.5), pos2(100.5, 50.5));
    let points = rounded_rect_points(rect, 10.0, Stroke::new(1.0, Color32::RED), 1.0);
    // The ends of each side, then 7 points round the corner after it.
    assert_eq!(points.len(), 4 * (2 + 7));
    assert_eq!(points[0..2], [pos2(0.5, 10.5), pos2(0.5, 40.5)]);
    assert_eq!(points[9..11], [pos2(10.5, 50.5), pos2(90.5, 50.5)]);
    assert_eq!(points[18..20], [pos2(100.5, 40.5), pos2(100.5, 10.5)]);
    assert_eq!(points[27..29], [pos2(90.5, 0.5), pos2(10.5, 0.5)]);
    let centers = [(10.5, 40.5), (90.5, 40.5), (90.5, 10.5), (10.5, 10.5)];
    for (corner, (x, y)) in centers.into_iter().enumerate() {
        for point in &points[9 * corner + 2..9 * corner + 9] {
            let radius = point.distance(pos2(x, y));
            assert!((radius - 10.0).abs() < 1e-4, "{corner}: {point:?}");
        }
    }
    for turn in turns(&points) {
        assert!(turn < 0.0, "{turn}");
    }
}

#[test]
fn corners_are_rounded_by_no_more_than_half_of_each_side() {
    // Like Java's, so 20 across but only 5 down.
    let rect = Rect::from_min_size(Pos2::ZERO, vec2(100.0, 10.0));
    let points = rounded_rect_points(rect, 20.0, Stroke::NONE, 1.0);
    // The left and right sides are rounded all the way, so their ends are
    // the same.
    assert_eq!(points.len(), 2 * (1 + 7) + 2 * (2 + 7));
    assert_eq!(points[0], pos2(0.0, 5.0));
    assert_eq!(points[8..10], [pos2(20.0, 10.0), pos2(80.0, 10.0)]);
    assert_eq!(points[17], pos2(100.0, 5.0));
    assert_eq!(Rect::from_points(&points), rect);
    for turn in turns(&points) {
        assert!(turn < 0.0, "{turn}");
    }
    // Rounded all the way round, like an ellipse.
    let points = rounded_rect_points(rect, 1000.0, Stroke::NONE, 1.0);
    assert_eq!(points.len(), 4 * (1 + 7));
    assert!(turns(&points).iter().all(|turn| *turn < 0.0));
}

#[test]
fn rounded_rects_are_where_egui_puts_a_solid_outline() {
    let rect = Rect::from_min_max(pos2(10.3, 20.6), pos2(110.2, 70.7));
    let outline = |width: f32, pixels_per_point: f32| {
        let stroke = Stroke::new(width, Color32::RED);
        Rect::from_points(&rounded_rect_points(rect, 10.0, stroke, pixels_per_point))
    };
    // On pixel centers for odd widths, else between pixels, as for the fill.
    let centers = Rect::from_min_max(pos2(10.5, 20.5), pos2(110.5, 70.5));
    let between = Rect::from_min_max(pos2(10.0, 21.0), pos2(110.0, 71.0));
    assert_eq!(outline(1.0, 1.0), centers);
    assert_eq!(outline(3.0, 1.0), centers);
    assert_eq!(outline(2.0, 1.0), between);
    assert_eq!(outline(0.0, 1.0), between);
    // At 2 pixels a point, half a point is a pixel.
    let half = Rect::from_min_max(pos2(10.25, 20.75), pos2(110.25, 70.75));
    assert_eq!(outline(0.5, 2.0), half);
}

#[test]
fn corners_rounded_less_than_half_a_pixel_are_square() {
    let rect = Rect::from_min_size(Pos2::ZERO, vec2(100.0, 50.0));
    assert!(rounded_rect_points(rect, 0.0, Stroke::NONE, 1.0).is_empty());
    assert!(rounded_rect_points(rect, 0.4, Stroke::NONE, 1.0).is_empty());
    assert!(!rounded_rect_points(rect, 0.4, Stroke::NONE, 2.0).is_empty());
}

#[test]
fn flat_inverted_or_endless_rects_have_no_rounded_corners() {
    for size in [
        vec2(0.0, 50.0),
        vec2(100.0, 0.0),
        vec2(-100.0, 50.0),
        vec2(f32::INFINITY, 50.0),
    ] {
        let rect = Rect::from_min_size(Pos2::ZERO, size);
        let points = rounded_rect_points(rect, 10.0, Stroke::NONE, 1.0);
        assert!(points.is_empty(), "{size:?}");
    }
}

#[test]
fn rounded_rects_of_absurd_sizes_get_a_bounded_number_of_corners() {
    let huge = Rect::from_min_size(Pos2::ZERO, Vec2::splat(1e10));
    let points = rounded_rect_points(huge, 1e10, Stroke::NONE, 2.0);
    assert_eq!(points.len(), 4 * MAX_QUARTER_POINTS as usize);
}
