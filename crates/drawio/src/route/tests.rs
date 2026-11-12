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
    }
}

fn fixed(bounds: Rect, point: Point) -> End {
    End {
        bounds,
        point: Some(point),
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
fn loops_go_round_the_right_side() {
    let path = route(
        Router::Orthogonal,
        &floating(SOURCE),
        &floating(SOURCE),
        &[],
    );
    assert_eq!(path.bends, vec![at(120.0, 12.5), at(120.0, 37.5)]);
    assert_eq!((path.source, path.target), (at(50.0, 12.5), at(50.0, 37.5)));
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
        &fixed(rect(19.0, 129.0, 2.0, 2.0), at(20.0, 130.0)),
        &floating(left),
        &[],
    );
    assert_eq!(path.target, at(90.0, 130.0));
}
