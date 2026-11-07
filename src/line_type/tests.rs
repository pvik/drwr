use egui::{pos2, vec2};

use super::*;

/// The pieces of `strips` that dashes cover, as pairs of (x, y).
fn pieces(strips: &[[Pos2; 2]], pattern: &[[f32; 2]], unit: f32) -> Vec<[(f32, f32); 2]> {
    dashes(strips, pattern, unit)
        .into_iter()
        .flatten()
        .map(xy)
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
fn patterns_can_alternate_several_dashes() {
    let line = [[pos2(0.0, 25.0), pos2(0.0, 5.0)]];
    assert_eq!(
        pieces(&line, &[[4.0, 2.0], [1.0, 2.0]], 1.0),
        [
            [(0.0, 25.0), (0.0, 21.0)],
            [(0.0, 19.0), (0.0, 18.0)],
            [(0.0, 16.0), (0.0, 12.0)],
            [(0.0, 10.0), (0.0, 9.0)],
            [(0.0, 7.0), (0.0, 5.0)],
        ]
    );
}

#[test]
fn dashes_run_on_from_one_strip_to_the_next() {
    // Like the sides of an outline, which needn't meet.
    let strips = [
        [pos2(0.0, 0.0), pos2(5.0, 0.0)],
        [pos2(6.0, 1.0), pos2(6.0, 11.0)],
    ];
    let dashes: Vec<Vec<_>> = dashes(&strips, &[[8.0, 2.0]], 1.0)
        .into_iter()
        .map(|dash| dash.into_iter().map(xy).collect())
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
fn ellipses_are_drawn_as_polygons_clockwise_from_the_top() {
    let (center, radius) = (pos2(100.0, 50.0), vec2(75.0, 40.0));
    let points = ellipse_points(center, radius, 1.0);
    assert_eq!(points.len(), 32);
    assert_eq!(points[0], pos2(100.0, 10.0));
    assert_eq!(points[8], pos2(175.0, 50.0));
    assert_eq!(points[16], pos2(100.0, 90.0));
    assert_eq!(points[24], pos2(25.0, 50.0));
    for pair in points[..9].windows(2) {
        assert!(pair[1].x > pair[0].x && pair[1].y > pair[0].y, "{pair:?}");
    }
    for point in &points {
        let offset = (*point - center) / radius;
        assert!((offset.length() - 1.0).abs() < 1e-5, "{point:?}");
    }
}

#[test]
fn bigger_ellipses_get_more_corners() {
    assert_eq!(ellipse_points(Pos2::ZERO, vec2(10.0, 10.0), 1.0).len(), 32);
    assert_eq!(
        ellipse_points(Pos2::ZERO, vec2(400.0, 100.0), 1.0).len(),
        100
    );
    assert_eq!(
        ellipse_points(Pos2::ZERO, vec2(400.0, 100.0), 2.0).len(),
        200
    );
}

#[test]
fn ellipses_without_width_or_height_have_no_corners() {
    assert!(ellipse_points(Pos2::ZERO, vec2(0.0, 10.0), 1.0).is_empty());
    assert!(ellipse_points(Pos2::ZERO, vec2(10.0, 0.0), 1.0).is_empty());
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
