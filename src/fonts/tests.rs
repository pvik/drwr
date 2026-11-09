use std::sync::Arc;

use fontique::Blob;
use yed::FontStyle::{Bold, BoldItalic, Italic, Other, Plain};

use super::*;

/// With egui's Hack and Ubuntu Light installed, Hack as the monospace family.
fn installed() -> Fonts {
    let mut collection = without_system_fonts();
    let mut install = |name: &str| {
        let bytes = FontDefinitions::default().font_data[name].bytes().to_vec();
        collection.register_fonts(Blob::new(Arc::new(bytes)), None)[0].0
    };
    let hack = install("Hack");
    install("Ubuntu-Light");
    collection.set_generic_families(GenericFamily::Monospace, [hack].into_iter());
    Fonts::with(Library::new(collection))
}

/// Which of egui's fonts `fonts` draws a family in, by their names in egui.
fn drawn_in(fonts: &Fonts, name: &str, style: yed::FontStyle) -> Vec<String> {
    let builtin = FontDefinitions::default().font_data;
    let faces = fonts.fonts_for_family(&family(name, &style));
    faces
        .iter()
        .map(|face| {
            let same = builtin
                .iter()
                .find(|(_, data)| data.bytes() == face.data.bytes());
            same.map_or_else(|| face.name.clone(), |(name, _)| name.clone())
        })
        .collect()
}

#[test]
fn families_name_the_family_and_style() {
    assert_eq!(
        family("Noto Sans", &Bold),
        FontFamily::Name("label:bold:Noto Sans".into())
    );
    assert_eq!(
        parse("label:bolditalic:Noto Sans"),
        Some(("Noto Sans", FontStyle::Italic, FontWeight::BOLD))
    );
    assert_eq!(
        parse("label:italic:a:b"),
        Some(("a:b", FontStyle::Italic, FontWeight::NORMAL))
    );
    // Unknown styles are drawn plain.
    assert_eq!(
        family("Serif", &Other("x:y".to_owned())),
        family("Serif", &Plain)
    );
    assert_eq!(parse("Noto Sans"), None);
}

#[test]
fn installed_families_are_drawn_in_themselves() {
    let fonts = installed();
    assert_eq!(drawn_in(&fonts, "Hack", Plain), ["Hack"]);
    assert_eq!(drawn_in(&fonts, "hack", Plain), ["Hack"]);
    // The closest face the family has.
    assert_eq!(drawn_in(&fonts, "Ubuntu", BoldItalic), ["Ubuntu-Light"]);
}

#[test]
fn other_families_are_drawn_in_a_generic_family() {
    let fonts = installed();
    // Java's logical fonts.
    assert_eq!(drawn_in(&fonts, "Monospaced", Plain), ["Hack"]);
    assert_eq!(drawn_in(&fonts, "dialoginput", Italic), ["Hack"]);
    // Missing ones, going by their names.
    assert_eq!(drawn_in(&fonts, "Noto Mono", Plain), ["Hack"]);
    assert_eq!(drawn_in(&fonts, "Courier New", Bold), ["Hack"]);
}

#[test]
fn families_are_drawn_in_eguis_fonts_without_the_systems() {
    let fonts = Fonts::default();
    for name in ["Dialog", "Serif", "Noto Sans", "Hack"] {
        assert_eq!(
            drawn_in(&fonts, name, Plain),
            ["Ubuntu-Light", "egui-icons"],
            "{name}"
        );
    }
    for name in ["Monospaced", "Noto Mono"] {
        let drawn = drawn_in(&fonts, name, Bold);
        assert_eq!(drawn, ["Hack", "Ubuntu-Light", "egui-icons"], "{name}");
    }
}

#[test]
fn only_label_families_are_provided() {
    let fonts = installed();
    for family in [
        FontFamily::Proportional,
        FontFamily::Monospace,
        FontFamily::Name("Hack".into()),
        // As a label's, but without the prefix.
        FontFamily::Name("plain:Hack".into()),
    ] {
        assert!(fonts.fonts_for_family(&family).is_empty(), "{family:?}");
    }
}

#[test]
fn the_families_offered_are_the_installed_and_logical_ones_sorted() {
    assert_eq!(
        installed().families(),
        [
            "Dialog",
            "DialogInput",
            "Hack",
            "Monospaced",
            "SansSerif",
            "Serif",
            "Ubuntu"
        ]
    );
    assert_eq!(
        Fonts::default().families(),
        ["Dialog", "DialogInput", "Monospaced", "SansSerif", "Serif"]
    );
}

#[test]
fn styles_read_as_in_yed_or_as_written() {
    let names: Vec<_> = STYLES.iter().map(style_name).collect();
    assert_eq!(names, ["Plain", "Bold", "Italic", "Bold italic"]);
    assert_eq!(style_name(&Other("wavy".to_owned())), "wavy");
}
