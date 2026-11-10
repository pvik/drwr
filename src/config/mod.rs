//! What drwr keeps in its config folder across restarts: the settings, and
//! the files last opened or saved.

use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::style::Style;
use crate::{atomic_file, canvas, export};

/// How many files File > Recent Files lists.
pub const MAX_RECENT_FILES: usize = 10;
/// The least time between writes of the settings, in seconds, so that a drag
/// in a number field doesn't write them every frame.
pub const SAVE_INTERVAL: f64 = 1.0;
const SETTINGS: &str = "settings.toml";
const RECENT_FILES: &str = "recent-files.toml";

/// What the user set that outlasts files and restarts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// What the View menu turns on for the canvas.
    pub view: canvas::Options,
    /// How the drawing was last exported.
    pub export: export::Settings,
    /// What the user last set on nodes, lines and labels, for new ones.
    #[serde(skip_serializing_if = "is_default")]
    pub style: Style,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// The recent files as their file has them.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
struct RecentFiles {
    files: Vec<PathBuf>,
}

/// Keeps the settings and the recent files in a folder, or only in memory
/// without one, as in tests.
#[derive(Default)]
pub struct Config {
    folder: Option<PathBuf>,
    /// Most recent first.
    recent_files: Vec<PathBuf>,
    /// The settings as last read or written.
    saved: Settings,
    /// When the settings were last written, in egui's seconds.
    saved_at: Option<f64>,
}

impl Config {
    /// Reads what drwr's config folder keeps, e.g. `~/.config/drwr` on Linux,
    /// making it first if missing. Without one, nothing outlasts this run,
    /// and stderr says why.
    pub fn load() -> Config {
        let Some(dirs) = directories::ProjectDirs::from("", "", crate::NAME) else {
            eprintln!("warning: no home folder, so settings won't be kept");
            return Config::default();
        };
        let folder = dirs.config_dir();
        if let Err(err) = create_folder(folder) {
            let folder = folder.display();
            eprintln!("warning: couldn't make {folder}, so settings won't be kept: {err}");
            return Config::default();
        }
        Config::load_from(folder.to_path_buf())
    }

    /// Reads what `folder` keeps.
    pub fn load_from(folder: PathBuf) -> Config {
        let saved = read(&folder.join(SETTINGS)).unwrap_or_default();
        let recent: Option<RecentFiles> = read(&folder.join(RECENT_FILES));
        let mut recent_files = recent.map(|recent| recent.files).unwrap_or_default();
        recent_files.truncate(MAX_RECENT_FILES);
        Config {
            folder: Some(folder),
            recent_files,
            saved,
            saved_at: None,
        }
    }

    /// The settings as last read or written.
    pub fn settings(&self) -> &Settings {
        &self.saved
    }

    /// Writes `settings` if they changed, unless they were written less than
    /// [`SAVE_INTERVAL`] before `now`, in egui's seconds. Returns how long
    /// until they may be.
    pub fn save(&mut self, settings: &Settings, now: f64) -> Option<f64> {
        if *settings == self.saved {
            return None;
        }
        if let Some(at) = self.saved_at
            && now - at < SAVE_INTERVAL
        {
            return Some(at + SAVE_INTERVAL - now);
        }
        self.write_settings(settings);
        self.saved_at = Some(now);
        None
    }

    /// Writes `settings` if they changed, however recently they were written,
    /// as when drwr quits.
    pub fn flush(&mut self, settings: &Settings) {
        if *settings != self.saved {
            self.write_settings(settings);
        }
    }

    /// Whether `settings` are as last written, or found.
    #[cfg(any(target_os = "macos", test))]
    pub fn has_written(&self, settings: &Settings) -> bool {
        *settings == self.saved
    }

    fn write_settings(&mut self, settings: &Settings) {
        if let Some(folder) = &self.folder {
            write(&folder.join(SETTINGS), settings);
        }
        // Even if that failed, rather than try again every frame.
        self.saved = settings.clone();
    }

    /// The files last opened or saved, most recent first.
    pub fn recent_files(&self) -> &[PathBuf] {
        &self.recent_files
    }

    /// Puts the file at `path` first in the recent files.
    pub fn add_recent_file(&mut self, path: &Path) {
        // So that a file reached by two paths is listed once.
        let path = dunce::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        // TOML strings can't hold the others.
        if path.to_str().is_none() {
            return;
        }
        self.change_recent_files(|files| {
            files.retain(|file| *file != path);
            files.insert(0, path);
        });
    }

    pub fn remove_recent_file(&mut self, path: &Path) {
        self.change_recent_files(|files| files.retain(|file| file != path));
    }

    /// Changes the recent files with `change`, starting from the folder's,
    /// which another drwr may have changed, and writes them there.
    fn change_recent_files(&mut self, change: impl FnOnce(&mut Vec<PathBuf>)) {
        let file = self.folder.as_ref().map(|folder| folder.join(RECENT_FILES));
        if let Some(file) = &file
            && let Some(read) = read::<RecentFiles>(file)
        {
            self.recent_files = read.files;
        }
        change(&mut self.recent_files);
        self.recent_files.truncate(MAX_RECENT_FILES);
        if let Some(file) = &file {
            let files = self.recent_files.clone();
            write(file, &RecentFiles { files });
        }
    }
}

/// Makes `folder` and its missing parents, private to the user as the XDG
/// spec asks.
fn create_folder(folder: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(folder)
}

/// Reads the TOML file at `path`, as `T::default()` if it's missing. Returns
/// `None` if it can't, saying why on stderr.
fn read<T: DeserializeOwned + Default>(path: &Path) -> Option<T> {
    let read = match fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|err| err.to_string()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(T::default()),
        Err(err) => Err(err.to_string()),
    };
    let warn = |err| eprintln!("warning: couldn't read {}: {err}", path.display());
    read.map_err(warn).ok()
}

/// Writes `value` to the file at `path` as TOML, or says why it couldn't on
/// stderr.
fn write(path: &Path, value: &impl Serialize) {
    let text = toml::to_string_pretty(value).map_err(io::Error::other);
    let written =
        text.and_then(|text| atomic_file::write(path, |file| file.write_all(text.as_bytes())));
    if let Err(err) = written {
        eprintln!("warning: couldn't write {}: {err}", path.display());
    }
}

#[cfg(test)]
mod tests;
