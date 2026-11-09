use super::*;

fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
    Rect {
        x,
        y,
        width,
        height,
    }
}

/// Targets reaching 5 points, with a node at (0, 0) 100 wide and 50 high.
fn one_node() -> Targets {
    let mut targets = Targets::new(5.0);
    targets.add_node(rect(0.0, 0.0, 100.0, 50.0));
    targets
}

fn lines_of(rect: Rect) -> Vec<Feature> {
    Feature::box_lines(&rect).to_vec()
}

#[test]
fn boxes_snap_their_middles_and_sides_onto_the_same_of_a_node() {
    let targets = one_node();
    // Its middle is 2 below the node's, and its sides far from the node's.
    assert_eq!(
        targets.snap(&lines_of(rect(200.0, 22.0, 40.0, 10.0))),
        [None, Some(-2.0)]
    );
    // Its top is 3 below the node's.
    assert_eq!(
        targets.snap(&lines_of(rect(200.0, 3.0, 40.0, 100.0))),
        [None, Some(-3.0)]
    );
}

#[test]
fn the_nearest_line_within_reach_wins() {
    let mut targets = one_node();
    targets.add_node(rect(300.0, 3.0, 100.0, 50.0));
    // Its middle, at 27, is 2 below one node's and 1 above the other's.
    assert_eq!(
        targets.snap(&lines_of(rect(200.0, 22.0, 40.0, 10.0))),
        [None, Some(1.0)]
    );
    // Nothing within reach.
    assert_eq!(
        targets.snap(&lines_of(rect(200.0, 90.0, 40.0, 10.0))),
        [None, None]
    );
}

#[test]
fn sides_touch_the_opposite_sides_of_nodes_beside_them() {
    let targets = one_node();
    // Its left side is 3 right of the node's right side, which it's beside.
    assert_eq!(
        targets.snap(&lines_of(rect(103.0, 10.0, 40.0, 10.0))),
        [Some(-3.0), None]
    );
    // Below the node, it's not beside it.
    assert_eq!(
        targets.snap(&lines_of(rect(103.0, 80.0, 40.0, 10.0))),
        [None, None]
    );
    // Its bottom is 2 above the node's top, which it's above.
    assert_eq!(
        targets.snap(&lines_of(rect(10.0, -32.0, 40.0, 30.0))),
        [None, Some(2.0)]
    );
}

#[test]
fn middles_snap_only_onto_middles() {
    let targets = one_node();
    // Its middle is 2 left of the node's right side.
    assert_eq!(
        targets.snap(&lines_of(rect(88.0, 10.0, 20.0, 10.0))),
        [None, None]
    );
}

#[test]
fn sides_snap_alone() {
    let targets = one_node();
    let right = Feature::side(&rect(10.0, 200.0, 93.0, 10.0), Axis::X, Side::End);
    assert_eq!(targets.snap(&[right]), [Some(-3.0), None]);
    // Only its right side is dragged, not its left, 3 right of the node's.
    let right = Feature::side(&rect(3.0, 200.0, 50.0, 10.0), Axis::X, Side::End);
    assert_eq!(targets.snap(&[right]), [None, None]);
}

#[test]
fn points_snap_onto_any_line_of_a_node_and_onto_points() {
    let mut targets = one_node();
    assert_eq!(
        targets.snap(&Feature::point(point(52.0, 103.0))),
        [Some(-2.0), None]
    );
    assert_eq!(
        targets.snap(&Feature::point(point(98.0, 48.0))),
        [Some(2.0), Some(2.0)]
    );
    targets.add_point(point(300.0, 101.0));
    assert_eq!(
        targets.snap(&Feature::point(point(52.0, 103.0))),
        [Some(-2.0), Some(-2.0)]
    );
}

#[test]
fn points_snap_onto_only_the_middles_of_some_nodes() {
    let mut targets = Targets::new(5.0);
    targets.add_middles(rect(0.0, 0.0, 100.0, 50.0));
    assert_eq!(
        targets.snap(&Feature::point(point(98.0, 48.0))),
        [None, None]
    );
    assert_eq!(
        targets.snap(&Feature::point(point(52.0, 23.0))),
        [Some(-2.0), Some(2.0)]
    );
}

#[test]
fn boxes_do_not_snap_onto_points() {
    let mut targets = Targets::new(5.0);
    targets.add_point(point(52.0, 23.0));
    assert_eq!(
        targets.snap(&lines_of(rect(0.0, 0.0, 100.0, 50.0))),
        [None, None]
    );
}

#[test]
fn snapped_middles_line_up() {
    let mut targets = Targets::new(5.0);
    targets.add_node(rect(0.1, 0.0, 33.3, 50.0));
    let moved = rect(5.0, 100.0, 23.7, 10.0);
    let [Some(dx), None] = targets.snap(&lines_of(moved)) else {
        panic!("it snaps across x only");
    };
    let snapped = rect(moved.x + dx, moved.y, moved.width, moved.height);
    let [guide] = targets.guides(&lines_of(snapped))[..] else {
        panic!("one guide shows");
    };
    assert_eq!((guide.axis, guide.at), (Axis::X, 0.1 + 33.3 / 2.0));
}

#[test]
fn a_guide_spans_what_lines_up_along_it() {
    let targets = one_node();
    // Its middle is in line with the node's, below it.
    let guides = targets.guides(&lines_of(rect(30.0, 200.0, 40.0, 20.0)));
    let guide = Guide {
        axis: Axis::X,
        at: 50.0,
        from: 0.0,
        to: 220.0,
    };
    assert_eq!(guides, [guide]);
}

#[test]
fn what_lines_up_along_one_line_shows_one_guide() {
    let mut targets = one_node();
    targets.add_node(rect(0.0, 300.0, 100.0, 50.0));
    let guides = targets.guides(&lines_of(rect(0.0, 100.0, 60.0, 20.0)));
    let guide = Guide {
        axis: Axis::X,
        at: 0.0,
        from: 0.0,
        to: 350.0,
    };
    assert_eq!(guides, [guide]);
}

#[test]
fn points_show_guides_to_the_points_they_line_up_with() {
    let mut targets = Targets::new(5.0);
    targets.add_point(point(300.0, 101.0));
    let guides = targets.guides(&Feature::point(point(52.0, 101.0)));
    let guide = Guide {
        axis: Axis::Y,
        at: 101.0,
        from: 52.0,
        to: 300.0,
    };
    assert_eq!(guides, [guide]);
}

fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}
