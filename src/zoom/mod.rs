//! The canvas's zoom levels, in screen points per graph point.

/// The levels the zoom dropdown offers.
pub const PRESETS: [f32; 4] = [0.5, 1.0, 1.5, 2.0];

/// The levels zooming in and out steps through.
const STEPS: [f32; 12] = [
    0.1, 0.15, 0.2, 0.3, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0,
];

pub const MIN: f32 = STEPS[0];
pub const MAX: f32 = STEPS[STEPS.len() - 1];

/// How `zoom` reads, e.g. "125%".
pub fn text(zoom: f32) -> String {
    format!("{}%", percent(zoom))
}

/// The next step in from `zoom` that reads differently, if there is one.
pub fn step_in(zoom: f32) -> Option<f32> {
    let shown = percent(zoom);
    STEPS.into_iter().find(|&step| percent(step) > shown)
}

/// The next step out from `zoom` that reads differently, if there is one.
pub fn step_out(zoom: f32) -> Option<f32> {
    let shown = percent(zoom);
    STEPS.into_iter().rev().find(|&step| percent(step) < shown)
}

/// In whole percents, as the zoom reads.
fn percent(zoom: f32) -> f32 {
    (zoom * 100.0).round()
}

#[cfg(test)]
mod tests;
