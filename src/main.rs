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
mod shortcuts;
mod side_panel;
mod status_bar;
#[cfg(test)]
mod testing;
mod text_dialog;
mod text_field;
mod tool;
mod toolbar;
mod zoom;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use action::Action;
use canvas::Canvas;
use color_picker::ColorPicker;
use document::{ColorProperty, LabelRef};
use fonts::Fonts;
use graphml::GraphMl;
use graphml::yed::NodeGraphics;
use open_dialog::OpenDialog;
use side_panel::SidePanel;
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
    /// The id of the selected node.
    selected: Option<String>,
    canvas: Canvas,
    /// The fonts labels are drawn in, which egui is given too.
    fonts: Arc<Fonts>,
    side_panel: SidePanel,
    open_dialog: OpenDialog,
    /// Picks the colors of the selected node.
    color_picker: ColorPicker<ColorProperty>,
    label_dialog: TextDialog<LabelRef>,
    error: Option<String>,
    /// Whether a text field had the focus at the end of the last frame; see
    /// [`shortcuts::pressed`].
    typing: bool,
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
        self.selected = None;
        // Labels being edited belong to the old graph.
        self.canvas = Canvas::default();
        self.side_panel = SidePanel::default();
        self.color_picker.close();
        self.label_dialog.close();
        Ok(())
    }

    /// Edits the graphics of the selected node, if there is one.
    fn edit_selected(&mut self, edit: impl FnOnce(&mut NodeGraphics)) {
        if let Some(id) = &self.selected {
            document::edit_graphics(&mut self.graphml, id, edit);
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
            Action::MoveBy(dx, dy) => self.edit_selected(|graphics| {
                graphics.geometry.x += dx;
                graphics.geometry.y += dy;
            }),
            Action::EditColor(property) => {
                if let Some(id) = &self.selected
                    && let Some(graphics) = document::graphics_of(&self.graphml, id)
                {
                    let title = format!("{} color", property.name());
                    let color = property.get(&graphics);
                    self.color_picker
                        .open(property, title, color, property.optional());
                }
            }
            Action::SetColor(property, color) => {
                self.edit_selected(|graphics| property.set(graphics, color));
            }
            Action::SetBorderWidth(width) => {
                self.edit_selected(|graphics| graphics.border.width = width);
            }
            Action::SetBorderType(line_type) => {
                self.edit_selected(|graphics| graphics.border.line_type = line_type);
            }
            Action::EditLabel => {
                if let Some(id) = &self.selected {
                    let label = LabelRef {
                        node: id.clone(),
                        index: 0,
                    };
                    if let Some(text) = document::label_text(&self.graphml, &label) {
                        self.canvas.edit_label(label, text);
                    }
                }
            }
            Action::EditLabelInDialog(label) => {
                if let Some(text) = document::label_text(&self.graphml, &label) {
                    self.label_dialog.open(label, "Label", text);
                }
            }
            Action::SetLabel(label, text) => document::set_label(&mut self.graphml, &label, &text),
            Action::SetFont(label, font) => {
                document::edit_label(&mut self.graphml, &label, |label| label.font = font);
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
        if let Some(action) = menu::show(ui, &self.open_dialog) {
            self.perform(action, ui.ctx(), frame);
        }
        let zoom = self.canvas.zoom();
        if let Some(action) = toolbar::show(ui, &self.open_dialog, self.tool, zoom) {
            self.perform(action, ui.ctx(), frame);
        }
        status_bar::show(ui, self.tool, self.file.as_deref());
        let selected = self.selected.as_deref();
        for action in self
            .side_panel
            .show(ui, &self.graphml, selected, &self.fonts)
        {
            self.perform(action, ui.ctx(), frame);
        }
        let actions = self
            .canvas
            .show(ui, self.tool, &mut self.graphml, &mut self.selected);
        for action in actions {
            self.perform(action, ui.ctx(), frame);
        }

        if let Some((property, color)) = self.color_picker.show(ui.ctx()) {
            self.perform(Action::SetColor(property, color), ui.ctx(), frame);
        }
        if let Some((label, text)) = self.label_dialog.show(ui.ctx()) {
            self.perform(Action::SetLabel(label, text), ui.ctx(), frame);
        }
        error_dialog::show(ui, &mut self.error);
        self.typing = ui.ctx().text_edit_focused();
    }
}

#[cfg(test)]
mod tests;
