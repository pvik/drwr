use egui::{
    AsIdSalt, Button, ComboBox, Id, Key, Modifiers, PopupCloseBehavior, ScrollArea, TextEdit,
    TextStyle, TextWrapMode, Ui, vec2,
};

/// How many families the list shows at once.
const ROWS: f32 = 12.0;
/// Wider than the dropdown, which would cut many family names short.
const MIN_WIDTH: f32 = 240.0;

/// A dropdown of font families, with a field that narrows them down to those
/// with the words typed in it.
#[derive(Default)]
pub struct FontPicker {
    list: Option<List>,
}

/// The families of an open dropdown.
struct List {
    /// The dropdown's.
    id: Id,
    families: Vec<String>,
    filter: String,
    /// The indices of the families the filter matches.
    matches: Vec<usize>,
    /// Of the matches, the one Enter picks.
    highlighted: Option<usize>,
    /// Whether the field has yet to take the focus.
    focus: bool,
    scroll: Scroll,
    /// How far down the families are scrolled.
    offset: f32,
}

/// How to scroll the families to the highlighted one.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scroll {
    No,
    /// To the middle, as when the list opens.
    ToMiddle,
    /// Only as far as it takes to show it.
    IntoView,
}

impl FontPicker {
    /// Shows a dropdown for picking a family other than `current`, from the
    /// `families` listed when it opens. Returns the family picked.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        id_salt: impl AsIdSalt,
        current: &str,
        families: impl FnOnce() -> Vec<String>,
    ) -> Option<String> {
        let id = ui.make_persistent_id(id_salt);
        let mut picked = None;
        let shown = ComboBox::from_id_salt(id)
            .selected_text(current)
            .width(ui.available_width())
            .truncate()
            // The list scrolls by itself, under the field.
            .height(f32::INFINITY)
            // So that clicking the field leaves it open.
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show_ui(ui, |ui| {
                let list = match &mut self.list {
                    Some(list) if list.id == id => list,
                    list => list.insert(List::new(id, families(), current)),
                };
                picked = list.show(ui);
                if picked.is_some() {
                    ui.close();
                }
            });
        if shown.inner.is_none() && self.list.as_ref().is_some_and(|list| list.id == id) {
            self.list = None;
        }
        picked.filter(|family| family != current)
    }
}

impl List {
    fn new(id: Id, families: Vec<String>, current: &str) -> Self {
        List {
            id,
            matches: (0..families.len()).collect(),
            // Found regardless of case, like the font itself.
            highlighted: families
                .iter()
                .position(|family| family.eq_ignore_ascii_case(current)),
            families,
            filter: String::new(),
            focus: true,
            scroll: Scroll::ToMiddle,
            offset: 0.0,
        }
    }

    /// Shows the field and the families it matches. Returns the family picked.
    fn show(&mut self, ui: &mut Ui) -> Option<String> {
        ui.set_min_width(MIN_WIDTH);
        let field = self.id.with("filter");
        // Taken from the field, which would otherwise move its cursor, or
        // drop the focus on Enter. Counted, as a slow frame gets several.
        let focused = ui.memory(|memory| memory.has_focus(field));
        let take = |key| match focused {
            true => ui.input_mut(|input| input.count_and_consume_key(Modifiers::NONE, key)),
            false => 0,
        };
        let (up, down, enter) = (
            take(Key::ArrowUp),
            take(Key::ArrowDown),
            take(Key::Enter) > 0,
        );
        if up + down > 0 {
            self.step(down as isize - up as isize);
        }
        let response = ui.add(
            TextEdit::singleline(&mut self.filter)
                .id(field)
                .hint_text("Search")
                .desired_width(f32::INFINITY),
        );
        if response.changed() {
            self.filter_families();
        }
        // A new dropdown is laid out unseen first, which takes neither.
        let seen = !ui.is_sizing_pass();
        if seen && std::mem::take(&mut self.focus) {
            response.request_focus();
        }
        let scroll = match seen {
            true => std::mem::replace(&mut self.scroll, Scroll::No),
            false => Scroll::No,
        };

        let mut picked = enter.then(|| self.highlighted_family()).flatten();
        if self.matches.is_empty() {
            ui.weak("No matches");
            return picked;
        }
        // Each row as tall as a button, which `show_rows` needs to know.
        let spacing = ui.spacing();
        let text_height = ui.text_style_height(&TextStyle::Button);
        let row_height =
            (text_height + 2.0 * spacing.button_padding.y).max(spacing.interact_size.y);
        let pitch = row_height + spacing.item_spacing.y;
        let mut list = ScrollArea::vertical()
            .max_height(ROWS * pitch)
            .auto_shrink([false, true]);
        // Before the rows are laid out, which `Ui::scroll_to_rect` would be
        // after, leaving them blank for a frame when the list shrinks.
        if let Some(offset) = self.scroll_offset(scroll, row_height, pitch) {
            list = list.vertical_scroll_offset(offset);
        }
        let output = list.show_rows(ui, row_height, self.matches.len(), |ui, rows| {
            ui.style_mut().wrap_mode = Some(TextWrapMode::Truncate);
            for row in rows {
                let family = &self.families[self.matches[row]];
                let button = Button::selectable(self.highlighted == Some(row), family.as_str())
                    .min_size(vec2(0.0, row_height));
                if ui.add(button).clicked() {
                    picked = Some(family.clone());
                }
            }
        });
        self.offset = output.state.clamped_offset().y;
        picked
    }

    /// How far down to scroll the families, rows `pitch` apart, for `scroll`.
    fn scroll_offset(&self, scroll: Scroll, row_height: f32, pitch: f32) -> Option<f32> {
        let top = self.highlighted? as f32 * pitch;
        let height = self.matches.len() as f32 * pitch - (pitch - row_height);
        let view = height.min(ROWS * pitch);
        let offset = match scroll {
            Scroll::No => return None,
            Scroll::ToMiddle => top - (view - row_height) / 2.0,
            Scroll::IntoView => self.offset.max(top + row_height - view).min(top),
        };
        Some(offset.clamp(0.0, (height - view).max(0.0)))
    }

    /// Moves the highlight `by` matches down, or up if negative, within them.
    fn step(&mut self, by: isize) {
        let Some(last) = self.matches.len().checked_sub(1) else {
            return;
        };
        self.highlighted = Some(match self.highlighted {
            Some(highlighted) => highlighted.saturating_add_signed(by).min(last),
            None => 0,
        });
        self.scroll = Scroll::IntoView;
    }

    /// Narrows the families down to those with every word typed, and
    /// highlights the first.
    fn filter_families(&mut self) {
        let words: Vec<String> = self
            .filter
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        self.matches = (0..self.families.len())
            .filter(|&index| {
                let family = self.families[index].to_lowercase();
                words.iter().all(|word| family.contains(word.as_str()))
            })
            .collect();
        self.highlighted = (!self.matches.is_empty()).then_some(0);
        self.scroll = Scroll::IntoView;
    }

    fn highlighted_family(&self) -> Option<String> {
        let index = self.matches[self.highlighted?];
        Some(self.families[index].clone())
    }
}

#[cfg(test)]
mod tests;
