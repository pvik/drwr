use std::collections::BTreeMap;

use graphml::yed::Color;

/// A cell's style: its `key=value` pairs over draw.io's default stylesheet.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Style {
    values: BTreeMap<String, String>,
    /// The named styles it uses, such as `ellipse` or `text`.
    pub names: Vec<String>,
}

/// What draw.io's default stylesheet (styles/default.xml) gives every
/// vertex, every edge, and each named style.
const DEFAULT_VERTEX: &[(&str, &str)] = &[
    ("shape", "label"),
    ("perimeter", "rectanglePerimeter"),
    ("fontSize", "12"),
    ("fontFamily", "Helvetica"),
    ("align", "center"),
    ("verticalAlign", "middle"),
    ("fillColor", "default"),
    ("strokeColor", "default"),
    ("fontColor", "default"),
];

const DEFAULT_EDGE: &[(&str, &str)] = &[
    ("shape", "connector"),
    ("labelBackgroundColor", "default"),
    ("endArrow", "classic"),
    ("fontSize", "11"),
    ("fontFamily", "Helvetica"),
    ("align", "center"),
    ("verticalAlign", "middle"),
    ("rounded", "1"),
    ("strokeColor", "default"),
    ("fontColor", "default"),
];

const TEXT: &[(&str, &str)] = &[
    ("fillColor", "none"),
    ("gradientColor", "none"),
    ("strokeColor", "none"),
    ("align", "left"),
    ("verticalAlign", "top"),
];

const LABEL: &[(&str, &str)] = &[
    ("fontStyle", "1"),
    ("align", "left"),
    ("verticalAlign", "middle"),
    ("spacing", "2"),
    ("spacingLeft", "52"),
    ("imageWidth", "42"),
    ("imageHeight", "42"),
    ("rounded", "1"),
];

const FANCY: &[(&str, &str)] = &[("shadow", "1"), ("glass", "1")];

const IMAGE: &[(&str, &str)] = &[
    ("shape", "image"),
    ("labelBackgroundColor", "default"),
    ("verticalAlign", "top"),
    ("verticalLabelPosition", "bottom"),
];

const GRAY: &[(&str, &str)] = &[
    ("gradientColor", "#B3B3B3"),
    ("fillColor", "#F5F5F5"),
    ("strokeColor", "#666666"),
];

const BLUE: &[(&str, &str)] = &[
    ("gradientColor", "#7EA6E0"),
    ("fillColor", "#DAE8FC"),
    ("strokeColor", "#6C8EBF"),
];

const GREEN: &[(&str, &str)] = &[
    ("gradientColor", "#97D077"),
    ("fillColor", "#D5E8D4"),
    ("strokeColor", "#82B366"),
];

const TURQUOISE: &[(&str, &str)] = &[
    ("gradientColor", "#67AB9F"),
    ("fillColor", "#D5E8D4"),
    ("strokeColor", "#6A9153"),
];

const YELLOW: &[(&str, &str)] = &[
    ("gradientColor", "#FFD966"),
    ("fillColor", "#FFF2CC"),
    ("strokeColor", "#D6B656"),
];

const ORANGE: &[(&str, &str)] = &[
    ("gradientColor", "#FFA500"),
    ("fillColor", "#FFCD28"),
    ("strokeColor", "#D79B00"),
];

const RED: &[(&str, &str)] = &[
    ("gradientColor", "#EA6B66"),
    ("fillColor", "#F8CECC"),
    ("strokeColor", "#B85450"),
];

const PINK: &[(&str, &str)] = &[
    ("gradientColor", "#B5739D"),
    ("fillColor", "#E6D0DE"),
    ("strokeColor", "#996185"),
];

const PURPLE: &[(&str, &str)] = &[
    ("gradientColor", "#8C6C9C"),
    ("fillColor", "#E1D5E7"),
    ("strokeColor", "#9673A6"),
];

fn named(name: &str) -> &'static [&'static [(&'static str, &'static str)]] {
    match name {
        "text" => &[TEXT],
        "edgeLabel" => &[
            TEXT,
            &[("labelBackgroundColor", "default"), ("fontSize", "11")],
        ],
        "label" => &[LABEL],
        "icon" => &[
            LABEL,
            &[
                ("align", "center"),
                ("imageAlign", "center"),
                ("verticalLabelPosition", "bottom"),
                ("verticalAlign", "top"),
                ("labelBackgroundColor", "default"),
                ("spacing", "0"),
                ("spacingLeft", "0"),
                ("spacingTop", "6"),
                ("fontStyle", "0"),
                ("imageWidth", "48"),
                ("imageHeight", "48"),
            ],
        ],
        "swimlane" => &[&[
            ("shape", "swimlane"),
            ("fontSize", "12"),
            ("fontStyle", "1"),
            ("startSize", "23"),
        ]],
        "group" => &[&[
            ("verticalAlign", "top"),
            ("fillColor", "none"),
            ("strokeColor", "none"),
            ("gradientColor", "none"),
            ("pointerEvents", "0"),
        ]],
        "ellipse" => &[&[("shape", "ellipse"), ("perimeter", "ellipsePerimeter")]],
        "rhombus" => &[&[("shape", "rhombus"), ("perimeter", "rhombusPerimeter")]],
        "triangle" => &[&[("shape", "triangle"), ("perimeter", "trianglePerimeter")]],
        "line" => &[&[
            ("shape", "line"),
            ("strokeWidth", "4"),
            ("labelBackgroundColor", "default"),
            ("verticalAlign", "top"),
            ("spacingTop", "8"),
        ]],
        "image" => &[IMAGE],
        "roundImage" => &[IMAGE, &[("perimeter", "ellipsePerimeter")]],
        "rhombusImage" => &[IMAGE, &[("perimeter", "rhombusPerimeter")]],
        "arrow" => &[&[
            ("shape", "arrow"),
            ("edgeStyle", "none"),
            ("fillColor", "default"),
        ]],
        "fancy" => &[FANCY],
        "gray" => &[FANCY, GRAY],
        "plain-gray" => &[GRAY],
        "blue" => &[FANCY, BLUE],
        "plain-blue" => &[BLUE],
        "green" => &[FANCY, GREEN],
        "plain-green" => &[GREEN],
        "turquoise" => &[FANCY, TURQUOISE],
        "plain-turquoise" => &[TURQUOISE],
        "yellow" => &[FANCY, YELLOW],
        "plain-yellow" => &[YELLOW],
        "orange" => &[FANCY, ORANGE],
        "plain-orange" => &[ORANGE],
        "red" => &[FANCY, RED],
        "plain-red" => &[RED],
        "pink" => &[FANCY, PINK],
        "plain-pink" => &[PINK],
        "purple" => &[FANCY, PURPLE],
        "plain-purple" => &[PURPLE],
        _ => &[],
    }
}

impl Style {
    /// The style a vertex or edge gets from its style string. Values of
    /// `inherit` come from `parent`.
    pub fn new(style: &str, edge: bool, parent: Option<&Style>) -> Self {
        let mut this = Style::default();
        // A leading ';' leaves out the defaults.
        if !style.starts_with(';') {
            this.merge(if edge { DEFAULT_EDGE } else { DEFAULT_VERTEX });
        }
        for part in style.split(';').filter(|part| !part.is_empty()) {
            match part.split_once('=') {
                Some((key, "inherit")) => {
                    let inherited = parent.and_then(|parent| parent.get(key));
                    match inherited {
                        Some(value) => this.set(key, value),
                        None => this.set(key, "none"),
                    }
                }
                Some((key, value)) => this.set(key, value),
                None => {
                    for pairs in named(part) {
                        this.merge(pairs);
                    }
                    this.names.push(part.to_owned());
                }
            }
        }
        // Colors that name another, which draw.io's renderer looks up.
        for key in ["fillColor", "strokeColor", "gradientColor", "fontColor"] {
            let (owner, named) = match this.get(key) {
                Some("parentFillColor") => (parent, "fillColor"),
                Some("parentStrokeColor") => (parent, "strokeColor"),
                Some(name @ ("fillColor" | "strokeColor")) if name != key => (Some(&this), name),
                _ => continue,
            };
            let value = match owner.and_then(|owner| owner.get(named)) {
                Some("default") if named == "fillColor" => "#FFFFFF",
                Some("default") => "#000000",
                Some(value) => value,
                None => "none",
            }
            .to_owned();
            this.set(key, &value);
        }
        this
    }

    fn merge(&mut self, pairs: &[(&str, &str)]) {
        for (key, value) in pairs {
            self.set(key, value);
        }
    }

    /// Sets `key`, or removes it for `none`, as draw.io does.
    fn set(&mut self, key: &str, value: &str) {
        if value == "none" {
            self.values.remove(key);
        } else {
            self.values.insert(key.to_owned(), value.to_owned());
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    pub fn is(&self, key: &str, value: &str) -> bool {
        self.get(key) == Some(value)
    }

    /// Whether `key` is 1, as draw.io writes true and tests for it.
    pub fn flag(&self, key: &str) -> bool {
        self.parsed(key) == Some(1.0)
    }

    /// Whether `key` is 1, or `default` if it's not set.
    pub fn flag_or(&self, key: &str, default: bool) -> bool {
        self.get(key).map_or(default, |_| self.flag(key))
    }

    pub fn number(&self, key: &str, default: f64) -> f64 {
        self.parsed(key).unwrap_or(default)
    }

    /// The number under `key`, if it's one.
    pub fn parsed(&self, key: &str) -> Option<f64> {
        let value = self.get(key)?.trim().parse::<f64>().ok();
        value.filter(|value| value.is_finite())
    }

    /// The color under `key`; `None` if there's none. `default` is the color
    /// draw.io's light theme gives `default`.
    pub fn color(&self, key: &str, default: Color) -> Option<Color> {
        match self.get(key)? {
            "default" => Some(default),
            value => parse_color(value),
        }
    }
}

/// Parses a CSS color as draw.io writes them: `#RRGGBB`, `#RGB`, `rgb()`,
/// a few names, or `light-dark(light, dark)`, taking the light one.
pub(crate) fn parse_color(text: &str) -> Option<Color> {
    let text = text.trim();
    if let Some(inner) = text
        .strip_prefix("light-dark(")
        .and_then(|rest| rest.strip_suffix(')'))
    {
        return parse_color(first_argument(inner)?);
    }
    if let Some(hex) = text.strip_prefix('#') {
        return match hex.len() {
            3 | 4 => {
                let long: String = hex.chars().flat_map(|c| [c, c]).collect();
                Color::from_hex(&format!("#{long}"))
            }
            _ => Color::from_hex(text),
        };
    }
    if let Some(args) = (text
        .strip_prefix("rgba(")
        .or_else(|| text.strip_prefix("rgb(")))
    .and_then(|rest| rest.strip_suffix(')'))
    {
        let parts: Vec<&str> = args.split(',').map(str::trim).collect();
        let channel = |i: usize| parts.get(i)?.parse::<f64>().ok();
        let byte = |value: f64| value.clamp(0.0, 255.0).round() as u8;
        let alpha = parts.get(3).map_or(Some(1.0), |a| a.parse::<f64>().ok())?;
        return Some(Color {
            r: byte(channel(0)?),
            g: byte(channel(1)?),
            b: byte(channel(2)?),
            a: byte(alpha * 255.0),
        });
    }
    let rgb = match text.to_ascii_lowercase().as_str() {
        "black" => 0x000000,
        "white" => 0xFFFFFF,
        "red" => 0xFF0000,
        "green" => 0x008000,
        "blue" => 0x0000FF,
        "yellow" => 0xFFFF00,
        "orange" => 0xFFA500,
        "purple" => 0x800080,
        "gray" | "grey" => 0x808080,
        "lightgray" | "lightgrey" => 0xD3D3D3,
        "darkgray" | "darkgrey" => 0xA9A9A9,
        _ => return None,
    };
    let [_, r, g, b] = u32::to_be_bytes(rgb);
    Some(Color::rgb(r, g, b))
}

/// `a` of `a, b`, splitting at the first comma outside parentheses.
fn first_argument(text: &str) -> Option<&str> {
    let mut depth = 0;
    for (i, c) in text.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => return Some(&text[..i]),
            _ => {}
        }
    }
    None
}
