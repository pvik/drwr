use egui::accesskit::Toggled;

use super::*;
use crate::testing::ScratchDir;

/// Starts the app keeping its settings in `dir`.
fn start(dir: &ScratchDir) -> Harness {
    Harness::with_config(Config::load_from(dir.path("")))
}

fn settings_file(dir: &ScratchDir) -> String {
    std::fs::read_to_string(dir.path("settings.toml")).unwrap()
}

impl Harness {
    /// Runs the frames of `seconds`, one a sixtieth of a second on, as egui
    /// counts frames without a time.
    fn wait(&mut self, seconds: f64) {
        for _ in 0..(seconds * 60.0).ceil() as usize {
            self.run(vec![]);
        }
    }
}

#[test]
fn view_menu_options_outlast_a_restart() {
    let dir = ScratchDir::new("settings-view");
    let mut harness = start(&dir);
    harness.press(CTRL, Key::Quote);
    assert!(settings_file(&dir).contains("grid_lines = true"));
    harness.click_widget("View");
    harness.click_widget("Snap Lines");
    // Too soon after the grid lines to be written yet.
    assert!(settings_file(&dir).contains("snap_lines = true"));
    assert!(!harness.close_window());

    let mut harness = start(&dir);
    let options = canvas::Options {
        grid_lines: true,
        snap_lines: false,
    };
    assert_eq!(harness.app.settings.view, options);
    harness.click_widget("View");
    let toggled = |harness: &mut Harness, label| harness.widget(label).toggled();
    assert_eq!(
        toggled(&mut harness, "Canvas Grid Lines Ctrl+'"),
        Some(Toggled::True)
    );
    assert_eq!(toggled(&mut harness, "Snap Lines"), Some(Toggled::False));
}

#[test]
fn a_change_soon_after_another_is_written_a_second_later() {
    let dir = ScratchDir::new("settings-later");
    let mut harness = start(&dir);
    harness.press(CTRL, Key::Quote);
    harness.press(CTRL, Key::Quote);
    let output = harness.run_input(RawInput::default());
    let root = &output.viewport_output[&egui::ViewportId::ROOT];
    assert!(root.repaint_delay.as_secs_f64() <= config::SAVE_INTERVAL);
    output.drop_without_applying_deltas();
    assert!(settings_file(&dir).contains("grid_lines = true"));
    harness.wait(config::SAVE_INTERVAL);
    assert!(settings_file(&dir).contains("grid_lines = false"));
}

#[test]
fn quitting_at_once_waits_for_settings_to_be_written_and_changes_saved() {
    let dir = ScratchDir::new("settings-quit");
    let mut harness = start(&dir);
    assert!(harness.app.can_quit_at_once());
    harness.press(CTRL, Key::Quote);
    harness.press(CTRL, Key::Quote);
    assert!(!harness.app.can_quit_at_once());
    harness.wait(config::SAVE_INTERVAL);
    assert!(harness.app.can_quit_at_once());
    harness.draw_box();
    assert!(!harness.app.can_quit_at_once());
}

#[test]
fn the_new_item_style_and_export_settings_outlast_a_restart() {
    let dir = ScratchDir::new("settings-style");
    let mut harness = start(&dir);
    harness.draw_box();
    harness.perform(Action::SetColor(ColorProperty::Fill, Some(RED)));
    harness.press(CTRL, Key::E);
    harness.click_widget("SVG");
    harness.click_widget("Export…");
    harness.answer(None);
    assert_eq!(
        harness.app.settings.export.format,
        crate::export::Format::Svg
    );
    harness.wait(config::SAVE_INTERVAL);
    let settings = harness.app.settings.clone();

    let mut harness = start(&dir);
    assert_eq!(harness.app.settings, settings);
    harness.draw_box();
    let id = harness.app.selected_node().unwrap().to_owned();
    assert_eq!(harness.graphics(&id).fill.color, Some(RED));
}

#[test]
fn recent_files_outlast_a_restart() {
    let dir = ScratchDir::new("settings-recent");
    let file = dir.path("a.graphml");
    std::fs::copy(FIXTURE, &file).unwrap();
    let file = dunce::canonicalize(file).unwrap();
    start(&dir).app.open(&file).unwrap();
    assert_eq!(start(&dir).app.config.recent_files(), [file]);
}
