use super::*;

/// The zooms the canvas allows, at many steps between.
fn zooms() -> impl Iterator<Item = f32> {
    (0..=400).map(|step| crate::zoom::MIN * 40f32.powf(step as f32 / 400.0))
}

/// The lines drawn across 1,000 points of screen with the graph's origin
/// a little left of it.
fn drawn(zoom: f32) -> Vec<(f32, f32)> {
    let lines = lines(Rangef::new(0.0, 1000.0), -123.4, zoom);
    lines.into_iter().filter(|&(_, dark)| dark > 0.0).collect()
}

#[test]
fn lines_are_never_too_close_or_too_far_apart_on_screen() {
    for zoom in zooms() {
        let lines = drawn(zoom);
        let gaps = lines.windows(2).map(|pair| pair[1].0 - pair[0].0);
        let (least, most) = gaps.fold((f32::MAX, 0f32), |(least, most), gap| {
            (least.min(gap), most.max(gap))
        });
        assert!(least >= MIN_GAP - 1e-3, "{least} apart at {zoom}");
        // The spacings grow by at most 5 times.
        assert!(most <= 5.0 * MIN_GAP, "{most} apart at {zoom}");
    }
}

#[test]
fn lines_fade_rather_than_pop_as_the_zoom_changes() {
    for zoom in zooms() {
        let before = lines(Rangef::new(0.0, 1000.0), 0.0, zoom);
        let after = lines(Rangef::new(0.0, 1100.0), 0.0, zoom * 1.01);
        // The same graph points, 1% further from the origin.
        for (at, dark) in before {
            let moved = after.iter().find(|(a, _)| (a - at * 1.01).abs() < 1e-2);
            let then = moved.map_or(0.0, |&(_, dark)| dark);
            assert!((then - dark).abs() < 0.01, "{dark} became {then} at {zoom}");
        }
    }
}

#[test]
fn rounder_lines_are_darker() {
    // 10 points apart at 100%: lines 50 and 100 points apart are darker.
    let lines = lines(Rangef::new(0.0, 200.0), 0.0, 1.0);
    let dark = |at: f32| lines.iter().find(|&&(a, _)| a == at).unwrap().1;
    assert!(0.0 < dark(10.0) && dark(10.0) < dark(50.0));
    assert!(dark(50.0) < dark(100.0));
    assert_eq!(dark(100.0), dark(200.0));
    assert_eq!(dark(100.0), DARK);
}
