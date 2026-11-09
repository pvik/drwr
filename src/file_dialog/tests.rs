use std::path::{Path, PathBuf};

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
fn a_file_to_save_gets_the_graphml_extension_if_it_has_none() {
    for (picked, saved) in [
        ("/tmp/plan", "/tmp/plan.graphml"),
        ("/tmp/plan.graphml", "/tmp/plan.graphml"),
        ("/tmp/plan.xml", "/tmp/plan.xml"),
        ("/tmp/v1.2/plan", "/tmp/v1.2/plan.graphml"),
    ] {
        let mut dialog = open_on(save_request());
        dialog.answer(Some(PathBuf::from(picked)));
        let (_, path) = dialog.picked().unwrap();
        assert_eq!(path, Path::new(saved), "{picked}");
    }
}

#[test]
fn a_file_to_open_keeps_its_name() {
    let mut dialog = open_on(Request::Open);
    dialog.answer(Some(PathBuf::from("/tmp/plan")));
    assert_eq!(dialog.picked(), Some((1, PathBuf::from("/tmp/plan"))));
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
