use super::*;

#[test]
fn zoom_reads_in_whole_percents() {
    assert_eq!(text(1.0), "100%");
    assert_eq!(text(0.15), "15%");
    assert_eq!(text(1.2214), "122%");
    assert_eq!(text(4.0), "400%");
}

#[test]
fn zooming_in_and_out_steps_through_the_levels() {
    let mut zoom = 1.0;
    let mut zoomed_in = Vec::new();
    while let Some(next) = step_in(zoom) {
        zoomed_in.push(text(next));
        zoom = next;
    }
    assert_eq!(zoomed_in, ["125%", "150%", "200%", "300%", "400%"]);

    let mut zoomed_out = Vec::new();
    while let Some(next) = step_out(zoom) {
        zoomed_out.push(text(next));
        zoom = next;
    }
    let expected = [
        "300%", "200%", "150%", "125%", "100%", "75%", "50%", "30%", "20%", "15%", "10%",
    ];
    assert_eq!(zoomed_out, expected);
}

#[test]
fn levels_between_steps_go_to_the_next_one() {
    assert_eq!(step_in(1.2214), Some(1.25));
    assert_eq!(step_out(1.2214), Some(1.0));
}

#[test]
fn steps_that_read_the_same_are_skipped() {
    // Reads as 125% already, so stepping to 125% would show no change.
    assert_eq!(step_in(1.2496), Some(1.5));
    assert_eq!(step_out(0.7504), Some(0.5));
}

#[test]
fn there_are_no_steps_past_the_limits() {
    assert_eq!(step_in(MAX), None);
    assert_eq!(step_out(MIN), None);
    // Within half a percent of a limit reads as the limit.
    assert_eq!(step_in(3.996), None);
}
