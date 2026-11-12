use egui::{Align, Button, Context, EventFilter, Key, Layout, Modifiers, ScrollArea, Sense, Ui};

use crate::dialog;

const MIN_WIDTH: f32 = 240.0;
/// As wide as a prompt at most, long names cut short.
const MAX_WIDTH: f32 = 400.0;
/// Beyond this, the list scrolls, so the buttons stay in the window.
const MAX_LIST_HEIGHT: f32 = 300.0;

/// Up and Down move the selection instead of the focus.
const ARROWS: EventFilter = EventFilter {
    tab: false,
    horizontal_arrows: false,
    vertical_arrows: true,
    escape: false,
};

/// A modal dialog asking which page of a file to open, for a target of type
/// `T`, which tells the caller what the pages are of.
pub struct PageDialog<T> {
    dialog: Option<Dialog<T>>,
}

struct Dialog<T> {
    target: T,
    /// The file's name.
    file: String,
    pages: Vec<String>,
    /// The index of the page Open opens.
    selected: usize,
    /// Whether Open has yet to take the focus.
    focus: bool,
    /// Whether the selected page has yet to be scrolled into view.
    scroll: bool,
}

impl<T> Default for PageDialog<T> {
    fn default() -> Self {
        PageDialog { dialog: None }
    }
}

impl<T> PageDialog<T> {
    pub fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    /// Opens the dialog on the `pages` of the file named `file`, which has
    /// some, with the first selected, unless it's open already.
    pub fn open(&mut self, target: T, file: impl Into<String>, pages: Vec<String>) {
        if self.is_open() || pages.is_empty() {
            return;
        }
        self.dialog = Some(Dialog {
            target,
            file: file.into(),
            pages,
            selected: 0,
            focus: true,
            scroll: false,
        });
    }

    /// Shows the dialog while it's open. Returns its target and the index of
    /// the page picked, once it's opened.
    pub fn show(&mut self, ctx: &Context) -> Option<(T, usize)> {
        let picked = dialog::show(ctx, "page_dialog", &mut self.dialog, Dialog::ui)?;
        Some((picked.target, picked.selected))
    }
}

impl<T> Dialog<T> {
    /// Returns whether the selected page was opened.
    fn ui(&mut self, ui: &mut Ui) -> bool {
        ui.set_min_width(MIN_WIDTH);
        ui.set_max_width(MAX_WIDTH);
        ui.strong("Open page");
        let count = self.pages.len();
        ui.label(format!(
            "{} has {count} pages. Pick the one to open:",
            self.file
        ));
        // Counted, as a slow frame gets several key repeats.
        let (up, down) = ui.input_mut(|input| {
            let mut presses = |key| input.count_and_consume_key(Modifiers::NONE, key);
            (presses(Key::ArrowUp), presses(Key::ArrowDown))
        });
        if up + down > 0 {
            self.selected = (self.selected + down).saturating_sub(up).min(count - 1);
            self.scroll = true;
        }
        let mut opened = false;
        ScrollArea::vertical()
            .max_height(MAX_LIST_HEIGHT)
            // So the arrows scroll at once, not over a few frames.
            .animated(false)
            .show(ui, |ui| {
                ui.with_layout(Layout::top_down_justified(Align::LEFT), |ui| {
                    for (index, name) in self.pages.iter().enumerate() {
                        let selected = index == self.selected;
                        // Left out of the focus, which stays on the buttons.
                        let row = Button::selectable(selected, name.as_str())
                            .sense(Sense::CLICK)
                            .truncate();
                        let row = ui.add(row);
                        if row.clicked() {
                            self.selected = index;
                        }
                        opened |= row.double_clicked();
                        if selected && std::mem::take(&mut self.scroll) {
                            row.scroll_to_me(None);
                        }
                    }
                });
            });
        ui.separator();
        ui.horizontal(|ui| {
            let open = ui.button("Open");
            // So Enter opens, though a menu or dialog just closed may still
            // have the focus.
            if std::mem::take(&mut self.focus) || ui.memory(|memory| memory.focused().is_none()) {
                open.request_focus();
            }
            let cancel = ui.button("Cancel");
            for button in [&open, &cancel] {
                ui.memory_mut(|memory| memory.set_focus_lock_filter(button.id, ARROWS));
            }
            if cancel.clicked() {
                ui.close();
            }
            opened |= open.clicked();
        });
        opened
    }
}

#[cfg(test)]
mod tests;
