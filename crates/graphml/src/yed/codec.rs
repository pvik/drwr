//! Reading and writing typed values in yEd elements. Writes leave attributes
//! alone when they already hold the value, so untouched numbers keep their
//! original text and unchanged files stay byte-identical.

use std::collections::BTreeMap;

use super::Color;
use crate::{
    Error,
    xml::{Element, XmlNode},
};

/// A value stored in an XML attribute.
pub(crate) trait Codec: Sized + PartialEq {
    fn decode(text: &str) -> Option<Self>;
    fn encode(&self) -> String;
}

/// A value stored in a child element, such as `y:Geometry`.
pub(crate) trait Part: Sized + PartialEq {
    fn read(element: &Element) -> Result<Self, Error>;
    fn write(&self, element: &mut Element);
}

/// Defines an enum written as one of a set of keywords, keeping unknown ones.
macro_rules! keyword_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident { $($(#[$variant_meta:meta])* $variant:ident = $keyword:literal,)+ }
    ) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, Hash)]
        pub enum $name {
            $($(#[$variant_meta])* $variant,)+
            /// A keyword this library doesn't know, kept as written.
            Other(String),
        }

        impl $name {
            pub fn as_str(&self) -> &str {
                match self { $(Self::$variant => $keyword,)+ Self::Other(keyword) => keyword }
            }
        }

        impl From<&str> for $name {
            fn from(keyword: &str) -> Self {
                match keyword { $($keyword => Self::$variant,)+ _ => Self::Other(keyword.to_owned()) }
            }
        }

        impl $crate::yed::codec::Codec for $name {
            fn decode(text: &str) -> Option<Self> {
                Some(text.into())
            }

            fn encode(&self) -> String {
                self.as_str().to_owned()
            }
        }
    };
}
pub(crate) use keyword_enum;

impl Codec for f64 {
    fn decode(text: &str) -> Option<Self> {
        text.trim().parse().ok()
    }

    fn encode(&self) -> String {
        java_double(*self)
    }
}

impl Codec for u32 {
    fn decode(text: &str) -> Option<Self> {
        text.trim().parse().ok()
    }

    fn encode(&self) -> String {
        self.to_string()
    }
}

impl Codec for bool {
    fn decode(text: &str) -> Option<Self> {
        match text.trim() {
            t if t.eq_ignore_ascii_case("true") => Some(true),
            t if t.eq_ignore_ascii_case("false") => Some(false),
            _ => None,
        }
    }

    fn encode(&self) -> String {
        self.to_string()
    }
}

impl Codec for String {
    fn decode(text: &str) -> Option<Self> {
        Some(text.to_owned())
    }

    fn encode(&self) -> String {
        self.clone()
    }
}

impl Codec for Color {
    fn decode(text: &str) -> Option<Self> {
        Color::from_hex(text.trim())
    }

    fn encode(&self) -> String {
        self.to_string()
    }
}

/// Formats like Java's `Double.toString`, which yEd uses: `30.0`, `1.0E-4`.
pub(crate) fn java_double(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }
    let magnitude = value.abs();
    if magnitude == 0.0 || (1e-3..1e7).contains(&magnitude) {
        let text = value.to_string();
        if text.contains('.') {
            text
        } else {
            text + ".0"
        }
    } else {
        let text = format!("{value:e}");
        let (mantissa, exponent) = text.split_once('e').expect("`{:e}` has an exponent");
        let point = if mantissa.contains('.') { "" } else { ".0" };
        format!("{mantissa}{point}E{exponent}")
    }
}

/// Reads attribute `name`, or `None` if it's missing.
pub(crate) fn get<T: Codec>(element: &Element, name: &str) -> Result<Option<T>, Error> {
    let Some(value) = element.attr(name) else {
        return Ok(None);
    };
    T::decode(value)
        .map(Some)
        .ok_or_else(|| Error::InvalidValue {
            element: element.name.clone(),
            attribute: name.to_owned(),
            value: value.to_owned(),
        })
}

pub(crate) fn get_or<T: Codec>(element: &Element, name: &str, default: T) -> Result<T, Error> {
    Ok(get(element, name)?.unwrap_or(default))
}

/// Sets attribute `name`, unless it already holds `value`.
pub(crate) fn set<T: Codec>(element: &mut Element, name: &str, value: &T) {
    if element.attr(name).and_then(T::decode).as_ref() != Some(value) {
        element.set_attr(name, value.encode());
    }
}

/// Reads a color that yEd writes as `color="#…"`, or as `hasColor="false"`
/// if there's none (each pair of attributes has its own names).
pub(crate) fn get_color(element: &Element, has: &str, color: &str) -> Result<Option<Color>, Error> {
    if get::<bool>(element, has)? == Some(false) {
        return Ok(None);
    }
    get(element, color)
}

pub(crate) fn set_color(element: &mut Element, has: &str, color: &str, value: Option<Color>) {
    match value {
        Some(value) => {
            if element.attr(has).and_then(bool::decode) == Some(false) {
                element.attributes.remove(has);
            }
            set(element, color, &value);
        }
        None => {
            element.attributes.remove(color);
            set(element, has, &false);
        }
    }
}

/// Stands in for a missing child, so that it reads as all defaults.
static MISSING: Element = Element {
    name: String::new(),
    attributes: BTreeMap::new(),
    children: Vec::new(),
};

/// Reads the part stored in `parent`'s child `local_name`.
pub(crate) fn read_part<T: Part>(parent: &Element, local_name: &str) -> Result<T, Error> {
    T::read(parent.child(local_name).unwrap_or(&MISSING))
}

/// Writes `part` into `parent`'s child `local_name`. A missing child is added
/// after those named in `after`, unless its absence already means `part`.
pub(crate) fn write_part<T: Part>(
    parent: &mut Element,
    local_name: &str,
    after: &[&str],
    part: &T,
) {
    let missing = parent.child(local_name).is_none();
    if missing && T::read(&MISSING).is_ok_and(|default| default == *part) {
        return;
    }
    part.write(child_or_insert(parent, local_name, after));
}

fn child_or_insert<'a>(
    parent: &'a mut Element,
    local_name: &str,
    after: &[&str],
) -> &'a mut Element {
    let index = match position(parent, local_name) {
        Some(index) => index,
        None => {
            let index = insertion_index(parent, after);
            let child = Element::new(qualified(parent, local_name));
            parent.children.insert(index, XmlNode::Element(child));
            index
        }
    };
    match &mut parent.children[index] {
        XmlNode::Element(child) => child,
        _ => unreachable!("`index` holds an element"),
    }
}

/// Replaces `parent`'s children called `local_name` with `elements`, in the
/// same place, or after the children named in `after` if there were none.
pub(crate) fn replace_children(
    parent: &mut Element,
    local_name: &str,
    after: &[&str],
    elements: Vec<Element>,
) {
    let index = position(parent, local_name).unwrap_or_else(|| insertion_index(parent, after));
    parent.children.retain(|node| {
        node.as_element()
            .is_none_or(|e| e.local_name() != local_name)
    });
    let elements = elements.into_iter().map(XmlNode::Element);
    parent.children.splice(index..index, elements);
}

fn position(parent: &Element, local_name: &str) -> Option<usize> {
    parent.children.iter().position(|node| {
        node.as_element()
            .is_some_and(|e| e.local_name() == local_name)
    })
}

fn insertion_index(parent: &Element, after: &[&str]) -> usize {
    parent
        .children
        .iter()
        .rposition(|node| {
            node.as_element()
                .is_some_and(|e| after.contains(&e.local_name()))
        })
        .map_or(0, |index| index + 1)
}

/// `local_name` with `parent`'s namespace prefix.
pub(crate) fn qualified(parent: &Element, local_name: &str) -> String {
    match parent.prefix() {
        Some(prefix) => format!("{prefix}:{local_name}"),
        None => local_name.to_owned(),
    }
}

#[cfg(test)]
mod tests;
