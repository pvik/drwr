use std::collections::BTreeMap;

/// Counts of what a page lost in converting, for notes on it.
#[derive(Debug, Default)]
pub(super) struct Notes {
    /// Shapes yEd has nothing like, by draw.io name.
    pub unknown: BTreeMap<String, usize>,
    pub linked_images: usize,
    pub raster_images: usize,
    /// Images inside boxes, such as icons beside their text.
    pub box_images: usize,
    pub rotated: usize,
    /// Shapes draw.io points some way yEd can't.
    pub turned: usize,
    /// Swimlane titles draw.io writes upwards.
    pub vertical_titles: usize,
    pub dashed_lines: usize,
    pub hidden_layers: usize,
    pub hidden_shapes: usize,
    pub hidden_lines: usize,
    pub loose_ends: usize,
    /// Lines with an end on nothing drawn and no point to end at.
    pub unplaced_lines: usize,
    pub curved: usize,
    /// Labels whose rich text lost its formatting.
    pub formatting: usize,
}

impl Notes {
    pub fn sentences(&self) -> Vec<String> {
        let mut sentences = Vec::new();
        let mut say = |count: usize, sentence: &dyn Fn(usize) -> String| {
            if count > 0 {
                sentences.push(sentence(count));
            }
        };
        let unknown: usize = self.unknown.values().sum();
        say(unknown, &|n| {
            let mut names: Vec<(&String, &usize)> = self.unknown.iter().collect();
            names.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            let mut listed: Vec<String> = names
                .iter()
                .take(3)
                .map(|(name, count)| format!("{name} ({count})"))
                .collect();
            if names.len() > 3 {
                listed.push("…".to_owned());
            }
            format!(
                "{} yEd has nothing like became {}: {}.",
                count(n, "shape", "shapes"),
                plural(n, "a box", "boxes"),
                listed.join(", ")
            )
        });
        say(self.linked_images, &|n| {
            format!(
                "{} the file only links to became empty {}.",
                count(n, "image", "images"),
                plural(n, "box", "boxes")
            )
        });
        say(self.raster_images, &|n| {
            format!(
                "{} became empty {}.",
                count(n, "PNG or JPEG image", "PNG or JPEG images"),
                plural(n, "box", "boxes")
            )
        });
        say(self.box_images, &|n| {
            format!(
                "{} inside boxes {} left out.",
                count(n, "image", "images"),
                plural(n, "was", "were")
            )
        });
        say(self.rotated, &|n| {
            format!(
                "{} {} shown unrotated.",
                count(n, "rotated shape", "rotated shapes"),
                plural(n, "is", "are")
            )
        });
        say(self.turned, &|n| {
            format!(
                "{} {} the way yEd draws {}, not as draw.io turned {}.",
                count(n, "shape", "shapes"),
                plural(n, "points", "point"),
                plural(n, "it", "them"),
                plural(n, "it", "them")
            )
        });
        say(self.vertical_titles, &|n| {
            format!(
                "{} written upwards {} shown level.",
                count(n, "title", "titles"),
                plural(n, "is", "are")
            )
        });
        say(self.dashed_lines, &|n| {
            format!(
                "{} {} drawn solid.",
                count(n, "dashed line shape", "dashed line shapes"),
                plural(n, "is", "are")
            )
        });
        let hidden = self.hidden_shapes + self.hidden_lines;
        say(self.hidden_layers + hidden, &|_| {
            let mut parts = Vec::new();
            if self.hidden_layers > 0 {
                parts.push(count(self.hidden_layers, "hidden layer", "hidden layers"));
            }
            if hidden > 0 {
                parts.push(count(hidden, "hidden item", "hidden items"));
            }
            format!(
                "{} left out.",
                join_with_were(&parts, self.hidden_layers + hidden)
            )
        });
        say(self.loose_ends, &|n| {
            format!(
                "{} got {} to end on.",
                count(n, "loose line end", "loose line ends"),
                plural(n, "a small invisible node", "small invisible nodes")
            )
        });
        say(self.unplaced_lines, &|n| {
            format!(
                "{} with an end on nothing shown {} left out.",
                count(n, "line", "lines"),
                plural(n, "was", "were")
            )
        });
        say(self.curved, &|n| {
            format!(
                "{} {} drawn with straight segments.",
                count(n, "curved line", "curved lines"),
                plural(n, "is", "are")
            )
        });
        say(self.formatting, &|n| {
            format!(
                "{} lost {} rich-text formatting.",
                count(n, "label", "labels"),
                plural(n, "its", "their")
            )
        });
        sentences
    }
}

/// `n` and the noun for it, e.g. "1 shape" or "3 shapes".
fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", plural(n, one, many))
}

fn plural<'a>(n: usize, one: &'a str, many: &'a str) -> &'a str {
    if n == 1 { one } else { many }
}

/// "1 hidden layer and 2 hidden items were", or "… was" for one thing.
fn join_with_were(parts: &[String], total: usize) -> String {
    format!("{} {}", parts.join(" and "), plural(total, "was", "were"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sentences_count_what_was_lost() {
        let mut notes = Notes {
            rotated: 1,
            loose_ends: 3,
            hidden_layers: 1,
            hidden_shapes: 2,
            ..Notes::default()
        };
        notes.unknown.insert("mxgraph.aws4.lambda".into(), 3);
        notes.unknown.insert("actor".into(), 1);
        assert_eq!(
            notes.sentences(),
            [
                "4 shapes yEd has nothing like became boxes: mxgraph.aws4.lambda (3), actor (1).",
                "1 rotated shape is shown unrotated.",
                "1 hidden layer and 2 hidden items were left out.",
                "3 loose line ends got small invisible nodes to end on.",
            ]
        );
        assert!(Notes::default().sentences().is_empty());
    }
}
