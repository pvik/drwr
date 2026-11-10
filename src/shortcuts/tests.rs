use egui::os::OperatingSystem;
use egui::{Event, Id, Modal, RawInput};

use super::*;
use crate::testing::{CTRL, key};

const NONE: Modifiers = Modifiers::NONE;

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
        (CTRL.plus(Modifiers::SHIFT), Key::S, Action::SaveAs),
        (CTRL, Key::E, Action::Export),
        (CTRL, Key::Q, Action::Quit),
        (CTRL, Key::X, Action::Cut),
        (CTRL, Key::C, Action::Copy),
        (CTRL, Key::V, Action::Paste),
        (CTRL, Key::Enter, Action::EditLabel),
        (CTRL, Key::Plus, Action::ZoomIn),
        (CTRL, Key::Equals, Action::ZoomIn),
        (CTRL, Key::Minus, Action::ZoomOut),
        (CTRL, Key::Num0, Action::SetZoom(1.0)),
        (CTRL, Key::Quote, Action::ToggleGridLines),
        (CTRL, Key::ArrowLeft, Action::MoveBy(-1.0, 0.0)),
        (CTRL, Key::ArrowRight, Action::MoveBy(1.0, 0.0)),
        (CTRL, Key::ArrowUp, Action::MoveBy(0.0, -1.0)),
        (CTRL, Key::ArrowDown, Action::MoveBy(0.0, 1.0)),
        (NONE, Key::ArrowLeft, Action::PanBy(-40.0, 0.0)),
        (NONE, Key::ArrowRight, Action::PanBy(40.0, 0.0)),
        (NONE, Key::ArrowUp, Action::PanBy(0.0, -40.0)),
        (NONE, Key::ArrowDown, Action::PanBy(0.0, 40.0)),
        (NONE, Key::PageUp, Action::Raise),
        (NONE, Key::PageDown, Action::Lower),
        (NONE, Key::Escape, Action::SelectTool(Tool::Cursor)),
        (NONE, Key::B, Action::SelectTool(Tool::Box)),
        (NONE, Key::C, Action::SelectTool(Tool::Circle)),
        (NONE, Key::E, Action::SelectTool(Tool::Ellipse)),
        (NONE, Key::L, Action::SelectTool(Tool::Line)),
        (NONE, Key::A, Action::SelectTool(Tool::Arrow)),
    ] {
        let picked = frame(&ctx, vec![key(modifiers, k)], |ctx| pressed(ctx, false));
        assert_eq!(picked, [action], "{modifiers:?} {k:?}");
    }
}

#[test]
fn every_press_in_a_frame_picks_its_action() {
    let ctx = Context::default();
    let repeat = Event::Key {
        key: Key::ArrowRight,
        physical_key: None,
        pressed: true,
        repeat: true,
        modifiers: CTRL,
    };
    let events = vec![
        key(CTRL, Key::ArrowRight),
        repeat,
        key(CTRL, Key::ArrowDown),
    ];
    let picked = frame(&ctx, events, |ctx| pressed(ctx, false));
    let right = Action::MoveBy(1.0, 0.0);
    assert_eq!(picked, [right.clone(), right, Action::MoveBy(0.0, 1.0)]);
}

#[test]
fn clipboard_events_pick_their_actions() {
    let ctx = Context::default();
    let events = vec![Event::Cut, Event::Copy, Event::Paste("text".to_owned())];
    let (picked, left) = frame(&ctx, events, |ctx| {
        (pressed(ctx, false), ctx.input(|i| i.events.len()))
    });
    let paste = Action::PasteText("text".to_owned());
    assert_eq!(picked, [Action::Cut, Action::Copy, paste]);
    assert_eq!(left, 0);
}

#[test]
fn a_field_typed_in_keeps_the_clipboard_events() {
    let ctx = Context::default();
    let events = vec![Event::Copy, Event::Paste("text".to_owned())];
    let (picked, left) = frame(&ctx, events, |ctx| {
        (pressed(ctx, true), ctx.input(|i| i.events.len()))
    });
    assert!(picked.is_empty(), "{picked:?}");
    assert_eq!(left, 2);
}

#[test]
fn a_field_typed_in_keeps_the_keys_but_save_save_as_and_quit() {
    let ctx = Context::default();
    let events = vec![
        key(CTRL, Key::S),
        key(CTRL.plus(Modifiers::SHIFT), Key::S),
        key(CTRL, Key::Q),
        key(NONE, Key::B),
        key(CTRL, Key::N),
        Event::Copy,
    ];
    let (picked, left) = frame(&ctx, events, |ctx| {
        (pressed(ctx, true), ctx.input(|i| i.events.len()))
    });
    assert_eq!(picked, [Action::SaveAs, Action::Save, Action::Quit]);
    assert_eq!(left, 3);

    // Not while a menu has the keys.
    frame(&ctx, vec![], |ctx| Popup::open_id(ctx, Id::unique("menu")));
    let picked = frame(&ctx, vec![key(CTRL, Key::S)], |ctx| pressed(ctx, true));
    assert!(picked.is_empty(), "{picked:?}");
}

#[test]
fn other_keys_pick_nothing() {
    let ctx = Context::default();
    for (modifiers, k) in [(NONE, Key::O), (CTRL, Key::B), (Modifiers::ALT, Key::F)] {
        let picked = frame(&ctx, vec![key(modifiers, k)], |ctx| pressed(ctx, false));
        assert!(picked.is_empty(), "{modifiers:?} {k:?}: {picked:?}");
    }
}

#[test]
fn a_focused_widget_keeps_the_plain_arrows() {
    let ctx = Context::default();
    // As when tabbed to.
    frame(&ctx, vec![], |ctx| {
        ctx.memory_mut(|memory| memory.request_focus(Id::unique("button")));
    });
    let arrows = vec![key(NONE, Key::ArrowRight), key(CTRL, Key::ArrowRight)];
    let picked = frame(&ctx, arrows, |ctx| pressed(ctx, false));
    // Egui moves the focus on with plain arrows only.
    assert_eq!(picked, [Action::MoveBy(1.0, 0.0)]);
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
fn a_field_typed_in_keeps_alt_f() {
    let ctx = Context::default();
    let mut text = String::new();
    // As when clicked into.
    ctx.run_ui(RawInput::default(), |ui| {
        ui.text_edit_singleline(&mut text).request_focus();
    })
    .drop_without_applying_deltas();
    let (file_menu, f_left) = frame(&ctx, vec![key(Modifiers::ALT, Key::F)], |ctx| {
        let file_menu = file_menu_pressed(ctx);
        (file_menu, ctx.input(|i| i.key_pressed(Key::F)))
    });
    assert!(!file_menu);
    assert!(f_left);
}

#[test]
fn an_open_menu_keeps_the_keys() {
    let ctx = Context::default();
    frame(&ctx, vec![], |ctx| Popup::open_id(ctx, Id::unique("menu")));
    let (picked, escape_left) = frame(&ctx, vec![key(NONE, Key::Escape)], |ctx| {
        (
            pressed(ctx, false),
            ctx.input(|i| i.key_pressed(Key::Escape)),
        )
    });
    assert!(picked.is_empty(), "{picked:?}");
    assert!(escape_left);
}

#[test]
fn a_field_typed_in_last_frame_keeps_the_keys() {
    let ctx = Context::default();
    let (picked, escape_left) = frame(&ctx, vec![key(NONE, Key::Escape)], |ctx| {
        (
            pressed(ctx, true),
            ctx.input(|i| i.key_pressed(Key::Escape)),
        )
    });
    assert!(picked.is_empty(), "{picked:?}");
    assert!(escape_left);
}

#[test]
fn a_modal_dialog_keeps_the_keys() {
    let ctx = Context::default();
    let modal = |ctx: &Context| Modal::new(Id::unique("modal")).show(ctx, |_| {});
    frame(&ctx, vec![], modal);
    let picked = frame(&ctx, vec![key(NONE, Key::B)], |ctx| {
        let picked = pressed(ctx, false);
        modal(ctx);
        picked
    });
    assert!(picked.is_empty(), "{picked:?}");
    let file_menu = frame(&ctx, vec![key(Modifiers::ALT, Key::F)], file_menu_pressed);
    assert!(!file_menu);
}

#[test]
fn text_reads_like_the_shortcut() {
    let ctx = Context::default();
    ctx.set_os(OperatingSystem::Nix);
    assert_eq!(text(&ctx, &Action::Open).as_deref(), Some("Ctrl+O"));
    assert_eq!(text(&ctx, &Action::Save).as_deref(), Some("Ctrl+S"));
    assert_eq!(text(&ctx, &Action::SaveAs).as_deref(), Some("Ctrl+Shift+S"));
    assert_eq!(text(&ctx, &Action::Quit).as_deref(), Some("Ctrl+Q"));
    assert_eq!(text(&ctx, &Action::Paste).as_deref(), Some("Ctrl+V"));
    let cursor = text(&ctx, &Action::SelectTool(Tool::Cursor));
    assert_eq!(cursor.as_deref(), Some("Escape"));
    // The first of an action's shortcuts.
    assert_eq!(text(&ctx, &Action::ZoomIn).as_deref(), Some("Ctrl+Plus"));
    // Not "Ctrl+Quote", as egui has it.
    let grid = text(&ctx, &Action::ToggleGridLines);
    assert_eq!(grid.as_deref(), Some("Ctrl+'"));
}

#[test]
fn text_reads_like_the_shortcut_on_macs() {
    let ctx = Context::default();
    ctx.set_os(OperatingSystem::Mac);
    // Egui spells out Cmd if its fonts lack ⌘, which it can tell only once a
    // frame has loaded them.
    frame(&ctx, vec![], |_| {});
    assert_eq!(text(&ctx, &Action::Open).as_deref(), Some("Cmd+O"));
    assert_eq!(text(&ctx, &Action::SaveAs).as_deref(), Some("Shift+Cmd+S"));
    let grid = text(&ctx, &Action::ToggleGridLines);
    assert_eq!(grid.as_deref(), Some("Cmd+'"));
}
