// Windows release builds open no console window beside the app's.
#![cfg_attr(
    all(windows, not(debug_assertions), not(test)),
    windows_subsystem = "windows"
)]

mod about_dialog;
mod action;
mod app_icon;
mod atomic_file;
mod canvas;
mod cli;
mod color_picker;
mod config;
mod dialog;
mod document;
mod error_dialog;
mod export;
mod export_dialog;
mod file_dialog;
mod font_picker;
mod fonts;
mod history;
mod label_position;
mod line_type;
#[cfg(target_os = "macos")]
mod macos;
mod menu;
mod prompt;
mod scene;
mod selection;
mod shortcuts;
mod side_panel;
mod snap;
mod stacking;
mod status_bar;
mod style;
mod svg_image;
#[cfg(test)]
mod testing;
mod text_dialog;
mod text_field;
mod tool;
mod toolbar;
mod unsaved_dialog;
mod zoom;

use std::borrow::Cow;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;

use action::{Action, Enabled, FontPart};
use canvas::{Canvas, Drawing};
use color_picker::ColorPicker;
use config::Config;
use document::{ColorProperty, Graphics, Item, LabelRef};
use export_dialog::{Accepted, ExportDialog};
use file_dialog::{FileDialog, Request};
use fonts::Fonts;
use graphml::{GraphMl, yed};
use history::{Document, Merge};
use scene::Scene;
use selection::Selection;
use side_panel::SidePanel;
use svg_image::SvgImage;
use text_dialog::TextDialog;
use tool::Tool;
use unsaved_dialog::Answer;

/// The app's name, which is also the Wayland app id.
const NAME: &str = "drwr";
/// How wide or high an inserted SVG is at most, in points, as some are
/// drawn thousands of points across.
const MAX_SVG_SIZE: f32 = 100.0;

fn main() -> eframe::Result {
    // Without a console of their own, Windows release builds print to the one
    // they were started from, if any.
    #[cfg(all(windows, not(debug_assertions)))]
    unsafe {
        use windows_sys::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};
        AttachConsole(ATTACH_PARENT_PROCESS);
    }
    match cli::parse(std::env::args_os().skip(1)) {
        Ok(cli::Command::Run { file }) => {
            let mut app = DrwrApp::new(Fonts::system(), Config::load());
            if let Some(path) = file
                && let Err(err) = app.open(&path)
            {
                eprintln!("error: couldn't open {path:?}: {err}");
                std::process::exit(1);
            }
            run_gui(app)
        }
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

fn run_gui(app: DrwrApp) -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_icon(app_icon::window_icon()),
        ..Default::default()
    };
    eframe::run_native(
        NAME,
        options,
        Box::new(|cc| {
            #[cfg(target_os = "macos")]
            macos::ask_before_quitting();
            app.init(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
}

#[derive(Default)]
struct DrwrApp {
    tool: Tool,
    /// The graph, with its scene and undo history.
    doc: Document,
    /// Where the graph is saved, as an absolute path; `None` if it isn't.
    file: Option<PathBuf>,
    /// The graph as last opened, saved or made new.
    saved: GraphMl,
    /// Whether the graph differs from `saved`, as of version `checked` of it.
    modified: bool,
    checked: u64,
    selected: Selection,
    /// What was selected before the edits not yet recorded as an undo step.
    selected_before: Selection,
    /// What this frame's edits may merge with in the undo history, if there
    /// were any: `None` if they differ.
    frame_merge: Option<Option<Merge>>,
    /// What edits of the actions being performed may merge with.
    merge_with: Option<Merge>,
    /// What the user set that outlasts files and restarts.
    settings: config::Settings,
    /// Keeps `settings` and the recent files across restarts.
    config: Config,
    canvas: Canvas,
    /// The fonts labels are drawn in, which egui is given too.
    fonts: Arc<Fonts>,
    side_panel: SidePanel,
    file_dialog: FileDialog<FilePurpose>,
    /// Picks the colors of the selected nodes and edges.
    color_picker: ColorPicker<ColorProperty>,
    label_dialog: TextDialog<LabelEdit>,
    export_dialog: ExportDialog,
    /// Whether the About dialog is open.
    about: bool,
    /// The exports writing their files in the background.
    exports: Vec<Writing>,
    /// The question waiting for an answer.
    prompt: Option<Prompt>,
    /// Whether the window was asked to close, which the end of the frame
    /// decides on.
    close_requested: bool,
    /// Whether the window may close although the graph is modified.
    quitting: bool,
    /// The window title last set.
    title: String,
    error: Option<String>,
    /// Whether a text field had the focus at the end of the last frame; see
    /// [`shortcuts::pressed`].
    typing: bool,
}

/// Labels the label dialog edits together, and the text it opened on.
struct LabelEdit {
    labels: Vec<LabelRef>,
    text: String,
}

/// What the file dialog picks a file for.
enum FilePurpose {
    Open,
    InsertSvg,
    /// Saving the graph, then closing it for this, if anything.
    Save(Option<CloseFor>),
    Export(Arc<ExportJob>),
}

/// A drawing to export, as it was when the export dialog was accepted.
struct ExportJob {
    drawing: Drawing,
    settings: export::Settings,
}

/// An export writing the file at `path` on its own thread, which returns why
/// it couldn't.
struct Writing {
    path: PathBuf,
    thread: JoinHandle<Result<(), String>>,
}

/// What closes the graph, once its changes are saved or dropped.
#[derive(Clone, Debug)]
enum CloseFor {
    New,
    Open,
    /// Opening the file at this path.
    OpenFile(PathBuf),
    Quit,
}

/// A question for the user, and what it's for.
#[derive(Clone)]
enum Prompt {
    /// Whether to save the changes before closing the graph for this.
    Unsaved(CloseFor),
    /// Whether to save over the file at this path, then close the graph for
    /// this, if anything.
    Replace(PathBuf, Option<CloseFor>),
    /// Whether to export over the file at this path.
    ReplaceExport(PathBuf, Arc<ExportJob>),
}

impl DrwrApp {
    /// The app with a new drawing open, its labels drawn in `fonts`, and the
    /// settings `config` keeps.
    fn new(fonts: Fonts, config: Config) -> Self {
        let mut app = DrwrApp {
            fonts: Arc::new(fonts),
            settings: config.settings().clone(),
            config,
            ..DrwrApp::default()
        };
        app.replace_graph(yed::new_document(), None);
        app
    }

    /// Sets up `ctx` to show the app in.
    fn init(&self, ctx: &egui::Context) {
        // Needed to render the toolbar's SVG icons.
        egui_extras::install_image_loaders(ctx);
        ctx.add_font_provider(self.fonts.clone());
        // Ctrl+Plus, Ctrl+Minus and Ctrl+0 zoom the canvas instead.
        ctx.options_mut(|options| options.zoom_with_keyboard = false);
    }

    /// Replaces the graph with the GraphML file at `path`, or keeps it on error.
    fn open(&mut self, path: &Path) -> Result<(), graphml::Error> {
        let path = std::path::absolute(path)?;
        let graphml = GraphMl::read_file(&path)?;
        self.config.add_recent_file(&path);
        self.replace_graph(graphml, Some(path));
        Ok(())
    }

    /// Opens the file at `path`, or shows why it can't.
    fn open_or_show_error(&mut self, path: &Path) {
        if let Err(err) = self.open(path) {
            self.error = Some(format!("Couldn't open {}:\n{err}", path.display()));
        }
    }

    /// Opens the recent file at `path`, first asking whether to save the
    /// graph's changes, unless it's gone, which drops it from the list.
    fn open_recent(&mut self, path: PathBuf, ctx: &egui::Context, frame: &eframe::Frame) {
        if let Ok(false) = std::fs::exists(&path) {
            let gone = "It no longer exists, so it's been removed from Recent Files.";
            self.error = Some(format!("Couldn't open {}:\n{gone}", path.display()));
            self.config.remove_recent_file(&path);
        } else {
            self.close_graph(CloseFor::OpenFile(path), ctx, frame);
        }
    }

    /// Adds the SVG file at `path` as a node in the middle of the view, at
    /// the size yEd gives it but no bigger than [`MAX_SVG_SIZE`], and selects
    /// it.
    fn insert_svg(&mut self, path: &Path) -> Result<(), String> {
        let svg = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
        let size = SvgImage::parse(&svg)?.bounds().size();
        let size = size * (MAX_SVG_SIZE / size.max_elem()).min(1.0);
        let center = self.canvas.view_center() - size / 2.0;
        // To a thousandth of a point, so the file gets 39.2, not 39.199997.
        let points = |value: f32| (f64::from(value) * 1000.0).round() / 1000.0;
        let geometry = yed::Rect {
            x: center.x.round().into(),
            y: center.y.round().into(),
            width: points(size.x),
            height: points(size.y),
        };
        let id = document::add_svg_node(self.doc.graphml_mut(), &svg, geometry);
        self.selected = Selection::node(id);
        self.edited(None);
        Ok(())
    }

    /// Replaces the graph with `graphml`, saved at `file`, dropping what was
    /// selected, edited or viewed in the old one.
    fn replace_graph(&mut self, graphml: GraphMl, file: Option<PathBuf>) {
        self.saved = graphml.clone();
        self.doc = Document::new(graphml);
        self.file = file;
        self.modified = false;
        self.checked = self.doc.version();
        self.selected = Selection::default();
        self.selected_before = Selection::default();
        self.frame_merge = None;
        // Labels being edited belong to the old graph.
        self.canvas = Canvas::default();
        self.side_panel = SidePanel::default();
        self.color_picker.close();
        self.label_dialog.close();
        self.export_dialog.close();
    }

    /// Updates whether the graph is modified, returning whether it is.
    fn check_modified(&mut self) -> bool {
        self.doc.sync();
        self.modified = *self.doc.graphml() != self.saved;
        self.checked = self.doc.version();
        self.modified
    }

    /// Whether quitting right away would lose nothing: no changes are unsaved
    /// or being typed, no export is being written, and no settings wait to be.
    #[cfg(any(target_os = "macos", test))]
    fn can_quit_at_once(&self) -> bool {
        !self.modified
            && !self.typing
            && self.exports.is_empty()
            && self.config.has_written(&self.settings)
    }

    /// Updates whether the graph is modified, if it changed since that was
    /// last checked.
    fn update_modified(&mut self) {
        if self.doc.version() != self.checked {
            self.check_modified();
        }
    }

    /// Records the edits since the last undo step as a step.
    fn record(&mut self) {
        let merge = self.frame_merge.take().flatten();
        let (before, after) = (&self.selected_before, &self.selected);
        self.doc.record(before, after, merge);
        self.selected_before = self.selected.clone();
    }

    /// Notes an edit, which may merge with the last undo step if it has
    /// `merge`, as do this frame's other edits.
    fn edited(&mut self, merge: Option<Merge>) {
        self.frame_merge = Some(match self.frame_merge {
            Some(other) if other != merge => None,
            _ => merge,
        });
    }

    /// Undoes or redoes a step with `step`, selecting what was selected then.
    fn step(&mut self, step: fn(&mut Document) -> Option<Selection>) {
        // Edits not recorded yet make the last step.
        self.record();
        if let Some(selected) = step(&mut self.doc) {
            self.selected = selected;
            self.selected_before = self.selected.clone();
            // They're of the graph as it was.
            self.canvas.cancel_edits();
            self.side_panel.cancel_edit();
        }
    }

    /// The name of the graph's file, if it has one.
    fn file_name(&self) -> Option<Cow<'_, str>> {
        Some(self.file.as_deref()?.file_name()?.to_string_lossy())
    }

    /// Titles the window with the file's name, marked if the graph is
    /// modified, when that changes.
    fn update_title(&mut self, ctx: &egui::Context) {
        let name = self.file_name().unwrap_or(status_bar::NOT_SAVED.into());
        let marker = if self.modified { "*" } else { "" };
        let title = format!("{name}{marker} — {NAME}");
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }

    /// Closes the graph for `close_for`, asking first whether to save its
    /// changes, if it has any.
    fn close_graph(&mut self, close_for: CloseFor, ctx: &egui::Context, frame: &eframe::Frame) {
        if !self.check_modified() {
            self.drop_graph(close_for, ctx, frame);
        } else if !self.file_dialog.is_open() {
            // Not asked while a file dialog is open, which saving may need.
            self.prompt = Some(Prompt::Unsaved(close_for));
        }
    }

    /// Closes the graph for `close_for`, dropping any changes: makes a new
    /// one, asks for one to open, or quits.
    fn drop_graph(&mut self, close_for: CloseFor, ctx: &egui::Context, frame: &eframe::Frame) {
        match close_for {
            CloseFor::New => self.replace_graph(yed::new_document(), None),
            CloseFor::Open => {
                (self.file_dialog).open(FilePurpose::Open, Request::Open, ctx, frame);
            }
            CloseFor::OpenFile(path) => self.open_or_show_error(&path),
            CloseFor::Quit => {
                self.quitting = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Saves the graph to its file, or asks where if it has none, then closes
    /// it for `then`, if anything, once saved.
    fn save(&mut self, then: Option<CloseFor>, ctx: &egui::Context, frame: &eframe::Frame) {
        match self.file.clone() {
            Some(file) => self.save_to(file, then, ctx, frame),
            None => self.ask_where_to_save(then, ctx, frame),
        }
    }

    /// Asks where to save the graph, suggesting its file, then saves it there
    /// and closes it for `then`, if anything.
    fn ask_where_to_save(
        &mut self,
        then: Option<CloseFor>,
        ctx: &egui::Context,
        frame: &eframe::Frame,
    ) {
        let name = self.file_name().unwrap_or("untitled.graphml".into());
        let folder = self.file.as_deref().and_then(Path::parent);
        let request = Request::Save {
            name: name.into_owned(),
            folder: folder.map(Path::to_path_buf),
        };
        (self.file_dialog).open(FilePurpose::Save(then), request, ctx, frame);
    }

    /// Saves the graph where the file dialog picked, then closes it for
    /// `then`, if anything. A name without an extension gets ".graphml",
    /// asking first if that file exists, as the dialog only checked the name.
    fn save_picked(
        &mut self,
        mut path: PathBuf,
        then: Option<CloseFor>,
        ctx: &egui::Context,
        frame: &eframe::Frame,
    ) {
        if path.extension().is_none() {
            path.set_extension("graphml");
            if path.exists() {
                self.prompt = Some(Prompt::Replace(path, then));
                return;
            }
        }
        self.save_to(path, then, ctx, frame);
    }

    /// Writes the graph to `path`, which becomes its file, then closes it for
    /// `then`, if anything; or shows why it can't.
    fn save_to(
        &mut self,
        path: PathBuf,
        then: Option<CloseFor>,
        ctx: &egui::Context,
        frame: &eframe::Frame,
    ) {
        if let Err(err) = atomic_file::write(&path, |file| self.doc.graphml().write_to(file)) {
            self.error = Some(format!("Couldn't save {}:\n{err}", path.display()));
            return;
        }
        self.config.add_recent_file(&path);
        self.file = Some(path);
        self.saved = self.doc.graphml().clone();
        self.modified = false;
        self.checked = self.doc.version();
        if let Some(then) = then {
            self.drop_graph(then, ctx, frame);
        }
    }

    /// Opens the export dialog on what the canvas draws, and on what of that
    /// is selected.
    fn open_export_dialog(&mut self, ctx: &egui::Context) {
        let scene = self.doc.scene();
        let everything = Drawing::of(ctx, scene, None).bounds;
        let selected = (!self.selected.is_empty())
            .then(|| Drawing::of(ctx, scene, Some(&self.selected)).bounds);
        (self.export_dialog).open(self.settings.export.clone(), everything, selected);
    }

    /// Asks where to export the drawing as the export dialog was `accepted`,
    /// suggesting the graph's name and folder, then exports it there.
    fn ask_where_to_export(
        &mut self,
        accepted: Accepted,
        ctx: &egui::Context,
        frame: &eframe::Frame,
    ) {
        let Accepted {
            settings,
            selected_only,
        } = accepted;
        self.settings.export = settings.clone();
        let only = selected_only.then_some(&self.selected);
        let drawing = Drawing::of(ctx, self.doc.scene(), only);
        let format = settings.format;
        let file = self.file.as_deref();
        let stem = file.and_then(Path::file_stem);
        let stem = stem.map_or("untitled".into(), |stem| stem.to_string_lossy());
        let request = Request::Export {
            format,
            name: format!("{stem}.{}", format.extension()),
            folder: file.and_then(Path::parent).map(Path::to_path_buf),
        };
        let job = Arc::new(ExportJob { drawing, settings });
        (self.file_dialog).open(FilePurpose::Export(job), request, ctx, frame);
    }

    /// Exports `job` where the file dialog picked. A name without an
    /// extension gets the format's, asking first if that file exists, as the
    /// dialog only checked the name.
    fn export_picked(&mut self, mut path: PathBuf, job: Arc<ExportJob>, ctx: &egui::Context) {
        if path.extension().is_none() {
            path.set_extension(job.settings.format.extension());
            if path.exists() {
                self.prompt = Some(Prompt::ReplaceExport(path, job));
                return;
            }
        }
        self.export_to(path, job, ctx);
    }

    /// Writes the file of `job` to `path` in the background, as a big PNG
    /// takes a while, then repaints `ctx`.
    fn export_to(&mut self, path: PathBuf, job: Arc<ExportJob>, ctx: &egui::Context) {
        let (fonts, ctx, to) = (self.fonts.clone(), ctx.clone(), path.clone());
        let thread = std::thread::Builder::new()
            .name("export".to_owned())
            .spawn(move || {
                let written = export::export(&job.drawing, &job.settings, &fonts)
                    .map_err(|err| err.to_string())
                    .and_then(|bytes| {
                        atomic_file::write(&to, |file| file.write_all(&bytes))
                            .map_err(|err| err.to_string())
                    });
                ctx.request_repaint();
                written
            });
        match thread {
            Ok(thread) => self.exports.push(Writing { path, thread }),
            Err(err) => self.error = Some(export_error(&path, &err.to_string())),
        }
    }

    /// Shows why the exports that have finished, or all of them once done if
    /// `wait`, couldn't write their files.
    fn finish_exports(&mut self, wait: bool) {
        let (done, writing) = std::mem::take(&mut self.exports)
            .into_iter()
            .partition(|export| wait || export.thread.is_finished());
        self.exports = writing;
        for Writing { path, thread } in done {
            let stopped = || Err("The export stopped unexpectedly.".to_owned());
            let written = thread.join().unwrap_or_else(|_| stopped());
            if let Err(err) = written {
                self.error = Some(export_error(&path, &err));
            }
        }
    }

    /// Asks the question waiting for an answer, if any, then does what the
    /// answer says.
    fn show_prompt(&mut self, ctx: &egui::Context, frame: &eframe::Frame) {
        match self.prompt.clone() {
            Some(Prompt::Unsaved(close_for)) => {
                let Some(answer) = unsaved_dialog::show(ctx, self.file.as_deref()) else {
                    return;
                };
                self.prompt = None;
                match answer {
                    Answer::Save => self.save(Some(close_for), ctx, frame),
                    Answer::DontSave => self.drop_graph(close_for, ctx, frame),
                    Answer::Cancel => {}
                }
            }
            Some(Prompt::Replace(path, then)) => {
                let Some(replace) = ask_to_replace(ctx, &path) else {
                    return;
                };
                self.prompt = None;
                if replace {
                    self.save_to(path, then, ctx, frame);
                }
            }
            Some(Prompt::ReplaceExport(path, job)) => {
                let Some(replace) = ask_to_replace(ctx, &path) else {
                    return;
                };
                self.prompt = None;
                if replace {
                    self.export_to(path, job, ctx);
                }
            }
            None => {}
        }
    }

    /// Edits the graphics of each selected node and edge.
    fn edit_selected(&mut self, edit: impl Fn(&mut Graphics)) {
        for item in self.selected.items() {
            document::edit(self.doc.graphml_mut(), item, &edit);
        }
    }

    /// Puts the selected nodes, with the selected edges between them, on the
    /// clipboard. Returns whether any nodes were selected.
    fn copy(&self, ctx: &egui::Context) -> bool {
        let Some(copied) = document::copy(self.doc.graphml(), self.selected.items()) else {
            return false;
        };
        ctx.copy_text(copied.to_xml());
        true
    }

    /// Puts the top-level nodes in the order `restacked` gives, if it moves
    /// any of the selected.
    fn restack(&mut self, restacked: fn(&GraphMl, &Scene, &Selection) -> Option<Vec<usize>>) {
        let (graphml, scene) = self.doc.read();
        if let Some(order) = restacked(graphml, scene, &self.selected) {
            document::reorder_nodes(self.doc.graphml_mut(), &order);
        }
    }

    /// Removes everything selected, nodes with their edges.
    fn remove_selected(&mut self) {
        let selected = std::mem::take(&mut self.selected);
        document::remove_items(self.doc.graphml_mut(), selected.items());
        // The Delete button's click would otherwise apply a label edited on
        // the canvas to the edge moved into its place.
        self.canvas.cancel_label_edit();
    }

    /// Performs `action`, from a shortcut, the menu or the toolbar, unless
    /// it's Undo, Redo, Cut, Copy or in the File menu. Those go on `waiting`,
    /// taking the focus from any text field, so that the side panel and the
    /// canvas apply what was typed there first.
    fn perform_or_wait(
        &mut self,
        action: Option<Action>,
        waiting: &mut Vec<Action>,
        ctx: &egui::Context,
        frame: &eframe::Frame,
    ) {
        match action {
            Some(
                action @ (Action::Undo
                | Action::Redo
                | Action::Cut
                | Action::Copy
                | Action::New
                | Action::Open
                | Action::OpenRecent(_)
                | Action::Save
                | Action::SaveAs
                | Action::Export
                | Action::About
                | Action::Quit),
            ) => {
                ctx.memory_mut(|memory| memory.stop_text_input());
                waiting.push(action);
            }
            Some(action) => self.perform(action, ctx, frame),
            None => {}
        }
    }

    /// Does what the user asked for, wherever they asked.
    fn perform(&mut self, action: Action, ctx: &egui::Context, frame: &eframe::Frame) {
        let edits = self.doc.edits();
        let merge = match action {
            Action::MoveBy(..) => Some(Merge::Nudge),
            _ => self.merge_with,
        };
        match action {
            // Disabled, as the open file dialog is for the graph as it is.
            Action::New
            | Action::Open
            | Action::OpenRecent(_)
            | Action::Save
            | Action::SaveAs
            | Action::Export
            | Action::InsertSvg
                if self.file_dialog.is_open() => {}
            Action::New => self.close_graph(CloseFor::New, ctx, frame),
            Action::Open => self.close_graph(CloseFor::Open, ctx, frame),
            Action::OpenRecent(path) => self.open_recent(path, ctx, frame),
            Action::Save => self.save(None, ctx, frame),
            Action::SaveAs => self.ask_where_to_save(None, ctx, frame),
            Action::Export => self.open_export_dialog(ctx),
            Action::InsertSvg => {
                let purpose = FilePurpose::InsertSvg;
                self.file_dialog
                    .open(purpose, Request::InsertSvg, ctx, frame);
            }
            Action::About => self.about = true,
            Action::Quit => self.close_graph(CloseFor::Quit, ctx, frame),
            Action::Undo => self.step(Document::undo),
            Action::Redo => self.step(Document::redo),
            Action::Cut => {
                if self.copy(ctx) {
                    self.remove_selected();
                }
            }
            Action::Copy => {
                self.copy(ctx);
            }
            Action::Paste => {
                ctx.send_viewport_cmd(egui::ViewportCommand::RequestPaste);
                // For the frame the paste event comes in.
                ctx.request_repaint();
            }
            Action::PasteText(text) => {
                if let Ok(copied) = GraphMl::parse(&text) {
                    let pasted = document::paste(self.doc.graphml_mut(), copied);
                    if !pasted.is_empty() {
                        self.selected = Selection::of(pasted);
                    }
                }
            }
            Action::SelectTool(tool) => self.tool = tool,
            Action::ZoomIn => {
                if let Some(zoom) = zoom::step_in(self.canvas.zoom()) {
                    self.canvas.set_zoom(zoom);
                }
            }
            Action::ZoomOut => {
                if let Some(zoom) = zoom::step_out(self.canvas.zoom()) {
                    self.canvas.set_zoom(zoom);
                }
            }
            Action::SetZoom(zoom) => self.canvas.set_zoom(zoom),
            Action::PanBy(dx, dy) => self.canvas.pan_by(egui::vec2(dx, dy)),
            Action::ToggleGridLines => {
                self.settings.view.grid_lines = !self.settings.view.grid_lines;
            }
            Action::ToggleSnapLines => {
                self.settings.view.snap_lines = !self.settings.view.snap_lines;
            }
            Action::MoveBy(dx, dy) => match (self.selected.single(), self.selected.bend()) {
                (Some(&Item::Edge(index)), Some(bend)) => {
                    document::move_bends(self.doc.graphml_mut(), index, Some(bend), dx, dy);
                }
                _ => document::move_items(self.doc.graphml_mut(), self.selected.items(), dx, dy),
            },
            Action::Delete => match (self.selected.single(), self.selected.bend()) {
                (Some(&Item::Edge(index)), Some(bend)) => {
                    document::remove_bend(self.doc.graphml_mut(), index, bend);
                    self.selected = Selection::edge(index);
                }
                _ if !self.selected.is_empty() => self.remove_selected(),
                _ => {}
            },
            Action::Raise => self.restack(stacking::raised),
            Action::Lower => self.restack(stacking::lowered),
            Action::EditColor(property) => {
                let mut graphics = (self.selected.items().iter())
                    .filter_map(|item| document::graphics(self.doc.graphml(), item));
                // The first's, as the side panel shows none when they differ.
                // A text color is a label's, which some items may lack.
                let first = match property {
                    ColorProperty::Text(index) => graphics.find(|g| g.labels().len() > index),
                    _ => graphics.next(),
                };
                if let Some(graphics) = first {
                    let title = format!("{} color", property.name());
                    let color = property.get(graphics.as_ref());
                    self.color_picker
                        .open(property, title, color, property.optional());
                }
            }
            Action::SetColor(property, color) => {
                self.edit_selected(|graphics| property.set(graphics, color));
                (self.settings.style).set_color(&self.selected, property, color);
            }
            Action::SetLineWidth(width) => {
                self.edit_selected(|graphics| graphics.stroke_mut().width = width);
                self.settings.style.set_line_width(&self.selected, width);
            }
            Action::SetLineType(line_type) => {
                self.edit_selected(|graphics| {
                    graphics.stroke_mut().line_type = line_type.clone();
                });
                (self.settings.style).set_line_type(&self.selected, &line_type);
            }
            Action::SetCornerRadius(radius) => {
                for id in self.selected.nodes() {
                    document::set_corner_radius(self.doc.graphml_mut(), id, radius);
                }
                (self.settings.style).set_corner_radius(&self.selected, radius);
            }
            Action::SetArrow(end, arrow) => {
                for index in self.selected.edges() {
                    document::set_arrow(self.doc.graphml_mut(), index, end, arrow);
                }
            }
            Action::EditLabel => {
                if let Some(item) = self.selected.single() {
                    let label = LabelRef {
                        item: item.clone(),
                        index: 0,
                    };
                    if let Some(text) = document::label_text(self.doc.graphml(), &label) {
                        self.canvas.edit_label(label, text);
                    }
                }
            }
            Action::EditLabelInDialog(labels) => {
                let texts = labels
                    .iter()
                    .map(|label| document::label_text(self.doc.graphml(), label));
                if let Some(texts) = texts.collect::<Option<Vec<_>>>()
                    && let Some(first) = texts.first()
                {
                    // Blank if they differ, as in their field.
                    let same = texts.iter().all(|text| text == first);
                    let text = if same { first.clone() } else { String::new() };
                    let edit = LabelEdit {
                        labels,
                        text: text.clone(),
                    };
                    self.label_dialog.open(edit, "Label", text);
                }
            }
            Action::SetLabel(label, text) => {
                document::set_label(self.doc.graphml_mut(), &label, &text, &self.settings.style);
            }
            Action::SetFont(label, part) => {
                self.settings.style.set_font(&part);
                document::edit_label(self.doc.graphml_mut(), &label, |label| {
                    let font = &mut label.font;
                    match part {
                        FontPart::Family(family) => font.family = family,
                        FontPart::Style(style) => font.style = style,
                        FontPart::Size(size) => font.size = size,
                    }
                });
            }
            Action::SetLabelPosition(label, position) => {
                document::set_label_position(self.doc.graphml_mut(), &label, position);
            }
        }
        if self.doc.edits() != edits {
            self.edited(merge);
        }
    }
}

/// Why exporting to `path` failed.
fn export_error(path: &Path, err: &str) -> String {
    format!("Couldn't export {}:\n{err}", path.display())
}

/// Asks whether to replace the file at `path`. Returns the answer once given.
fn ask_to_replace(ctx: &egui::Context, path: &Path) -> Option<bool> {
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let question = format!("{name} already exists. Replace it?");
    prompt::ask(ctx, &question, &[("Replace", true), ("Cancel", false)])
}

impl eframe::App for DrwrApp {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        match self.file_dialog.picked() {
            Some((FilePurpose::Open, path)) => self.open_or_show_error(&path),
            Some((FilePurpose::InsertSvg, path)) => {
                if let Err(err) = self.insert_svg(&path) {
                    self.error = Some(format!("Couldn't insert {}:\n{err}", path.display()));
                }
            }
            Some((FilePurpose::Save(then), path)) => self.save_picked(path, then, ctx, frame),
            Some((FilePurpose::Export(job), path)) => self.export_picked(path, job, ctx),
            None => {}
        }
        self.finish_exports(false);
        // Here, as `ui` doesn't run while the window is hidden, and it closes
        // after this frame unless cancelled.
        let close = ctx.input(|input| input.viewport().close_requested());
        if close && !self.quitting && (self.check_modified() || ctx.text_edit_focused()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            // Applies a label being typed, which then counts as a change.
            ctx.memory_mut(|memory| memory.stop_text_input());
            self.close_requested = true;
        } else if close {
            // Rather than leave a file half written.
            self.finish_exports(true);
            self.config.flush(&self.settings);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        // Unless edits wait to be recorded, as tests make between frames.
        if !self.doc.has_unrecorded() {
            self.selected_before = self.selected.clone();
        }
        let mut waiting = Vec::new();
        // Before the menu, so an Escape that closes it doesn't also select the
        // Cursor tool.
        for action in shortcuts::pressed(ui.ctx(), self.typing) {
            self.perform_or_wait(Some(action), &mut waiting, ui.ctx(), frame);
        }
        let (graphml, scene) = self.doc.read();
        let raise = stacking::raised(graphml, scene, &self.selected).is_some();
        let lower = stacking::lowered(graphml, scene, &self.selected).is_some();
        // Panels are laid out outside-in in the order they're added, so the
        // canvas (the central panel) must come last.
        let enabled = Enabled {
            file: !self.file_dialog.is_open(),
            copy: self.selected.has_nodes(),
            delete: !self.selected.is_empty(),
            raise,
            lower,
            undo: self.doc.can_undo(),
            redo: self.doc.can_redo(),
            zoom_in: zoom::step_in(self.canvas.zoom()).is_some(),
            zoom_out: zoom::step_out(self.canvas.zoom()).is_some(),
        };
        let recent_files = self.config.recent_files();
        let menu = menu::show(ui, enabled, self.settings.view, recent_files);
        self.perform_or_wait(menu, &mut waiting, ui.ctx(), frame);
        let (tool, zoom) = (self.tool, self.canvas.zoom());
        let toolbar = toolbar::show(ui, enabled, tool, zoom);
        self.perform_or_wait(toolbar, &mut waiting, ui.ctx(), frame);
        let exporting = !self.exports.is_empty();
        status_bar::show(
            ui,
            self.tool,
            exporting,
            self.file.as_deref(),
            self.modified,
        );
        let side =
            (self.side_panel).show(ui, enabled, self.doc.read(), &self.selected, &self.fonts);
        self.merge_with = self.side_panel.dragged();
        for action in side {
            self.perform(action, ui.ctx(), frame);
        }
        self.merge_with = None;
        let edits = self.doc.edits();
        let actions = self.canvas.show(
            ui,
            self.tool,
            &mut self.doc,
            &mut self.selected,
            &self.settings.style,
            self.settings.view,
        );
        if self.doc.edits() != edits {
            self.edited(None);
        }
        for action in actions.into_iter().chain(waiting) {
            self.perform(action, ui.ctx(), frame);
        }

        if let Some((property, color)) = self.color_picker.show(ui.ctx()) {
            self.perform(Action::SetColor(property, color), ui.ctx(), frame);
        }
        if let Some((edit, text)) = self.label_dialog.show(ui.ctx())
            // Unchanged, it would blank labels that differ.
            && text != edit.text
        {
            for label in edit.labels {
                self.perform(Action::SetLabel(label, text.clone()), ui.ctx(), frame);
            }
        }
        if let Some(accepted) = self.export_dialog.show(ui.ctx()) {
            self.ask_where_to_export(accepted, ui.ctx(), frame);
        }
        about_dialog::show(ui.ctx(), &mut self.about);
        self.record();
        self.update_modified();
        if std::mem::take(&mut self.close_requested) {
            self.close_graph(CloseFor::Quit, ui.ctx(), frame);
        }
        self.show_prompt(ui.ctx(), frame);
        error_dialog::show(ui.ctx(), &mut self.error);
        self.update_title(ui.ctx());
        let now = ui.ctx().input(|input| input.time);
        if let Some(wait) = self.config.save(&self.settings, now) {
            ui.ctx().request_repaint_after_secs(wait as f32);
        }
        self.typing = ui.ctx().text_edit_focused();
        #[cfg(target_os = "macos")]
        macos::set_can_quit_at_once(self.can_quit_at_once());
    }
}

#[cfg(test)]
mod tests;
