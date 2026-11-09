//! How the canvas paints the scene.

use std::borrow::Cow;
use std::sync::Arc;

use egui::emath::TSTransform;
use egui::epaint::{CircleShape, PathShape, TextShape};
use egui::text::LayoutJob;
use egui::{
    Align, Align2, Color32, FontId, Galley, Mesh, Painter, Rangef, Rect, Stroke, StrokeKind, Vec2,
    pos2, vec2,
};
use graphml::yed::{self, Alignment, Label, NodeGraphics, Route};

use crate::color_picker::color32;
use crate::document::Item;
use crate::fonts;
use crate::line_type;
use crate::selection::Selection;
use crate::snap::{Axis, Guide};
use crate::style::Style;
use crate::svg_image::SvgImage;

use super::shown::{Anchor, EdgeLook, Outline, Shown, egui_rect, shape};
use super::textures::Textures;
use super::{
    BACKGROUND, Canvas, EdgeDrag, EdgePart, GUIDE, GUIDE_OVERHANG, HALO, HANDLE_SIZE,
    LABEL_PADDING, MAX_TEXT_PIXELS, Owner, Preview, SELECTION, SELECTION_BOX_FILL, SELECTION_GAP,
    edge, grid,
};

impl Canvas {
    /// Paints the scene `shown` on the canvas at `rect`, outlining what's
    /// `selected`, with what `preview` shows over it.
    pub(super) fn paint_scene(
        &self,
        ui: &egui::Ui,
        rect: Rect,
        shown: Shown,
        selected: &Selection,
        style: &Style,
        preview: Preview,
    ) {
        let Preview {
            drawing,
            drawn_edge,
            dragged_edge,
            boxed,
        } = preview;
        let (scene, painter) = (shown.scene, shown.painter);
        painter.rect_filled(rect, 0.0, BACKGROUND);
        if self.options.grid_lines {
            grid::paint(painter, rect, shown.to_screen);
        }
        let editing = self.editor.as_ref().map(|editor| &editor.label);
        // The label being edited shows in its editor instead.
        let hidden = |item: &Item| {
            let label = editing.filter(|label| label.item == *item);
            label.map(|label| label.index)
        };
        // While a box is dragged out, it shows what it would select.
        let kept = boxed.is_none_or(|(_, add)| add);
        let mut outlines = Vec::new();
        for (index, node) in scene.nodes.iter().enumerate() {
            let item = Item::Node(node.id.clone());
            let boxed = boxed.is_some_and(|(rect, _)| shown.in_box(index, rect));
            if boxed || kept && selected.contains(&item) {
                outlines.push(shown.node_rect(index).expand(SELECTION_GAP));
            }
            let image = node.image.as_deref();
            shown.paint_node(
                &node.graphics,
                node.radius,
                image,
                &self.textures,
                hidden(&item),
            );
        }
        let selected_edge = match selected.single() {
            Some(&Item::Edge(index)) => Some(index),
            _ => None,
        };
        for edge in &scene.edges {
            let item = Item::Edge(edge.index);
            let boxed = boxed.is_some_and(|(rect, _)| shown.edge_in_box(edge, rect));
            let chosen = boxed || kept && selected.contains(&item);
            let look = match &dragged_edge {
                Some(look) if selected_edge == Some(edge.index) => Cow::Borrowed(look),
                _ => Cow::Owned(EdgeLook::of(edge)),
            };
            shown.paint_edge(&look, chosen, hidden(&item));
        }
        if let Some(look) = &drawn_edge {
            shown.paint_edge(look, false, None);
        }
        // The node the edge being drawn, or the port being dragged, would
        // end on.
        let ending = match (&drawn_edge, self.edge_drag) {
            (Some(look), _) => Some(look.target),
            (
                None,
                Some(EdgeDrag {
                    part: EdgePart::End(end),
                    ..
                }),
            ) => dragged_edge.as_ref().map(|look| look.anchor(end)),
            _ => None,
        };
        if let Some(Anchor::Node(node)) = ending {
            let rect = shown.node_rect(node).expand(SELECTION_GAP);
            painter.rect_stroke(rect, 0.0, SELECTION, StrokeKind::Outside);
        }
        if let Some(graphics) = drawing {
            shown.paint_node(&graphics, style.corner_radius(), None, &self.textures, None);
        }
        // Over the nodes, so one on top can't hide them.
        for outline in outlines {
            painter.rect_stroke(outline, 0.0, SELECTION, StrokeKind::Outside);
        }
        paint_guides(painter, shown.to_screen, &self.guides);
        if kept
            && let Some(index) = selected_edge
            && let Some(edge) = scene.edge(index)
        {
            let look = match &dragged_edge {
                Some(look) => Cow::Borrowed(look),
                None => Cow::Owned(EdgeLook::of(edge)),
            };
            shown.paint_handles(&look, selected.bend());
        }
        if let Some((rect, _)) = boxed {
            let stroke = Stroke::new(1.0, SELECTION.color);
            painter.rect(rect, 0.0, SELECTION_BOX_FILL, stroke, StrokeKind::Inside);
        }
        // Outlined, since the light theme's panels are nearly white too.
        let border = ui.visuals().widgets.noninteractive.bg_stroke;
        painter.rect_stroke(rect, 0.0, border, StrokeKind::Inside);
    }
}

impl Shown<'_> {
    /// Paints a node with its labels, except the one at index `hidden`, unless
    /// it's a kind the canvas can't draw yet. `corner_radius` rounds the
    /// corners of a box, and an SVG node shows `image` from `textures`.
    fn paint_node(
        &self,
        graphics: &NodeGraphics,
        corner_radius: Option<f64>,
        image: Option<&SvgImage>,
        textures: &Textures,
        hidden: Option<usize>,
    ) {
        let Some(outline) = shape(graphics, corner_radius) else {
            return;
        };
        let painter = self.painter;
        let rect = egui_rect(graphics.geometry, self.to_screen);
        let fill = graphics.fill.color.map_or(Color32::TRANSPARENT, color32);
        let border = &graphics.border;
        let stroke = border.color.map_or(Stroke::NONE, |color| {
            Stroke::new(border.width as f32 * self.to_screen.scaling, color32(color))
        });
        // Like yEd, which centers borders on the outline, over the fill.
        match outline {
            Outline::Rect(radius) => {
                let radius = radius * self.to_screen.scaling;
                let ppp = painter.pixels_per_point();
                let points = line_type::rounded_rect_points(rect, radius, stroke, ppp);
                if points.is_empty() {
                    painter.rect_filled(rect, 0.0, fill);
                    line_type::paint_rect_outline(painter, rect, stroke, &border.line_type);
                } else {
                    // On the border's polygon, so their corners line up.
                    let filled = PathShape::convex_polygon(points.clone(), fill, Stroke::NONE);
                    painter.add(filled);
                    line_type::paint_outline(painter, points, stroke, &border.line_type);
                }
            }
            Outline::Ellipse => {
                let points = line_type::ellipse_points(rect, painter.pixels_per_point());
                painter.add(PathShape::convex_polygon(points, fill, Stroke::NONE));
                line_type::paint_ellipse_outline(painter, rect, stroke, &border.line_type);
            }
            // Drawn without its fill and border, as yEd draws it.
            Outline::Svg => {
                if let Some(image) = image
                    && rect.intersects(painter.clip_rect())
                {
                    // At the zoom labels are laid out at, so it isn't drawn
                    // anew every frame while the zoom changes.
                    let scale = self.text_zoom * painter.pixels_per_point();
                    let geometry = graphics.geometry;
                    let pixels = vec2(geometry.width as f32, geometry.height as f32) * scale;
                    if let Some(texture) = textures.get(painter.ctx(), image, pixels) {
                        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                        painter.image(texture, rect, uv, Color32::WHITE);
                    }
                }
            }
        }

        let owner = Owner::Node(graphics.geometry);
        for (index, label) in graphics.labels.iter().enumerate() {
            if label.visible && hidden != Some(index) {
                self.paint_label(&owner, label);
            }
        }
    }

    /// Paints an edge as `look` has it, with its labels but the one at index
    /// `hidden`, over a halo if it's `chosen`.
    fn paint_edge(&self, look: &EdgeLook, chosen: bool, hidden: Option<usize>) {
        let path = self.path(look);
        let line = &look.graphics.line;
        let width = line.width as f32 * self.to_screen.scaling;
        if chosen {
            let halo = Stroke::new(width + 2.0 * HALO.width, HALO.color);
            self.painter.add(PathShape::line(path.clone(), halo));
        }
        let stroke = line
            .color
            .map_or(Stroke::NONE, |color| Stroke::new(width, color32(color)));
        let arrows = &look.graphics.arrows;
        edge::paint(self.painter, path, stroke, &line.line_type, arrows);
        let owner = Owner::Edge(self.route(look));
        for (index, label) in look.graphics.labels.iter().enumerate() {
            if label.visible && hidden != Some(index) {
                self.paint_label(&owner, label);
            }
        }
    }

    /// Paints the handles on the ports and bends of the selected edge, as
    /// `look` has it, filling the one on the bend at index `bend`.
    fn paint_handles(&self, look: &EdgeLook, bend: Option<usize>) {
        let route = self.unclipped(look);
        let outline = Stroke::new(1.0, SELECTION.color);
        for port in [route[0], route[route.len() - 1]] {
            self.painter.add(CircleShape {
                center: port,
                radius: HANDLE_SIZE / 2.0,
                fill: Color32::WHITE,
                stroke: outline,
            });
        }
        for (index, &at) in route[1..route.len() - 1].iter().enumerate() {
            let fill = if bend == Some(index) {
                SELECTION.color
            } else {
                Color32::WHITE
            };
            let rect = Rect::from_center_size(at, Vec2::splat(HANDLE_SIZE));
            self.painter
                .rect(rect, 0.0, fill, outline, StrokeKind::Inside);
        }
    }

    /// Paints a label of `owner` where yEd puts it, lined up by its alignment,
    /// in its font and color. It's laid out at `text_zoom`, then scaled to the
    /// zoom.
    fn paint_label(&self, owner: &Owner, label: &Label) {
        let color = color32(label.text_color);
        let (galley, label_rect) = self.label_layout(owner, label);
        let align = Align2([galley.job.halign, Align::Center]);
        let scale = self.to_screen.scaling / self.text_zoom;
        let size = galley.size() * scale;
        let rect = align.align_size_within_rect(size, label_rect);
        let pos = rect.min - galley.rect.min.to_vec2() * scale;
        if scale == 1.0 {
            self.painter.galley(pos, galley, color);
        } else {
            let mut text = TextShape::new(pos, galley, color);
            // About `pos`.
            text.transform(TSTransform::new(pos.to_vec2() * (1.0 - scale), scale));
            self.painter.add(text);
        }
    }

    /// A label's text laid out at `text_zoom`, and where on screen the label
    /// goes on `owner`.
    pub fn label_layout(&self, owner: &Owner, label: &Label) -> (Arc<Galley>, Rect) {
        let font = label_font(label, self.text_zoom, self.painter.pixels_per_point());
        let color = color32(label.text_color);
        let mut job = LayoutJob::simple(label.text.clone(), font, color, f32::INFINITY);
        job.halign = halign(&label.alignment);
        let galley = self.painter.layout_job(job);
        let size = galley.size() * (self.to_screen.scaling / self.text_zoom);
        let rect = label_rect(self.to_screen, owner, label, size);
        (galley, rect)
    }

    /// Where an edge label goes along `route`, in graph coordinates.
    pub fn edge_label_bounds(&self, route: &Route, label: &Label) -> yed::Rect {
        let font = label_font(label, self.text_zoom, self.painter.pixels_per_point());
        let text = (self.painter).layout_no_wrap(label.text.clone(), font, Color32::PLACEHOLDER);
        placed_on(route, label, text.size() / self.text_zoom)
    }
}

/// Where a label of `owner`, with text `size` big on screen, goes on screen:
/// where yEd puts it, or for a new label, which yEd hasn't sized yet, where
/// its position puts it.
pub fn label_rect(to_screen: TSTransform, owner: &Owner, label: &Label, size: Vec2) -> Rect {
    let text = size / to_screen.scaling;
    let node = match owner {
        Owner::Node(node) => *node,
        Owner::Edge(route) => return egui_rect(placed_on(route, label, text), to_screen),
    };
    let mut bounds = label.bounds;
    if !(bounds.width > 0.0 && bounds.height > 0.0) {
        let size = text + Vec2::splat(2.0 * LABEL_PADDING);
        let geometry = yed::Rect {
            width: node.width,
            height: node.height,
            ..yed::Rect::default()
        };
        match label.placed(&geometry, size.x.into(), size.y.into()) {
            Some(placed) => bounds = placed,
            None => return egui_rect(node, to_screen),
        }
    }
    // Relative to the node.
    let node_min = to_screen * pos2(node.x as f32, node.y as f32);
    let to_node = TSTransform::new(node_min.to_vec2(), to_screen.scaling);
    egui_rect(bounds, to_node)
}

/// Where an edge label with text `text` big goes along `route`, both in
/// graph points: where yEd puts it, or where it last did if its model is
/// one this library doesn't read.
fn placed_on(route: &Route, label: &Label, text: Vec2) -> yed::Rect {
    let bounds = label.bounds;
    let (width, height) = if bounds.width > 0.0 && bounds.height > 0.0 {
        (bounds.width, bounds.height)
    } else {
        let size = text + Vec2::splat(2.0 * LABEL_PADDING);
        (size.x.into(), size.y.into())
    };
    label.placed_on(route, width, height).unwrap_or_else(|| {
        let start = route.points[0];
        yed::Rect {
            x: start.x + bounds.x,
            y: start.y + bounds.y,
            width,
            height,
        }
    })
}

/// A label's font at `zoom`. Its size counts as at least a point, since egui
/// can't draw text of size 0, and is capped at what egui can draw.
pub fn label_font(label: &Label, zoom: f32, pixels_per_point: f32) -> FontId {
    let font = &label.font;
    let size = font.size.max(1) as f32 * zoom;
    FontId::new(
        size.min(MAX_TEXT_PIXELS / pixels_per_point),
        fonts::family(&font.family, &font.style),
    )
}

/// How a label's lines line up.
pub fn halign(alignment: &Alignment) -> Align {
    match alignment {
        Alignment::Left => Align::LEFT,
        Alignment::Right => Align::RIGHT,
        _ => Align::Center,
    }
}

/// Paints `guides`, in graph points, a pixel wide and a little longer than
/// what they span.
fn paint_guides(painter: &Painter, to_screen: TSTransform, guides: &[Guide]) {
    if guides.is_empty() {
        return;
    }
    let pixels_per_point = painter.pixels_per_point();
    let mut mesh = Mesh::default();
    for guide in guides {
        let at = |along: f64| {
            let (x, y) = match guide.axis {
                Axis::X => (guide.at, along),
                Axis::Y => (along, guide.at),
            };
            to_screen * pos2(x as f32, y as f32)
        };
        let (from, to) = (at(guide.from), at(guide.to));
        let line = match guide.axis {
            Axis::X => Rect::from_x_y_ranges(pixel(from.x, pixels_per_point), from.y..=to.y),
            Axis::Y => Rect::from_x_y_ranges(from.x..=to.x, pixel(from.y, pixels_per_point)),
        };
        let overhang = match guide.axis {
            Axis::X => vec2(0.0, GUIDE_OVERHANG),
            Axis::Y => vec2(GUIDE_OVERHANG, 0.0),
        };
        mesh.add_colored_rect(line.expand2(overhang), GUIDE);
    }
    painter.add(mesh);
}

/// The pixel a line a pixel wide at `at` on screen is drawn on, as egui puts
/// a 1 px border there.
pub(super) fn pixel(at: f32, pixels_per_point: f32) -> Rangef {
    let start = (at * pixels_per_point).floor();
    Rangef::new(start, start + 1.0) / pixels_per_point
}
