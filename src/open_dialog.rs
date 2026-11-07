use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

/// The File > Open dialog. It runs on its own thread, because a Wayland
/// compositor may disconnect an app whose UI thread stops reading events.
#[derive(Default)]
pub struct OpenDialog {
    picked: Option<Receiver<Option<PathBuf>>>,
}

impl OpenDialog {
    pub fn is_open(&self) -> bool {
        self.picked.is_some()
    }

    /// Opens the dialog over `parent`, showing only GraphML files, unless it's
    /// open already.
    pub fn open(&mut self, ctx: &egui::Context, parent: &eframe::Frame) {
        if self.is_open() {
            return;
        }
        let dialog = rfd::FileDialog::new()
            .set_title("Open")
            .add_filter("GraphML", &["graphml"])
            .set_parent(parent);
        let (sender, receiver) = mpsc::channel();
        let ctx = ctx.clone();
        thread::spawn(move || {
            // Sending fails only once the app has quit.
            let _ = sender.send(dialog.pick_file());
            ctx.request_repaint();
        });
        self.picked = Some(receiver);
    }

    /// Takes the picked file, once the dialog has closed.
    pub fn picked(&mut self) -> Option<PathBuf> {
        let result = self.picked.as_ref()?.try_recv();
        if !matches!(result, Err(TryRecvError::Empty)) {
            self.picked = None;
        }
        result.ok().flatten()
    }
}
