//! What the canvas draws of the scene, as marks in graph coordinates, so that
//! export draws the same.

use std::sync::Arc;

use egui::emath::TSTransform;
use egui::{Align, Align2, LayerId, Painter, Pos2, Rect};
use graphml::yed::{self, Color, Label, LineType, Stroke};

use crate::scene::{Scene, SceneNode};
use crate::selection::Selection;
use crate::svg_image::SvgImage;

use super::shown::{EdgeLook, Outline, Shown, egui_rect, shape};
use super::{Owner, edge};

/// SVG's default, which export leaves lines with.
const MITER_LIMIT: f32 = 4.0;

/// The marks the canvas draws, bottom up, and what they cover.
#[derive(Clone, Debug)]
pub struct Drawing {
    pub marks: Vec<Mark>,
    /// In graph points; [`Rect::NOTHING`] if there are no marks.
    pub bounds: Rect,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    /// A node's shape filling `rect`, with its border centered on the outline.
    Node {
        rect: Rect,
        outline: Outline,
        fill: Option<Color>,
        border: Option<Pen>,
    },
    /// A line through `points`, with butt ends.
    Line {
        points: Vec<Pos2>,
        pen: Pen,
    },
    /// A polygon filled with `color`: an arrowhead.
    Polygon {
        points: Vec<Pos2>,
        color: Color,
    },
    /// An SVG node's drawing, stretched to fill `rect`.
    Image {
        rect: Rect,
        image: Arc<SvgImage>,
    },
    Text(Text),
}

/// How a line or a border is drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct Pen {
    pub width: f32,
    pub color: Color,
    pub line_type: LineType,
}

/// A label's text, in rows laid out as on the canvas.
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    pub rows: Vec<Row>,
    /// Which end of each row, or its middle, the row's `x` is at.
    pub align: Align,
    pub font: yed::Font,
    pub color: Color,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub text: String,
    pub x: f32,
    pub baseline: f32,
}

impl Drawing {
    /// What the canvas draws of `scene`, or of the items `only` has, with the
    /// labels laid out in `ctx`'s fonts.
    pub fn of(ctx: &egui::Context, scene: &Scene, only: Option<&Selection>) -> Self {
        let painter = Painter::new(ctx.clone(), LayerId::background(), Rect::EVERYTHING);
        let shown = Shown {
            scene,
            painter: &painter,
            to_screen: TSTransform::IDENTITY,
            text_zoom: 1.0,
        };
        let mut drawing = Drawing {
            marks: Vec::new(),
            bounds: Rect::NOTHING,
        };
        for node in &scene.nodes {
            if only.is_none_or(|only| only.contains_node(&node.id)) {
                drawing.add_node(shown, node);
            }
        }
        for edge in &scene.edges {
            if only.is_none_or(|only| only.contains_edge(edge.index)) {
                drawing.add_edge(shown, &EdgeLook::of(edge));
            }
        }
        drawing
    }

    /// Adds a node and its labels, as the canvas paints them.
    fn add_node(&mut self, shown: Shown, node: &SceneNode) {
        let graphics = &node.graphics;
        let Some(outline) = shape(graphics, node.radius) else {
            return;
        };
        let rect = egui_rect(graphics.geometry, TSTransform::IDENTITY);
        if outline == Outline::Svg {
            if let Some(image) = node.image.clone()
                && rect.is_positive()
                && rect.is_finite()
            {
                self.cover(rect);
                self.marks.push(Mark::Image { rect, image });
            }
        } else if rect.is_finite() && rect.width() >= 0.0 && rect.height() >= 0.0 {
            let border = pen(&graphics.border);
            self.cover(rect.expand(border.as_ref().map_or(0.0, |pen| pen.width / 2.0)));
            self.marks.push(Mark::Node {
                rect,
                outline,
                fill: graphics.fill.color,
                border,
            });
        }
        let owner = Owner::Node(graphics.geometry);
        for label in graphics.labels.iter().filter(|label| label.visible) {
            self.add_label(shown, &owner, label);
        }
    }

    /// Adds an edge, with its arrowheads and labels, as the canvas paints it.
    fn add_edge(&mut self, shown: Shown, look: &EdgeLook) {
        let graphics = &look.graphics;
        let mut path = shown.path(look);
        path.dedup();
        if let Some(pen) = pen(&graphics.line)
            && path.len() >= 2
        {
            let (points, heads) = edge::with_arrows(path, pen.width, &graphics.arrows);
            if points.len() >= 2 {
                self.cover_line(&points, pen.width);
                self.marks.push(Mark::Line {
                    points,
                    pen: pen.clone(),
                });
            }
            for points in heads {
                points.iter().for_each(|&point| self.cover_point(point));
                let color = pen.color;
                self.marks.push(Mark::Polygon { points, color });
            }
        }
        let owner = Owner::Edge(shown.route(look));
        for label in graphics.labels.iter().filter(|label| label.visible) {
            self.add_label(shown, &owner, label);
        }
    }

    /// Adds a label of `owner`, laid out as the canvas lays it out.
    fn add_label(&mut self, shown: Shown, owner: &Owner, label: &Label) {
        let (galley, rect) = shown.label_layout(owner, label);
        let halign = galley.job.halign;
        let placed = Align2([halign, Align::Center]).align_size_within_rect(galley.size(), rect);
        let pos = placed.min - galley.rect.min.to_vec2();
        let mut rows = Vec::new();
        for placed in &galley.rows {
            let glyphs = &placed.row.glyphs;
            let Some(first) = glyphs.first() else {
                continue;
            };
            let (left, width) = (pos.x + placed.pos.x, placed.row.size.x);
            rows.push(Row {
                text: glyphs.iter().map(|glyph| glyph.chr).collect(),
                x: match halign {
                    Align::Min => left,
                    Align::Center => left + width / 2.0,
                    Align::Max => left + width,
                },
                baseline: pos.y + placed.pos.y + first.pos.y,
            });
            self.cover(placed.rect().translate(pos.to_vec2()));
        }
        if !rows.is_empty() {
            self.marks.push(Mark::Text(Text {
                rows,
                align: halign,
                font: label.font.clone(),
                color: label.text_color,
            }));
        }
    }

    fn cover(&mut self, rect: Rect) {
        self.bounds = self.bounds.union(rect);
    }

    fn cover_point(&mut self, point: Pos2) {
        self.bounds.extend_with(point);
    }

    /// Covers a line through `points`, `width` wide, with butt ends and miter
    /// joins.
    fn cover_line(&mut self, points: &[Pos2], width: f32) {
        let half = width / 2.0;
        for pair in points.windows(2) {
            let across = (pair[1] - pair[0]).normalized().rot90() * half;
            for &point in pair {
                self.cover_point(point + across);
                self.cover_point(point - across);
            }
        }
        for corner in points.windows(3) {
            let into = (corner[1] - corner[0]).normalized();
            let out = (corner[2] - corner[1]).normalized();
            // Of half the angle between the segments.
            let sin = ((1.0 + into.dot(out)) / 2.0).sqrt();
            let outward = into - out;
            // Sharper corners are beveled, which the segments cover.
            if sin * MITER_LIMIT >= 1.0 && outward.length() > 1e-6 {
                self.cover_point(corner[1] + outward.normalized() * (half / sin));
            }
        }
    }
}

/// How the canvas draws `stroke`, if it draws it at all.
fn pen(stroke: &Stroke) -> Option<Pen> {
    let color = stroke.color.filter(|color| color.a > 0)?;
    let width = stroke.width as f32;
    (width > 0.0 && width.is_finite()).then(|| Pen {
        width,
        color,
        line_type: stroke.line_type.clone(),
    })
}
