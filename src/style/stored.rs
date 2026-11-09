//! How the settings file writes a style's colors and keywords, each `None`
//! until set. A value it can't read stays unset.

use graphml::yed::{Color, FontStyle, LineType};
use serde::{Deserialize, Deserializer, Serializer};

/// A color as `#RRGGBB` or `#RRGGBBAA`, or `"none"` for none.
pub mod color_or_none {
    use super::*;

    const NONE: &str = "none";

    pub fn serialize<S: Serializer>(
        value: &Option<Option<Color>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(Some(color)) => serializer.collect_str(color),
            Some(None) => serializer.serialize_str(NONE),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Option<Color>>, D::Error> {
        let text = String::deserialize(deserializer)?;
        Ok(match text.as_str() {
            NONE => Some(None),
            hex => Color::from_hex(hex).map(Some),
        })
    }
}

/// A color as `#RRGGBB` or `#RRGGBBAA`.
pub mod color {
    use super::*;

    pub fn serialize<S: Serializer>(
        value: &Option<Color>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(color) => serializer.collect_str(color),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Color>, D::Error> {
        Ok(Color::from_hex(&String::deserialize(deserializer)?))
    }
}

/// A yEd keyword, such as `"dashed"` for a line type.
pub mod keyword {
    use super::*;

    pub trait Keyword: for<'a> From<&'a str> {
        fn keyword(&self) -> &str;
        /// Whether it's one the graphml crate knows, which the side panel
        /// sets.
        fn is_known(&self) -> bool;
    }

    impl Keyword for LineType {
        fn keyword(&self) -> &str {
            self.as_str()
        }

        fn is_known(&self) -> bool {
            !matches!(self, LineType::Other(_))
        }
    }

    impl Keyword for FontStyle {
        fn keyword(&self) -> &str {
            self.as_str()
        }

        fn is_known(&self) -> bool {
            !matches!(self, FontStyle::Other(_))
        }
    }

    pub fn serialize<T: Keyword, S: Serializer>(
        value: &Option<T>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(value) => serializer.serialize_str(value.keyword()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, T: Keyword, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<T>, D::Error> {
        let value = T::from(String::deserialize(deserializer)?.as_str());
        Ok(value.is_known().then_some(value))
    }
}
