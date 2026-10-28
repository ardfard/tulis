//! The document model: paragraphs made of segments, with alternates and ghosts.

use std::ops::Range;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ParaId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OverflowId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AiKind {
    Rewrite,
    Trim,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Source {
    Human,
    Ai {
        provider: String,
        model: String,
        kind: AiKind,
    },
}

impl Source {
    pub fn is_ai(&self) -> bool {
        matches!(self, Source::Ai { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AltOption {
    pub text: String,
    pub source: Source,
}

impl AltOption {
    pub fn human(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            source: Source::Human,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SegKind {
    Plain(String),
    Alt {
        options: Vec<AltOption>,
        active: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Segment {
    pub kind: SegKind,
    pub ghost: bool,
}

impl Segment {
    pub fn plain(text: impl Into<String>, ghost: bool) -> Self {
        Self {
            kind: SegKind::Plain(text.into()),
            ghost,
        }
    }

    pub fn alt(options: Vec<AltOption>, active: usize, ghost: bool) -> Self {
        Self {
            kind: SegKind::Alt { options, active },
            ghost,
        }
    }

    /// The visible text: the plain string, or the active option's text.
    pub fn text(&self) -> &str {
        match &self.kind {
            SegKind::Plain(text) => text,
            SegKind::Alt { options, active } => &options[*active].text,
        }
    }

    pub fn text_mut(&mut self) -> &mut String {
        match &mut self.kind {
            SegKind::Plain(text) => text,
            SegKind::Alt { options, active } => &mut options[*active].text,
        }
    }

    pub fn is_alt(&self) -> bool {
        matches!(self.kind, SegKind::Alt { .. })
    }

    /// True when this is an alternate whose active option came from an AI.
    pub fn active_is_ai(&self) -> bool {
        match &self.kind {
            SegKind::Alt { options, active } => options[*active].source.is_ai(),
            SegKind::Plain(_) => false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParaVersion {
    pub segments: Vec<Segment>,
    pub source: Source,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Paragraph {
    pub id: ParaId,
    pub versions: Vec<ParaVersion>,
    pub active: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AltMark {
    pub ai: bool,
}

/// A run of a paragraph's rendered text with uniform styling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub ghost: bool,
    pub alt: Option<AltMark>,
}

impl Paragraph {
    pub fn new(id: ParaId, text: impl Into<String>) -> Self {
        let mut segments = vec![Segment::plain(text, false)];
        normalize_segments(&mut segments);
        Self {
            id,
            versions: vec![ParaVersion {
                segments,
                source: Source::Human,
            }],
            active: 0,
        }
    }

    pub fn segments(&self) -> &[Segment] {
        &self.versions[self.active].segments
    }

    pub fn segments_mut(&mut self) -> &mut Vec<Segment> {
        &mut self.versions[self.active].segments
    }

    /// Rendered text of the active version, ghosts included.
    pub fn text(&self) -> String {
        self.segments().iter().map(Segment::text).collect()
    }

    /// Rendered text without ghosted segments.
    pub fn export_text(&self) -> String {
        self.segments()
            .iter()
            .filter(|s| !s.ghost)
            .map(Segment::text)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.segments().iter().map(|s| s.text().len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Byte range of each segment of the active version within the rendered text.
    pub fn segment_ranges(&self) -> Vec<Range<usize>> {
        let mut start = 0;
        self.segments()
            .iter()
            .map(|s| {
                let range = start..start + s.text().len();
                start = range.end;
                range
            })
            .collect()
    }

    pub fn spans(&self) -> Vec<Span> {
        self.segments()
            .iter()
            .zip(self.segment_ranges())
            .filter(|(_, range)| !range.is_empty())
            .map(|(seg, range)| Span {
                range,
                ghost: seg.ghost,
                alt: seg.is_alt().then(|| AltMark {
                    ai: seg.active_is_ai(),
                }),
            })
            .collect()
    }

    /// Whether the paragraph carries anything that plain Markdown would lose.
    pub fn has_metadata(&self) -> bool {
        self.versions.len() > 1
            || self
                .versions
                .iter()
                .any(|v| v.segments.iter().any(|s| s.ghost || s.is_alt()))
    }

    pub(crate) fn keep_only_active_version(&mut self) {
        let version = self.versions[self.active].clone();
        self.versions = vec![version];
        self.active = 0;
    }
}

/// Drop empty plain segments and merge adjacent plain segments with the same ghost flag.
/// An empty list becomes a single empty plain segment.
pub fn normalize_segments(segments: &mut Vec<Segment>) {
    let mut out: Vec<Segment> = Vec::with_capacity(segments.len());
    for seg in segments.drain(..) {
        if let SegKind::Plain(text) = &seg.kind {
            if text.is_empty() {
                continue;
            }
            if let Some(Segment {
                kind: SegKind::Plain(prev),
                ghost,
            }) = out.last_mut()
            {
                if *ghost == seg.ghost {
                    prev.push_str(text);
                    continue;
                }
            }
        }
        out.push(seg);
    }
    if out.is_empty() {
        out.push(Segment::plain("", false));
    }
    *segments = out;
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OverflowItem {
    pub id: OverflowId,
    pub text: String,
    pub created_at: DateTime<Utc>,
    pub from_paragraph: Option<ParaId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub paragraphs: Vec<Paragraph>,
    pub overflow: Vec<OverflowItem>,
    next_id: u64,
}

impl Default for Document {
    fn default() -> Self {
        Self::new()
    }
}

impl Document {
    pub fn new() -> Self {
        Self::from_texts::<&str>(&[])
    }

    pub fn from_texts<S: AsRef<str>>(texts: &[S]) -> Self {
        let mut doc = Self {
            paragraphs: Vec::new(),
            overflow: Vec::new(),
            next_id: 1,
        };
        for text in texts {
            let p = doc.new_paragraph(text.as_ref());
            doc.paragraphs.push(p);
        }
        if doc.paragraphs.is_empty() {
            let p = doc.new_paragraph("");
            doc.paragraphs.push(p);
        }
        doc
    }

    pub fn fresh_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Creates (but does not insert) a paragraph with a fresh id.
    pub fn new_paragraph(&mut self, text: impl Into<String>) -> Paragraph {
        Paragraph::new(ParaId(self.fresh_id()), text)
    }

    pub fn para_index(&self, id: ParaId) -> Option<usize> {
        self.paragraphs.iter().position(|p| p.id == id)
    }

    /// Export text of every paragraph whose export text is non-empty.
    pub fn export_paragraphs(&self) -> Vec<String> {
        self.paragraphs
            .iter()
            .map(Paragraph::export_text)
            .filter(|t| !t.is_empty())
            .collect()
    }

    pub fn word_count(&self) -> usize {
        self.paragraphs
            .iter()
            .map(|p| p.export_text().split_whitespace().count())
            .sum()
    }
}

#[cfg(test)]
pub(crate) mod test_util {
    use super::*;

    pub(crate) fn alt_seg(texts: &[&str], active: usize) -> Segment {
        Segment::alt(
            texts.iter().map(|t| AltOption::human(*t)).collect(),
            active,
            false,
        )
    }

    pub(crate) fn para_with(segments: Vec<Segment>) -> Paragraph {
        Paragraph {
            id: ParaId(99),
            versions: vec![ParaVersion {
                segments,
                source: Source::Human,
            }],
            active: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_util::*;
    use super::*;

    #[test]
    fn normalize_merges_and_drops_empty_plain() {
        let mut segs = vec![
            Segment::plain("a", false),
            Segment::plain("", true),
            Segment::plain("b", false),
            Segment::plain("c", true),
        ];
        normalize_segments(&mut segs);
        assert_eq!(
            segs,
            vec![Segment::plain("ab", false), Segment::plain("c", true)]
        );
    }

    #[test]
    fn normalize_keeps_one_empty_plain() {
        let mut segs = vec![];
        normalize_segments(&mut segs);
        assert_eq!(segs, vec![Segment::plain("", false)]);
    }

    #[test]
    fn normalize_does_not_merge_across_alternates() {
        let mut segs = vec![
            Segment::plain("a", false),
            alt_seg(&["x"], 0),
            Segment::plain("b", false),
        ];
        normalize_segments(&mut segs);
        assert_eq!(segs.len(), 3);
    }

    #[test]
    fn text_and_export_text() {
        let p = para_with(vec![
            Segment::plain("I ", false),
            alt_seg(&["love", "adore"], 1),
            Segment::plain(" very much", true),
            Segment::plain(" tea.", false),
        ]);
        assert_eq!(p.text(), "I adore very much tea.");
        assert_eq!(p.export_text(), "I adore tea.");
        assert_eq!(p.len(), 22);
    }

    #[test]
    fn spans_mark_ghosts_and_alts() {
        let p = para_with(vec![
            Segment::plain("I ", false),
            alt_seg(&["love"], 0),
            Segment::plain("!", true),
        ]);
        assert_eq!(
            p.spans(),
            vec![
                Span {
                    range: 0..2,
                    ghost: false,
                    alt: None
                },
                Span {
                    range: 2..6,
                    ghost: false,
                    alt: Some(AltMark { ai: false })
                },
                Span {
                    range: 6..7,
                    ghost: true,
                    alt: None
                },
            ]
        );
    }

    #[test]
    fn has_metadata_detects_alts_ghosts_and_versions() {
        assert!(!para_with(vec![Segment::plain("a", false)]).has_metadata());
        assert!(para_with(vec![Segment::plain("a", true)]).has_metadata());
        assert!(para_with(vec![alt_seg(&["a"], 0)]).has_metadata());
    }

    #[test]
    fn from_texts_assigns_unique_ids_and_is_never_empty() {
        let d = Document::from_texts(&["a", "b"]);
        assert_eq!(d.paragraphs.len(), 2);
        assert_ne!(d.paragraphs[0].id, d.paragraphs[1].id);
        assert_eq!(Document::new().paragraphs.len(), 1);
        assert_eq!(Document::new().paragraphs[0].text(), "");
    }

    #[test]
    fn word_count_ignores_ghosts() {
        let mut d = Document::from_texts(&["one two"]);
        d.paragraphs[0]
            .segments_mut()
            .push(Segment::plain(" three", true));
        assert_eq!(d.word_count(), 2);
    }

    #[test]
    fn document_round_trips_through_json() {
        let d = Document::from_texts(&["hello"]);
        let json = serde_json::to_string(&d).unwrap();
        let back: Document = serde_json::from_str(&json).unwrap();
        assert_eq!(back, d);
    }
}
