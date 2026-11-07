mod action;
mod canvas;
mod cli;
mod color_picker;
mod document;
mod error_dialog;
mod menu;
mod open_dialog;
mod shortcuts;
mod side_panel;
mod status_bar;
mod tool;
mod toolbar;

use std::path::{Path, PathBuf};

use action::Action;
use canvas::Canvas;
use color_picker::ColorPicker;
use document::ColorProperty;
use graphml::GraphMl;
use open_dialog::OpenDialog;
use tool::Tool;

fn main() -> eframe::Result {
    match cli::parse(std::env::args_os().skip(1)) {
        Ok(cli::Command::Run { file }) => {
            let mut app = DrwrApp::default();
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
            // Needed to render the toolbar's SVG icons.
            egui_extras::install_image_loaders(&cc.egui_ctx);
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
    open_dialog: OpenDialog,
    /// Picks the colors of the selected node.
    color_picker: ColorPicker<ColorProperty>,
    error: Option<String>,
}

impl DrwrApp {
    /// Replaces the graph with the GraphML file at `path`, or keeps it on error.
    fn open(&mut self, path: &Path) -> Result<(), graphml::Error> {
        let path = std::path::absolute(path)?;
        self.graphml = GraphMl::read_file(&path)?;
        self.file = Some(path);
        self.selected = None;
        self.color_picker.close();
        Ok(())
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
            Action::EditColor(property) => {
                if let Some(id) = &self.selected
                    && let Some(graphics) = document::graphics_of(&self.graphml, id)
                {
                    let title = format!("{} color", property.name());
                    self.color_picker
                        .open(property, title, property.get(&graphics));
                }
            }
            Action::SetColor(property, color) => {
                if let Some(id) = &self.selected {
                    document::edit_graphics(&mut self.graphml, id, |graphics| {
                        property.set(graphics, color);
                    });
                }
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
        if let Some(action) = shortcuts::pressed(ui.ctx()) {
            self.perform(action, ui.ctx(), frame);
        }
        // Panels are laid out outside-in in the order they're added, so the
        // canvas (the central panel) must come last.
        if let Some(action) = menu::show(ui, &self.open_dialog) {
            self.perform(action, ui.ctx(), frame);
        }
        if let Some(action) = toolbar::show(ui, &self.open_dialog, self.tool) {
            self.perform(action, ui.ctx(), frame);
        }
        status_bar::show(ui, self.tool, self.file.as_deref());
        if let Some(action) = side_panel::show(ui, &self.graphml, self.selected.as_deref()) {
            self.perform(action, ui.ctx(), frame);
        }
        self.canvas
            .show(ui, self.tool, &mut self.graphml, &mut self.selected);

        if let Some((property, color)) = self.color_picker.show(ui.ctx()) {
            self.perform(Action::SetColor(property, color), ui.ctx(), frame);
        }
        error_dialog::show(ui, &mut self.error);
    }
}

#[cfg(test)]
mod tests;
