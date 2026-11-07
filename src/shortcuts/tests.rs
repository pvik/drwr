use egui::os::OperatingSystem;
use egui::{Event, Id, Modal, RawInput};

use super::*;

const NONE: Modifiers = Modifiers::NONE;
/// Ctrl as egui-winit reports it outside macOS.
const CTRL: Modifiers = Modifiers::CTRL.plus(Modifiers::COMMAND);

fn key(modifiers: Modifiers, key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers,
    }
}

/// Runs a frame with `events`, returning what `f` returns in it.
fn frame<R>(ctx: &Context, events: Vec<Event>, mut f: impl FnMut(&Context) -> R) -> R {
    let mut result = None;
    let input = RawInput {
        events,
        ..RawInput::default()
    };
    ctx.run_ui(input, |ui| result = Some(f(ui.ctx())))
        .drop_without_applying_deltas();
    result.expect("the frame ran")
}

#[test]
fn shortcuts_pick_their_actions() {
    let ctx = Context::default();
    for (modifiers, k, action) in [
        (CTRL, Key::N, Action::New),
        (CTRL, Key::O, Action::Open),
        (CTRL, Key::S, Action::Save),
        (NONE, Key::Escape, Action::SelectTool(Tool::Cursor)),
        (NONE, Key::B, Action::SelectTool(Tool::Box)),
        (NONE, Key::C, Action::SelectTool(Tool::Circle)),
        (NONE, Key::E, Action::SelectTool(Tool::Ellipse)),
        (NONE, Key::L, Action::SelectTool(Tool::Line)),
        (NONE, Key::A, Action::SelectTool(Tool::Arrow)),
        (NONE, Key::T, Action::SelectTool(Tool::TextBox)),
    ] {
        let picked = frame(&ctx, vec![key(modifiers, k)], pressed);
        assert_eq!(picked, Some(action), "{modifiers:?} {k:?}");
    }
}

#[test]
fn other_keys_pick_nothing() {
    let ctx = Context::default();
    for (modifiers, k) in [(NONE, Key::O), (CTRL, Key::B), (Modifiers::ALT, Key::F)] {
        let picked = frame(&ctx, vec![key(modifiers, k)], pressed);
        assert_eq!(picked, None, "{modifiers:?} {k:?}");
    }
}

#[test]
fn alt_f_is_the_file_menu_shortcut() {
    let ctx = Context::default();
    let alt_f = frame(&ctx, vec![key(Modifiers::ALT, Key::F)], file_menu_pressed);
    let f = frame(&ctx, vec![key(NONE, Key::F)], file_menu_pressed);
    assert!(alt_f);
    assert!(!f);
}

#[test]
fn an_open_menu_keeps_the_keys() {
    let ctx = Context::default();
    frame(&ctx, vec![], |ctx| Popup::open_id(ctx, Id::unique("menu")));
    let (picked, escape_left) = frame(&ctx, vec![key(NONE, Key::Escape)], |ctx| {
        (pressed(ctx), ctx.input(|i| i.key_pressed(Key::Escape)))
    });
    assert_eq!(picked, None);
    assert!(escape_left);
}

#[test]
fn a_modal_dialog_keeps_the_keys() {
    let ctx = Context::default();
    let modal = |ctx: &Context| Modal::new(Id::unique("modal")).show(ctx, |_| {});
    frame(&ctx, vec![], modal);
    let picked = frame(&ctx, vec![key(NONE, Key::B)], |ctx| {
        let picked = pressed(ctx);
        modal(ctx);
        picked
    });
    assert_eq!(picked, None);
    let file_menu = frame(&ctx, vec![key(Modifiers::ALT, Key::F)], file_menu_pressed);
    assert!(!file_menu);
}

#[test]
fn text_reads_like_the_shortcut() {
    let ctx = Context::default();
    ctx.set_os(OperatingSystem::Nix);
    assert_eq!(text(&ctx, Action::Open).as_deref(), Some("Ctrl+O"));
    let cursor = text(&ctx, Action::SelectTool(Tool::Cursor));
    assert_eq!(cursor.as_deref(), Some("Escape"));
}
