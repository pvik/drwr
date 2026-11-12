use std::path::PathBuf;

use super::*;

fn save_request() -> Request {
    Request::Save {
        name: "untitled.graphml".to_owned(),
        folder: None,
    }
}

/// A dialog open on `request` for target 1.
fn open_on(request: Request) -> FileDialog<u8> {
    let mut dialog = FileDialog::default();
    let frame = eframe::Frame::_new_kittest();
    dialog.open(1, request, &egui::Context::default(), &frame);
    dialog
}

#[test]
fn the_file_picked_comes_with_the_target_once_answered() {
    let mut dialog = open_on(Request::Open);
    assert!(dialog.is_open());
    assert_eq!(dialog.picked(), None);
    assert!(dialog.is_open());

    dialog.answer(Some(PathBuf::from("/tmp/a.graphml")));
    assert_eq!(dialog.picked(), Some((1, PathBuf::from("/tmp/a.graphml"))));
    assert!(!dialog.is_open());
}

#[test]
fn cancelling_closes_the_dialog_without_a_file() {
    let mut dialog = open_on(save_request());
    dialog.answer(None);
    assert_eq!(dialog.picked(), None);
    assert!(!dialog.is_open());
}

#[test]
fn the_file_picked_keeps_its_name() {
    for request in [Request::Open, save_request()] {
        let mut dialog = open_on(request.clone());
        dialog.answer(Some(PathBuf::from("/tmp/plan")));
        let picked = dialog.picked();
        assert_eq!(picked, Some((1, PathBuf::from("/tmp/plan"))), "{request:?}");
    }
}

#[test]
fn opening_an_open_dialog_keeps_its_target_and_request() {
    let mut dialog = open_on(Request::Open);
    let frame = eframe::Frame::_new_kittest();
    dialog.open(2, save_request(), &egui::Context::default(), &frame);
    assert_eq!(dialog.request(), Some(&Request::Open));
    dialog.answer(Some(PathBuf::from("/tmp/plan")));
    assert_eq!(dialog.picked(), Some((1, PathBuf::from("/tmp/plan"))));
}

#[test]
fn open_offers_graphml_and_drawio_files_together_first_and_save_only_graphml() {
    let diagrams = ["graphml", "drawio", "dio", "xml"];
    let drawio = ["drawio", "dio", "xml"];
    assert_eq!(
        filters(&Request::Open),
        [
            ("Diagrams", diagrams.to_vec()),
            ("GraphML", vec!["graphml"]),
            ("draw.io", drawio.to_vec()),
        ]
    );
    assert_eq!(filters(&save_request()), [("GraphML", vec!["graphml"])]);
    let export = Request::Export {
        format: Format::Svg,
        name: "plan.svg".to_owned(),
        folder: None,
    };
    assert_eq!(filters(&export), [("SVG", vec!["svg"])]);
}
