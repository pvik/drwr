//! The document being edited, with its scene and undo history. A sync
//! compares it with a copy, which finds every edit however it was made, and
//! reads only the changed nodes and edges into the scene.

mod delta;

use graphml::GraphMl;

use crate::scene::{Cache, Scene};
use crate::selection::Selection;

use delta::Delta;

/// Roughly how many bytes the undo history may take.
const MAX_HISTORY: usize = 100 << 20;

/// A kind of edit whose steps merge while nothing else happens in between.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Merge {
    /// Nudging the selection by keys.
    Nudge,
    /// Dragging a number field, by its id and when the drag started.
    Drag(egui::Id, f64),
}

#[derive(Debug)]
struct Step {
    /// In the order they were made.
    deltas: Vec<Delta>,
    /// What was selected before the step and after it.
    before: Selection,
    after: Selection,
    merge: Option<Merge>,
    size: usize,
}

pub struct Document {
    graphml: GraphMl,
    /// `graphml` as of the last sync, which the scene was read from.
    synced: GraphMl,
    /// Whether `graphml` was borrowed mutably since the last sync.
    dirty: bool,
    edits: u64,
    version: u64,
    cache: Cache,
    /// Changes synced since the last step was recorded.
    pending: Vec<Delta>,
    undo: Vec<Step>,
    redo: Vec<Step>,
    /// Whether the last step may merge with the next.
    open: bool,
    limit: usize,
}

impl Default for Document {
    fn default() -> Self {
        Document::new(GraphMl::default())
    }
}

impl Document {
    pub fn new(graphml: GraphMl) -> Self {
        Document {
            cache: Cache::read(&graphml),
            synced: graphml.clone(),
            graphml,
            dirty: false,
            edits: 0,
            version: 0,
            pending: Vec::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            open: false,
            limit: MAX_HISTORY,
        }
    }

    pub fn graphml(&self) -> &GraphMl {
        &self.graphml
    }

    /// Borrows the document to change it, which the next sync finds.
    pub fn graphml_mut(&mut self) -> &mut GraphMl {
        self.dirty = true;
        self.edits += 1;
        &mut self.graphml
    }

    /// Counts the mutable borrows.
    pub fn edits(&self) -> u64 {
        self.edits
    }

    /// Changes whenever a sync, undo or redo changes the document.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// The scene, synced with the document.
    pub fn scene(&mut self) -> &Scene {
        self.sync();
        self.cache.scene()
    }

    /// The document and its scene, synced.
    pub fn read(&mut self) -> (&GraphMl, &Scene) {
        self.sync();
        (&self.graphml, self.cache.scene())
    }

    /// Reads what changed since the last sync into the scene, keeping the
    /// change for the next step.
    pub fn sync(&mut self) {
        if !std::mem::take(&mut self.dirty) {
            return;
        }
        let Some(delta) = Delta::update(&mut self.synced, &self.graphml) else {
            return;
        };
        self.cache.update(&self.graphml, &delta.changed());
        self.version += 1;
        push(&mut self.pending, delta);
    }

    /// Records the changes since the last step as one, made with `before`
    /// selected and leaving `after`. It joins the last step if that has the
    /// same `merge` and nothing else happened since, not even a new selection.
    pub fn record(&mut self, before: &Selection, after: &Selection, merge: Option<Merge>) {
        self.sync();
        if self.pending.is_empty() {
            if before != after {
                self.open = false;
            }
            return;
        }
        let deltas = std::mem::take(&mut self.pending);
        self.redo.clear();
        let last = self.undo.last_mut().filter(|last| {
            self.open && merge.is_some() && last.merge == merge && last.after == *before
        });
        match last {
            Some(last) => {
                let mut grew = false;
                for delta in deltas {
                    grew |= push(&mut last.deltas, delta);
                }
                last.after = after.clone();
                if grew {
                    last.size = last.deltas.iter().map(Delta::size).sum();
                }
            }
            None => self.undo.push(Step {
                size: deltas.iter().map(Delta::size).sum(),
                deltas,
                before: before.clone(),
                after: after.clone(),
                merge,
            }),
        }
        self.open = merge.is_some();
        self.forget_oldest();
    }

    pub fn has_unrecorded(&self) -> bool {
        self.dirty || !self.pending.is_empty()
    }

    #[cfg(test)]
    pub fn reads(&self) -> usize {
        self.cache.reads
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || !self.pending.is_empty() || self.dirty
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Undoes the last step, returning what was selected before it.
    pub fn undo(&mut self) -> Option<Selection> {
        // Callers record first; this keeps unrecorded edits undoable anyway.
        self.record(&Selection::default(), &Selection::default(), None);
        let mut step = self.undo.pop()?;
        for delta in step.deltas.iter_mut().rev() {
            self.swap(delta);
        }
        let selected = step.before.clone();
        self.redo.push(step);
        self.open = false;
        Some(selected)
    }

    /// Redoes the last step undone, returning what was selected after it.
    pub fn redo(&mut self) -> Option<Selection> {
        self.record(&Selection::default(), &Selection::default(), None);
        let mut step = self.redo.pop()?;
        for delta in &mut step.deltas {
            self.swap(delta);
        }
        let selected = step.after.clone();
        self.undo.push(step);
        self.open = false;
        Some(selected)
    }

    /// Puts `delta`'s parts in the document, keeping the ones they replace.
    fn swap(&mut self, delta: &mut Delta) {
        delta.swap(&mut self.graphml);
        delta.copy_changes(&self.graphml, &mut self.synced);
        self.cache.update(&self.graphml, &delta.changed());
        self.version += 1;
    }

    /// Drops the oldest steps while the history is too large, but the last.
    fn forget_oldest(&mut self) {
        let steps = self.undo.iter().chain(&self.redo);
        let mut size: usize = steps.map(|step| step.size).sum();
        let mut drop = 0;
        while size > self.limit && drop + 1 < self.undo.len() {
            size -= self.undo[drop].size;
            drop += 1;
        }
        self.undo.drain(..drop);
    }
}

/// Adds `delta`, merging it into the last if it can. Returns whether that
/// kept more parts.
fn push(deltas: &mut Vec<Delta>, delta: Delta) -> bool {
    let delta = match deltas.last_mut() {
        Some(last) => match last.absorb(delta) {
            Ok(grew) => return grew,
            Err(delta) => delta,
        },
        None => delta,
    };
    deltas.push(delta);
    true
}

#[cfg(test)]
mod tests;
