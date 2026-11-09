use graphml::yed::{Color, FontStyle, LineType};

use super::*;
use crate::action::FontPart;
use crate::document::ColorProperty;
use crate::selection::Selection;
use crate::testing::ScratchDir;

/// Settings with everything set, and nothing as by default.
fn changed_settings() -> Settings {
    let mut style = Style::default();
    let (nodes, edges) = (Selection::node("n0"), Selection::edge(0));
    style.set_color(&nodes, ColorProperty::Fill, Some(Color::rgb(0xFF, 0, 0)));
    style.set_color(&nodes, ColorProperty::Gradient, Some(Color::WHITE));
    style.set_color(&nodes, ColorProperty::Border, None);
    style.set_line_width(&nodes, 2.0);
    style.set_line_type(&nodes, &LineType::Dashed);
    style.set_corner_radius(&nodes, 8.0);
    let blue = Color {
        a: 0x80,
        ..Color::rgb(0, 0, 0xFF)
    };
    style.set_color(&edges, ColorProperty::Line, Some(blue));
    style.set_line_width(&edges, 1.5);
    style.set_line_type(&edges, &LineType::Dotted);
    style.set_font(&FontPart::Family("Monospaced".to_owned()));
    style.set_font(&FontPart::Style(FontStyle::Bold));
    style.set_font(&FontPart::Size(14));
    style.set_color(&nodes, ColorProperty::Text(0), Some(Color::rgb(0, 0x80, 0)));
    Settings {
        view: canvas::Options {
            grid_lines: true,
            snap_lines: false,
        },
        export: export::Settings {
            format: export::Format::Svg,
            margin: 20,
            transparent: true,
            scale: 200,
            outlines: true,
        },
        style,
    }
}

/// Writes `text` as the settings file in `dir`, then reads what it keeps.
fn load_settings(dir: &ScratchDir, text: &str) -> Settings {
    fs::write(dir.path(SETTINGS), text).unwrap();
    Config::load_from(dir.path("")).settings().clone()
}

fn read_settings(dir: &ScratchDir) -> String {
    fs::read_to_string(dir.path(SETTINGS)).unwrap()
}

#[test]
fn settings_written_read_back_the_same() {
    let dir = ScratchDir::new("config-round-trip");
    let settings = changed_settings();
    Config::load_from(dir.path("")).save(&settings, 0.0);
    assert_eq!(*Config::load_from(dir.path("")).settings(), settings);
}

#[test]
fn the_settings_file_is_toml_in_sections() {
    let dir = ScratchDir::new("config-text");
    Config::load_from(dir.path("")).save(&changed_settings(), 0.0);
    let text = r##"[view]
grid_lines = true
snap_lines = false

[export]
format = "svg"
margin = 20
transparent = true
scale = 200
outlines = true

[style]
fill = "#FF0000"
gradient = "#FFFFFF"
corner_radius = 8.0
font_family = "Monospaced"
font_style = "bold"
font_size = 14
text_color = "#008000"

[style.border]
color = "none"
width = 2.0
line_type = "dashed"

[style.line]
color = "#0000FF80"
width = 1.5
line_type = "dotted"
"##;
    assert_eq!(read_settings(&dir), text);
}

#[test]
fn styles_never_set_are_left_out() {
    let dir = ScratchDir::new("config-unset-style");
    let settings = Settings {
        view: canvas::Options {
            grid_lines: true,
            ..canvas::Options::default()
        },
        ..Settings::default()
    };
    Config::load_from(dir.path("")).save(&settings, 0.0);
    let text = "[view]
grid_lines = true
snap_lines = true

[export]
format = \"png\"
margin = 10
transparent = false
scale = 100
outlines = false
";
    assert_eq!(read_settings(&dir), text);
    assert_eq!(*Config::load_from(dir.path("")).settings(), settings);
}

#[test]
fn an_empty_folder_has_the_defaults() {
    let dir = ScratchDir::new("config-empty");
    let config = Config::load_from(dir.path(""));
    assert_eq!(*config.settings(), Settings::default());
    assert!(config.recent_files().is_empty());
}

#[test]
fn settings_missing_from_the_file_keep_their_defaults() {
    let dir = ScratchDir::new("config-partial");
    let settings = load_settings(&dir, "[view]\ngrid_lines = true\n");
    let expected = canvas::Options {
        grid_lines: true,
        snap_lines: true,
    };
    assert_eq!(settings.view, expected);
    assert_eq!(settings.export, export::Settings::default());
    assert_eq!(settings.style, Style::default());
}

#[test]
fn values_this_version_cant_read_are_left_as_by_default() {
    let dir = ScratchDir::new("config-unknown-values");
    let text = r##"
rulers = true

[export]
format = "pdf"
margin = 5

[style]
fill = "red"
font_style = "oblique"
text_color = "#00FF00"

[style.border]
line_type = "wavy"
width = 3.0
"##;
    let settings = load_settings(&dir, text);
    let export = export::Settings {
        margin: 5,
        ..export::Settings::default()
    };
    assert_eq!(settings.export, export);
    let mut style = Style::default();
    let node = Selection::node("n0");
    style.set_color(&node, ColorProperty::Text(0), Some(Color::rgb(0, 0xFF, 0)));
    style.set_line_width(&node, 3.0);
    assert_eq!(settings.style, style);
}

#[test]
fn a_settings_file_that_isnt_toml_gives_the_defaults() {
    let dir = ScratchDir::new("config-not-toml");
    assert_eq!(load_settings(&dir, "grid_lines = [\n"), Settings::default());
    let wrong_type = "[view]\ngrid_lines = \"yes\"\n";
    assert_eq!(load_settings(&dir, wrong_type), Settings::default());
}

#[test]
fn unchanged_settings_arent_written() {
    let dir = ScratchDir::new("config-unchanged");
    let mut config = Config::load_from(dir.path(""));
    assert_eq!(config.save(&Settings::default(), 0.0), None);
    config.flush(&Settings::default());
    assert!(dir.names().is_empty(), "{:?}", dir.names());
}

#[test]
fn settings_are_written_at_most_once_a_second() {
    let dir = ScratchDir::new("config-interval");
    let mut config = Config::load_from(dir.path(""));
    let mut settings = Settings::default();
    settings.view.grid_lines = true;
    assert_eq!(config.save(&settings, 10.0), None);
    assert!(read_settings(&dir).contains("grid_lines = true"));

    settings.view.snap_lines = false;
    assert_eq!(config.save(&settings, 10.25), Some(0.75));
    assert!(read_settings(&dir).contains("snap_lines = true"));
    assert_eq!(config.save(&settings, 11.0), None);
    assert!(read_settings(&dir).contains("snap_lines = false"));
}

#[test]
fn flush_writes_changed_settings_however_recently_they_were_written() {
    let dir = ScratchDir::new("config-flush");
    let mut config = Config::load_from(dir.path(""));
    let mut settings = Settings::default();
    settings.view.grid_lines = true;
    config.save(&settings, 0.0);
    settings.view.grid_lines = false;
    assert!(config.save(&settings, 0.1).is_some());
    config.flush(&settings);
    assert!(read_settings(&dir).contains("grid_lines = false"));
    assert_eq!(config.save(&settings, 0.2), None);
}

/// Makes empty files with `names` in `dir`, returning their canonical paths.
fn files(dir: &ScratchDir, names: &[&str]) -> Vec<PathBuf> {
    let paths = names.iter().map(|name| {
        let path = dir.path(name);
        fs::write(&path, "").unwrap();
        fs::canonicalize(path).unwrap()
    });
    paths.collect()
}

#[test]
fn recent_files_put_the_latest_first_once() {
    let dir = ScratchDir::new("config-recent-order");
    let [a, b] = files(&dir, &["a.graphml", "b.graphml"]).try_into().unwrap();
    let mut config = Config::load_from(dir.path(""));
    for path in [&a, &b, &a] {
        config.add_recent_file(path);
    }
    assert_eq!(config.recent_files(), [a.clone(), b.clone()]);
    assert_eq!(Config::load_from(dir.path("")).recent_files(), [a, b]);
}

#[test]
fn recent_files_keep_the_latest_ten() {
    let dir = ScratchDir::new("config-recent-ten");
    let names: Vec<_> = (0..12).map(|i| format!("{i}.graphml")).collect();
    let names: Vec<_> = names.iter().map(String::as_str).collect();
    let paths = files(&dir, &names);
    let mut config = Config::load_from(dir.path(""));
    for path in &paths {
        config.add_recent_file(path);
    }
    let latest: Vec<_> = paths.iter().rev().take(MAX_RECENT_FILES).cloned().collect();
    assert_eq!(config.recent_files(), latest);
    assert_eq!(Config::load_from(dir.path("")).recent_files(), latest);
}

#[test]
fn a_file_reached_by_two_paths_is_listed_once() {
    let dir = ScratchDir::new("config-recent-canonical");
    fs::create_dir(dir.path("sub")).unwrap();
    let [file] = files(&dir, &["a.graphml"]).try_into().unwrap();
    let mut config = Config::load_from(dir.path(""));
    config.add_recent_file(&dir.path("sub/../a.graphml"));
    config.add_recent_file(&file);
    assert_eq!(config.recent_files(), [file]);
}

#[test]
fn recent_files_keep_what_another_drwr_added() {
    let dir = ScratchDir::new("config-recent-shared");
    let [a, b, c] = files(&dir, &["a.graphml", "b.graphml", "c.graphml"])
        .try_into()
        .unwrap();
    let mut first = Config::load_from(dir.path(""));
    let mut second = Config::load_from(dir.path(""));
    first.add_recent_file(&a);
    second.add_recent_file(&b);
    first.add_recent_file(&c);
    assert_eq!(first.recent_files(), [c.clone(), b.clone(), a.clone()]);
    second.remove_recent_file(&b);
    assert_eq!(second.recent_files(), [c, a]);
}

#[test]
fn removed_recent_files_stay_removed() {
    let dir = ScratchDir::new("config-recent-remove");
    let [a, b] = files(&dir, &["a.graphml", "b.graphml"]).try_into().unwrap();
    let mut config = Config::load_from(dir.path(""));
    config.add_recent_file(&a);
    config.add_recent_file(&b);
    config.remove_recent_file(&a);
    assert_eq!(config.recent_files(), std::slice::from_ref(&b));
    assert_eq!(Config::load_from(dir.path("")).recent_files(), [b]);
}

#[test]
fn without_a_folder_recent_files_last_until_drwr_quits() {
    let dir = ScratchDir::new("config-recent-memory");
    let [a, b] = files(&dir, &["a.graphml", "b.graphml"]).try_into().unwrap();
    let mut config = Config::default();
    config.add_recent_file(&a);
    config.add_recent_file(&b);
    config.remove_recent_file(&a);
    assert_eq!(config.recent_files(), [b]);
    assert_eq!(dir.names(), ["a.graphml", "b.graphml"]);
}

#[test]
fn the_recent_files_file_lists_paths() {
    let dir = ScratchDir::new("config-recent-text");
    let [a, b] = files(&dir, &["a.graphml", "b \"c\".graphml"])
        .try_into()
        .unwrap();
    let mut config = Config::load_from(dir.path(""));
    config.add_recent_file(&a);
    config.add_recent_file(&b);
    let text = fs::read_to_string(dir.path(RECENT_FILES)).unwrap();
    let quoted = |path: &Path| toml::Value::from(path.to_str().unwrap()).to_string();
    assert_eq!(
        text,
        format!("files = [\n    {},\n    {},\n]\n", quoted(&b), quoted(&a))
    );
}

#[cfg(unix)]
#[test]
fn the_config_folder_is_made_private_with_its_parents() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = ScratchDir::new("config-folder");
    let folder = dir.path("config/drwr");
    create_folder(&folder).unwrap();
    for path in [dir.path("config"), folder.clone()] {
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700, "{path:?}");
    }
    // It may exist already.
    create_folder(&folder).unwrap();
}
