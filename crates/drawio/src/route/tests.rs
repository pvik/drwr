use super::*;

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

fn at(x: f64, y: f64) -> Point {
    Point { x, y }
}

fn floating(bounds: Rect) -> End {
    End {
        bounds,
        point: None,
        loose: false,
    }
}

fn fixed(bounds: Rect, point: Point) -> End {
    End {
        bounds,
        point: Some(point),
        loose: false,
    }
}

/// A loose end at `point`, on the small node converting gives it.
fn loose(point: Point) -> End {
    End {
        bounds: rect(point.x - 1.0, point.y - 1.0, 2.0, 2.0),
        point: Some(point),
        loose: true,
    }
}

/// Upper left, and lower right of it, apart both ways.
const SOURCE: Rect = Rect {
    x: 0.0,
    y: 0.0,
    width: 100.0,
    height: 50.0,
};
const TARGET: Rect = Rect {
    x: 300.0,
    y: 100.0,
    width: 100.0,
    height: 50.0,
};

#[test]
fn straight_lines_run_through_the_placed_points() {
    let path = route(
        Router::Straight,
        &floating(SOURCE),
        &fixed(TARGET, at(300.0, 110.0)),
        &[at(150.0, 70.0)],
    );
    assert_eq!(
        path,
        Path {
            source: at(50.0, 25.0),
            target: at(300.0, 110.0),
            bends: vec![at(150.0, 70.0)],
        }
    );
}

#[test]
fn orthogonal_lines_between_diagonal_nodes_bend_once() {
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &floating(TARGET),
        &[],
    );
    // Out of the side, into the top, from middle to middle.
    assert_eq!(path.bends, vec![at(350.0, 25.0)]);
    assert_eq!(
        (path.source, path.target),
        (at(50.0, 25.0), at(350.0, 125.0))
    );
}

#[test]
fn orthogonal_lines_between_stacked_nodes_turn_halfway() {
    let below = rect(30.0, 200.0, 100.0, 50.0);
    let path = route(Router::Orthogonal, &floating(SOURCE), &floating(below), &[]);
    assert_eq!(path.bends, vec![at(50.0, 125.0), at(80.0, 125.0)]);
    let aligned = rect(0.0, 200.0, 100.0, 50.0);
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &floating(aligned),
        &[],
    );
    assert!(path.bends.is_empty());
}

#[test]
fn orthogonal_lines_leave_fixed_points_the_way_their_side_faces() {
    // Out of the right side, into the left one: turn halfway across the gap.
    let path = route(
        Router::Orthogonal,
        &fixed(SOURCE, at(100.0, 25.0)),
        &fixed(TARGET, at(300.0, 125.0)),
        &[],
    );
    assert_eq!(path.bends, vec![at(200.0, 25.0), at(200.0, 125.0)]);
    assert_eq!(
        (path.source, path.target),
        (at(100.0, 25.0), at(300.0, 125.0))
    );
    // Out of the bottom into the left side of a node up and to the right:
    // out, then round.
    let up = rect(300.0, -200.0, 100.0, 50.0);
    let path = route(
        Router::Orthogonal,
        &fixed(SOURCE, at(50.0, 50.0)),
        &fixed(up, at(300.0, -175.0)),
        &[],
    );
    assert_eq!(
        path.bends,
        vec![at(50.0, 70.0), at(280.0, 70.0), at(280.0, -175.0)]
    );
}

#[test]
fn orthogonal_lines_between_side_by_side_nodes_turn_halfway() {
    let beside = rect(300.0, 30.0, 100.0, 50.0);
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &floating(beside),
        &[],
    );
    assert_eq!(path.bends, vec![at(200.0, 25.0), at(200.0, 55.0)]);
    assert_eq!(
        (path.source, path.target),
        (at(50.0, 25.0), at(350.0, 55.0))
    );
    // Either way round, and up rather than down.
    let path = route(
        Router::Orthogonal,
        &floating(beside),
        &floating(SOURCE),
        &[],
    );
    assert_eq!(path.bends, vec![at(200.0, 55.0), at(200.0, 25.0)]);
    let below = rect(30.0, 200.0, 100.0, 50.0);
    let path = route(Router::Orthogonal, &floating(below), &floating(SOURCE), &[]);
    assert_eq!(path.bends, vec![at(80.0, 125.0), at(50.0, 125.0)]);
}

#[test]
fn orthogonal_lines_count_touching_nodes_as_not_apart() {
    let route_to = |target: Rect| {
        route(
            Router::Orthogonal,
            &floating(SOURCE),
            &floating(target),
            &[],
        )
    };
    // Touching side to side but apart up and down: stacked, two bends.
    let path = route_to(rect(100.0, 100.0, 100.0, 50.0));
    assert_eq!(path.bends, vec![at(50.0, 75.0), at(150.0, 75.0)]);
    let path = route_to(rect(300.0, 50.0, 100.0, 50.0));
    assert_eq!(path.bends, vec![at(200.0, 25.0), at(200.0, 75.0)]);
    // Touching and overlapping the other way: straight, no detour.
    let path = route_to(rect(0.0, 50.0, 100.0, 50.0));
    assert_eq!(
        path,
        Path {
            source: at(50.0, 25.0),
            target: at(50.0, 75.0),
            bends: vec![],
        }
    );
    let path = route_to(rect(100.0, 0.0, 100.0, 50.0));
    assert_eq!(
        path,
        Path {
            source: at(50.0, 25.0),
            target: at(150.0, 25.0),
            bends: vec![],
        }
    );
}

#[test]
fn orthogonal_lines_go_out_and_round_from_sides_facing_away() {
    // Out of the right side of a node above another: out, down, back over.
    let below = rect(0.0, 200.0, 100.0, 50.0);
    let path = route(
        Router::Orthogonal,
        &fixed(SOURCE, at(100.0, 25.0)),
        &floating(below),
        &[],
    );
    assert_eq!(
        path.bends,
        vec![at(120.0, 25.0), at(120.0, 180.0), at(50.0, 180.0)]
    );
    assert_eq!(path.target, at(50.0, 225.0));
    // Out of the left side towards a node up and to the right.
    let up = rect(300.0, -200.0, 100.0, 50.0);
    let path = route(
        Router::Orthogonal,
        &fixed(SOURCE, at(0.0, 25.0)),
        &floating(up),
        &[],
    );
    assert_eq!(
        path.bends,
        vec![at(-20.0, 25.0), at(-20.0, -130.0), at(350.0, -130.0)]
    );
    assert_eq!(path.target, at(350.0, -175.0));
    // From a floating end into the bottom of a node lower down.
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &fixed(TARGET, at(350.0, 150.0)),
        &[],
    );
    assert_eq!(
        path.bends,
        vec![at(120.0, 25.0), at(120.0, 170.0), at(350.0, 170.0)]
    );
    assert_eq!(path.source, at(50.0, 25.0));
    // Right above the point it goes to, still out of its side first.
    let under = rect(50.0, 200.0, 100.0, 50.0);
    let path = route(
        Router::Orthogonal,
        &fixed(SOURCE, at(100.0, 25.0)),
        &fixed(under, at(100.0, 200.0)),
        &[],
    );
    assert_eq!(
        path.bends,
        vec![at(120.0, 25.0), at(120.0, 180.0), at(100.0, 180.0)]
    );
}

#[test]
fn orthogonal_lines_leaving_the_same_way_go_round_the_far_side() {
    let path = route(
        Router::Orthogonal,
        &fixed(SOURCE, at(100.0, 25.0)),
        &fixed(TARGET, at(400.0, 125.0)),
        &[],
    );
    assert_eq!(path.bends, vec![at(420.0, 25.0), at(420.0, 125.0)]);
    let path = route(
        Router::Orthogonal,
        &fixed(SOURCE, at(50.0, 0.0)),
        &fixed(TARGET, at(350.0, 100.0)),
        &[],
    );
    assert_eq!(path.bends, vec![at(50.0, -20.0), at(350.0, -20.0)]);
}

#[test]
fn orthogonal_lines_facing_away_cross_between_the_nodes() {
    // Out of the left side, into the right side of a node lower down.
    let left = fixed(SOURCE, at(0.0, 25.0));
    let path = route(
        Router::Orthogonal,
        &left,
        &fixed(TARGET, at(400.0, 125.0)),
        &[],
    );
    assert_eq!(
        path.bends,
        vec![
            at(-20.0, 25.0),
            at(-20.0, 75.0),
            at(420.0, 75.0),
            at(420.0, 125.0)
        ]
    );
    // Or higher up.
    let up = rect(300.0, -200.0, 100.0, 50.0);
    let path = route(
        Router::Orthogonal,
        &left,
        &fixed(up, at(400.0, -175.0)),
        &[],
    );
    assert_eq!(
        path.bends,
        vec![
            at(-20.0, 25.0),
            at(-20.0, -75.0),
            at(420.0, -75.0),
            at(420.0, -175.0)
        ]
    );
    // Out of the top, into the bottom of a node lower down on either side.
    let path = route(
        Router::Orthogonal,
        &fixed(SOURCE, at(50.0, 0.0)),
        &fixed(TARGET, at(350.0, 150.0)),
        &[],
    );
    assert_eq!(
        path.bends,
        vec![
            at(50.0, -20.0),
            at(200.0, -20.0),
            at(200.0, 170.0),
            at(350.0, 170.0)
        ]
    );
    let right = rect(300.0, 0.0, 100.0, 50.0);
    let lower_left = rect(0.0, 100.0, 100.0, 50.0);
    let path = route(
        Router::Orthogonal,
        &fixed(right, at(350.0, 0.0)),
        &fixed(lower_left, at(50.0, 150.0)),
        &[],
    );
    assert_eq!(
        path.bends,
        vec![
            at(350.0, -20.0),
            at(200.0, -20.0),
            at(200.0, 170.0),
            at(50.0, 170.0)
        ]
    );
}

#[test]
fn orthogonal_lines_facing_away_go_past_nodes_that_overlap_or_touch() {
    // Out of the left side, into the right side: below both.
    let into_right = |target: Rect, point: Point| {
        let source = fixed(SOURCE, at(0.0, 25.0));
        route(Router::Orthogonal, &source, &fixed(target, point), &[]).bends
    };
    assert_eq!(
        into_right(rect(300.0, 10.0, 100.0, 30.0), at(400.0, 25.0)),
        vec![
            at(-20.0, 25.0),
            at(-20.0, 70.0),
            at(420.0, 70.0),
            at(420.0, 25.0)
        ]
    );
    assert_eq!(
        into_right(rect(300.0, 10.0, 100.0, 60.0), at(400.0, 40.0)),
        vec![
            at(-20.0, 25.0),
            at(-20.0, 90.0),
            at(420.0, 90.0),
            at(420.0, 40.0)
        ]
    );
    assert_eq!(
        into_right(rect(300.0, 50.0, 100.0, 50.0), at(400.0, 75.0)),
        vec![
            at(-20.0, 25.0),
            at(-20.0, 120.0),
            at(420.0, 120.0),
            at(420.0, 75.0)
        ]
    );
    assert_eq!(
        into_right(rect(300.0, -50.0, 100.0, 50.0), at(400.0, -25.0)),
        vec![
            at(-20.0, 25.0),
            at(-20.0, 70.0),
            at(420.0, 70.0),
            at(420.0, -25.0)
        ]
    );
    // Out of the top, into the bottom of a node below: right of both.
    let top = fixed(SOURCE, at(50.0, 0.0));
    let narrow = rect(20.0, 200.0, 60.0, 50.0);
    let path = route(
        Router::Orthogonal,
        &top,
        &fixed(narrow, at(50.0, 250.0)),
        &[],
    );
    assert_eq!(
        path.bends,
        vec![
            at(50.0, -20.0),
            at(120.0, -20.0),
            at(120.0, 270.0),
            at(50.0, 270.0)
        ]
    );
    let wide = rect(20.0, 200.0, 100.0, 50.0);
    let path = route(Router::Orthogonal, &top, &fixed(wide, at(70.0, 250.0)), &[]);
    assert_eq!(
        path.bends,
        vec![
            at(50.0, -20.0),
            at(140.0, -20.0),
            at(140.0, 270.0),
            at(70.0, 270.0)
        ]
    );
}

#[test]
fn placed_points_fix_the_segments_they_lie_on() {
    // A point beside neither node: its segment runs across, reached by a turn
    // out of the source's bottom, as in draw.io.
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &floating(TARGET),
        &[at(200.0, 80.0)],
    );
    assert_eq!(
        path.bends,
        vec![at(50.0, 80.0), at(200.0, 80.0), at(200.0, 125.0)]
    );
    assert_eq!(
        (path.source, path.target),
        (at(50.0, 25.0), at(350.0, 125.0))
    );
    // A point level with the source: it leaves sideways there.
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &floating(TARGET),
        &[at(200.0, 30.0)],
    );
    assert_eq!(path.bends, vec![at(200.0, 30.0), at(200.0, 125.0)]);
    assert_eq!(path.source, at(50.0, 30.0));
}

#[test]
fn placed_points_beside_the_source_set_the_way_it_leaves() {
    // Below it: it leaves downward, then runs level at the point's height.
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &floating(TARGET),
        &[at(50.0, 80.0)],
    );
    assert_eq!(path.bends, vec![at(50.0, 80.0), at(350.0, 80.0)]);
    assert_eq!(
        (path.source, path.target),
        (at(50.0, 25.0), at(350.0, 125.0))
    );
    // A second point sets the run after that.
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &floating(TARGET),
        &[at(50.0, 80.0), at(200.0, 125.0)],
    );
    assert_eq!(
        path.bends,
        vec![at(50.0, 80.0), at(200.0, 80.0), at(200.0, 125.0)]
    );
    // Level with it: it leaves sideways, and a second point sets the
    // height of the run after the turn.
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &floating(TARGET),
        &[at(200.0, 30.0), at(350.0, 80.0)],
    );
    assert_eq!(
        path.bends,
        vec![at(200.0, 30.0), at(200.0, 80.0), at(350.0, 80.0)]
    );
    assert_eq!(
        (path.source, path.target),
        (at(50.0, 30.0), at(350.0, 125.0))
    );
}

#[test]
fn placed_points_level_with_the_target_set_where_it_enters() {
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &floating(TARGET),
        &[at(200.0, 110.0)],
    );
    assert_eq!(path.bends, vec![at(200.0, 25.0), at(200.0, 110.0)]);
    assert_eq!(
        (path.source, path.target),
        (at(50.0, 25.0), at(350.0, 110.0))
    );
    // Level with both, the source decides: it leaves at the point's height
    // and enters the target's middle.
    let beside = rect(300.0, 30.0, 100.0, 50.0);
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &floating(beside),
        &[at(200.0, 40.0)],
    );
    assert_eq!(path.bends, vec![at(200.0, 40.0), at(200.0, 55.0)]);
    assert_eq!(
        (path.source, path.target),
        (at(50.0, 40.0), at(350.0, 55.0))
    );
}

#[test]
fn placed_points_turn_out_of_fixed_points_the_way_they_lie() {
    // Below a point on the bottom: straight down to the point's height.
    let bottom = fixed(SOURCE, at(50.0, 50.0));
    let path = route(
        Router::Orthogonal,
        &bottom,
        &floating(TARGET),
        &[at(50.0, 80.0)],
    );
    assert_eq!(path.bends, vec![at(50.0, 80.0), at(350.0, 80.0)]);
    // Off to the side: down to its height first.
    let path = route(
        Router::Orthogonal,
        &bottom,
        &floating(TARGET),
        &[at(200.0, 80.0)],
    );
    assert_eq!(
        path.bends,
        vec![at(50.0, 80.0), at(200.0, 80.0), at(200.0, 125.0)]
    );
    // Level with the target, from the right side: across to its x first.
    let path = route(
        Router::Orthogonal,
        &fixed(SOURCE, at(100.0, 25.0)),
        &floating(TARGET),
        &[at(200.0, 125.0)],
    );
    assert_eq!(path.bends, vec![at(200.0, 25.0), at(200.0, 125.0)]);
}

#[test]
fn placed_points_on_the_source_are_left_out() {
    // On a floating source: the line leaves through it, and the target end
    // sets the way it runs from there.
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &floating(TARGET),
        &[at(60.0, 30.0)],
    );
    assert_eq!(path.bends, vec![at(60.0, 125.0)]);
    assert_eq!(path.source, at(60.0, 25.0));
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &floating(TARGET),
        &[at(60.0, 30.0), at(200.0, 125.0)],
    );
    assert_eq!(path.bends, vec![at(60.0, 125.0)]);
    // On a fixed start: the next point sets the turn.
    let start = fixed(SOURCE, at(100.0, 25.0));
    let path = route(
        Router::Orthogonal,
        &start,
        &floating(TARGET),
        &[at(100.0, 25.0), at(200.0, 25.0)],
    );
    assert_eq!(path.bends, vec![at(200.0, 25.0), at(200.0, 125.0)]);
    // A lone one there, level with the target: straight across.
    let beside = rect(300.0, 0.0, 100.0, 50.0);
    let path = route(
        Router::Orthogonal,
        &start,
        &floating(beside),
        &[at(100.0, 25.0)],
    );
    assert!(path.bends.is_empty());
    assert_eq!(path.target, at(350.0, 25.0));
}

#[test]
fn placed_points_within_a_point_of_the_sources_middle_line_up_with_it() {
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &floating(TARGET),
        &[at(50.6, 125.0)],
    );
    assert_eq!(path.bends, vec![at(50.0, 125.0)]);
    assert_eq!(path.source, at(50.0, 25.0));
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &floating(TARGET),
        &[at(200.0, 25.6)],
    );
    assert_eq!(path.bends, vec![at(200.0, 25.0), at(200.0, 125.0)]);
    assert_eq!(path.source, at(50.0, 25.0));
}

#[test]
fn placed_points_lead_into_fixed_targets() {
    // Level with a point on the left side: straight in.
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &fixed(TARGET, at(300.0, 125.0)),
        &[at(200.0, 125.0)],
    );
    assert_eq!(path.bends, vec![at(200.0, 25.0), at(200.0, 125.0)]);
    // Above one on the top: straight down into it.
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &fixed(TARGET, at(350.0, 100.0)),
        &[at(200.0, 60.0), at(350.0, 80.0)],
    );
    assert_eq!(
        path.bends,
        vec![at(200.0, 25.0), at(200.0, 60.0), at(350.0, 60.0)]
    );
}

#[test]
fn elbows_bend_halfway_between_the_nodes() {
    let path = route(
        Router::Elbow { vertical: false },
        &floating(SOURCE),
        &floating(TARGET),
        &[],
    );
    assert_eq!(path.bends, vec![at(200.0, 25.0), at(200.0, 125.0)]);
    let path = route(
        Router::Elbow { vertical: true },
        &floating(SOURCE),
        &floating(TARGET),
        &[],
    );
    assert_eq!(path.bends, vec![at(50.0, 75.0), at(350.0, 75.0)]);
    // Through a placed point's x.
    let path = route(
        Router::Elbow { vertical: false },
        &floating(SOURCE),
        &floating(TARGET),
        &[at(250.0, 90.0)],
    );
    assert_eq!(path.bends, vec![at(250.0, 25.0), at(250.0, 125.0)]);
}

#[test]
fn elbows_bend_halfway_between_the_nodes_either_way_round() {
    let path = route(
        Router::Elbow { vertical: false },
        &floating(TARGET),
        &floating(SOURCE),
        &[],
    );
    assert_eq!(path.bends, vec![at(200.0, 125.0), at(200.0, 25.0)]);
    let path = route(
        Router::Elbow { vertical: true },
        &floating(TARGET),
        &floating(SOURCE),
        &[],
    );
    assert_eq!(path.bends, vec![at(350.0, 75.0), at(50.0, 75.0)]);
}

#[test]
fn elbows_run_through_the_placed_point() {
    // Down out of the source at the point's x, level at its height.
    let path = route(
        Router::Elbow { vertical: true },
        &floating(SOURCE),
        &floating(TARGET),
        &[at(20.0, 75.0)],
    );
    assert_eq!(path.bends, vec![at(20.0, 75.0), at(350.0, 75.0)]);
    assert_eq!(path.source, at(20.0, 25.0));
    // Out of the source's side at the point's height.
    let path = route(
        Router::Elbow { vertical: false },
        &floating(SOURCE),
        &floating(TARGET),
        &[at(200.0, 30.0)],
    );
    assert_eq!(path.bends, vec![at(200.0, 30.0), at(200.0, 125.0)]);
    assert_eq!(path.source, at(50.0, 30.0));
    // Into the target's side at the point's height, without a bend on it.
    let path = route(
        Router::Elbow { vertical: true },
        &floating(SOURCE),
        &floating(TARGET),
        &[at(200.0, 110.0)],
    );
    assert_eq!(path.bends, vec![at(50.0, 110.0)]);
    assert_eq!(path.target, at(350.0, 110.0));
}

#[test]
fn elbows_turn_the_way_a_point_beyond_both_nodes_forces() {
    // Below both: top to bottom, though not vertical.
    let path = route(
        Router::Elbow { vertical: false },
        &floating(SOURCE),
        &floating(TARGET),
        &[at(200.0, 200.0)],
    );
    assert_eq!(path.bends, vec![at(50.0, 200.0), at(350.0, 200.0)]);
    // Right of both: side to side, though vertical.
    let path = route(
        Router::Elbow { vertical: true },
        &floating(SOURCE),
        &floating(TARGET),
        &[at(450.0, 75.0)],
    );
    assert_eq!(path.bends, vec![at(450.0, 25.0), at(450.0, 125.0)]);
    // Between them, either way round, each keeps its own way.
    let bends = |source: Rect, target: Rect, vertical: bool| {
        let (source, target) = (floating(source), floating(target));
        route(
            Router::Elbow { vertical },
            &source,
            &target,
            &[at(250.0, 75.0)],
        )
        .bends
    };
    assert_eq!(
        bends(SOURCE, TARGET, true),
        vec![at(50.0, 75.0), at(350.0, 75.0)]
    );
    assert_eq!(
        bends(TARGET, SOURCE, true),
        vec![at(350.0, 75.0), at(50.0, 75.0)]
    );
    assert_eq!(
        bends(TARGET, SOURCE, false),
        vec![at(250.0, 125.0), at(250.0, 25.0)]
    );
}

#[test]
fn elbows_turn_the_other_way_between_nodes_lined_up_edge_to_edge() {
    // One's right side in line with the other's left: top to bottom.
    let path = route(
        Router::Elbow { vertical: false },
        &floating(SOURCE),
        &floating(rect(100.0, 100.0, 100.0, 50.0)),
        &[],
    );
    assert_eq!(path.bends, vec![at(50.0, 75.0), at(150.0, 75.0)]);
    // One's bottom in line with the other's top: side to side.
    let path = route(
        Router::Elbow { vertical: true },
        &floating(SOURCE),
        &floating(rect(300.0, 50.0, 100.0, 50.0)),
        &[],
    );
    assert_eq!(path.bends, vec![at(200.0, 25.0), at(200.0, 75.0)]);
}

#[test]
fn entity_relation_lines_leave_the_facing_sides() {
    let path = route(
        Router::EntityRelation { segment: 30.0 },
        &floating(SOURCE),
        &floating(TARGET),
        &[],
    );
    assert_eq!(path.bends, vec![at(130.0, 25.0), at(270.0, 125.0)]);
}

#[test]
fn entity_relation_lines_leave_the_facing_sides_either_way_round() {
    let path = route(
        Router::EntityRelation { segment: 30.0 },
        &floating(TARGET),
        &floating(SOURCE),
        &[],
    );
    assert_eq!(path.bends, vec![at(270.0, 125.0), at(130.0, 25.0)]);
    assert_eq!(
        (path.source, path.target),
        (at(350.0, 125.0), at(50.0, 25.0))
    );
}

#[test]
fn entity_relation_lines_go_round_nodes_neither_left_of_the_other() {
    // Stacked: out of both right sides and round them.
    let path = route(
        Router::EntityRelation { segment: 30.0 },
        &floating(SOURCE),
        &floating(rect(50.0, 100.0, 100.0, 50.0)),
        &[],
    );
    assert_eq!(path.bends, vec![at(180.0, 25.0), at(180.0, 125.0)]);
    assert_eq!(
        (path.source, path.target),
        (at(50.0, 25.0), at(100.0, 125.0))
    );
}

#[test]
fn entity_relation_lines_between_close_nodes_turn_back_halfway() {
    // Too close for both segments: out, halfway down, back, and in.
    let path = route(
        Router::EntityRelation { segment: 30.0 },
        &floating(SOURCE),
        &floating(rect(120.0, 100.0, 100.0, 50.0)),
        &[],
    );
    assert_eq!(
        path.bends,
        vec![
            at(130.0, 25.0),
            at(130.0, 75.0),
            at(90.0, 75.0),
            at(90.0, 125.0)
        ]
    );
    assert_eq!(
        (path.source, path.target),
        (at(50.0, 25.0), at(170.0, 125.0))
    );
}

#[test]
fn loops_go_round_the_side_their_direction_picks() {
    let node = floating(SOURCE);
    let path = self_loop(&node, &node, None, 10.0, "west");
    assert_eq!(path.bends, vec![at(120.0, 15.0), at(120.0, 35.0)]);
    assert_eq!((path.source, path.target), (at(50.0, 25.0), at(50.0, 25.0)));
    let bends = |direction| self_loop(&node, &node, None, 10.0, direction).bends;
    assert_eq!(bends("east"), vec![at(-20.0, 15.0), at(-20.0, 35.0)]);
    assert_eq!(bends("north"), vec![at(40.0, -20.0), at(60.0, -20.0)]);
    assert_eq!(bends("south"), vec![at(40.0, 70.0), at(60.0, 70.0)]);
    // Fixed ends stay put.
    let fixed = fixed(SOURCE, at(100.0, 10.0));
    let path = self_loop(&fixed, &node, None, 10.0, "west");
    assert_eq!(
        (path.source, path.target),
        (at(100.0, 10.0), at(50.0, 25.0))
    );
}

#[test]
fn loops_go_through_the_point_placed_beside_their_node() {
    let node = floating(SOURCE);
    // Beside it, a loop that far out, at least a segment either way.
    let path = self_loop(&node, &node, Some(at(160.0, 30.0)), 10.0, "west");
    assert_eq!(path.bends, vec![at(160.0, 15.0), at(160.0, 35.0)]);
    let path = self_loop(&node, &node, Some(at(160.0, 60.0)), 10.0, "west");
    assert_eq!(path.bends, vec![at(160.0, -10.0), at(160.0, 60.0)]);
    // Above or below it, a level one from the middle.
    let path = self_loop(&node, &node, Some(at(80.0, -30.0)), 10.0, "west");
    assert_eq!(path.bends, vec![at(20.0, -30.0), at(80.0, -30.0)]);
    // A point on the node counts for nothing, nor one on its outline.
    let path = self_loop(&node, &node, Some(at(20.0, 20.0)), 10.0, "west");
    assert_eq!(path.bends, vec![at(120.0, 15.0), at(120.0, 35.0)]);
    let path = self_loop(&node, &node, Some(at(100.0, 20.0)), 10.0, "west");
    assert_eq!(path.bends, vec![at(120.0, 15.0), at(120.0, 35.0)]);
    let path = self_loop(&node, &node, Some(at(40.0, 0.0)), 10.0, "west");
    assert_eq!(path.bends, vec![at(120.0, 15.0), at(120.0, 35.0)]);
}

#[test]
fn bends_in_a_straight_run_are_dropped() {
    let mut bends = vec![
        at(0.0, 0.0),
        at(5.0, 0.0),
        at(10.0, 0.0),
        at(10.0, 0.2),
        at(10.0, 9.0),
    ];
    simplify(&mut bends);
    assert_eq!(bends, vec![at(0.0, 0.0), at(10.0, 0.0), at(10.0, 9.0)]);
}

#[test]
fn repeated_bends_are_dropped_before_a_slanted_run() {
    let mut bends = vec![at(130.0, 25.0), at(130.0, 25.0), at(270.0, 125.0)];
    simplify(&mut bends);
    assert_eq!(bends, vec![at(130.0, 25.0), at(270.0, 125.0)]);
}

#[test]
fn sequence_messages_run_level_at_their_placed_height() {
    let left = rect(85.0, 110.0, 10.0, 190.0);
    let right = rect(265.0, 120.0, 10.0, 170.0);
    let path = route(
        Router::Sequence,
        &floating(left),
        &floating(right),
        &[at(180.0, 150.0)],
    );
    assert!(path.bends.is_empty());
    assert_eq!(
        (path.source, path.target),
        (at(90.0, 150.0), at(270.0, 150.0))
    );
    // Kept within the span both ends share.
    let path = route(
        Router::Sequence,
        &floating(left),
        &floating(right),
        &[at(180.0, 50.0)],
    );
    assert_eq!(path.source, at(90.0, 120.0));
    // A loose start sets the height.
    let path = route(
        Router::Sequence,
        &loose(at(20.0, 130.0)),
        &floating(left),
        &[],
    );
    assert_eq!(path.target, at(90.0, 130.0));
}

#[test]
fn sequence_messages_fixed_out_of_reach_or_loose_go_straight_as_in_draw_io() {
    let left = rect(85.0, 110.0, 10.0, 190.0);
    // Too high for the lifeline: straight at its middle rather than kinked.
    let start = fixed(rect(0.0, 0.0, 40.0, 60.0), at(40.0, 50.0));
    let path = route(Router::Sequence, &start, &floating(left), &[]);
    assert_eq!(path.bends, vec![]);
    assert_eq!(path.target, at(90.0, 205.0));
    // A loose end keeps its placed point, level or not.
    let path = route(
        Router::Sequence,
        &loose(at(20.0, 50.0)),
        &floating(left),
        &[at(60.0, 150.0)],
    );
    assert_eq!(path.bends, vec![at(60.0, 150.0)]);
}

#[test]
fn orthogonal_lines_to_loose_ends_take_no_side_of_them() {
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &loose(at(50.0, 200.0)),
        &[],
    );
    assert_eq!(path.bends, vec![]);
    assert_eq!(
        (path.source, path.target),
        (at(50.0, 25.0), at(50.0, 200.0))
    );
    // Not round the far side of the end.
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &loose(at(-100.0, 40.0)),
        &[],
    );
    assert_eq!(path.bends, vec![at(-49.5, 25.0), at(-49.5, 40.0)]);
    let path = route(
        Router::Orthogonal,
        &loose(at(-100.0, 10.0)),
        &floating(TARGET),
        &[],
    );
    assert_eq!(path.bends, vec![at(350.0, 10.0)]);
}

#[test]
fn placed_points_on_a_fixed_start_are_dropped_but_still_set_the_first_turn() {
    // As draw.io's segment connector routes them.
    let start = fixed(SOURCE, at(100.0, 25.0));
    let bends = |hints: &[Point]| route(Router::Orthogonal, &start, &floating(TARGET), hints).bends;
    assert_eq!(bends(&[at(100.0, 25.0)]), vec![at(350.0, 25.0)]);
    assert_eq!(
        bends(&[at(100.0, 25.0), at(200.0, 60.0)]),
        vec![at(200.0, 25.0), at(200.0, 125.0)]
    );
    assert_eq!(
        bends(&[at(100.0, 25.0), at(200.0, 125.0)]),
        vec![at(100.0, 125.0)]
    );
}

#[test]
fn sequence_messages_without_a_placed_point_run_level_between_the_middles() {
    let left = rect(85.0, 110.0, 10.0, 190.0);
    let bar = rect(265.0, 120.0, 10.0, 100.0);
    let path = route(Router::Sequence, &floating(left), &floating(bar), &[]);
    assert!(path.bends.is_empty());
    assert_eq!(
        (path.source, path.target),
        (at(90.0, 187.5), at(270.0, 187.5))
    );
    // Ends sharing only one height meet there.
    let below = rect(265.0, 300.0, 10.0, 100.0);
    let path = route(Router::Sequence, &floating(left), &floating(below), &[]);
    assert!(path.bends.is_empty());
    assert_eq!(
        (path.source, path.target),
        (at(90.0, 300.0), at(270.0, 300.0))
    );
}

#[test]
fn sequence_messages_through_more_placed_points_keep_them() {
    let left = rect(85.0, 110.0, 10.0, 190.0);
    let right = rect(265.0, 120.0, 10.0, 170.0);
    let path = route(
        Router::Sequence,
        &floating(left),
        &floating(right),
        &[at(150.0, 150.0), at(200.0, 250.0)],
    );
    assert_eq!(path.bends, vec![at(150.0, 150.0), at(200.0, 250.0)]);
    assert_eq!(
        (path.source, path.target),
        (at(90.0, 150.0), at(270.0, 250.0))
    );
}
