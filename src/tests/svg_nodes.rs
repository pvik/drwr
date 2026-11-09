//! Inserting SVG files as nodes.

use std::fs;

use super::*;
use crate::testing::ScratchDir;

/// A disc 30 across, with a margin, on a 40 by 40 canvas.
const DISC: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"><circle cx="20" cy="20" r="15"/></svg>"#;

/// Writes `svg` to a file called `name` in `dir`, returning its path.
fn svg_file(dir: &ScratchDir, name: &str, svg: &str) -> PathBuf {
    let path = dir.path(name);
    fs::write(&path, svg).unwrap();
    path
}

/// Inserts the SVG file at `path` with the toolbar button, returning the id
/// of the node selected then, if any.
fn insert(harness: &mut Harness, path: &Path) -> Option<String> {
    harness.click_widget("Insert SVG");
    assert_eq!(harness.app.file_dialog.request(), Some(&Request::InsertSvg));
    harness.answer(Some(path));
    harness.app.selected_node().map(str::to_owned)
}

#[test]
fn inserted_svgs_go_selected_in_the_middle_of_the_view_at_their_size() {
    let dir = ScratchDir::new("insert-svg");
    let file = svg_file(&dir, "disc.svg", DISC);
    let mut harness = Harness::new();
    harness.perform(Action::PanBy(-300.0, 40.0));
    let id = insert(&mut harness, &file).unwrap();
    let graphics = harness.graphics(&id);
    let resource = Some("1".to_owned());
    assert_eq!(graphics.kind, NodeKind::Svg { resource });
    assert_eq!(
        yed::resource_text(harness.app.doc.graphml(), "1").as_deref(),
        Some(DISC)
    );
    // As big as what it draws, as yEd makes it.
    let corner = (harness.app.canvas.view_center() - vec2(15.0, 15.0)).round();
    let expected = yed::Rect {
        x: corner.x.into(),
        y: corner.y.into(),
        width: 30.0,
        height: 30.0,
    };
    assert_eq!(graphics.geometry, expected);
    assert!(harness.app.modified);

    // One step undoes it, SVG and all.
    harness.press(CTRL, Key::Z);
    assert_eq!(*harness.app.doc.graphml(), yed::new_document());
    assert!(harness.app.selected.is_empty());
    assert!(!harness.app.modified);
}

#[test]
fn i_asks_for_an_svg_while_no_other_file_is_asked_for() {
    let mut harness = Harness::new();
    harness.press(Modifiers::NONE, Key::I);
    assert_eq!(harness.app.file_dialog.request(), Some(&Request::InsertSvg));
    for button in ["Insert SVG", "Open", "Save"] {
        assert!(harness.widget(button).is_disabled(), "{button}");
    }
    harness.answer(None);
    assert!(!harness.app.file_dialog.is_open());
    assert!(!harness.widget("Insert SVG").is_disabled());
    assert_eq!(*harness.app.doc.graphml(), yed::new_document());

    harness.press(CTRL, Key::O);
    harness.press(Modifiers::NONE, Key::I);
    assert_eq!(harness.app.file_dialog.request(), Some(&Request::Open));
}

#[test]
fn big_svgs_are_inserted_100_points_across_at_most() {
    let dir = ScratchDir::new("insert-big-svg");
    let wide = r#"<svg xmlns="http://www.w3.org/2000/svg" width="2500" height="1000"><rect width="2500" height="1000"/></svg>"#;
    let file = svg_file(&dir, "wide.svg", wide);
    let mut harness = Harness::new();
    let id = insert(&mut harness, &file).unwrap();
    let geometry = harness.graphics(&id).geometry;
    assert_eq!((geometry.width, geometry.height), (100.0, 40.0));
    // Sizes are rounded off where floats miss them.
    let file = svg_file(&dir, "odd.svg", &DISC.replace("r=\"15\"", "r=\"19.6\""));
    let id = insert(&mut harness, &file).unwrap();
    assert_eq!(harness.graphics(&id).geometry.width, 39.2);
}

#[test]
fn files_that_are_not_svg_are_not_inserted() {
    let dir = ScratchDir::new("insert-not-svg");
    let mut harness = Harness::new();
    for file in [
        svg_file(&dir, "notes.svg", "notes"),
        dir.path("missing.svg"),
    ] {
        assert_eq!(insert(&mut harness, &file), None);
        let error = harness.app.error.take().unwrap_or_default();
        let message = format!("Couldn't insert {}:\n", file.display());
        assert!(error.starts_with(&message), "{error:?}");
        assert_eq!(*harness.app.doc.graphml(), yed::new_document());
    }
}

#[test]
fn the_same_svg_inserted_again_is_kept_once() {
    let dir = ScratchDir::new("insert-svg-twice");
    let file = svg_file(&dir, "disc.svg", DISC);
    let mut harness = Harness::new();
    let first = insert(&mut harness, &file).unwrap();
    let second = insert(&mut harness, &file).unwrap();
    assert_ne!(first, second);
    assert_eq!(
        harness.graphics(&first).kind,
        harness.graphics(&second).kind
    );
    let resources = yed::resources(harness.app.doc.graphml()).unwrap();
    assert_eq!(resources.elements().count(), 1);
}

#[test]
fn the_side_panel_leaves_out_what_yed_draws_no_svg_node_with() {
    let mut harness = fixture_harness();
    // The fixture's SVG node, n3.
    harness.click(vec2(320.0, 220.0));
    assert_eq!(harness.app.selected_node(), Some("n3"));
    let panel = harness.side_panel();
    assert_eq!(rows(&panel, "Shape"), ["SVGNode"]);
    for row in ["Fill", "Gradient", "Border", "Border width", "Border style"] {
        assert!(!panel.iter().any(|text| text == row), "{row}: {panel:?}");
    }
    // With a box, they show for the box.
    harness.with_ctrl(|harness| harness.click(vec2(160.0, 70.0)));
    let panel = harness.side_panel();
    assert!(panel.iter().any(|text| text == "Border width"), "{panel:?}");
}
