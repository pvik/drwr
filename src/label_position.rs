use egui::{Align2, Painter, Rect, Stroke, StrokeKind, Vec2, vec2};
use graphml::yed::LabelPosition;

/// The positions inside the node that the side panel offers, row by row.
pub const INSIDE: [LabelPosition; 9] = [
    LabelPosition::TopLeft,
    LabelPosition::Top,
    LabelPosition::TopRight,
    LabelPosition::Left,
    LabelPosition::Center,
    LabelPosition::Right,
    LabelPosition::BottomLeft,
    LabelPosition::Bottom,
    LabelPosition::BottomRight,
];

/// The positions around the node that the side panel offers after those.
pub const OUTSIDE: [LabelPosition; 8] = [
    LabelPosition::NorthWest,
    LabelPosition::North,
    LabelPosition::NorthEast,
    LabelPosition::West,
    LabelPosition::East,
    LabelPosition::SouthWest,
    LabelPosition::South,
    LabelPosition::SouthEast,
];

/// How a position reads in the side panel: like yEd's, compass points for
/// those outside the node.
pub fn name(position: LabelPosition) -> &'static str {
    match position {
        LabelPosition::Center => "Center",
        LabelPosition::Top => "Top",
        LabelPosition::Bottom => "Bottom",
        LabelPosition::Left => "Left",
        LabelPosition::Right => "Right",
        LabelPosition::TopLeft => "Top left",
        LabelPosition::TopRight => "Top right",
        LabelPosition::BottomLeft => "Bottom left",
        LabelPosition::BottomRight => "Bottom right",
        LabelPosition::North => "North",
        LabelPosition::South => "South",
        LabelPosition::East => "East",
        LabelPosition::West => "West",
        LabelPosition::NorthWest => "Northwest",
        LabelPosition::NorthEast => "Northeast",
        LabelPosition::SouthWest => "Southwest",
        LabelPosition::SouthEast => "Southeast",
    }
}

/// Paints a sample of `position` filling `rect`, which is twice as wide as
/// high: a node's outline with a bar where the label goes.
pub fn paint_sample(painter: &Painter, rect: Rect, stroke: Stroke, position: LabelPosition) {
    let bar = vec2(0.3, 0.1) * rect.height();
    let gap = 0.06 * rect.height();
    // Leaves room for a bar outside each side.
    let node = rect.shrink2(bar + Vec2::splat(gap));
    let room = if OUTSIDE.contains(&position) {
        rect
    } else {
        node.shrink(stroke.width + gap)
    };
    let bar = side(position).align_size_within_rect(bar, room);
    painter.rect_stroke(node, 0.0, stroke, StrokeKind::Inside);
    painter.rect_filled(bar, 0.0, stroke.color);
}

/// Which side or corner of the node a position is by.
fn side(position: LabelPosition) -> Align2 {
    use LabelPosition::*;
    match position {
        TopLeft | NorthWest => Align2::LEFT_TOP,
        Top | North => Align2::CENTER_TOP,
        TopRight | NorthEast => Align2::RIGHT_TOP,
        Left | West => Align2::LEFT_CENTER,
        Center => Align2::CENTER_CENTER,
        Right | East => Align2::RIGHT_CENTER,
        BottomLeft | SouthWest => Align2::LEFT_BOTTOM,
        Bottom | South => Align2::CENTER_BOTTOM,
        BottomRight | SouthEast => Align2::RIGHT_BOTTOM,
    }
}
