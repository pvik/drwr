//! Tests of the undo history and of keeping the scene in sync.

use graphml::yed::{NodeGraphics, NodeKind, ShapeType};

use super::*;
use crate::document::{self, Item};
use crate::scene::{Changed, Run};

const FIXTURE: &str = include_str!("../../crates/graphml/tests/fixtures/yed.graphml");

fn fixture() -> Document {
    Document::new(GraphMl::parse(FIXTURE).unwrap())
}

fn node(id: &str) -> Item {
    Item::Node(id.to_owned())
}

fn rectangle() -> NodeGraphics {
    NodeGraphics::new(NodeKind::Shape(ShapeType::Rectangle))
}

/// Records the edits so far as a step, with nothing selected.
fn record(doc: &mut Document) {
    doc.record(&Selection::default(), &Selection::default(), None);
}

/// Checks that the scene is what reading the whole document gives.
fn assert_scene_synced(doc: &mut Document) {
    let (graphml, scene) = doc.read();
    assert_eq!(*scene, Scene::read(graphml));
}

#[test]
fn undoing_every_step_restores_the_file_byte_for_byte() {
    let mut doc = fixture();
    let original = doc.graphml().to_xml();
    let edits: [fn(&mut GraphMl); 11] = [
        |g| document::move_items(g, &[node("n0"), node("n1")], 10.5, -3.25),
        |g| document::remove_items(g, &[node("n2")]),
        |g| {
            document::add_node(g, &rectangle());
        },
        // Adds the key corner radii are kept under.
        |g| document::set_corner_radius(g, "n0", 12.0),
        |g| document::remove_bend(g, 0, 1),
        |g| {
            let label = document::LabelRef {
                item: node("n1"),
                index: 0,
            };
            document::set_label(g, &label, "Changed", &crate::style::Style::default());
        },
        // n0's copy, n6, keeps its corner radius.
        |g| {
            let copied = document::copy(g, &[node("n4"), node("n0")]).unwrap();
            document::paste(g, copied);
        },
        |g| document::remove_items(g, &[node("n0"), Item::Edge(0)]),
        // Drops the SVG drawing with the last node drawing it.
        |g| document::remove_items(g, &[node("n3")]),
        // Brings it back with a copy from another file.
        |g| {
            let other = GraphMl::parse(FIXTURE).unwrap();
            let copied = document::copy(&other, &[node("n3")]).unwrap();
            document::paste(g, copied);
        },
        // Drops the key again with the last box keeping a radius.
        |g| document::set_corner_radius(g, "n6", 0.0),
    ];
    let mut states = vec![original.clone()];
    for edit in edits {
        edit(doc.graphml_mut());
        record(&mut doc);
        assert_scene_synced(&mut doc);
        states.push(doc.graphml().to_xml());
    }
    for state in states.iter().rev().skip(1) {
        doc.undo().expect("a step to undo");
        assert_scene_synced(&mut doc);
        assert_eq!(doc.graphml().to_xml(), *state);
    }
    assert_eq!(doc.graphml().to_xml(), original);
    assert_eq!(doc.undo(), None);
    for state in &states[1..] {
        doc.redo().expect("a step to redo");
        assert_scene_synced(&mut doc);
        assert_eq!(doc.graphml().to_xml(), *state);
    }
    assert_eq!(doc.redo(), None);
}

#[test]
fn a_step_keeps_copies_of_the_changed_nodes_only() {
    let mut doc = fixture();
    document::move_items(doc.graphml_mut(), &[node("n1")], 5.0, 0.0);
    record(&mut doc);
    let [delta] = doc.undo[0].deltas.as_slice() else {
        panic!("expected one delta");
    };
    let runs = vec![Run {
        at: 1,
        removed: 1,
        added: 1,
    }];
    let changed = Changed::Runs {
        nodes: runs,
        edges: vec![],
    };
    assert_eq!(delta.changed(), changed);
}

#[test]
fn the_scene_reads_only_what_changed() {
    let mut doc = fixture();
    let all = doc.cache.reads;
    assert_eq!(all, doc.graphml().graphs[0].nodes.len() + 2);
    doc.scene();
    assert_eq!(doc.cache.reads, all, "nothing changed");

    document::move_items(doc.graphml_mut(), &[node("n3")], 1.0, 1.0);
    doc.scene();
    assert_eq!(doc.cache.reads, all + 1);
    // A node with the edges of it.
    document::remove_items(doc.graphml_mut(), &[node("n2")]);
    doc.scene();
    assert_eq!(doc.cache.reads, all + 1, "removals read nothing");
    record(&mut doc);
    doc.undo();
    assert_eq!(doc.cache.reads, all + 1 + 2 + 1, "n3, then n2 and e1 back");
    assert_scene_synced(&mut doc);
}

#[test]
fn a_new_key_reads_everything_again() {
    let mut doc = fixture();
    let all = doc.cache.reads;
    document::set_corner_radius(doc.graphml_mut(), "n0", 9.0);
    doc.scene();
    assert_eq!(doc.cache.reads, 2 * all);
    assert_scene_synced(&mut doc);
}

#[test]
fn borrowing_without_changing_records_nothing() {
    let mut doc = fixture();
    doc.graphml_mut();
    record(&mut doc);
    assert!(doc.undo.is_empty());
    assert_eq!(doc.version(), 0);
}

#[test]
fn the_edits_recorded_together_are_undone_together() {
    let mut doc = fixture();
    let original = doc.graphml().clone();
    let graphml = doc.graphml_mut();
    let id = document::add_node(graphml, &rectangle());
    document::move_items(graphml, &[node(&id)], 30.0, 40.0);
    doc.sync();
    document::remove_items(doc.graphml_mut(), &[node("n1")]);
    record(&mut doc);
    assert_eq!(doc.undo.len(), 1);
    doc.undo();
    assert_eq!(*doc.graphml(), original);
    assert_scene_synced(&mut doc);
}

#[test]
fn undo_selects_what_was_selected_before_and_redo_what_after() {
    let mut doc = fixture();
    let before = Selection::of([node("n0"), Item::Edge(1)]);
    document::remove_items(doc.graphml_mut(), before.items());
    doc.record(&before, &Selection::default(), None);
    let id = document::add_node(doc.graphml_mut(), &rectangle());
    doc.record(&Selection::default(), &Selection::node(&id), None);

    assert_eq!(doc.undo(), Some(Selection::default()));
    assert_eq!(doc.undo(), Some(before));
    assert_eq!(doc.redo(), Some(Selection::default()));
    assert_eq!(doc.redo(), Some(Selection::node(id)));
}

#[test]
fn a_new_edit_leaves_nothing_to_redo() {
    let mut doc = fixture();
    document::move_items(doc.graphml_mut(), &[node("n0")], 1.0, 0.0);
    record(&mut doc);
    doc.undo();
    assert!(doc.can_redo());
    document::move_items(doc.graphml_mut(), &[node("n1")], 1.0, 0.0);
    assert!(doc.can_redo(), "until it's recorded");
    assert_eq!(doc.redo(), None);
    assert!(!doc.can_redo());
}

#[test]
fn edits_not_recorded_yet_are_undone_first() {
    let mut doc = fixture();
    let original = doc.graphml().clone();
    assert!(!doc.can_undo());
    document::move_items(doc.graphml_mut(), &[node("n0")], 1.0, 0.0);
    assert!(doc.can_undo(), "before a sync");
    doc.sync();
    assert!(doc.can_undo(), "once synced");
    record(&mut doc);
    let moved = doc.graphml().clone();
    document::move_items(doc.graphml_mut(), &[node("n1")], 1.0, 0.0);
    doc.undo();
    assert_eq!(*doc.graphml(), moved);
    doc.undo();
    assert_eq!(*doc.graphml(), original);
}

/// Nudges `items` right by a point, as one frame of nudging.
fn nudge(doc: &mut Document, items: &Selection) {
    document::move_items(doc.graphml_mut(), items.items(), 1.0, 0.0);
    doc.record(items, items, Some(Merge::Nudge));
}

#[test]
fn a_run_of_nudges_is_one_step() {
    let mut doc = fixture();
    let original = doc.graphml().clone();
    let both = Selection::of([node("n0"), node("n1")]);
    for _ in 0..30 {
        nudge(&mut doc, &both);
    }
    let [step] = doc.undo.as_slice() else {
        panic!("expected one step");
    };
    let [delta] = step.deltas.as_slice() else {
        panic!("expected the nudges' deltas merged");
    };
    let run = |len| Run {
        at: 0,
        removed: len,
        added: len,
    };
    // With the bends of the edge joining them.
    let changed = Changed::Runs {
        nodes: vec![run(2)],
        edges: vec![run(1)],
    };
    assert_eq!(delta.changed(), changed);
    doc.undo();
    assert_eq!(*doc.graphml(), original);
}

#[test]
fn merged_steps_undo_what_each_changed() {
    let mut doc = fixture();
    let original = doc.graphml().clone();
    let both = Selection::of([node("n0"), node("n3")]);
    document::move_items(doc.graphml_mut(), &[node("n0")], 1.0, 0.0);
    doc.record(&both, &both, Some(Merge::Nudge));
    document::move_items(doc.graphml_mut(), &[node("n3")], 0.0, 1.0);
    doc.record(&both, &both, Some(Merge::Nudge));
    assert_eq!(doc.undo.len(), 1);
    doc.undo();
    assert_eq!(*doc.graphml(), original);
    doc.redo();
    let moved = |id| document::graphics_of(doc.graphml(), id).unwrap().geometry;
    let was = |id| document::graphics_of(&original, id).unwrap().geometry;
    assert_eq!(moved("n0").x, was("n0").x + 1.0);
    assert_eq!(moved("n3").y, was("n3").y + 1.0);
}

#[test]
fn a_new_selection_ends_a_run_of_nudges() {
    let mut doc = fixture();
    let n0 = Selection::node("n0");
    nudge(&mut doc, &n0);
    doc.record(&n0, &Selection::default(), None);
    doc.record(&Selection::default(), &n0, None);
    nudge(&mut doc, &n0);
    assert_eq!(doc.undo.len(), 2);
}

#[test]
fn another_edit_ends_a_run_of_nudges() {
    let mut doc = fixture();
    let n0 = Selection::node("n0");
    nudge(&mut doc, &n0);
    document::set_corner_radius(doc.graphml_mut(), "n0", 0.0);
    doc.record(&n0, &n0, None);
    nudge(&mut doc, &n0);
    assert_eq!(doc.undo.len(), 3);
}

#[test]
fn undo_ends_a_run_of_nudges() {
    let mut doc = fixture();
    let n0 = Selection::node("n0");
    nudge(&mut doc, &n0);
    doc.record(&n0, &Selection::default(), None);
    doc.record(&Selection::default(), &n0, None);
    nudge(&mut doc, &n0);
    nudge(&mut doc, &n0);
    doc.undo();
    // Not into the step before the one undone, though its selection is the same.
    nudge(&mut doc, &n0);
    nudge(&mut doc, &n0);
    assert_eq!(doc.undo.len(), 2);
    doc.undo();
    doc.undo();
    assert_eq!(*doc.graphml(), *fixture().graphml());
}

#[test]
fn merged_steps_undo_to_the_keys_from_before_them() {
    let mut doc = fixture();
    let n0 = Selection::node("n0");
    let id = egui::Id::unique("radius");
    // Adds the key corner radii are kept under, then drops it with yEd's.
    for radius in [10.0, 12.0, 4.0] {
        document::set_corner_radius(doc.graphml_mut(), "n0", radius);
        doc.record(&n0, &n0, Some(Merge::Drag(id, 0.0)));
    }
    assert_eq!(doc.undo.len(), 1);
    doc.undo();
    assert_eq!(*doc.graphml(), *fixture().graphml());
}

#[test]
fn drags_merge_only_with_the_rest_of_their_drag() {
    let mut doc = fixture();
    let n0 = Selection::node("n0");
    let id = egui::Id::unique("width");
    for (radius, since) in [(1.0, 0.0), (2.0, 0.0), (3.0, 5.0)] {
        document::set_corner_radius(doc.graphml_mut(), "n0", radius);
        doc.record(&n0, &n0, Some(Merge::Drag(id, since)));
    }
    assert_eq!(doc.undo.len(), 2);
    doc.undo();
    assert_eq!(document::corner_radius(doc.graphml(), "n0"), Some(2.0));
    doc.undo();
    assert_eq!(*doc.graphml(), *fixture().graphml());
}

#[test]
fn the_first_node_of_a_document_without_graphs_can_be_undone() {
    let mut doc = Document::default();
    let id = document::add_node(doc.graphml_mut(), &rectangle());
    record(&mut doc);
    assert_scene_synced(&mut doc);
    doc.undo();
    assert_eq!(*doc.graphml(), GraphMl::default());
    assert_scene_synced(&mut doc);
    doc.redo();
    assert!(document::graphics_of(doc.graphml(), &id).is_some());
    assert_scene_synced(&mut doc);
}

#[test]
fn edits_after_an_undo_are_undone_exactly() {
    type Edit = fn(&mut GraphMl);
    let undone: [(GraphMl, Edit); 3] = [
        // Adds the key corner radii are kept under.
        (GraphMl::parse(FIXTURE).unwrap(), |g| {
            document::set_corner_radius(g, "n0", 10.0)
        }),
        (GraphMl::parse(FIXTURE).unwrap(), |g| {
            document::remove_items(g, &[Item::Edge(0)])
        }),
        // Adds the graph.
        (GraphMl::default(), |g| {
            document::add_node(g, &rectangle());
        }),
    ];
    for (original, edit) in undone {
        let mut doc = Document::new(original.clone());
        edit(doc.graphml_mut());
        record(&mut doc);
        doc.undo();
        document::add_node(doc.graphml_mut(), &rectangle());
        record(&mut doc);
        assert_scene_synced(&mut doc);
        doc.undo();
        assert_eq!(*doc.graphml(), original);
    }
}

#[test]
fn removing_one_of_two_identical_edges_undoes_exactly() {
    let mut graphml = GraphMl::parse(FIXTURE).unwrap();
    // The same and without ids, as files from elsewhere than yEd may have.
    let mut twin = graphml.graphs[0].edges[0].clone();
    twin.id = None;
    graphml.graphs[0].edges = vec![twin.clone(), twin];
    let mut doc = Document::new(graphml.clone());
    document::remove_items(doc.graphml_mut(), &[Item::Edge(1)]);
    record(&mut doc);
    assert_scene_synced(&mut doc);
    let removed = doc.graphml().clone();
    doc.undo();
    assert_eq!(*doc.graphml(), graphml);
    assert_scene_synced(&mut doc);
    doc.redo();
    assert_eq!(*doc.graphml(), removed);
}

#[test]
fn the_version_changes_with_each_change_found_undone_or_redone() {
    let mut doc = fixture();
    document::move_items(doc.graphml_mut(), &[node("n0")], 1.0, 0.0);
    doc.sync();
    assert_eq!(doc.version(), 1);
    record(&mut doc);
    assert_eq!(doc.version(), 1);
    doc.undo();
    doc.redo();
    assert_eq!(doc.version(), 3);
}

#[test]
fn the_oldest_steps_go_once_the_history_is_too_large() {
    let mut doc = fixture();
    doc.limit = 20_000;
    for _ in 0..20 {
        document::move_items(doc.graphml_mut(), &[node("n0")], 1.0, 0.0);
        record(&mut doc);
    }
    let kept = doc.undo.len();
    assert!(1 < kept && kept < 20, "{kept} steps kept");
    let size: usize = doc.undo.iter().map(|step| step.size).sum();
    assert!(size <= doc.limit);
    for _ in 0..kept {
        doc.undo().unwrap();
    }
    assert_eq!(doc.undo(), None);
    let moved = document::graphics_of(doc.graphml(), "n0")
        .unwrap()
        .geometry
        .x;
    let original = document::graphics_of(fixture().graphml(), "n0").unwrap();
    assert_eq!(moved, original.geometry.x + (20 - kept) as f64);
}

#[test]
fn a_history_as_large_as_allowed_keeps_every_step() {
    let mut doc = fixture();
    let n0 = Selection::node("n0");
    document::move_items(doc.graphml_mut(), &[node("n1")], 1.0, 0.0);
    record(&mut doc);
    nudge(&mut doc, &n0);
    doc.limit = doc.undo.iter().map(|step| step.size).sum();
    // Merges without growing, and checks the size again.
    nudge(&mut doc, &n0);
    assert_eq!(doc.undo.len(), 2);
}

#[test]
fn the_latest_step_is_kept_however_large() {
    let mut doc = fixture();
    doc.limit = 1;
    document::remove_items(doc.graphml_mut(), &[node("n0"), node("n5")]);
    record(&mut doc);
    assert_eq!(doc.undo.len(), 1);
}

/// A tiny random number generator, so the sequence is the same every run.
struct Lcg(u64);

impl Lcg {
    fn below(&mut self, n: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % n.max(1)
    }
}

/// A random edit of `doc`, of the kinds drwr makes.
fn random_edit(doc: &mut GraphMl, rng: &mut Lcg) {
    let ids: Vec<String> = document::top_nodes(doc)
        .iter()
        .map(|node| node.id.clone())
        .collect();
    let edges = document::top_edges(doc).len();
    let some_node = |rng: &mut Lcg| node(&ids[rng.below(ids.len())]);
    match rng.below(10) {
        0 if !ids.is_empty() => {
            let items = [some_node(rng), some_node(rng)];
            let dx = rng.below(40) as f64 - 20.25;
            document::move_items(doc, &items, dx, 0.5);
        }
        1 if !ids.is_empty() => document::remove_items(doc, &[some_node(rng)]),
        2 if edges > 0 => document::remove_items(doc, &[Item::Edge(rng.below(edges))]),
        3 if ids.len() > 1 => {
            let (Item::Node(a), Item::Node(b)) = (some_node(rng), some_node(rng)) else {
                unreachable!();
            };
            let graphics = graphml::yed::EdgeGraphics::new(graphml::yed::EdgeKind::PolyLine);
            document::add_edge(doc, &a, &b, &graphics);
        }
        4 if !ids.is_empty() => {
            let id = &ids[rng.below(ids.len())];
            document::set_corner_radius(doc, id, rng.below(3) as f64 * 4.0);
        }
        5 if !ids.is_empty() => {
            let items = [some_node(rng)];
            if let Some(copied) = document::copy(doc, &items) {
                document::paste(doc, copied);
            }
        }
        6 if edges > 0 => document::move_bends(doc, rng.below(edges), None, 3.0, -2.0),
        7 if !ids.is_empty() => {
            let label = document::LabelRef {
                item: some_node(rng),
                index: 0,
            };
            let text = format!("label {}", rng.below(100));
            document::set_label(doc, &label, &text, &crate::style::Style::default());
        }
        8 if ids.len() > 1 => {
            let mut order: Vec<usize> = (0..ids.len()).collect();
            let moved = order.remove(rng.below(ids.len()));
            order.insert(rng.below(ids.len()), moved);
            document::reorder_nodes(doc, &order);
        }
        _ => {
            let mut graphics = rectangle();
            graphics.geometry.x = rng.below(500) as f64;
            document::add_node(doc, &graphics);
        }
    }
}

#[test]
fn random_edits_undo_and_redo_exactly() {
    for seed in 0..8 {
        let mut rng = Lcg(seed);
        let mut doc = fixture();
        let mut states = vec![doc.graphml().to_xml()];
        for _ in 0..40 {
            // Several edits per step at times, some synced in between.
            for _ in 0..1 + rng.below(3) {
                random_edit(doc.graphml_mut(), &mut rng);
                if rng.below(2) == 0 {
                    doc.sync();
                }
            }
            let merge = (rng.below(3) == 0).then_some(Merge::Nudge);
            let n0 = Selection::node("n0");
            doc.record(&n0, &n0, merge);
            assert_scene_synced(&mut doc);
            // A merged step replaces the last state.
            if doc.undo.len() < states.len() {
                states.pop();
            }
            states.push(doc.graphml().to_xml());
            assert_eq!(doc.undo.len() + 1, states.len(), "seed {seed}");
        }
        // Each also leaves the copy the next sync compares with up to date.
        for state in states.iter().rev().skip(1) {
            doc.undo().unwrap();
            assert_eq!(doc.graphml().to_xml(), *state, "seed {seed}");
            assert!(doc.synced == doc.graphml, "seed {seed}");
        }
        assert_scene_synced(&mut doc);
        for state in &states[1..] {
            doc.redo().unwrap();
            assert_eq!(doc.graphml().to_xml(), *state, "seed {seed}");
            assert!(doc.synced == doc.graphml, "seed {seed}");
        }
        assert_scene_synced(&mut doc);
    }
}

/// Edits real GraphML files, then undoes and redoes the edits, checking the
/// scene and the files' text along the way:
///
/// ```sh
/// GRAPHML_CORPUS=~/diagrams cargo test --bin drwr corpus -- --ignored
/// ```
#[test]
#[ignore = "needs GRAPHML_CORPUS, a list of .graphml files or folders separated like PATH"]
fn corpus_edits_undo_and_redo_exactly() {
    let paths = std::env::var_os("GRAPHML_CORPUS").expect("GRAPHML_CORPUS is not set");
    let mut files = Vec::new();
    for path in std::env::split_paths(&paths) {
        collect(&path, &mut files);
    }
    assert!(!files.is_empty(), "no .graphml files in GRAPHML_CORPUS");
    let edits: [fn(&mut GraphMl); 7] = [
        |g| {
            let items = all_items(g);
            document::move_items(g, &items, 0.5, -1.25);
        },
        |g| {
            for item in all_items(g) {
                document::edit(g, &item, |graphics| graphics.stroke_mut().width = 2.0);
            }
        },
        |g| {
            for item in all_items(g) {
                if let Item::Node(id) = item {
                    document::set_corner_radius(g, &id, 8.0);
                }
            }
        },
        |g| {
            let items: Vec<_> = all_items(g).into_iter().take(3).collect();
            if let Some(copied) = document::copy(g, &items) {
                document::paste(g, copied);
            }
        },
        |g| {
            let first = Selection::of(all_items(g).into_iter().take(3));
            if let Some(order) = crate::stacking::raised(g, &Scene::read(g), &first) {
                document::reorder_nodes(g, &order);
            }
        },
        |g| {
            let edges = document::top_edges(g).len();
            let odd: Vec<_> = (1..edges).step_by(2).map(Item::Edge).collect();
            document::remove_items(g, &odd);
        },
        |g| {
            let first: Vec<_> = all_items(g).into_iter().take(1).collect();
            document::remove_items(g, &first);
        },
    ];
    for file in &files {
        let text = std::fs::read_to_string(file).unwrap();
        let mut doc = Document::new(GraphMl::parse(&text).unwrap());
        let mut states = vec![text];
        for edit in edits {
            let steps = doc.undo.len();
            edit(doc.graphml_mut());
            record(&mut doc);
            let (graphml, scene) = doc.read();
            assert!(*scene == Scene::read(graphml), "{}", file.display());
            // Edits that change nothing, such as removing edges from a
            // file without any, make no step.
            if doc.undo.len() > steps {
                states.push(doc.graphml().to_xml());
            }
        }
        for state in states.iter().rev().skip(1) {
            doc.undo();
            assert!(doc.graphml().to_xml() == *state, "{}", file.display());
        }
        let (graphml, scene) = doc.read();
        assert!(*scene == Scene::read(graphml), "{}", file.display());
        for state in &states[1..] {
            doc.redo();
            assert!(doc.graphml().to_xml() == *state, "{}", file.display());
        }
    }
    eprintln!("{} files edited, undone and redone", files.len());
}

/// The top-level nodes, then the top-level edges, of `doc`.
fn all_items(doc: &GraphMl) -> Vec<Item> {
    let nodes = document::top_nodes(doc).iter().map(|n| node(&n.id));
    nodes
        .chain((0..document::top_edges(doc).len()).map(Item::Edge))
        .collect()
}

fn collect(path: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
    if path.is_dir() {
        for entry in std::fs::read_dir(path).unwrap() {
            collect(&entry.unwrap().path(), files);
        }
    } else if path
        .extension()
        .is_some_and(|extension| extension == "graphml")
    {
        files.push(path.to_owned());
    }
}
