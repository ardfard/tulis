//! Word and character counts for the header, and the `# ` title marker.

use std::collections::HashMap;
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
            // The title is detected on the full text (as rendering does), so a ghosted `#`
            // still hides the marker.
            let text = p.text();
            let skip = if i == 0 { title_marker_len(&text) } else { 0 };
            let range = DocPos::new(i, skip)..DocPos::new(i, text.len());
            Counts::of(&doc.export_text_in(range))
        })
        .fold(Counts::default(), Counts::add)
}

/// The document's word count, as `document_counts` gives it, with `cuts` left out as well. Each
/// cut is a paragraph index and a byte range of that paragraph's rendered text (the Lab's trim
/// preview counts the words that would remain).
pub fn words_without(doc: &Document, cuts: &[(usize, Range<usize>)]) -> usize {
    words_without_grouped(doc, &group_cuts(cuts))
}

/// `cuts` by paragraph index, ready for `words_without_grouped`.
pub fn group_cuts(cuts: &[(usize, Range<usize>)]) -> HashMap<usize, Vec<Range<usize>>> {
    let mut by_para: HashMap<usize, Vec<Range<usize>>> = HashMap::new();
    for (para, range) in cuts {
        by_para.entry(*para).or_default().push(range.clone());
    }
    by_para
}

/// `words_without` with the cuts already grouped by paragraph: one pass over the document, with
/// no search through every cut for every paragraph.
pub fn words_without_grouped(doc: &Document, cuts: &HashMap<usize, Vec<Range<usize>>>) -> usize {
    doc.paragraphs
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let text = p.text();
            let skip = if i == 0 { title_marker_len(&text) } else { 0 };
            // Ghosts and cuts together, by start, so one sweep over the text decides each char.
            let mut left_out: Vec<Range<usize>> = p
                .spans()
                .into_iter()
                .filter(|s| s.ghost)
                .map(|s| s.range)
                .chain(cuts.get(&i).into_iter().flatten().cloned())
                .collect();
            left_out.sort_by_key(|r| r.start);
            let (mut next, mut covered_to) = (0, 0);
            let kept: String = text
                .char_indices()
                .filter(|(at, _)| {
                    while next < left_out.len() && left_out[next].start <= *at {
                        covered_to = covered_to.max(left_out[next].end);
                        next += 1;
                    }
                    *at >= skip && *at >= covered_to
                })
                .map(|(_, c)| c)
                .collect();
            Counts::of(&kept).words
        })
        .sum()
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
    fn words_without_leaves_out_cuts_ghosts_and_the_title_marker() {
        let mut d = Document::from_texts(&["# My title", "I really love green tea.", "Cut me."]);
        d.toggle_ghost(1, 14..20).unwrap(); // "green "
        assert_eq!(document_counts(&d).words, 8);
        assert_eq!(words_without(&d, &[]), 8);
        assert_eq!(words_without(&d, &[(1, 2..9), (2, 0..7)]), 5);
        // Part of a word leaves the rest of it.
        assert_eq!(words_without(&d, &[(1, 3..5)]), 8);
    }

    #[test]
    fn the_grouped_count_matches_the_flat_one() {
        let mut d = Document::from_texts(&["# My title", "I really love green tea.", "Cut me."]);
        d.toggle_ghost(1, 14..20).unwrap();
        // Overlapping and out-of-order cuts in one paragraph, and a paragraph with none.
        let cuts = [(2, 0..3), (1, 12..20), (1, 2..9), (1, 5..11), (2, 4..7)];
        let grouped = group_cuts(&cuts);
        assert_eq!(grouped[&1].len(), 3);
        assert_eq!(
            words_without_grouped(&d, &grouped),
            words_without(&d, &cuts)
        );
        assert_eq!(words_without_grouped(&d, &grouped), 4);
        assert_eq!(words_without_grouped(&d, &HashMap::new()), 8);
    }

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
    fn document_counts_detect_the_title_with_a_ghosted_hash() {
        let mut doc = Document::from_texts(&["# My title"]);
        doc.set_ghost(0, 0..1, true).unwrap();
        assert_eq!(document_counts(&doc), Counts { words: 2, chars: 8 });
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
