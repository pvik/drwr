use egui::pos2;

use super::*;

/// The pieces of `strips` that dashes cover, as pairs of (x, y).
fn pieces(strips: &[[Pos2; 2]], pattern: &[[f32; 2]], unit: f32) -> Vec<[(f32, f32); 2]> {
    dashes(strips, pattern, unit)
        .into_iter()
        .map(|[from, to]| [(from.x, from.y), (to.x, to.y)])
        .collect()
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
    assert_eq!(
        pieces(&strips, &[[8.0, 2.0]], 1.0),
        [
            [(0.0, 0.0), (5.0, 0.0)],
            [(6.0, 1.0), (6.0, 4.0)],
            [(6.0, 6.0), (6.0, 11.0)],
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
