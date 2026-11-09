mod action;
mod canvas;
mod cli;
mod color_picker;
mod dialog;
mod document;
mod error_dialog;
mod file_dialog;
mod font_picker;
mod fonts;
mod label_position;
mod line_type;
mod menu;
mod prompt;
mod selection;
mod shortcuts;
mod side_panel;
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

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use action::{Action, FontPart};
use canvas::Canvas;
use color_picker::ColorPicker;
use document::{ColorProperty, Graphics, Item, LabelRef};
use file_dialog::{FileDialog, Request};
use fonts::Fonts;
use graphml::{GraphMl, yed};
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
    graphml: GraphMl,
    /// Where `graphml` is saved, as an absolute path; `None` if it isn't.
    file: Option<PathBuf>,
    /// `graphml` as last opened, saved or made new.
    saved: GraphMl,
    /// Whether `graphml` differs from `saved`, as of the last frame that could
    /// have changed it.
    modified: bool,
    /// Whether an action was performed since `modified` was last updated.
    performed: bool,
    selected: Selection,
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
        self.graphml = graphml;
        self.file = file;
        self.modified = false;
        self.selected = Selection::default();
        // Labels being edited belong to the old graph.
        self.canvas = Canvas::default();
        self.side_panel = SidePanel::default();
        self.color_picker.close();
        self.label_dialog.close();
    }

    /// Updates whether the graph is modified, returning whether it is.
    fn check_modified(&mut self) -> bool {
        self.modified = self.graphml != self.saved;
        self.modified
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
        let file = self.file.as_deref();
        let name = (file.and_then(Path::file_name))
            .map_or("untitled.graphml".into(), OsStr::to_string_lossy);
        let request = Request::Save {
            name: name.into_owned(),
            folder: file.and_then(Path::parent).map(Path::to_path_buf),
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
        if let Err(err) = self.graphml.write_file(&path) {
            self.error = Some(format!("Couldn't save {}:\n{err}", path.display()));
            return;
        }
        self.file = Some(path);
        self.saved = self.graphml.clone();
        self.modified = false;
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
            document::edit(&mut self.graphml, item, &edit);
        }
    }

    /// Puts the selected nodes, with the selected edges between them, on the
    /// clipboard. Returns whether any nodes were selected.
    fn copy(&self, ctx: &egui::Context) -> bool {
        let Some(copied) = document::copy(&self.graphml, self.selected.items()) else {
            return false;
        };
        ctx.copy_text(copied.to_xml());
        true
    }

    /// Removes everything selected, nodes with their edges.
    fn remove_selected(&mut self) {
        let selected = std::mem::take(&mut self.selected);
        document::remove_items(&mut self.graphml, selected.items());
        // The Delete button's click would otherwise apply a label edited on
        // the canvas to the edge moved into its place.
        self.canvas.cancel_label_edit();
    }

    /// Performs `action`, from a shortcut, the menu or the toolbar, unless
    /// it's Cut, Copy or in the File menu. Those go on `waiting`, taking the
    /// focus from any text field, so that the side panel and the canvas apply
    /// what was typed there first.
    fn perform_or_wait(
        &mut self,
        action: Option<Action>,
        waiting: &mut Vec<Action>,
        ctx: &egui::Context,
        frame: &eframe::Frame,
    ) {
        match action {
            Some(
                action @ (Action::Cut
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
        self.performed = true;
        match action {
            // Disabled, as the open file dialog is for the graph as it is.
            Action::New | Action::Open | Action::Save | Action::SaveAs
                if self.file_dialog.is_open() => {}
            Action::New => self.close_graph(CloseFor::New, ctx, frame),
            Action::Open => self.close_graph(CloseFor::Open, ctx, frame),
            Action::Save => self.save(None, ctx, frame),
            Action::SaveAs => self.ask_where_to_save(None, ctx, frame),
            Action::Quit => self.close_graph(CloseFor::Quit, ctx, frame),
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
                    let pasted = document::paste(&mut self.graphml, copied);
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
                    document::move_bends(&mut self.graphml, index, Some(bend), dx, dy);
                }
                _ => document::move_items(&mut self.graphml, self.selected.items(), dx, dy),
            },
            Action::Delete => match (self.selected.single(), self.selected.bend()) {
                (Some(&Item::Edge(index)), Some(bend)) => {
                    document::remove_bend(&mut self.graphml, index, bend);
                    self.selected = Selection::edge(index);
                }
                _ if !self.selected.is_empty() => self.remove_selected(),
                _ => {}
            },
            Action::EditColor(property) => {
                let mut graphics = (self.selected.items().iter())
                    .filter_map(|item| document::graphics(&self.graphml, item));
                // The first's, as the side panel shows none when they differ.
                // A text color is a label's, which some items may lack.
                let first = match property {
                    ColorProperty::Text(index) => graphics.find(|g| g.labels().len() > index),
                    _ => graphics.next(),
                };
                if let Some(graphics) = first {
                    let title = format!("{} color", property.name());
                    let color = property.get(&graphics);
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
                    document::set_corner_radius(&mut self.graphml, id, radius);
                }
                self.style.set_corner_radius(&self.selected, radius);
            }
            Action::SetArrow(end, arrow) => {
                for index in self.selected.edges() {
                    document::set_arrow(&mut self.graphml, index, end, arrow);
                }
            }
            Action::EditLabel => {
                if let Some(item) = self.selected.single() {
                    let label = LabelRef {
                        item: item.clone(),
                        index: 0,
                    };
                    if let Some(text) = document::label_text(&self.graphml, &label) {
                        self.canvas.edit_label(label, text);
                    }
                }
            }
            Action::EditLabelInDialog(labels) => {
                let texts = labels
                    .iter()
                    .map(|label| document::label_text(&self.graphml, label));
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
                document::set_label(&mut self.graphml, &label, &text, &self.style);
            }
            Action::SetFont(label, part) => {
                self.style.set_font(&part);
                document::edit_label(&mut self.graphml, &label, |label| {
                    let font = &mut label.font;
                    match part {
                        FontPart::Family(family) => font.family = family,
                        FontPart::Style(style) => font.style = style,
                        FontPart::Size(size) => font.size = size,
                    }
                });
            }
            Action::SetLabelPosition(label, position) => {
                document::set_label_position(&mut self.graphml, &label, position);
            }
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
        let mut waiting = Vec::new();
        // Before the menu, so an Escape that closes it doesn't also select the
        // Cursor tool.
        for action in shortcuts::pressed(ui.ctx(), self.typing) {
            self.perform_or_wait(Some(action), &mut waiting, ui.ctx(), frame);
        }
        // Panels are laid out outside-in in the order they're added, so the
        // canvas (the central panel) must come last.
        let can_copy = self.selected.has_nodes();
        let file_dialog_open = self.file_dialog.is_open();
        let menu = menu::show(ui, file_dialog_open, can_copy);
        self.perform_or_wait(menu, &mut waiting, ui.ctx(), frame);
        let (tool, zoom) = (self.tool, self.canvas.zoom());
        let toolbar = toolbar::show(ui, file_dialog_open, can_copy, tool, zoom);
        self.perform_or_wait(toolbar, &mut waiting, ui.ctx(), frame);
        status_bar::show(ui, self.tool, self.file.as_deref(), self.modified);
        for action in self
            .side_panel
            .show(ui, &self.graphml, &self.selected, &self.fonts)
        {
            self.perform(action, ui.ctx(), frame);
        }
        let actions = self.canvas.show(
            ui,
            self.tool,
            &mut self.graphml,
            &mut self.selected,
            &self.style,
        );
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
        // Only actions and the canvas edit the graph, the canvas when a button
        // is let go, so frames with no action and only pointer moves skip
        // comparing it.
        let pressed = ui.input(|input| {
            (input.raw.events.iter()).any(|event| {
                !matches!(
                    event,
                    egui::Event::PointerMoved(_) | egui::Event::MouseMoved(_)
                )
            })
        });
        if std::mem::take(&mut self.performed) || pressed {
            self.check_modified();
        }
        if std::mem::take(&mut self.close_requested) {
            self.close_graph(CloseFor::Quit, ui.ctx(), frame);
        }
        self.show_prompt(ui.ctx(), frame);
        error_dialog::show(ui.ctx(), &mut self.error);
        let name = (self.file.as_deref().and_then(Path::file_name))
            .map_or(status_bar::NOT_SAVED.into(), OsStr::to_string_lossy);
        let marker = if self.modified { "*" } else { "" };
        let title = format!("{name}{marker} — {NAME}");
        if title != self.title {
            ui.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
        self.typing = ui.ctx().text_edit_focused();
    }
}

#[cfg(test)]
mod tests;
