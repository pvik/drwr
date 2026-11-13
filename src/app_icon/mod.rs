//! drwr's icon, drawn in `assets/app_icon/drwr.svg`. The window and the
//! release packages use the icon files beside it, which the tests draw from
//! the SVG.

use egui::IconData;

/// The icon, 256 pixels across. Decoding it is over ten times faster than
/// drawing the SVG.
const PNG: &[u8] = include_bytes!("../../assets/app_icon/drwr.png");

/// The window's icon. On macOS it's empty, so the Dock shows the app bundle's
/// icon instead of eframe's default one.
pub fn window_icon() -> IconData {
    if cfg!(target_os = "macos") {
        return IconData::default();
    }
    eframe::icon_data::from_png_bytes(PNG).expect("drwr.png is a PNG")
}

#[cfg(test)]
mod tests;
