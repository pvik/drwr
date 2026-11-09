use egui::accesskit::Toggled;

use super::*;
use crate::export::Format;
use crate::testing::ScratchDir;

/// How PNG files start.
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

impl Harness {
    /// Opens the export dialog with Ctrl+E, then clicks the widgets labelled
    /// `labels` in it, then Export…, which asks where to export.
    fn export_with(&mut self, labels: &[&str]) {
        self.press(CTRL, Key::E);
        for label in labels {
            self.click_widget(label);
        }
        self.click_widget("Export…");
    }

    /// Runs a frame, returning whether the widget labelled `label` is checked.
    fn checked(&mut self, label: &str) -> bool {
        self.widget(label).toggled() == Some(Toggled::True)
    }

    /// Picks `path` in the export's file dialog, then waits for the file.
    fn export_to(&mut self, path: &Path) {
        self.answer(Some(path));
        self.app.finish_exports(true);
    }

    /// Closes the export dialog with `key`, or by clicking Cancel if `None`.
    fn close_export_dialog(&mut self, key: Option<Key>) {
        match key {
            Some(key) => self.press(Modifiers::NONE, key),
            None => self.click_widget("Cancel"),
        }
        // The dialog keeps the keys in the frame it closes in.
        self.run(vec![]);
    }
}

fn read_to_string(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn the_toolbar_button_file_menu_and_ctrl_e_open_the_export_dialog() {
    let mut harness = fixture_harness();
    harness.app.tool = Tool::Box;
    harness.click_widget("Export");
    assert!(harness.app.export_dialog.is_open());
    harness.close_export_dialog(Some(Key::Escape));
    assert!(!harness.app.export_dialog.is_open());
    // The dialog kept the Escape, which would pick the Cursor tool.
    assert_eq!(harness.app.tool, Tool::Box);

    harness.click_widget("File");
    harness.click_widget("Export… Ctrl+E");
    assert!(harness.app.export_dialog.is_open());
    harness.close_export_dialog(None);
    assert!(!harness.app.export_dialog.is_open());

    harness.press(CTRL, Key::E);
    assert!(harness.app.export_dialog.is_open());
}

#[test]
fn the_file_menu_has_export_after_save_as() {
    let mut harness = Harness::new();
    harness.click_widget("File");
    let items: Vec<_> = [
        ("Save As…", Action::SaveAs),
        ("Export…", Action::Export),
        ("Quit", Action::Quit),
    ]
    .iter()
    .map(|(name, action)| {
        let shortcut = shortcuts::text(&harness.ctx, action).unwrap();
        testing::center(&harness.widget(&format!("{name} {shortcut}"))).y
    })
    .collect();
    assert!(items.is_sorted(), "{items:?}");
    assert_eq!(
        shortcuts::text(&harness.ctx, &Action::Export).as_deref(),
        Some("Ctrl+E")
    );
}

#[test]
fn export_waits_while_a_file_dialog_is_open() {
    let mut harness = fixture_harness();
    harness.press(CTRL.plus(Modifiers::SHIFT), Key::S);
    assert!(harness.widget("Export").is_disabled());
    harness.press(CTRL, Key::E);
    assert!(!harness.app.export_dialog.is_open());
    harness.answer(None);
    assert!(!harness.widget("Export").is_disabled());
}

#[test]
fn exporting_asks_where_suggesting_the_graphs_name_and_folder() {
    let mut harness = fixture_harness();
    harness.export_with(&[]);
    assert!(!harness.app.export_dialog.is_open());
    let request = Request::Export {
        format: Format::Png,
        name: "yed.png".to_owned(),
        folder: Path::new(FIXTURE).parent().map(Path::to_path_buf),
    };
    assert_eq!(harness.app.file_dialog.request(), Some(&request));

    let mut harness = Harness::new();
    harness.draw_box();
    harness.export_with(&["SVG"]);
    let request = Request::Export {
        format: Format::Svg,
        name: "untitled.svg".to_owned(),
        folder: None,
    };
    assert_eq!(harness.app.file_dialog.request(), Some(&request));
}

#[test]
fn exports_are_written_where_picked_and_change_nothing_else() {
    let dir = ScratchDir::new("export-written");
    let mut harness = fixture_harness();
    harness.export_with(&[]);
    harness.export_to(&dir.path("picked.png"));
    assert!(
        std::fs::read(dir.path("picked.png"))
            .unwrap()
            .starts_with(PNG)
    );

    harness.export_with(&["SVG"]);
    harness.export_to(&dir.path("picked.svg"));
    let svg = read_to_string(&dir.path("picked.svg"));
    assert!(svg.starts_with("<?xml"), "{svg}");
    assert!(svg.contains(">DB</tspan>"), "{svg}");

    assert_eq!(dir.names(), ["picked.png", "picked.svg"]);
    assert_eq!(harness.app.file.as_deref(), Some(Path::new(FIXTURE)));
    assert!(!harness.app.modified);
    assert_eq!(harness.app.error, None);
}

#[test]
fn canvas_grid_lines_are_not_exported() {
    let dir = ScratchDir::new("export-grid");
    let mut harness = fixture_harness();
    let export = |harness: &mut Harness, name: &str| {
        for (format, extension) in [("PNG", "png"), ("SVG", "svg")] {
            harness.export_with(&[format]);
            harness.export_to(&dir.path(&format!("{name}.{extension}")));
        }
    };
    export(&mut harness, "plain");
    harness.press(CTRL, Key::Quote);
    assert!(harness.app.canvas_options.grid_lines);
    export(&mut harness, "grid");
    for extension in ["png", "svg"] {
        let read = |name: &str| std::fs::read(dir.path(&format!("{name}.{extension}"))).unwrap();
        assert_eq!(read("plain"), read("grid"), "{extension}");
    }
}

#[test]
fn only_names_without_an_extension_get_the_formats() {
    let dir = ScratchDir::new("export-extensions");
    std::fs::create_dir(dir.path("v1.2")).unwrap();
    for (picked, written) in [
        ("plan.PNG", "plan.PNG"),
        ("plan.image", "plan.image"),
        ("v1.2/plan", "v1.2/plan.png"),
    ] {
        let mut harness = fixture_harness();
        harness.export_with(&[]);
        harness.export_to(&dir.path(picked));
        let bytes = std::fs::read(dir.path(written)).unwrap();
        assert!(bytes.starts_with(PNG), "{picked}");
    }
}

#[test]
fn a_name_without_an_extension_asks_before_replacing_that_file() {
    let dir = ScratchDir::new("export-replace");
    let file = dir.path("plan.png");
    std::fs::write(&file, "kept").unwrap();
    let question = "plan.png already exists. Replace it?";
    let mut harness = fixture_harness();
    for (answer, replaced) in [("Cancel", false), ("Replace", true)] {
        harness.export_with(&[]);
        harness.export_to(&dir.path("plan"));
        assert!(harness.painted(question));
        harness.answer_prompt(answer);
        harness.app.finish_exports(true);
        assert!(!harness.painted(question));
        let bytes = std::fs::read(&file).unwrap();
        assert_eq!(bytes.starts_with(PNG), replaced, "{answer}");
    }
}

#[test]
fn selected_area_only_exports_only_the_selected_items() {
    let dir = ScratchDir::new("export-selected");
    let mut harness = fixture_harness();
    harness.app.selected = Selection::node("n0");
    harness.export_with(&["SVG", "Selected area only"]);
    harness.export_to(&dir.path("selected.svg"));
    let svg = read_to_string(&dir.path("selected.svg"));
    assert!(svg.contains(">Order</tspan>"), "{svg}");
    assert!(!svg.contains(">DB</tspan>"), "{svg}");
    assert!(!svg.contains("<ellipse"), "{svg}");
}

#[test]
fn selected_area_only_needs_a_selection() {
    let mut harness = fixture_harness();
    harness.press(CTRL, Key::E);
    assert!(harness.widget("Selected area only").is_disabled());
    harness.close_export_dialog(Some(Key::Escape));
    harness.app.selected = Selection::edge(0);
    harness.press(CTRL, Key::E);
    assert!(!harness.widget("Selected area only").is_disabled());
    assert!(harness.checked("Entire document"));
}

#[test]
fn settings_are_kept_but_the_area_starts_as_entire_document() {
    let mut harness = fixture_harness();
    harness.app.selected = Selection::node("n0");
    harness.export_with(&["SVG", "Transparent background", "Selected area only"]);
    harness.answer(None);
    harness.press(CTRL, Key::E);
    assert!(harness.checked("SVG"));
    assert!(harness.checked("Transparent background"));
    assert!(harness.checked("Entire document"));

    // Cancelling keeps none of what was changed.
    harness.click_widget("PNG");
    harness.close_export_dialog(None);
    // Across files too.
    harness.perform(Action::New);
    harness.press(CTRL, Key::E);
    assert!(harness.checked("SVG"));
}

#[test]
fn export_failures_are_shown() {
    let mut harness = fixture_harness();
    harness.export_with(&[]);
    harness.export_to(Path::new("/nonexistent/drwr/plan.png"));
    let error = harness.app.error.clone().expect("an error is shown");
    assert!(
        error.starts_with("Couldn't export /nonexistent/drwr/plan.png:\n"),
        "{error}"
    );
}

#[test]
fn the_export_button_exports_the_label_typed_first() {
    let dir = ScratchDir::new("export-typed");
    let mut harness = Harness::new();
    type_in_label_field(&mut harness, "Start");
    harness.type_text("Typed");
    harness.click_widget("Export");
    harness.click_widget("SVG");
    harness.click_widget("Export…");
    harness.export_to(&dir.path("typed.svg"));
    let svg = read_to_string(&dir.path("typed.svg"));
    assert!(svg.contains(">Typed</tspan>"), "{svg}");
}

#[test]
fn new_closes_the_export_dialog() {
    let mut harness = fixture_harness();
    harness.press(CTRL, Key::E);
    harness.perform(Action::New);
    assert!(!harness.app.export_dialog.is_open());
}

/// An export that writes nothing, and finishes once `finish` is sent.
fn blocked_export(finish: std::sync::mpsc::Receiver<()>) -> Writing {
    Writing {
        path: PathBuf::from("blocked.png"),
        thread: std::thread::spawn(move || {
            let _ = finish.recv();
            Ok(())
        }),
    }
}

#[test]
fn the_status_bar_says_exporting_while_a_file_is_written() {
    let mut harness = Harness::new();
    assert!(harness.painted("Ready"));
    let (finish, finished) = std::sync::mpsc::channel();
    harness.app.exports.push(blocked_export(finished));
    assert!(harness.painted("Exporting…"));
    finish.send(()).unwrap();
    harness.app.finish_exports(true);
    assert!(harness.painted("Ready"));
}

#[test]
fn closing_the_window_waits_for_the_exports() {
    let mut harness = Harness::new();
    let (finish, finished) = std::sync::mpsc::channel();
    harness.app.exports.push(blocked_export(finished));
    // Once the window has started closing.
    let sender = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));
        finish.send(()).unwrap();
    });
    assert!(!harness.close_window());
    assert!(harness.app.exports.is_empty());
    sender.join().unwrap();
}
