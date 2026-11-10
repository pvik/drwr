//! Has macOS's quits (Cmd+Q in winit's app menu, the Dock's Quit, logging
//! out) close drwr's window as its close button does, so that drwr asks about
//! unsaved changes first, where winit would quit at once.

use std::sync::atomic::{AtomicBool, Ordering};

use objc2::runtime::{AnyClass, AnyObject, Imp, NSObjectProtocol as _, Sel};
use objc2::{MainThreadMarker, sel};
use objc2_app_kit::{NSApplication, NSApplicationTerminateReply};

/// Whether quitting at once would lose nothing, as drwr finds every frame.
static CAN_QUIT_AT_ONCE: AtomicBool = AtomicBool::new(true);

pub fn set_can_quit_at_once(can: bool) {
    CAN_QUIT_AT_ONCE.store(can, Ordering::Relaxed);
}

type ShouldTerminate =
    unsafe extern "C-unwind" fn(*mut AnyObject, Sel, *mut AnyObject) -> NSApplicationTerminateReply;

/// Gives winit's app delegate the `applicationShouldTerminate:` it lacks.
/// Needs eframe's event loop, which sets that delegate.
pub fn ask_before_quitting() {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let Some(delegate) = app.delegate() else {
        return;
    };
    let class = AsRef::<AnyObject>::as_ref(&*delegate).class();
    // SAFETY: "Q@:@" spells `should_terminate`'s types: an NSUInteger reply,
    // the delegate, the selector and the app.
    let added = unsafe {
        objc2::ffi::class_addMethod(
            (class as *const AnyClass).cast_mut(),
            sel!(applicationShouldTerminate:),
            std::mem::transmute::<ShouldTerminate, Imp>(should_terminate),
            c"Q@:@".as_ptr(),
        )
    };
    if added.as_bool() {
        // In case AppKit noted what the delegate answers when it was set.
        app.setDelegate(None);
        app.setDelegate(Some(&*delegate));
    }
}

/// Quits at once if that loses nothing, or else cancels quitting and closes
/// drwr's window, which asks about unsaved changes and then quits.
unsafe extern "C-unwind" fn should_terminate(
    _delegate: *mut AnyObject,
    _cmd: Sel,
    _app: *mut AnyObject,
) -> NSApplicationTerminateReply {
    let quit = NSApplicationTerminateReply::TerminateNow;
    if CAN_QUIT_AT_ONCE.load(Ordering::Relaxed) {
        return quit;
    }
    let (Some(mtm), Some(winit_window)) = (MainThreadMarker::new(), AnyClass::get(c"WinitWindow"))
    else {
        return quit;
    };
    let app = NSApplication::sharedApplication(mtm);
    let windows = app.windows().to_vec();
    let windows: Vec<_> = windows
        .into_iter()
        .filter(|window| window.isKindOfClass(winit_window))
        .collect();
    // Rather than never quit.
    if windows.is_empty() {
        return quit;
    }
    // Quitting from the Dock or logging out leaves drwr behind other apps.
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
    for window in windows {
        if window.isMiniaturized() {
            window.deminiaturize(None);
        }
        window.performClose(None);
    }
    NSApplicationTerminateReply::TerminateCancel
}
