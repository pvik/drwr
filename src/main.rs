mod action;
mod atomic_file;
mod canvas;
mod cli;
mod color_picker;
mod dialog;
mod document;
mod error_dialog;
mod file_dialog;
mod font_picker;
mod fonts;
mod history;
mod label_position;
mod line_type;
mod menu;
mod prompt;
mod scene;
mod selection;
mod shortcuts;
mod side_panel;
mod stacking;
mod status_bar;
mod style;
#[cfg(test)]
mod testing;
mod text_dialog;
mod text_field;
mod tool;
mod toolbar;
mod unsaved_dialog;
mod zoom;

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use action::{Action, Enabled, FontPart};
use canvas::Canvas;
use color_picker::ColorPicker;
use document::{ColorProperty, Graphics, Item, LabelRef};
use file_dialog::{FileDialog, Request};
use fonts::Fonts;
use graphml::{GraphMl, yed};
use history::{Document, Merge};
use scene::Scene;
use selection::Selection;
use side_panel::SidePanel;
use style::Style;
use text_dialog::TextDialog;
use tool::Tool;
use unsaved_dialog::Answer;

/// The app's name, which is also the Wayland app id.
const NAME: &str = "drwr";

fn main() -> eframe::Result {
    match cli::parse(std::env::args_os().skip(1)) {
        Ok(cli::Command::Run { file }) => {
            let mut app = DrwrApp::new(Fonts::system());
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
    eframe::run_native(
        NAME,
        eframe::NativeOptions::default(),
        Box::new(|cc| {
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
    /// What the user last set on nodes, lines and labels, for new ones. Kept
    /// across files.
    style: Style,
    canvas: Canvas,
    /// The fonts labels are drawn in, which egui is given too.
    fonts: Arc<Fonts>,
    side_panel: SidePanel,
    file_dialog: FileDialog<FilePurpose>,
    /// Picks the colors of the selected nodes and edges.
    color_picker: ColorPicker<ColorProperty>,
    label_dialog: TextDialog<LabelEdit>,
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
    /// Saving the graph, then closing it for this, if anything.
    Save(Option<CloseFor>),
}

/// What closes the graph, once its changes are saved or dropped.
#[derive(Clone, Copy, Debug)]
enum CloseFor {
    New,
    Open,
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
}

impl DrwrApp {
    /// The app with a new drawing open, its labels drawn in `fonts`.
    fn new(fonts: Fonts) -> Self {
        let mut app = DrwrApp {
            fonts: Arc::new(fonts),
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
        self.replace_graph(graphml, Some(path));
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
    }

    /// Updates whether the graph is modified, returning whether it is.
    fn check_modified(&mut self) -> bool {
        self.doc.sync();
        self.modified = *self.doc.graphml() != self.saved;
        self.checked = self.doc.version();
        self.modified
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
        self.file = Some(path);
        self.saved = self.doc.graphml().clone();
        self.modified = false;
        self.checked = self.doc.version();
        if let Some(then) = then {
            self.drop_graph(then, ctx, frame);
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
                let name = path.file_name().unwrap_or_default().to_string_lossy();
                let question = format!("{name} already exists. Replace it?");
                let answers = [("Replace", true), ("Cancel", false)];
                let Some(replace) = prompt::ask(ctx, &question, &answers) else {
                    return;
                };
                self.prompt = None;
                if replace {
                    self.save_to(path, then, ctx, frame);
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
                | Action::Save
                | Action::SaveAs
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
            Action::New | Action::Open | Action::Save | Action::SaveAs
                if self.file_dialog.is_open() => {}
            Action::New => self.close_graph(CloseFor::New, ctx, frame),
            Action::Open => self.close_graph(CloseFor::Open, ctx, frame),
            Action::Save => self.save(None, ctx, frame),
            Action::SaveAs => self.ask_where_to_save(None, ctx, frame),
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
                self.style.set_color(&self.selected, property, color);
            }
            Action::SetLineWidth(width) => {
                self.edit_selected(|graphics| graphics.stroke_mut().width = width);
                self.style.set_line_width(&self.selected, width);
            }
            Action::SetLineType(line_type) => {
                self.edit_selected(|graphics| {
                    graphics.stroke_mut().line_type = line_type.clone();
                });
                self.style.set_line_type(&self.selected, &line_type);
            }
            Action::SetCornerRadius(radius) => {
                for id in self.selected.nodes() {
                    document::set_corner_radius(self.doc.graphml_mut(), id, radius);
                }
                self.style.set_corner_radius(&self.selected, radius);
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
                document::set_label(self.doc.graphml_mut(), &label, &text, &self.style);
            }
            Action::SetFont(label, part) => {
                self.style.set_font(&part);
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

impl eframe::App for DrwrApp {
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        match self.file_dialog.picked() {
            Some((FilePurpose::Open, path)) => {
                if let Err(err) = self.open(&path) {
                    self.error = Some(format!("Couldn't open {}:\n{err}", path.display()));
                }
            }
            Some((FilePurpose::Save(then), path)) => self.save_picked(path, then, ctx, frame),
            None => {}
        }
        // Here, as `ui` doesn't run while the window is hidden, and it closes
        // after this frame unless cancelled.
        let close = ctx.input(|input| input.viewport().close_requested());
        if close && !self.quitting && (self.check_modified() || ctx.text_edit_focused()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            // Applies a label being typed, which then counts as a change.
            ctx.memory_mut(|memory| memory.stop_text_input());
            self.close_requested = true;
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
        };
        let menu = menu::show(ui, enabled);
        self.perform_or_wait(menu, &mut waiting, ui.ctx(), frame);
        let (tool, zoom) = (self.tool, self.canvas.zoom());
        let toolbar = toolbar::show(ui, enabled, tool, zoom);
        self.perform_or_wait(toolbar, &mut waiting, ui.ctx(), frame);
        status_bar::show(ui, self.tool, self.file.as_deref(), self.modified);
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
            &self.style,
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
        self.record();
        self.update_modified();
        if std::mem::take(&mut self.close_requested) {
            self.close_graph(CloseFor::Quit, ui.ctx(), frame);
        }
        self.show_prompt(ui.ctx(), frame);
        error_dialog::show(ui.ctx(), &mut self.error);
        self.update_title(ui.ctx());
        self.typing = ui.ctx().text_edit_focused();
    }
}

#[cfg(test)]
mod tests;
