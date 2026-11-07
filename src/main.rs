mod action;
mod canvas;
mod cli;
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
    canvas: Canvas,
    open_dialog: OpenDialog,
    error: Option<String>,
}

impl DrwrApp {
    /// Replaces the graph with the GraphML file at `path`, or keeps it on error.
    fn open(&mut self, path: &Path) -> Result<(), graphml::Error> {
        let path = std::path::absolute(path)?;
        self.graphml = GraphMl::read_file(&path)?;
        self.file = Some(path);
        Ok(())
    }

    /// Does what the menu, the toolbar or a keyboard shortcut asked for.
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
        side_panel::show(ui);
        self.canvas.show(ui, self.tool, &mut self.graphml);

        error_dialog::show(ui, &mut self.error);
    }
}

#[cfg(test)]
mod tests;
