mod canvas;
mod cli;
mod menu;
mod status_bar;
mod toolbar;

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
    let options = eframe::NativeOptions::default();

    // The app name doubles as the default window title and the Wayland app id.
    eframe::run_native(
        "drwr",
        options,
        Box::new(|_cc| Ok(Box::new(DrwrApp::default()))),
    )
}

#[derive(Default)]
struct DrwrApp {}

impl eframe::App for DrwrApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Panels are laid out from the outside in, in the order they are added:
        // the menu bar ends up above the toolbar, and the canvas (the central
        // panel) must come last.
        menu::show(ui);
        toolbar::show(ui);
        status_bar::show(ui);
        canvas::show(ui);
    }
}
