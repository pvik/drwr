mod canvas;
mod cli;
mod menu;
mod side_panel;
mod status_bar;
mod tool;
mod toolbar;

use tool::Tool;

fn main() -> eframe::Result {
    match cli::parse(std::env::args_os().skip(1)) {
        Ok(cli::Command::Run) => run_gui(),
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

fn run_gui() -> eframe::Result {
    // The app name doubles as the default window title and the Wayland app id.
    eframe::run_native(
        "drwr",
        eframe::NativeOptions::default(),
        Box::new(|cc| {
            // Needed to render the toolbar's SVG icons.
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(DrwrApp::default()))
        }),
    )
}

#[derive(Default)]
struct DrwrApp {
    tool: Tool,
}

impl eframe::App for DrwrApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Panels are laid out outside-in in the order they're added, so the
        // canvas (the central panel) must come last.
        menu::show(ui);
        toolbar::show(ui, &mut self.tool);
        status_bar::show(ui, self.tool);
        side_panel::show(ui);
        canvas::show(ui);
    }
}
