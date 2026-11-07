use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread::JoinHandle;

use egui::epaint::text::VariationCoords;
use egui::{FontData, FontDefinitions, FontFamily, FontInsert, FontProvider, FontTweak};
use fontique::{
    Collection, CollectionOptions, FamilyInfo, FontStyle, FontWeight, FontWidth, GenericFamily,
    SourceCache,
};
use graphml::yed;

/// The styles the side panel offers, in its order.
pub const STYLES: [yed::FontStyle; 4] = [
    yed::FontStyle::Plain,
    yed::FontStyle::Bold,
    yed::FontStyle::Italic,
    yed::FontStyle::BoldItalic,
];

/// Java's logical fonts, which yEd offers besides the installed ones, and the
/// generic families Java draws them in.
const LOGICAL: [(&str, GenericFamily); 5] = [
    ("Dialog", GenericFamily::SansSerif),
    ("DialogInput", GenericFamily::Monospace),
    ("Monospaced", GenericFamily::Monospace),
    ("SansSerif", GenericFamily::SansSerif),
    ("Serif", GenericFamily::Serif),
];

/// Starts the names of the egui families that labels are drawn in.
const PREFIX: &str = "label:";

/// How a style reads in the side panel. Unknown ones read as written.
pub fn style_name(style: &yed::FontStyle) -> &str {
    match style {
        yed::FontStyle::Plain => "Plain",
        yed::FontStyle::Bold => "Bold",
        yed::FontStyle::Italic => "Italic",
        yed::FontStyle::BoldItalic => "Bold italic",
        yed::FontStyle::Other(keyword) => keyword,
    }
}

/// The egui family of text in the yEd font `family` and `style`, which
/// [`Fonts`] provides.
pub fn family(family: &str, style: &yed::FontStyle) -> FontFamily {
    let style = match style {
        yed::FontStyle::Other(_) => yed::FontStyle::Plain.as_str(),
        style => style.as_str(),
    };
    FontFamily::Name(format!("{PREFIX}{style}:{family}").into())
}

/// The yEd family and the style of an egui family from [`family`].
fn parse(name: &str) -> Option<(&str, FontStyle, FontWeight)> {
    let (style, family) = name.strip_prefix(PREFIX)?.split_once(':')?;
    let (style, weight) = match yed::FontStyle::from(style) {
        yed::FontStyle::Bold => (FontStyle::Normal, FontWeight::BOLD),
        yed::FontStyle::Italic => (FontStyle::Italic, FontWeight::NORMAL),
        yed::FontStyle::BoldItalic => (FontStyle::Italic, FontWeight::BOLD),
        _ => (FontStyle::Normal, FontWeight::NORMAL),
    };
    Some((family, style, weight))
}

/// The fonts labels are drawn in, found by the family names yEd writes: the
/// egui [`FontProvider`] of the families [`family`] returns.
pub struct Fonts {
    finding: Mutex<Option<JoinHandle<Library>>>,
    library: OnceLock<Mutex<Library>>,
}

struct Library {
    collection: Collection,
    sources: SourceCache,
    /// See [`Fonts::families`].
    families: Vec<String>,
}

impl Default for Fonts {
    /// Without the system's fonts, so labels are drawn in egui's own.
    fn default() -> Self {
        Fonts::with(Library::new(without_system_fonts()))
    }
}

impl Fonts {
    /// With the system's fonts, found in the background as that takes a while.
    pub fn system() -> Self {
        let finding = std::thread::Builder::new()
            .name("fonts".to_owned())
            .spawn(|| Library::new(Collection::new(CollectionOptions::default())));
        Fonts {
            finding: Mutex::new(finding.ok()),
            library: OnceLock::new(),
        }
    }

    fn with(library: Library) -> Self {
        Fonts {
            finding: Mutex::new(None),
            library: OnceLock::from(Mutex::new(library)),
        }
    }

    /// The families a label can be set in, sorted: the installed ones and
    /// Java's logical ones.
    pub fn families(&self) -> Vec<String> {
        self.library().families.clone()
    }

    /// Waits for the system's fonts, if they're still being found.
    fn library(&self) -> MutexGuard<'_, Library> {
        let library = self.library.get_or_init(|| {
            let library = match lock(&self.finding).take() {
                Some(finding) => finding
                    .join()
                    .unwrap_or_else(|_| Library::new(without_system_fonts())),
                // No thread could be started.
                None => Library::new(Collection::new(CollectionOptions::default())),
            };
            Mutex::new(library)
        });
        lock(library)
    }
}

impl FontProvider for Fonts {
    fn fonts_for_family(&self, family: &FontFamily) -> Vec<FontInsert> {
        let FontFamily::Name(name) = family else {
            return Vec::new();
        };
        let Some((family, style, weight)) = parse(name) else {
            return Vec::new();
        };
        let logical = logical(family);
        let generic = logical.unwrap_or_else(|| guess_generic(family));
        let mut library = self.library();
        let installed = match logical {
            Some(_) => None,
            None => library.collection.family_by_name(family),
        };
        let found = installed.or_else(|| library.generic(generic));
        match found.and_then(|found| library.load(&found, style, weight)) {
            Some(font) => vec![font],
            None => builtin(generic),
        }
    }
}

impl Library {
    fn new(mut collection: Collection) -> Self {
        let names: Vec<String> = collection.family_names().map(str::to_owned).collect();
        // Without the other names of the families, e.g. "Noto Sans Black",
        // which fontique doesn't tell apart from them.
        let mut families: Vec<String> = names
            .iter()
            .filter_map(|name| {
                let id = collection.family_id(name)?;
                collection.family_name(id).map(str::to_owned)
            })
            .chain(LOGICAL.map(|(name, _)| name.to_owned()))
            .collect();
        families.sort_by_cached_key(|family| family.to_lowercase());
        families.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        Library {
            collection,
            sources: SourceCache::default(),
            families,
        }
    }

    /// The first installed family of a generic one.
    fn generic(&mut self, generic: GenericFamily) -> Option<FamilyInfo> {
        let ids: Vec<_> = self.collection.generic_families(generic).collect();
        ids.into_iter().find_map(|id| self.collection.family(id))
    }

    /// The font of `family` closest to a style and weight.
    fn load(
        &mut self,
        family: &FamilyInfo,
        style: FontStyle,
        weight: FontWeight,
    ) -> Option<FontInsert> {
        let font = family.match_font(FontWidth::NORMAL, style, weight, false)?;
        let blob = font.load(Some(&mut self.sources))?;
        // Sets a variable font's weight. epaint can't fake a missing bold or
        // italic.
        let synthesis = font.synthesis(FontWidth::NORMAL, style, weight);
        let settings = synthesis.variation_settings();
        // egui keeps one face per name.
        let coords: String = settings
            .iter()
            .map(|(tag, value)| format!(":{tag}={value}"))
            .collect();
        let name = format!(
            "{PREFIX}{}:{}{coords}",
            font.source().id().to_u64(),
            font.index()
        );
        let tweak = FontTweak {
            coords: VariationCoords::new(settings.iter().copied()),
            ..FontTweak::default()
        };
        let (blob, _) = blob.into_raw_parts();
        Some(FontInsert {
            name,
            data: FontData::from_blob(blob, font.index()).tweak(tweak),
            families: Vec::new(),
        })
    }
}

/// The generic family Java draws a logical font in, if `family` is one.
fn logical(family: &str) -> Option<GenericFamily> {
    let found = LOGICAL
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(family));
    found.map(|&(_, generic)| generic)
}

/// The generic family to draw a family that isn't installed in, going by its
/// name.
fn guess_generic(family: &str) -> GenericFamily {
    let family = family.to_lowercase();
    if family.contains("mono") || family.contains("courier") {
        GenericFamily::Monospace
    } else if family.contains("serif") && !family.contains("sans") {
        GenericFamily::Serif
    } else {
        GenericFamily::SansSerif
    }
}

/// egui's own fonts, for when the system has none for `generic`.
fn builtin(generic: GenericFamily) -> Vec<FontInsert> {
    let family = match generic {
        GenericFamily::Monospace => FontFamily::Monospace,
        _ => FontFamily::Proportional,
    };
    FontDefinitions::default().fonts_for_family(&family)
}

fn without_system_fonts() -> Collection {
    Collection::new(CollectionOptions {
        shared: false,
        system_fonts: false,
    })
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests;
