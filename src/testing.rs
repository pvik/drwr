//! Helpers shared by tests.

use std::fs;
use std::io;
use std::path::PathBuf;

use egui::accesskit;
use egui::{
    Event, FullOutput, Key, Modifiers, MouseWheelSource, MouseWheelUnit, PointerButton, Pos2,
    TouchPhase, pos2, vec2,
};

/// Ctrl as egui-winit reports it outside macOS.
pub const CTRL: Modifiers = Modifiers::CTRL.plus(Modifiers::COMMAND);
/// Cmd as egui-winit reports it on macOS.
pub const CMD: Modifiers = Modifiers::MAC_CMD.plus(Modifiers::COMMAND);

/// A press of `key` with `modifiers` held.
pub fn key(modifiers: Modifiers, key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// The primary button going down, or up, at `pos`.
pub fn button(pos: Pos2, pressed: bool) -> Event {
    pointer_button(PointerButton::Primary, pos, pressed)
}

/// The middle button going down, or up, at `pos`.
pub fn middle_button(pos: Pos2, pressed: bool) -> Event {
    pointer_button(PointerButton::Middle, pos, pressed)
}

fn pointer_button(button: PointerButton, pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos,
        button,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// The wheel turning by `lines` notches, up if positive.
pub fn wheel(lines: f32) -> Event {
    Event::MouseWheel {
        unit: MouseWheelUnit::Line,
        delta: vec2(0.0, lines),
        phase: TouchPhase::Move,
        source: MouseWheelSource::Wheel,
        modifiers: Modifiers::NONE,
    }
}

/// The widgets a frame showed, read from its accessibility update.
pub fn widgets(output: &mut FullOutput) -> Vec<accesskit::Node> {
    let update = output.platform_output.accesskit_update.take();
    update.map_or_else(Vec::new, |update| {
        update.nodes.into_iter().map(|(_, node)| node).collect()
    })
}

/// The middle of a widget.
pub fn center(widget: &accesskit::Node) -> Pos2 {
    let bounds = widget.bounds().expect("widgets have bounds");
    pos2(
        ((bounds.x0 + bounds.x1) / 2.0) as f32,
        ((bounds.y0 + bounds.y1) / 2.0) as f32,
    )
}

/// Runs frames of an 800×600 window for a [`Drive`], keeping the widgets,
/// focus and shapes of the last one.
pub struct Driver {
    pub ctx: egui::Context,
    time: f64,
    pub widgets: Vec<accesskit::Node>,
    /// The widget with the keyboard focus, if any.
    pub focused: Option<accesskit::Node>,
    pub shapes: Vec<egui::Shape>,
}

impl Default for Driver {
    fn default() -> Self {
        let ctx = egui::Context::default();
        ctx.enable_accesskit();
        Driver {
            ctx,
            time: 0.0,
            widgets: Vec::new(),
            focused: None,
            shapes: Vec::new(),
        }
    }
}

impl Driver {
    /// The widget labelled `label`.
    pub fn widget(&self, label: &str) -> &accesskit::Node {
        (self.widgets.iter())
            .find(|widget| widget.label() == Some(label))
            .unwrap_or_else(|| panic!("no widget is labelled {label:?}"))
    }

    /// Whether the last frame painted `text`.
    pub fn painted(&self, text: &str) -> bool {
        (self.shapes.iter())
            .any(|shape| matches!(shape, egui::Shape::Text(shape) if shape.galley.text() == text))
    }

    /// The next frame's input, a tenth of a second on.
    fn input(&mut self, events: Vec<Event>) -> egui::RawInput {
        self.time += 0.1;
        egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))),
            time: Some(self.time),
            events,
            ..egui::RawInput::default()
        }
    }

    fn record(&mut self, mut output: FullOutput) {
        let (nodes, focus) = match output.platform_output.accesskit_update.take() {
            Some(update) => {
                // The root has the focus while no widget has.
                let root = update.tree.map(|tree| tree.root);
                let focus = (Some(update.focus) != root).then_some(update.focus);
                (update.nodes, focus)
            }
            None => (Vec::new(), None),
        };
        self.focused = (nodes.iter())
            .find(|(id, _)| Some(*id) == focus)
            .map(|(_, node)| node.clone());
        self.widgets = nodes.into_iter().map(|(_, node)| node).collect();
        let shapes = std::mem::take(&mut output.shapes).into_iter();
        self.shapes = shapes.map(|clipped| clipped.shape).collect();
        output.drop_without_applying_deltas();
    }
}

/// A test harness that its [`Driver`] shows, one frame at a time.
pub trait Drive {
    fn driver(&mut self) -> &mut Driver;

    /// Shows what's under test.
    fn show(&mut self, ui: &mut egui::Ui);

    fn frame(&mut self, events: Vec<Event>) {
        let input = self.driver().input(events);
        let ctx = self.driver().ctx.clone();
        let output = ctx.run_ui(input, |ui| self.show(ui));
        self.driver().record(output);
    }

    /// Clicks `at` `clicks` times in a row.
    fn click_at(&mut self, at: Pos2, clicks: usize) {
        // Too long since any earlier click to make this a double click.
        self.driver().time += 1.0;
        self.frame(vec![Event::PointerMoved(at)]);
        for _ in 0..clicks {
            for pressed in [true, false] {
                self.frame(vec![button(at, pressed)]);
            }
        }
        self.repaint();
    }

    /// Clicks the widget labelled `label`.
    fn click(&mut self, label: &str) {
        let at = center(self.driver().widget(label));
        self.click_at(at, 1);
    }

    fn double_click(&mut self, label: &str) {
        let at = center(self.driver().widget(label));
        self.click_at(at, 2);
    }

    fn press(&mut self, key: Key) {
        self.press_with(Modifiers::NONE, key);
    }

    fn press_with(&mut self, modifiers: Modifiers, key: Key) {
        self.frame(vec![self::key(modifiers, key)]);
        self.repaint();
    }

    fn type_text(&mut self, text: &str) {
        self.frame(vec![Event::Text(text.to_owned())]);
        self.repaint();
    }

    /// Runs the frame egui repaints right after any input, in which widgets
    /// shown before a change catch up.
    fn repaint(&mut self) {
        self.frame(vec![]);
    }
}

/// A directory of a test's own for its files, removed afterwards.
pub struct ScratchDir(PathBuf);

impl ScratchDir {
    pub fn new(test: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("drwr-{test}-{}", std::process::id()));
        // Left by a killed run whose pid has come round again.
        if let Err(err) = fs::remove_dir_all(&dir)
            && err.kind() != io::ErrorKind::NotFound
        {
            panic!("couldn't clear {}: {err}", dir.display());
        }
        fs::create_dir_all(&dir).unwrap();
        ScratchDir(dir)
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }

    /// The names of the files in it, sorted.
    pub fn names(&self) -> Vec<String> {
        let entries = fs::read_dir(&self.0).unwrap();
        let mut names: Vec<_> = entries
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
