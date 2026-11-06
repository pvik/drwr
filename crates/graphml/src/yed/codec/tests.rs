use super::java_double;

#[test]
fn doubles_are_formatted_like_java() {
    for (value, text) in [
        (0.0, "0.0"),
        (-0.0, "-0.0"),
        (30.0, "30.0"),
        (-2.5, "-2.5"),
        (356.89214797883415, "356.89214797883415"),
        (0.001, "0.001"),
        (1e-4, "1.0E-4"),
        (1.5e-5, "1.5E-5"),
        (1e7, "1.0E7"),
        (12345678.9, "1.23456789E7"),
        (f64::NAN, "NaN"),
        (f64::NEG_INFINITY, "-Infinity"),
    ] {
        assert_eq!(java_double(value), text, "{value}");
    }
}
