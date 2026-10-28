//! Alternates: word-level option groups and paragraph versions.

use std::ops::Range;

use crate::model::{
    AltOption, Document, ParaVersion, Paragraph, SegKind, Segment, Source, normalize_segments,
};
use crate::pos::DocPos;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum OpError {
    #[error("select some text first")]
    EmptySelection,
    #[error("selection must be within one paragraph")]
    MultiParagraph,
    #[error("nothing to do here")]
    NothingHere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AltLevel {
    Word,
    Paragraph,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AltInfo {
    pub level: AltLevel,
    pub index: usize,
    pub count: usize,
    pub span: Range<usize>,
}

impl Paragraph {
    /// The alternate at `offset`; edges count as inside and the earlier group wins.
    pub fn alt_at(&self, offset: usize) -> Option<usize> {
        let ranges = self.segment_ranges();
        (0..ranges.len()).find(|&i| {
            self.segments()[i].is_alt() && ranges[i].start <= offset && offset <= ranges[i].end
        })
    }

    /// The alternate whose span starts at `start` (the group being edited may be empty).
    pub fn alt_starting_at(&self, start: usize) -> Option<usize> {
        let ranges = self.segment_ranges();
        (0..ranges.len()).find(|&i| self.segments()[i].is_alt() && ranges[i].start == start)
    }

    /// Grow `range` so neither edge falls strictly inside an alternate.
    pub fn expand_to_alt_boundaries(&self, range: Range<usize>) -> Range<usize> {
        let (mut start, mut end) = (range.start, range.end);
        for (seg, r) in self.segments().iter().zip(self.segment_ranges()) {
            if seg.is_alt() {
                if r.start < start && start < r.end {
                    start = r.start;
                }
                if r.start < end && end < r.end {
                    end = r.end;
                }
            }
        }
        start..end
    }

    /// Ensure segment boundaries at the edges of `range` (after expanding it around alternates).
    /// Returns the expanded byte range and the indices of the segments covering it.
    pub(crate) fn isolate(&mut self, range: Range<usize>) -> (Range<usize>, Range<usize>) {
        let range = self.expand_to_alt_boundaries(range);
        self.split_segments_at(range.end);
        self.split_segments_at(range.start);
        let ranges = self.segment_ranges();
        let first = ranges
            .iter()
            .position(|r| r.start >= range.start && !r.is_empty())
            .unwrap_or(ranges.len());
        let end = ranges
            .iter()
            .rposition(|r| r.end <= range.end && !r.is_empty())
            .map_or(first, |i| i + 1);
        (range, first..end.max(first))
    }
}

impl Document {
    /// Find the alternate exactly covering `range`, or turn `range` into a one-option alternate.
    /// Returns the segment index.
    fn ensure_alt(&mut self, para: usize, range: Range<usize>) -> Result<usize, OpError> {
        if range.is_empty() {
            return Err(OpError::EmptySelection);
        }
        let p = &mut self.paragraphs[para];
        let ranges = p.segment_ranges();
        if let Some(i) = (0..ranges.len()).find(|&i| p.segments()[i].is_alt() && ranges[i] == range)
        {
            return Ok(i);
        }
        let (range, segs) = p.isolate(range);
        let removed: Vec<Segment> = p.segments_mut().drain(segs.clone()).collect();
        let text: String = removed.iter().map(Segment::text).collect();
        let ghost = removed.iter().all(|s| s.ghost);
        p.segments_mut().insert(
            segs.start,
            Segment::alt(vec![AltOption::human(text)], 0, ghost),
        );
        normalize_segments(p.segments_mut());
        Ok(p.alt_starting_at(range.start)
            .expect("alternate was just inserted"))
    }

    /// Make an alternate from a selection (or add an option to the group it exactly covers).
    /// The new option is a copy of the current text and becomes active. Returns the group's span.
    pub fn make_alt(&mut self, para: usize, range: Range<usize>) -> Result<Range<usize>, OpError> {
        let seg = self.ensure_alt(para, range)?;
        let p = &mut self.paragraphs[para];
        if let SegKind::Alt { options, active } = &mut p.segments_mut()[seg].kind {
            let copy = AltOption::human(options[*active].text.clone());
            options.push(copy);
            *active = options.len() - 1;
        }
        Ok(p.segment_ranges()[seg].clone())
    }

    /// Append AI-written options to the group at `range` (creating it) and show the first one.
    pub fn add_ai_alternates(
        &mut self,
        para: usize,
        range: Range<usize>,
        texts: Vec<String>,
        source: Source,
    ) -> Result<Range<usize>, OpError> {
        if texts.is_empty() {
            return Err(OpError::NothingHere);
        }
        let seg = self.ensure_alt(para, range)?;
        let p = &mut self.paragraphs[para];
        if let SegKind::Alt { options, active } = &mut p.segments_mut()[seg].kind {
            *active = options.len();
            options.extend(texts.into_iter().map(|text| AltOption {
                text,
                source: source.clone(),
            }));
        }
        Ok(p.segment_ranges()[seg].clone())
    }

    /// Clone the active version as a new, active version. Returns its index.
    pub fn make_para_version(&mut self, para: usize) -> usize {
        let p = &mut self.paragraphs[para];
        let segments = p.segments().to_vec();
        p.versions.push(ParaVersion {
            segments,
            source: Source::Human,
        });
        p.active = p.versions.len() - 1;
        p.active
    }

    /// Append AI-written paragraph versions and show the first one.
    pub fn add_ai_versions(
        &mut self,
        para: usize,
        texts: Vec<String>,
        source: Source,
    ) -> Result<(), OpError> {
        if texts.is_empty() {
            return Err(OpError::NothingHere);
        }
        let p = &mut self.paragraphs[para];
        p.active = p.versions.len();
        p.versions.extend(texts.into_iter().map(|text| ParaVersion {
            segments: vec![Segment::plain(text, false)],
            source: source.clone(),
        }));
        Ok(())
    }

    pub fn alt_info(&self, pos: DocPos) -> Option<AltInfo> {
        let p = self.paragraphs.get(pos.para)?;
        if let Some(i) = p.alt_at(pos.offset) {
            if let SegKind::Alt { options, active } = &p.segments()[i].kind {
                return Some(AltInfo {
                    level: AltLevel::Word,
                    index: *active,
                    count: options.len(),
                    span: p.segment_ranges()[i].clone(),
                });
            }
        }
        (p.versions.len() > 1).then(|| AltInfo {
            level: AltLevel::Paragraph,
            index: p.active,
            count: p.versions.len(),
            span: 0..p.len(),
        })
    }

    /// Cycle the alternate at `pos`, or else the paragraph's versions.
    pub fn cycle(&mut self, pos: DocPos, forward: bool) -> Option<AltInfo> {
        let step = |i: usize, n: usize| {
            if forward {
                (i + 1) % n
            } else {
                (i + n - 1) % n
            }
        };
        let p = &mut self.paragraphs[pos.para];
        if let Some(i) = p.alt_at(pos.offset) {
            let start = p.segment_ranges()[i].start;
            if let SegKind::Alt { options, active } = &mut p.segments_mut()[i].kind {
                *active = step(*active, options.len());
                let len = options[*active].text.len();
                return Some(AltInfo {
                    level: AltLevel::Word,
                    index: *active,
                    count: options.len(),
                    span: start..start + len,
                });
            }
        }
        if p.versions.len() > 1 {
            p.active = step(p.active, p.versions.len());
            return Some(AltInfo {
                level: AltLevel::Paragraph,
                index: p.active,
                count: p.versions.len(),
                span: 0..p.len(),
            });
        }
        None
    }

    /// Collapse the alternate at `pos` to its active option, or drop the other versions.
    pub fn accept(&mut self, pos: DocPos) -> bool {
        let p = &mut self.paragraphs[pos.para];
        if let Some(i) = p.alt_at(pos.offset) {
            let seg = &mut p.segments_mut()[i];
            seg.kind = SegKind::Plain(seg.text().to_string());
            normalize_segments(p.segments_mut());
            return true;
        }
        if p.versions.len() > 1 {
            p.keep_only_active_version();
            return true;
        }
        false
    }

    /// Delete the active option (or version) at `pos`. Returns the affected span afterwards.
    pub fn delete_option(&mut self, pos: DocPos) -> Option<Range<usize>> {
        let p = &mut self.paragraphs[pos.para];
        if let Some(i) = p.alt_at(pos.offset) {
            let start = p.segment_ranges()[i].start;
            let seg = &mut p.segments_mut()[i];
            let remaining = match &mut seg.kind {
                SegKind::Alt { options, active } => {
                    if options.len() > 1 {
                        options.remove(*active);
                        *active = (*active).min(options.len() - 1);
                    }
                    (options.len() == 1).then(|| options[0].text.clone())
                }
                SegKind::Plain(_) => None,
            };
            if let Some(text) = remaining {
                seg.kind = SegKind::Plain(text);
            }
            let len = seg.text().len();
            normalize_segments(p.segments_mut());
            return Some(start..start + len);
        }
        if p.versions.len() > 1 {
            let active = p.active;
            p.versions.remove(active);
            p.active = active.min(p.versions.len() - 1);
            return Some(0..p.len());
        }
        None
    }

    /// Replace `local` (a byte range in the active option of alternate `seg`) with `text`.
    /// Unlike `delete`, the option may become empty; call `finish_alt_edit` when editing ends.
    pub fn edit_alt(&mut self, para: usize, seg: usize, local: Range<usize>, text: &str) {
        self.paragraphs[para].segments_mut()[seg]
            .text_mut()
            .replace_range(local, text);
    }

    /// End alternate editing: drop an empty active option (showing the previous one), and turn a
    /// group left with a single option into plain text.
    pub fn finish_alt_edit(&mut self, para: usize, seg: usize) {
        let Some(p) = self.paragraphs.get_mut(para) else {
            return;
        };
        let Some(segment) = p.segments_mut().get_mut(seg) else {
            return;
        };
        let collapse = match &mut segment.kind {
            SegKind::Alt { options, active } => {
                if options[*active].text.is_empty() && options.len() > 1 {
                    options.remove(*active);
                    *active = (*active).saturating_sub(1);
                }
                (options.len() == 1).then(|| options[0].text.clone())
            }
            SegKind::Plain(_) => None,
        };
        if let Some(text) = collapse {
            segment.kind = SegKind::Plain(text);
        }
        normalize_segments(p.segments_mut());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::AiKind;

    fn doc() -> Document {
        Document::from_texts(&["I love tea"])
    }

    fn options(d: &Document, seg: usize) -> (Vec<String>, usize) {
        match &d.paragraphs[0].segments()[seg].kind {
            SegKind::Alt { options, active } => {
                (options.iter().map(|o| o.text.clone()).collect(), *active)
            }
            other => panic!("expected alt, got {other:?}"),
        }
    }

    fn ai() -> Source {
        Source::Ai {
            provider: "mock".into(),
            model: "m".into(),
            kind: AiKind::Rewrite,
        }
    }

    #[test]
    fn make_alt_from_selection_creates_group_with_active_copy() {
        let mut d = doc();
        assert_eq!(d.make_alt(0, 2..6), Ok(2..6));
        assert_eq!(options(&d, 1), (vec!["love".into(), "love".into()], 1));
        assert_eq!(d.paragraphs[0].text(), "I love tea");
    }

    #[test]
    fn make_alt_on_existing_group_adds_option() {
        let mut d = doc();
        d.make_alt(0, 2..6).unwrap();
        d.make_alt(0, 2..6).unwrap();
        assert_eq!(options(&d, 1).0.len(), 3);
        assert_eq!(options(&d, 1).1, 2);
    }

    #[test]
    fn make_alt_rejects_empty_selection() {
        assert_eq!(doc().make_alt(0, 3..3), Err(OpError::EmptySelection));
    }

    #[test]
    fn make_alt_overlapping_an_alt_expands_to_cover_it() {
        let mut d = doc();
        d.make_alt(0, 2..6).unwrap();
        assert_eq!(d.make_alt(0, 4..10), Ok(2..10));
        assert_eq!(d.paragraphs[0].text(), "I love tea");
        assert_eq!(
            options(&d, 1).0,
            vec!["love tea".to_string(), "love tea".to_string()]
        );
    }

    #[test]
    fn edit_alt_then_cycle_word_level() {
        let mut d = doc();
        d.make_alt(0, 2..6).unwrap();
        d.edit_alt(0, 1, 0..4, "adore");
        assert_eq!(d.paragraphs[0].text(), "I adore tea");
        let info = d.cycle(DocPos::new(0, 3), true).unwrap();
        assert_eq!(
            (info.level, info.index, info.count, info.span),
            (AltLevel::Word, 0, 2, 2..6)
        );
        assert_eq!(d.paragraphs[0].text(), "I love tea");
    }

    #[test]
    fn cycle_falls_back_to_paragraph_versions() {
        let mut d = doc();
        assert!(d.cycle(DocPos::new(0, 0), true).is_none());
        assert_eq!(d.make_para_version(0), 1);
        let info = d.cycle(DocPos::new(0, 0), false).unwrap();
        assert_eq!(
            (info.level, info.index, info.count),
            (AltLevel::Paragraph, 0, 2)
        );
    }

    #[test]
    fn alt_info_reports_group_under_cursor() {
        let mut d = doc();
        d.make_alt(0, 2..6).unwrap();
        let info = d.alt_info(DocPos::new(0, 6)).unwrap();
        assert_eq!(
            (info.level, info.index, info.count, info.span),
            (AltLevel::Word, 1, 2, 2..6)
        );
        assert!(d.alt_info(DocPos::new(0, 8)).is_none());
    }

    #[test]
    fn accept_collapses_alt_to_active_text() {
        let mut d = doc();
        d.make_alt(0, 2..6).unwrap();
        d.edit_alt(0, 1, 0..4, "adore");
        assert!(d.accept(DocPos::new(0, 3)));
        assert_eq!(
            d.paragraphs[0].segments(),
            &[Segment::plain("I adore tea", false)]
        );
    }

    #[test]
    fn accept_drops_other_paragraph_versions() {
        let mut d = doc();
        d.make_para_version(0);
        assert!(d.accept(DocPos::new(0, 0)));
        assert_eq!(d.paragraphs[0].versions.len(), 1);
        assert!(!d.accept(DocPos::new(0, 0)));
    }

    #[test]
    fn delete_option_collapses_group_left_with_one_option() {
        let mut d = doc();
        d.make_alt(0, 2..6).unwrap();
        d.edit_alt(0, 1, 0..4, "adore");
        assert_eq!(d.delete_option(DocPos::new(0, 3)), Some(2..6));
        assert_eq!(
            d.paragraphs[0].segments(),
            &[Segment::plain("I love tea", false)]
        );
    }

    #[test]
    fn delete_option_removes_active_version() {
        let mut d = doc();
        d.make_para_version(0);
        assert_eq!(d.delete_option(DocPos::new(0, 0)), Some(0..10));
        assert_eq!(d.paragraphs[0].versions.len(), 1);
    }

    #[test]
    fn finish_alt_edit_removes_empty_option_and_collapses() {
        let mut d = doc();
        d.make_alt(0, 2..6).unwrap();
        d.edit_alt(0, 1, 0..4, "");
        assert_eq!(d.paragraphs[0].text(), "I  tea");
        assert_eq!(d.paragraphs[0].alt_starting_at(2), Some(1));
        d.finish_alt_edit(0, 1);
        assert_eq!(d.paragraphs[0].text(), "I love tea");
        assert!(!d.paragraphs[0].has_metadata());
    }

    #[test]
    fn finish_alt_edit_keeps_group_with_several_options() {
        let mut d = doc();
        d.make_alt(0, 2..6).unwrap();
        d.edit_alt(0, 1, 0..4, "adore");
        d.finish_alt_edit(0, 1);
        assert_eq!(options(&d, 1), (vec!["love".into(), "adore".into()], 1));
    }

    #[test]
    fn ai_alternates_append_and_activate_first_new() {
        let mut d = doc();
        let span = d.add_ai_alternates(0, 2..6, vec!["adore".into(), "like".into()], ai());
        assert_eq!(span, Ok(2..7));
        assert_eq!(d.paragraphs[0].text(), "I adore tea");
        assert!(d.paragraphs[0].segments()[1].active_is_ai());
        assert_eq!(options(&d, 1).0.len(), 3);
    }

    #[test]
    fn ai_versions_become_active() {
        let mut d = doc();
        d.add_ai_versions(0, vec!["Tea is love.".into()], ai())
            .unwrap();
        assert_eq!(d.paragraphs[0].text(), "Tea is love.");
        assert_eq!(
            (d.paragraphs[0].active, d.paragraphs[0].versions.len()),
            (1, 2)
        );
        assert_eq!(
            d.add_ai_versions(0, vec![], ai()),
            Err(OpError::NothingHere)
        );
    }
}
