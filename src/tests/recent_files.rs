use directories::BaseDirs;

use super::*;
use crate::testing::ScratchDir;

/// Copies the yEd fixture into `dir` as `name`, returning the copy's
/// canonical path, which Recent Files lists.
fn copy_fixture(dir: &ScratchDir, name: &str) -> PathBuf {
    let path = dir.path(name);
    std::fs::copy(FIXTURE, &path).unwrap();
    std::fs::canonicalize(path).unwrap()
}

/// The label of the File > Recent Files item for `path`.
fn item(path: &Path) -> String {
    let dirs = BaseDirs::new();
    let (name, folder) = menu::describe(path, dirs.as_ref().map(BaseDirs::home_dir));
    format!("{name} {folder}")
}

impl Harness {
    fn open_recent_files(&mut self) {
        self.click_widget("File");
        self.click_widget("Recent Files");
    }

    /// Runs a frame, returning how far down the widget labelled `label` is.
    fn height_of(&mut self, label: &str) -> f32 {
        testing::center(&self.widget(label)).y
    }
}

#[test]
fn recent_files_come_after_open_and_are_disabled_while_empty() {
    let mut harness = Harness::new();
    harness.click_widget("File");
    assert!(harness.widget("Recent Files").is_disabled());
    let label = |name: &str, action| {
        let shortcut = shortcuts::text(&harness.ctx, &action).unwrap();
        format!("{name} {shortcut}")
    };
    let (open, save) = (label("Open…", Action::Open), label("Save", Action::Save));
    let heights = [
        harness.height_of(&open),
        harness.height_of("Recent Files"),
        harness.height_of(&save),
    ];
    assert!(heights.is_sorted(), "{heights:?}");
}

#[test]
fn files_opened_and_saved_are_listed_latest_first() {
    let dir = ScratchDir::new("recent-listed");
    let opened = copy_fixture(&dir, "opened.graphml");
    let mut harness = Harness::open(&opened);
    harness.press(CTRL.plus(Modifiers::SHIFT), Key::S);
    harness.answer(Some(&dir.path("saved.graphml")));
    let saved = std::fs::canonicalize(dir.path("saved.graphml")).unwrap();
    let listed = [saved.clone(), opened.clone()];
    assert_eq!(harness.app.config.recent_files(), listed);

    harness.open_recent_files();
    let (saved, opened) = (item(&saved), item(&opened));
    assert!(harness.height_of(&saved) < harness.height_of(&opened));
    assert!(!harness.widget(&saved).is_disabled());
}

#[cfg(unix)]
#[test]
fn recent_files_show_the_home_folder_as_a_tilde() {
    let home = BaseDirs::new().unwrap().home_dir().to_path_buf();
    let mut harness = Harness::new();
    for path in [
        home.join("drwr-test-plan.graphml"),
        home.join("Work/drwr test/flow chart.graphml"),
        PathBuf::from("/nonexistent/maps/net.graphml"),
    ] {
        harness.app.config.add_recent_file(&path);
    }
    harness.open_recent_files();
    for label in [
        "net.graphml /nonexistent/maps",
        "flow chart.graphml ~/Work/drwr test",
        "drwr-test-plan.graphml ~",
    ] {
        harness.widget(label);
    }
}

#[test]
fn a_recent_file_opens_from_the_file_menu() {
    let dir = ScratchDir::new("recent-open");
    let file = copy_fixture(&dir, "a.graphml");
    let mut harness = Harness::open(&file);
    harness.press(CTRL, Key::N);
    assert_eq!(harness.app.file, None);
    harness.open_recent_files();
    harness.click_widget(&item(&file));
    assert_eq!(harness.app.file.as_deref(), Some(file.as_path()));
    assert_eq!(
        *harness.app.doc.graphml(),
        GraphMl::read_file(FIXTURE).unwrap()
    );
    assert!(!Popup::is_any_open(&harness.ctx));
}

#[test]
fn the_keyboard_opens_a_recent_file() {
    let dir = ScratchDir::new("recent-keys");
    let file = copy_fixture(&dir, "a.graphml");
    let mut harness = Harness::open(&file);
    harness.press(CTRL, Key::N);
    harness.press(Modifiers::ALT, Key::F);
    // New, Open…, Recent Files, into it, then its first file. Each moves the
    // focus the frame after.
    let keys = [
        Key::ArrowDown,
        Key::ArrowDown,
        Key::ArrowDown,
        Key::ArrowRight,
    ];
    for key in keys.into_iter().chain([Key::Enter]) {
        harness.run(vec![]);
        harness.press(Modifiers::NONE, key);
    }
    assert_eq!(harness.app.file.as_deref(), Some(file.as_path()));
    harness.run(vec![]);
    assert!(!Popup::is_any_open(&harness.ctx));
}

#[test]
fn opening_a_recent_file_asks_to_save_changes_first() {
    let dir = ScratchDir::new("recent-unsaved");
    let file = copy_fixture(&dir, "a.graphml");
    let mut harness = Harness::open(&file);
    harness.press(CTRL, Key::N);
    harness.draw_box();
    harness.open_recent_files();
    harness.click_widget(&item(&file));
    assert!(harness.asks_to_save());
    assert_eq!(harness.app.file, None);
    harness.answer_prompt("Don't Save");
    assert_eq!(harness.app.file.as_deref(), Some(file.as_path()));
    assert!(!harness.app.modified);
}

#[test]
fn a_recent_file_thats_gone_leaves_the_list_without_asking_to_save() {
    let dir = ScratchDir::new("recent-gone");
    let file = copy_fixture(&dir, "a.graphml");
    let mut harness = Harness::open(&file);
    harness.press(CTRL, Key::N);
    harness.draw_box();
    std::fs::remove_file(&file).unwrap();
    harness.open_recent_files();
    harness.click_widget(&item(&file));
    assert!(!harness.asks_to_save());
    let error = harness.app.error.clone().expect("it says why");
    assert!(error.contains("no longer exists"), "{error}");
    assert!(harness.app.config.recent_files().is_empty());
    assert_eq!(harness.app.file, None);
    assert!(harness.app.modified);
}

#[test]
fn recent_files_wait_while_a_file_dialog_is_open() {
    let dir = ScratchDir::new("recent-dialog");
    let file = copy_fixture(&dir, "a.graphml");
    let mut harness = Harness::open(&file);
    harness.press(CTRL, Key::N);
    harness.press(CTRL, Key::O);
    assert!(harness.app.file_dialog.is_open());
    harness.perform(Action::OpenRecent(file));
    assert_eq!(harness.app.file, None);
    harness.click_widget("File");
    assert!(harness.widget("Recent Files").is_disabled());
}
