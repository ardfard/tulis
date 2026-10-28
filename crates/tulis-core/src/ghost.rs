//! Ghosting: dimming text so it can be judged without being deleted.

use std::ops::Range;

use crate::alternates::OpError;
use crate::model::{Document, normalize_segments};

impl Document {
    /// Set the ghost flag on `range`, grown to cover whole alternates. Returns that range.
    pub fn set_ghost(
        &mut self,
        para: usize,
        range: Range<usize>,
        ghost: bool,
    ) -> Result<Range<usize>, OpError> {
        if range.is_empty() {
            return Err(OpError::EmptySelection);
        }
        let p = &mut self.paragraphs[para];
        let (range, segs) = p.isolate(range);
        for seg in &mut p.segments_mut()[segs] {
            seg.ghost = ghost;
        }
        normalize_segments(p.segments_mut());
        Ok(range)
    }

    /// Ghost `range`, or revive it if it is entirely ghosted already.
    pub fn toggle_ghost(
        &mut self,
        para: usize,
        range: Range<usize>,
    ) -> Result<Range<usize>, OpError> {
        if range.is_empty() {
            return Err(OpError::EmptySelection);
        }
        let p = &self.paragraphs[para];
        let range = p.expand_to_alt_boundaries(range);
        let all_ghost = p
            .spans()
            .iter()
            .filter(|s| s.range.start < range.end && range.start < s.range.end)
            .all(|s| s.ghost);
        self.set_ghost(para, range, !all_ghost)
    }
}

#[cfg(test)]
mod tests {
    use crate::alternates::OpError;
    use crate::model::{Document, Segment};

    #[test]
    fn ghosting_hides_text_from_export_but_not_rendering() {
        let mut d = Document::from_texts(&["I really love tea"]);
        assert_eq!(d.toggle_ghost(0, 2..9), Ok(2..9));
        assert_eq!(d.paragraphs[0].text(), "I really love tea");
        assert_eq!(d.paragraphs[0].export_text(), "I love tea");
    }

    #[test]
    fn toggling_a_fully_ghosted_range_revives_it() {
        let mut d = Document::from_texts(&["I really love tea"]);
        d.toggle_ghost(0, 2..9).unwrap();
        d.toggle_ghost(0, 2..9).unwrap();
        assert_eq!(
            d.paragraphs[0].segments(),
            &[Segment::plain("I really love tea", false)]
        );
    }

    #[test]
    fn toggling_a_partly_ghosted_range_ghosts_all_of_it() {
        let mut d = Document::from_texts(&["abcdef"]);
        d.toggle_ghost(0, 1..3).unwrap();
        d.toggle_ghost(0, 0..6).unwrap();
        assert_eq!(
            d.paragraphs[0].segments(),
            &[Segment::plain("abcdef", true)]
        );
    }

    #[test]
    fn ghost_range_expands_around_alternates() {
        let mut d = Document::from_texts(&["I love tea"]);
        d.make_alt(0, 2..6).unwrap();
        assert_eq!(d.toggle_ghost(0, 4..8), Ok(2..8));
        assert_eq!(d.paragraphs[0].export_text(), "I ea");
    }

    #[test]
    fn empty_range_is_an_error() {
        let mut d = Document::from_texts(&["abc"]);
        assert_eq!(d.toggle_ghost(0, 1..1), Err(OpError::EmptySelection));
    }
}
