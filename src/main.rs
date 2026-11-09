mod action;
mod canvas;
mod cli;
mod color_picker;
mod document;
mod error_dialog;
mod font_picker;
mod fonts;
mod label_position;
mod line_type;
mod menu;
mod open_dialog;
mod selection;
mod shortcuts;
mod side_panel;
mod status_bar;
mod style;
#[cfg(test)]
mod testing;
mod text_dialog;
mod text_field;
mod tool;
mod toolbar;
mod zoom;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use action::{Action, FontPart};
use canvas::Canvas;
use color_picker::ColorPicker;
use document::{ColorProperty, Graphics, Item, LabelRef};
use fonts::Fonts;
use graphml::GraphMl;
use open_dialog::OpenDialog;
use selection::Selection;
use side_panel::SidePanel;
use style::Style;
use text_dialog::TextDialog;
use tool::Tool;

fn main() -> eframe::Result {
    match cli::parse(std::env::args_os().skip(1)) {
        Ok(cli::Command::Run { file }) => {
            let mut app = DrwrApp {
                fonts: Arc::new(Fonts::system()),
                ..DrwrApp::default()
            };
            if let Some(path) = file
                && let Err(err) = app.open(&path)
            {
                eprintln!("error: couldn't open {path:?}: {err}");
                std::process::exit(1);
            }
            run_gui(app)
        }
        Ok(cli::Command::Version) => {
            println!("{}", cli::VERSION);
            Ok(())
        }
        Err(err) => {
            // Exit status 2 is the conventional code for command-line usage errors.
            eprintln!("error: {err}\n\n{}", cli::USAGE);
            std::process::exit(2);
        }
    }
}

fn run_gui(app: DrwrApp) -> eframe::Result {
    // The app name doubles as the default window title and the Wayland app id.
    eframe::run_native(
        "drwr",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            app.init(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}

#[derive(Default)]
struct DrwrApp {
    tool: Tool,
    graphml: GraphMl,
    /// Where `graphml` is saved, as an absolute path; `None` if it isn't.
    file: Option<PathBuf>,
    selected: Selection,
    /// What the user last set on nodes, lines and labels, for new ones. Kept
    /// across files.
    style: Style,
    canvas: Canvas,
    /// The fonts labels are drawn in, which egui is given too.
    fonts: Arc<Fonts>,
    side_panel: SidePanel,
    open_dialog: OpenDialog,
    /// Picks the colors of the selected nodes and edges.
    color_picker: ColorPicker<ColorProperty>,
    label_dialog: TextDialog<LabelEdit>,
    error: Option<String>,
    /// Whether a text field had the focus at the end of the last frame; see
    /// [`shortcuts::pressed`].
    typing: bool,
}

/// Labels the label dialog edits together, and the text it opened on.
struct LabelEdit {
    labels: Vec<LabelRef>,
    text: String,
}

impl DrwrApp {
    /// Sets up `ctx` to show the app in.
    fn init(&self, ctx: &egui::Context) {
        // Needed to render the toolbar's SVG icons.
        egui_extras::install_image_loaders(ctx);
        ctx.add_font_provider(self.fonts.clone());
        // Ctrl+Plus, Ctrl+Minus and Ctrl+0 zoom the canvas instead.
        ctx.options_mut(|options| options.zoom_with_keyboard = false);
    }

    /// Replaces the graph with the GraphML file at `path`, or keeps it on error.
    fn open(&mut self, path: &Path) -> Result<(), graphml::Error> {
        let path = std::path::absolute(path)?;
        self.graphml = GraphMl::read_file(&path)?;
        self.file = Some(path);
        self.selected = Selection::default();
        // Labels being edited belong to the old graph.
        self.canvas = Canvas::default();
        self.side_panel = SidePanel::default();
        self.color_picker.close();
        self.label_dialog.close();
        Ok(())
    }

    /// Edits the graphics of each selected node and edge.
    fn edit_selected(&mut self, edit: impl Fn(&mut Graphics)) {
        for item in self.selected.items() {
            document::edit(&mut self.graphml, item, &edit);
        }
    }

    /// Puts the selected nodes, with the selected edges between them, on the
    /// clipboard. Returns whether any nodes were selected.
    fn copy(&self, ctx: &egui::Context) -> bool {
        let Some(copied) = document::copy(&self.graphml, self.selected.items()) else {
            return false;
        };
        ctx.copy_text(copied.to_xml());
        true
    }

    /// Removes everything selected, nodes with their edges.
    fn remove_selected(&mut self) {
        let selected = std::mem::take(&mut self.selected);
        document::remove_items(&mut self.graphml, selected.items());
        // The Delete button's click would otherwise apply a label edited on
        // the canvas to the edge moved into its place.
        self.canvas.cancel_label_edit();
    }

    /// Performs `action`, clicked in the menu or toolbar, unless it's Cut or
    /// Copy. Those go on `copies`, to wait for the side panel and the canvas,
    /// which apply a label typed there when the click takes away the focus.
    fn perform_or_wait(
        &mut self,
        action: Option<Action>,
        copies: &mut Vec<Action>,
        ctx: &egui::Context,
        frame: &eframe::Frame,
    ) {
        match action {
            Some(action @ (Action::Cut | Action::Copy)) => copies.push(action),
            Some(action) => self.perform(action, ctx, frame),
            None => {}
        }
    }

    /// Does what the user asked for, wherever they asked.
    fn perform(&mut self, action: Action, ctx: &egui::Context, frame: &eframe::Frame) {
        match action {
            Action::New => {
                // TODO: replace the graph with an empty, unsaved one.
            }
            Action::Open => self.open_dialog.open(ctx, frame),
            Action::Save => {
                // TODO: save the graph, asking for a file if it's unsaved.
            }
            Action::Cut => {
                if self.copy(ctx) {
                    self.remove_selected();
                }
            }
            Action::Copy => {
                self.copy(ctx);
            }
            Action::Paste => {
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                // For the frame the paste event comes in.
                ctx.request_repaint();
            }
            Action::PasteText(text) => {
                if let Ok(copied) = GraphMl::parse(&text) {
                    let pasted = document::paste(&mut self.graphml, copied);
                    if !pasted.is_empty() {
                        self.selected = Selection::of(pasted);
                    }
                }
            }
            Action::SelectTool(tool) => self.tool = tool,
            Action::ZoomIn => {
                if let Some(zoom) = zoom::step_in(self.canvas.zoom()) {
                    self.canvas.set_zoom(zoom);
                }
            }
            Action::ZoomOut => {
                if let Some(zoom) = zoom::step_out(self.canvas.zoom()) {
                    self.canvas.set_zoom(zoom);
                }
            }
            Action::SetZoom(zoom) => self.canvas.set_zoom(zoom),
            Action::PanBy(dx, dy) => self.canvas.pan_by(egui::vec2(dx, dy)),
            Action::MoveBy(dx, dy) => match (self.selected.single(), self.selected.bend()) {
                (Some(&Item::Edge(index)), Some(bend)) => {
                    document::move_bends(&mut self.graphml, index, Some(bend), dx, dy);
                }
                _ => document::move_items(&mut self.graphml, self.selected.items(), dx, dy),
            },
            Action::Delete => match (self.selected.single(), self.selected.bend()) {
                (Some(&Item::Edge(index)), Some(bend)) => {
                    document::remove_bend(&mut self.graphml, index, bend);
                    self.selected = Selection::edge(index);
                }
                _ if !self.selected.is_empty() => self.remove_selected(),
                _ => {}
            },
            Action::EditColor(property) => {
                let mut graphics = (self.selected.items().iter())
                    .filter_map(|item| document::graphics(&self.graphml, item));
                // The first's, as the side panel shows none when they differ.
                // A text color is a label's, which some items may lack.
                let first = match property {
                    ColorProperty::Text(index) => graphics.find(|g| g.labels().len() > index),
                    _ => graphics.next(),
                };
                if let Some(graphics) = first {
                    let title = format!("{} color", property.name());
                    let color = property.get(&graphics);
                    self.color_picker
                        .open(property, title, color, property.optional());
                }
            }
            Action::SetColor(property, color) => {
                self.edit_selected(|graphics| property.set(graphics, color));
                self.style.set_color(&self.selected, property, color);
            }
            Action::SetLineWidth(width) => {
                self.edit_selected(|graphics| graphics.stroke_mut().width = width);
                self.style.set_line_width(&self.selected, width);
            }
            Action::SetLineType(line_type) => {
                self.edit_selected(|graphics| {
                    graphics.stroke_mut().line_type = line_type.clone();
                });
                self.style.set_line_type(&self.selected, &line_type);
            }
            Action::SetCornerRadius(radius) => {
                for id in self.selected.nodes() {
                    document::set_corner_radius(&mut self.graphml, id, radius);
                }
                self.style.set_corner_radius(&self.selected, radius);
            }
            Action::SetArrow(end, arrow) => {
                for index in self.selected.edges() {
                    document::set_arrow(&mut self.graphml, index, end, arrow);
                }
            }
            Action::EditLabel => {
                if let Some(item) = self.selected.single() {
                    let label = LabelRef {
                        item: item.clone(),
                        index: 0,
                    };
                    if let Some(text) = document::label_text(&self.graphml, &label) {
                        self.canvas.edit_label(label, text);
                    }
                }
            }
            Action::EditLabelInDialog(labels) => {
                let texts = labels
                    .iter()
                    .map(|label| document::label_text(&self.graphml, label));
                if let Some(texts) = texts.collect::<Option<Vec<_>>>()
                    && let Some(first) = texts.first()
                {
                    // Blank if they differ, as in their field.
                    let same = texts.iter().all(|text| text == first);
                    let text = if same { first.clone() } else { String::new() };
                    let edit = LabelEdit {
                        labels,
                        text: text.clone(),
                    };
                    self.label_dialog.open(edit, "Label", text);
                }
            }
            Action::SetLabel(label, text) => {
                document::set_label(&mut self.graphml, &label, &text, &self.style);
            }
            Action::SetFont(label, part) => {
                self.style.set_font(&part);
                document::edit_label(&mut self.graphml, &label, |label| {
                    let font = &mut label.font;
                    match part {
                        FontPart::Family(family) => font.family = family,
                        FontPart::Style(style) => font.style = style,
                        FontPart::Size(size) => font.size = size,
                    }
                });
            }
            Action::SetLabelPosition(label, position) => {
                document::set_label_position(&mut self.graphml, &label, position);
            }
        }
    }
}

impl eframe::App for DrwrApp {
    fn logic(&mut self, _ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if let Some(path) = self.open_dialog.picked()
            && let Err(err) = self.open(&path)
        {
            self.error = Some(format!("Couldn't open {}:\n{err}", path.display()));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // Before the menu, so an Escape that closes it doesn't also select the
        // Cursor tool.
        for action in shortcuts::pressed(ui.ctx(), self.typing) {
            self.perform(action, ui.ctx(), frame);
        }
        // Panels are laid out outside-in in the order they're added, so the
        // canvas (the central panel) must come last.
        let can_copy = self.selected.nodes().next().is_some();
        let mut copies = Vec::new();
        let menu = menu::show(ui, &self.open_dialog, can_copy);
        self.perform_or_wait(menu, &mut copies, ui.ctx(), frame);
        let (tool, zoom) = (self.tool, self.canvas.zoom());
        let toolbar = toolbar::show(ui, &self.open_dialog, can_copy, tool, zoom);
        self.perform_or_wait(toolbar, &mut copies, ui.ctx(), frame);
        status_bar::show(ui, self.tool, self.file.as_deref());
        for action in self
            .side_panel
            .show(ui, &self.graphml, &self.selected, &self.fonts)
        {
            self.perform(action, ui.ctx(), frame);
        }
        let actions = self.canvas.show(
            ui,
            self.tool,
            &mut self.graphml,
            &mut self.selected,
            &self.style,
        );
        for action in actions.into_iter().chain(copies) {
            self.perform(action, ui.ctx(), frame);
        }

        if let Some((property, color)) = self.color_picker.show(ui.ctx()) {
            self.perform(Action::SetColor(property, color), ui.ctx(), frame);
        }
        if let Some((edit, text)) = self.label_dialog.show(ui.ctx())
            // Unchanged, it would blank labels that differ.
            && text != edit.text
        {
            for label in edit.labels {
                self.perform(Action::SetLabel(label, text.clone()), ui.ctx(), frame);
            }
        }
        error_dialog::show(ui, &mut self.error);
        self.typing = ui.ctx().text_edit_focused();
    }
}

#[cfg(test)]
mod tests;
