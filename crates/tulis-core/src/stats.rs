//! Word and character counts for the header, and the `# ` title marker.

use std::ops::Range;

use crate::{DocPos, Document};

/// A first paragraph starting with this renders as a bold title. The marker stays in the text.
pub const TITLE_MARKER: &str = "# ";

/// Byte length of the title marker at the start of `text` (0 when there is none).
pub fn title_marker_len(text: &str) -> usize {
    if text.starts_with(TITLE_MARKER) {
        TITLE_MARKER.len()
    } else {
        0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub words: usize,
    pub chars: usize,
}

impl Counts {
    /// Whitespace-separated words, and characters other than line breaks.
    pub fn of(text: &str) -> Self {
        Self {
            words: text.split_whitespace().count(),
            chars: text.chars().filter(|&c| c != '\n').count(),
        }
    }

    fn add(self, other: Self) -> Self {
        Self {
            words: self.words + other.words,
            chars: self.chars + other.chars,
        }
    }

    /// `"N words · M chars"`, with `" selected"` appended for a selection.
    pub fn label(self, selected: bool) -> String {
        let plural =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        let mut label = format!(
            "{} · {}",
            plural(self.words, "word", "words"),
            plural(self.chars, "char", "chars")
        );
        if selected {
            label.push_str(" selected");
        }
        label
    }
}

/// Counts for the whole document. Ghosts and the title marker are left out.
pub fn document_counts(doc: &Document) -> Counts {
    doc.paragraphs
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let text = p.export_text();
            let skip = if i == 0 { title_marker_len(&text) } else { 0 };
            Counts::of(&text[skip..])
        })
        .fold(Counts::default(), Counts::add)
}

/// Counts for `range`. Ghosts and the title marker are left out.
pub fn selection_counts(doc: &Document, range: Range<DocPos>) -> Counts {
    let mut start = range.start;
    if start.para == 0 {
        let marker = title_marker_len(&doc.paragraphs[0].text());
        start.offset = start.offset.max(marker);
    }
    if start >= range.end {
        return Counts::default();
    }
    Counts::of(&doc.export_text_in(start..range.end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_words_and_chars() {
        assert_eq!(
            Counts::of("one two  three"),
            Counts {
                words: 3,
                chars: 14
            }
        );
        assert_eq!(
            Counts::of("héllo\nwörld"),
            Counts {
                words: 2,
                chars: 10
            }
        );
        assert_eq!(Counts::of(""), Counts::default());
    }

    #[test]
    fn labels_pluralize_and_mark_selections() {
        assert_eq!(
            Counts { words: 1, chars: 1 }.label(false),
            "1 word · 1 char"
        );
        assert_eq!(
            Counts { words: 2, chars: 9 }.label(true),
            "2 words · 9 chars selected"
        );
    }

    #[test]
    fn document_counts_skip_ghosts_and_the_title_marker() {
        let mut doc = Document::from_texts(&["# My title", "I really love tea"]);
        doc.set_ghost(1, 2..9, true).unwrap();
        assert_eq!(
            document_counts(&doc),
            Counts {
                words: 5,
                chars: 18
            }
        );
    }

    #[test]
    fn selection_counts_skip_ghosts_and_the_title_marker() {
        let mut doc = Document::from_texts(&["# Title", "I really love tea"]);
        doc.set_ghost(1, 2..9, true).unwrap();
        let all = DocPos::new(0, 0)..DocPos::new(1, 17);
        assert_eq!(
            selection_counts(&doc, all),
            Counts {
                words: 4,
                chars: 15
            }
        );
        let marker_only = DocPos::new(0, 0)..DocPos::new(0, 2);
        assert_eq!(selection_counts(&doc, marker_only), Counts::default());
    }

    #[test]
    fn title_marker_needs_hash_and_space() {
        assert_eq!(title_marker_len("# Title"), 2);
        assert_eq!(title_marker_len("#Title"), 0);
        assert_eq!(title_marker_len("## Sub"), 0);
        assert_eq!(title_marker_len(""), 0);
    }
}
