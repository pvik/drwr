//! Turns draw.io's HTML labels into plain text.

use std::mem;

/// A label's plain text, and the font style its markup gives all of it.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Text {
    pub text: String,
    pub bold: bool,
    pub italic: bool,
    /// Whether markup other than line breaks and whole-label bold or italic
    /// was dropped.
    pub lost_formatting: bool,
}

/// The text of an HTML label, broken into lines where the HTML breaks them.
pub(crate) fn from_html(html: &str) -> Text {
    let mut out = Writer::default();
    let mut rest = html;
    while let Some(i) = rest.find(['<', '&']) {
        out.text(&rest[..i]);
        rest = &rest[i..];
        if rest.starts_with('&') {
            let (decoded, len) = entity(rest);
            out.chars(&decoded);
            rest = &rest[len..];
            continue;
        }
        match tag(rest) {
            Some((tag, len)) => {
                out.tag(&tag);
                rest = &rest[len..];
            }
            None => {
                out.chars("<");
                rest = &rest[1..];
            }
        }
    }
    out.text(rest);
    out.finish()
}

/// The text of a plain label.
pub(crate) fn from_plain(text: &str) -> Text {
    Text {
        text: text.replace("\r\n", "\n"),
        ..Text::default()
    }
}

#[derive(Debug, PartialEq)]
struct Tag {
    name: String,
    closing: bool,
    /// The attributes, as written.
    attributes: String,
}

/// Parses the tag, comment or declaration at the start of `text`, returning
/// it (`None` for comments) and its length.
fn tag(text: &str) -> Option<(Tag, usize)> {
    if let Some(comment) = text.strip_prefix("<!--") {
        let end = comment.find("-->").map_or(text.len(), |i| i + 7);
        return Some((Tag::ignored(), end));
    }
    if text.starts_with("<!") || text.starts_with("<?") {
        return Some((Tag::ignored(), text.find('>')? + 1));
    }
    let inner = &text[1..];
    let closing = inner.starts_with('/');
    let inner = inner.strip_prefix('/').unwrap_or(inner);
    let name_len = inner
        .find(|c: char| !c.is_ascii_alphanumeric())
        .unwrap_or(inner.len());
    if name_len == 0 {
        return None;
    }
    // The end, skipping `>` in quoted attribute values.
    let mut quote = None;
    for (i, c) in inner[name_len..].char_indices() {
        match (c, quote) {
            ('"' | '\'', None) => quote = Some(c),
            (c, Some(q)) if c == q => quote = None,
            ('>', None) => {
                let attributes = inner[name_len..name_len + i].trim_end_matches('/');
                let len = text.len() - inner.len() + name_len + i + 1;
                let tag = Tag {
                    name: inner[..name_len].to_ascii_lowercase(),
                    closing,
                    attributes: attributes.trim().to_owned(),
                };
                return Some((tag, len));
            }
            _ => {}
        }
    }
    None
}

impl Tag {
    fn ignored() -> Self {
        Tag {
            name: String::new(),
            closing: false,
            attributes: String::new(),
        }
    }
}

/// Decodes the entity at the start of `text`, returning it and its length;
/// an unknown one stays as written.
fn entity(text: &str) -> (String, usize) {
    let end = text[1..]
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '#')
        .map_or(text.len(), |i| i + 1);
    let name = &text[1..end];
    let terminated = text[end..].starts_with(';');
    let len = end + usize::from(terminated);
    let decoded = if let Some(number) = name.strip_prefix('#') {
        let code = match number.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok(),
            None => number.parse().ok(),
        };
        code.and_then(char::from_u32).map(String::from)
    } else if terminated {
        named_entity(name).map(String::from)
    } else {
        None
    };
    match decoded {
        Some(decoded) => (decoded, len),
        None => ("&".to_owned(), 1),
    }
}

fn named_entity(name: &str) -> Option<&'static str> {
    Some(match name {
        "nbsp" => "\u{a0}",
        "amp" => "&",
        "lt" => "<",
        "gt" => ">",
        "quot" => "\"",
        "apos" => "'",
        "copy" => "©",
        "reg" => "®",
        "trade" => "™",
        "hellip" => "…",
        "mdash" => "—",
        "ndash" => "–",
        "lsquo" => "‘",
        "rsquo" => "’",
        "ldquo" => "“",
        "rdquo" => "”",
        "laquo" => "«",
        "raquo" => "»",
        "bull" => "•",
        "middot" => "·",
        "deg" => "°",
        "times" => "×",
        "divide" => "÷",
        "plusmn" => "±",
        "euro" => "€",
        "pound" => "£",
        "yen" => "¥",
        "cent" => "¢",
        "sect" => "§",
        "para" => "¶",
        "larr" => "←",
        "rarr" => "→",
        "uarr" => "↑",
        "darr" => "↓",
        "harr" => "↔",
        "lArr" => "⇐",
        "rArr" => "⇒",
        "hArr" => "⇔",
        "le" => "≤",
        "ge" => "≥",
        "ne" => "≠",
        "zwnj" | "zwj" | "shy" => "",
        _ => return None,
    })
}

#[derive(Default)]
struct Writer {
    lines: Vec<String>,
    line: String,
    /// Whitespace waits to become one space until more text follows.
    space: bool,
    bold: usize,
    italic: usize,
    /// Inside `<style>` or `<script>`, whose text isn't shown.
    hidden: usize,
    any_text: bool,
    all_bold: bool,
    all_italic: bool,
    some_bold: bool,
    some_italic: bool,
    lost_formatting: bool,
}

const BLOCKS: &[&str] = &[
    "div",
    "p",
    "li",
    "ul",
    "ol",
    "tr",
    "table",
    "tbody",
    "thead",
    "tfoot",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "blockquote",
    "pre",
    "hr",
    "section",
    "article",
    "header",
    "footer",
    "dl",
    "dt",
    "dd",
    "center",
];

/// Tags whose formatting plain text can't keep.
const FORMATTING: &[&str] = &[
    "font", "u", "s", "strike", "del", "ins", "sup", "sub", "h1", "h2", "h3", "h4", "h5", "h6",
    "mark", "code", "pre", "small", "big", "img", "a",
];

/// CSS properties that change how text looks.
const STYLED: &[&str] = &[
    "color",
    "font-size",
    "font-family",
    "font-weight",
    "font-style",
    "text-decoration",
    "background-color",
    "background",
];

impl Writer {
    /// Adds text, collapsing its whitespace as HTML does.
    fn text(&mut self, text: &str) {
        for piece in text.split_inclusive(|c: char| c.is_ascii_whitespace()) {
            let word = piece.trim_end_matches(|c: char| c.is_ascii_whitespace());
            self.chars(word);
            if word.len() < piece.len() {
                self.space = true;
            }
        }
    }

    /// Adds text whose whitespace is kept.
    fn chars(&mut self, text: &str) {
        if text.is_empty() || self.hidden > 0 {
            return;
        }
        if mem::take(&mut self.space) && !self.line.is_empty() {
            self.line.push(' ');
        }
        self.line.push_str(text);
        if text.chars().any(|c| !c.is_whitespace()) {
            if !self.any_text {
                self.any_text = true;
                self.all_bold = true;
                self.all_italic = true;
            }
            self.all_bold &= self.bold > 0;
            self.all_italic &= self.italic > 0;
            self.some_bold |= self.bold > 0;
            self.some_italic |= self.italic > 0;
        }
    }

    fn tag(&mut self, tag: &Tag) {
        let name = tag.name.as_str();
        let depth = |count: &mut usize| match tag.closing {
            true => *count = count.saturating_sub(1),
            false => *count += 1,
        };
        match name {
            "" => return,
            "br" => {
                self.space = false;
                self.lines.push(mem::take(&mut self.line));
                return;
            }
            "b" | "strong" => depth(&mut self.bold),
            "i" | "em" => depth(&mut self.italic),
            "style" | "script" | "head" | "title" => depth(&mut self.hidden),
            "td" | "th" if !tag.closing && !self.line.is_empty() => self.space = true,
            _ => {}
        }
        if BLOCKS.contains(&name) {
            self.space = false;
            if !self.line.is_empty() {
                self.lines.push(mem::take(&mut self.line));
            }
            if name == "li" && !tag.closing {
                self.line.push_str("• ");
            }
        }
        if !tag.closing && (FORMATTING.contains(&name) || styled(&tag.attributes)) {
            self.lost_formatting = true;
        }
    }

    fn finish(mut self) -> Text {
        if !self.line.is_empty() {
            self.lines.push(mem::take(&mut self.line));
        }
        let bold = self.any_text && self.all_bold;
        let italic = self.any_text && self.all_italic;
        // Bold or italic on part of the text is formatting lost.
        let partly = (self.some_bold && !bold) || (self.some_italic && !italic);
        Text {
            text: self.lines.join("\n").replace('\u{a0}', " "),
            bold,
            italic,
            lost_formatting: self.lost_formatting || partly,
        }
    }
}

/// Whether a tag's `style` attribute changes how its text looks.
fn styled(attributes: &str) -> bool {
    let Some(start) = attributes.find("style=") else {
        return false;
    };
    let value = attributes[start + 6..].trim_start_matches(['"', '\'']);
    let value = value.split(['"', '\'']).next().unwrap_or_default();
    value.split(';').any(|declaration| {
        let Some((property, value)) = declaration.split_once(':') else {
            return false;
        };
        let value = value.trim().to_ascii_lowercase();
        STYLED.contains(&property.trim().to_ascii_lowercase().as_str())
            && !matches!(
                value.as_str(),
                "initial" | "inherit" | "unset" | "normal" | "transparent" | "none" | ""
            )
    })
}

#[cfg(test)]
mod tests;
