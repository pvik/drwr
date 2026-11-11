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
    let (above, below) = (rect(0.0, 0.0, 100.0, 50.0), rect(0.0, 300.0, 100.0, 50.0));
    let guide = Guide {
        axis: Axis::X,
        at: 0.0,
        from: 0.0,
        to: 350.0,
    };
    // Whichever node comes first.
    for nodes in [[above, below], [below, above]] {
        let mut targets = Targets::new(5.0);
        for node in nodes {
            targets.add_node(node);
        }
        let guides = targets.guides(&lines_of(rect(0.0, 100.0, 60.0, 20.0)));
        assert_eq!(guides, [guide]);
    }
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

/// Targets reaching 5 points, with nodes in `rects`.
fn nodes(rects: &[Rect]) -> Targets {
    let mut targets = Targets::new(5.0);
    for &rect in rects {
        targets.add_node(rect);
    }
    targets
}

fn block(rect: Rect) -> Vec<Feature> {
    Feature::block(&[rect])
}

/// A row of nodes 0 to 100 and 300 to 400 along x, 50 high.
fn two_apart() -> Targets {
    nodes(&[rect(0.0, 0.0, 100.0, 50.0), rect(300.0, 0.0, 100.0, 50.0)])
}

#[test]
fn blocks_snap_halfway_between_the_nodes_beside_them() {
    let targets = two_apart();
    // 3 left of halfway.
    assert_eq!(
        targets.snap(&block(rect(147.0, 10.0, 100.0, 30.0))),
        [Some(3.0), None]
    );
    let gaps = EqualGaps {
        axis: Axis::X,
        gaps: vec![[100.0, 150.0], [250.0, 300.0]],
        band: [10.0, 40.0],
        start: 0.0,
    };
    let snapped = block(rect(150.0, 10.0, 100.0, 30.0));
    assert_eq!(targets.equal_gaps(&snapped), [gaps]);
}

#[test]
fn blocks_snap_as_far_past_a_node_as_it_is_past_the_next() {
    // 2 further from the second node than it is from the first.
    let targets = nodes(&[rect(0.0, 0.0, 100.0, 50.0), rect(150.0, 0.0, 100.0, 50.0)]);
    assert_eq!(
        targets.snap(&block(rect(302.0, 0.0, 60.0, 50.0))),
        [Some(-2.0), None]
    );
    let gaps = EqualGaps {
        axis: Axis::X,
        gaps: vec![[100.0, 150.0], [250.0, 300.0]],
        band: [0.0, 50.0],
        start: 0.0,
    };
    let snapped = block(rect(300.0, 0.0, 60.0, 50.0));
    assert_eq!(targets.equal_gaps(&snapped), [gaps]);
    // And the other way, 2 nearer.
    let targets = nodes(&[rect(300.0, 0.0, 100.0, 50.0), rect(450.0, 0.0, 100.0, 50.0)]);
    assert_eq!(
        targets.snap(&block(rect(192.0, 0.0, 60.0, 50.0))),
        [Some(-2.0), None]
    );
}

#[test]
fn columns_space_too() {
    let targets = nodes(&[rect(0.0, 0.0, 50.0, 100.0), rect(0.0, 300.0, 50.0, 100.0)]);
    assert_eq!(
        targets.snap(&block(rect(10.0, 147.0, 30.0, 100.0))),
        [None, Some(3.0)]
    );
}

#[test]
fn a_block_of_several_boxes_spaces_as_one() {
    let boxes = [rect(147.0, 10.0, 40.0, 30.0), rect(207.0, 20.0, 40.0, 30.0)];
    assert_eq!(two_apart().snap(&Feature::block(&boxes)), [Some(3.0), None]);
}

#[test]
fn only_nodes_overlapping_the_block_across_are_beside_it() {
    // The second node is below it.
    let targets = nodes(&[rect(0.0, 0.0, 100.0, 50.0), rect(300.0, 100.0, 100.0, 50.0)]);
    assert_eq!(
        targets.snap(&block(rect(147.0, 10.0, 100.0, 30.0))),
        [None, None]
    );
    // Beside a taller block, but not beside each other.
    let targets = nodes(&[rect(0.0, 0.0, 100.0, 50.0), rect(300.0, 150.0, 100.0, 50.0)]);
    assert_eq!(
        targets.snap(&block(rect(147.0, 0.0, 100.0, 200.0))),
        [None, None]
    );
    // Its top only touches their bottoms.
    assert_eq!(
        two_apart().snap(&block(rect(147.0, 50.0, 100.0, 30.0))),
        [None, None]
    );
}

#[test]
fn only_the_nearest_nodes_count() {
    let mut targets = two_apart();
    // Between the block and the node right of it.
    targets.add_node(rect(260.0, 0.0, 20.0, 50.0));
    assert_eq!(
        targets.snap(&block(rect(147.0, 10.0, 100.0, 30.0))),
        [None, None]
    );
}

#[test]
fn touching_nodes_make_no_gaps() {
    let targets = nodes(&[rect(0.0, 0.0, 100.0, 50.0), rect(100.0, 0.0, 100.0, 50.0)]);
    let touching = block(rect(200.0, 0.0, 50.0, 50.0));
    assert_eq!(
        targets.snap(&block(rect(202.0, 0.0, 50.0, 50.0))),
        [None, None]
    );
    assert_eq!(targets.equal_gaps(&touching), []);
}

#[test]
fn the_nearest_of_gaps_and_lines_wins() {
    let moved = rect(147.0, 32.0, 100.0, 6.0);
    let features = [lines_of(moved), block(moved)].concat();
    // Below, its left side 2 right of the block's, nearer than halfway.
    let mut targets = two_apart();
    targets.add_node(rect(149.0, 200.0, 30.0, 30.0));
    assert_eq!(targets.snap(&features), [Some(2.0), None]);
    // 4 right, further.
    let mut targets = two_apart();
    targets.add_node(rect(151.0, 200.0, 30.0, 30.0));
    assert_eq!(targets.snap(&features), [Some(3.0), None]);
}

#[test]
fn a_run_takes_in_every_equal_gap_along_the_row() {
    let row = [0.0, 150.0, 300.0].map(|x| rect(x, 0.0, 100.0, 50.0));
    let mut targets = nodes(&row);
    // Nearer the first node than the gaps.
    targets.add_node(rect(-70.0, 0.0, 60.0, 50.0));
    let gaps = EqualGaps {
        axis: Axis::X,
        gaps: vec![[100.0, 150.0], [250.0, 300.0], [400.0, 450.0]],
        band: [0.0, 50.0],
        start: 0.0,
    };
    let snapped = block(rect(450.0, 0.0, 60.0, 50.0));
    assert_eq!(targets.equal_gaps(&snapped), [gaps]);
}

#[test]
fn a_run_goes_both_ways_from_halfway() {
    let row = [0.0, 150.0, 400.0, 550.0].map(|x| rect(x, 0.0, 100.0, 50.0));
    let gaps = EqualGaps {
        axis: Axis::X,
        gaps: vec![
            [100.0, 150.0],
            [250.0, 300.0],
            [350.0, 400.0],
            [500.0, 550.0],
        ],
        band: [0.0, 50.0],
        start: 0.0,
    };
    let snapped = block(rect(300.0, 0.0, 50.0, 50.0));
    assert_eq!(nodes(&row).equal_gaps(&snapped), [gaps]);
}

#[test]
fn unequal_gaps_either_side_make_a_run_each() {
    let row = [(0.0, 100.0), (150.0, 100.0), (380.0, 100.0), (500.0, 100.0)];
    let targets = nodes(&row.map(|(x, width)| rect(x, 0.0, width, 50.0)));
    let run = |gaps: Vec<[f64; 2]>| EqualGaps {
        axis: Axis::X,
        gaps,
        band: [0.0, 50.0],
        start: 0.0,
    };
    let snapped = block(rect(300.0, 0.0, 60.0, 50.0));
    assert_eq!(
        targets.equal_gaps(&snapped),
        [
            run(vec![[100.0, 150.0], [250.0, 300.0]]),
            run(vec![[360.0, 380.0], [480.0, 500.0]]),
        ]
    );
}

#[test]
fn nodes_in_a_run_all_overlap_one_band() {
    // The first node is beside the block, but not beside the second node.
    let targets = nodes(&[rect(0.0, 60.0, 100.0, 40.0), rect(150.0, 0.0, 100.0, 40.0)]);
    let tall = |x: f64| block(rect(x, 0.0, 60.0, 100.0));
    assert_eq!(targets.snap(&tall(302.0)), [None, None]);
    assert_eq!(targets.equal_gaps(&tall(300.0)), []);
}

fn point(x: f64, y: f64) -> Point {
    Point { x, y }
}
