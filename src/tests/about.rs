use super::*;

const PROJECT_PAGE: &str = "https://pvik.github.io/drwr/";

impl Harness {
    /// Clicks File > About drwr.
    fn open_about(&mut self) {
        self.click_widget("File");
        self.click_widget("About drwr");
        assert!(self.app.about, "the About dialog didn't open");
        // Close takes the focus the frame after.
        self.run(vec![]);
    }

    /// Runs a frame with `events`, returning the URLs it opened.
    fn opened_urls(&mut self, events: Vec<Event>) -> Vec<String> {
        let input = RawInput {
            events,
            ..RawInput::default()
        };
        let output = self.run_input(input);
        let urls = (output.platform_output.commands.iter())
            .filter_map(|command| match command {
                egui::OutputCommand::OpenUrl(open) => Some(open.url.clone()),
                _ => None,
            })
            .collect();
        output.drop_without_applying_deltas();
        urls
    }
}

#[test]
fn file_about_shows_the_version_commit_and_project_page() {
    let mut harness = Harness::new();
    harness.open_about();
    let version = format!("Version {}", env!("CARGO_PKG_VERSION"));
    let commit = format!("Commit {}", env!("DRWR_GIT_SHORT_COMMIT"));
    for text in [version.as_str(), commit.as_str(), PROJECT_PAGE] {
        assert!(harness.painted(text), "{text:?} wasn't painted");
    }
}

#[test]
fn the_file_menu_has_about_drwr_between_export_and_quit() {
    let mut harness = Harness::new();
    harness.click_widget("File");
    let label = |name: &str, action| {
        let shortcut = shortcuts::text(&harness.ctx, &action).unwrap();
        format!("{name} {shortcut}")
    };
    let export = label("Export…", Action::Export);
    let quit = label("Quit", Action::Quit);
    let items: Vec<_> = [export.as_str(), "About drwr", quit.as_str()]
        .iter()
        .map(|label| testing::center(&harness.widget(label)).y)
        .collect();
    assert!(items.is_sorted(), "{items:?}");
}

#[test]
fn close_enter_and_escape_close_the_about_dialog_keeping_the_keys() {
    let mut harness = Harness::new();
    harness.press(Modifiers::NONE, Key::B);
    for key in [None, Some(Key::Enter), Some(Key::Escape)] {
        harness.open_about();
        match key {
            Some(key) => harness.press(Modifiers::NONE, key),
            None => harness.click_widget("Close"),
        }
        assert!(!harness.app.about, "{key:?} left it open");
        // The dialog keeps the keys in the frame it closes in.
        harness.run(vec![]);
    }
    // Escape didn't also pick the Cursor tool.
    assert_eq!(harness.app.tool, Tool::Box);
}

#[test]
fn the_link_opens_the_project_page_on_a_click_or_tab_and_enter() {
    let mut harness = Harness::new();
    harness.open_about();
    let at = testing::center(&harness.widget(PROJECT_PAGE));
    harness.run(vec![Event::PointerMoved(at)]);
    harness.run(vec![button(at, true)]);
    assert_eq!(harness.opened_urls(vec![button(at, false)]), [PROJECT_PAGE]);
    assert!(harness.app.about);

    harness.press(Modifiers::NONE, Key::Escape);
    harness.run(vec![]);
    harness.open_about();
    // From Close to the link; the focus moves the frame after.
    harness.press(Modifiers::NONE, Key::Tab);
    harness.run(vec![]);
    let enter = testing::key(Modifiers::NONE, Key::Enter);
    assert_eq!(harness.opened_urls(vec![enter]), [PROJECT_PAGE]);
    assert!(harness.app.about);
}

#[test]
fn the_keyboard_opens_the_about_dialog_from_the_file_menu() {
    let mut harness = Harness::new();
    harness.press(Modifiers::ALT, Key::F);
    // New, Open…, Save, Save As…, Export…, then About drwr, as Recent Files
    // is disabled without any. Each moves the focus the frame after.
    let downs = std::iter::repeat_n(Key::ArrowDown, 6);
    for key in downs.chain([Key::Enter]) {
        harness.run(vec![]);
        harness.press(Modifiers::NONE, key);
    }
    harness.run(vec![]);
    // The Enter that picked it didn't close it too.
    assert!(harness.app.about);
    assert!(!Popup::is_any_open(&harness.ctx));
    harness.press(Modifiers::NONE, Key::Enter);
    assert!(!harness.app.about);
}
