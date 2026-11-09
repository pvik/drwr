use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

/// A native dialog asking for a GraphML file to open, or where to save one,
/// for a target of type `T`, which tells the caller what the file is for. It
/// runs on its own thread, because a Wayland compositor may disconnect an app
/// whose UI thread stops reading events.
pub struct FileDialog<T> {
    asked: Option<Asked<T>>,
}

/// What a file dialog asks for.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    Open,
    /// Where to save, suggesting the file name `name` in `folder`.
    Save {
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

    /// Opens the dialog over `parent`, showing only GraphML files, unless it's
    /// open already.
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
        #[cfg(not(test))]
        show(&request, sender, ctx, parent);
        #[cfg(test)]
        let _ = (ctx, parent);
        self.asked = Some(Asked {
            target,
            picked,
            #[cfg(test)]
            request,
            #[cfg(test)]
            answer: sender,
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
#[cfg(not(test))]
fn show(
    request: &Request,
    sender: Sender<Option<PathBuf>>,
    ctx: &egui::Context,
    parent: &eframe::Frame,
) {
    let dialog = rfd::FileDialog::new()
        .add_filter("GraphML", &["graphml"])
        .set_parent(parent);
    let dialog = match request {
        Request::Open => dialog.set_title("Open"),
        Request::Save { name, folder } => {
            let dialog = dialog.set_title("Save As").set_file_name(name);
            match folder {
                Some(folder) => dialog.set_directory(folder),
                None => dialog,
            }
        }
    };
    let save = matches!(request, Request::Save { .. });
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

#[cfg(test)]
mod tests;
