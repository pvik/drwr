mod canvas;
mod cli;
mod error_dialog;
mod menu;
mod open_dialog;
mod side_panel;
mod status_bar;
mod tool;
mod toolbar;

use std::path::Path;

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
    open_dialog: OpenDialog,
    error: Option<String>,
}

impl DrwrApp {
    /// Replaces the graph with the GraphML file at `path`, or keeps it on error.
    fn open(&mut self, path: &Path) -> Result<(), graphml::Error> {
        self.graphml = GraphMl::read_file(path)?;
        Ok(())
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
        // Panels are laid out outside-in in the order they're added, so the
        // canvas (the central panel) must come last.
        menu::show(ui, frame, &mut self.open_dialog);
        toolbar::show(ui, &mut self.tool);
        status_bar::show(ui, self.tool);
        side_panel::show(ui);
        canvas::show(ui);

        error_dialog::show(ui, &mut self.error);
    }
}

#[cfg(test)]
mod tests;
