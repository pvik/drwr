use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

use crate::export::Format;

/// A native dialog asking for a GraphML or draw.io file to open, an SVG file
/// to insert, or where to save or export to, for a target of type `T`, which
/// tells the caller what the file is for. It runs on its own thread, because
/// a Wayland compositor may disconnect an app whose UI thread stops reading
/// events.
pub struct FileDialog<T> {
    asked: Option<Asked<T>>,
}

/// What a file dialog asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    Open,
    InsertSvg,
    /// Where to save, suggesting the file name `name` in `folder`.
    Save {
        name: String,
        folder: Option<PathBuf>,
    },
    /// Where to export to as `format`, suggesting the file name `name` in
    /// `folder`.
    Export {
        format: Format,
        name: String,
        folder: Option<PathBuf>,
    },
}

struct Asked<T> {
    target: T,
    picked: Receiver<Option<PathBuf>>,
    #[cfg(test)]
    request: Request,
    /// Tests answer in place of rfd, which would show the user's real dialog.
    #[cfg(test)]
    answer: Sender<Option<PathBuf>>,
}

impl<T> Default for FileDialog<T> {
    fn default() -> Self {
        FileDialog { asked: None }
    }
}

impl<T> FileDialog<T> {
    pub fn is_open(&self) -> bool {
        self.asked.is_some()
    }

    /// Opens the dialog over `parent`, showing only the files of the format
    /// asked for, unless it's open already.
    pub fn open(
        &mut self,
        target: T,
        request: Request,
        ctx: &egui::Context,
        parent: &eframe::Frame,
    ) {
        if self.is_open() {
            return;
        }
        let (sender, picked) = mpsc::channel();
        #[cfg(test)]
        let answer = sender.clone();
        if !cfg!(test) {
            show(&request, sender, ctx, parent);
        }
        self.asked = Some(Asked {
            target,
            picked,
            #[cfg(test)]
            request,
            #[cfg(test)]
            answer,
        });
    }

    /// Takes the target and the file picked, once the dialog has closed.
    pub fn picked(&mut self) -> Option<(T, PathBuf)> {
        let result = self.asked.as_ref()?.picked.try_recv();
        if matches!(result, Err(TryRecvError::Empty)) {
            return None;
        }
        let asked = self.asked.take()?;
        Some((asked.target, result.ok().flatten()?))
    }
}

#[cfg(test)]
impl<T> FileDialog<T> {
    /// What the open dialog asks for.
    pub fn request(&self) -> Option<&Request> {
        self.asked.as_ref().map(|asked| &asked.request)
    }

    /// Picks `path` in the open dialog, or cancels it if `None`.
    pub fn answer(&self, path: Option<PathBuf>) {
        let asked = self.asked.as_ref().expect("the dialog is open");
        asked
            .answer
            .send(path)
            .expect("the dialog waits for a file");
    }
}

/// Shows rfd's dialog for `request` on its own thread, which sends the file
/// picked.
fn show(
    request: &Request,
    sender: Sender<Option<PathBuf>>,
    ctx: &egui::Context,
    parent: &eframe::Frame,
) {
    let (title, saved) = match request {
        Request::Open => ("Open", None),
        Request::InsertSvg => ("Insert SVG", None),
        Request::Save { name, folder } => ("Save As", Some((name, folder))),
        Request::Export { name, folder, .. } => ("Export", Some((name, folder))),
    };
    let mut dialog = rfd::FileDialog::new().set_title(title).set_parent(parent);
    for (name, extensions) in filters(request) {
        dialog = dialog.add_filter(name, &extensions);
    }
    if let Some((name, folder)) = saved {
        dialog = dialog.set_file_name(name);
        if let Some(folder) = folder {
            dialog = dialog.set_directory(folder);
        }
    }
    let save = saved.is_some();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let picked = if save {
            dialog.save_file()
        } else {
            dialog.pick_file()
        };
        // Sending fails only once the app has quit.
        let _ = sender.send(picked);
        ctx.request_repaint();
    });
}

/// The kinds of file the dialog for `request` offers, each a name and its
/// extensions, the first chosen to begin with.
fn filters(request: &Request) -> Vec<(&'static str, Vec<&'static str>)> {
    match request {
        Request::Open => vec![
            ("Diagrams", vec!["graphml", "drawio", "dio", "xml"]),
            ("GraphML", vec!["graphml"]),
            ("draw.io", vec!["drawio", "dio", "xml"]),
        ],
        Request::InsertSvg => vec![("SVG", vec!["svg"])],
        Request::Save { .. } => vec![("GraphML", vec!["graphml"])],
        Request::Export { format, .. } => vec![(format.name(), vec![format.extension()])],
    }
}

#[cfg(test)]
mod tests;
