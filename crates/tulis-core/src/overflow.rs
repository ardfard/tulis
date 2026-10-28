//! The overflow panel's storage: text stashed outside the document.

use std::ops::Range;

use chrono::{DateTime, Utc};

use crate::model::{Document, OverflowId, OverflowItem};
use crate::pos::DocPos;

impl Document {
    /// Move the export text of `range` into overflow. Returns `None` if there is nothing to stash.
    pub fn to_overflow(&mut self, range: Range<DocPos>, now: DateTime<Utc>) -> Option<OverflowId> {
        let text = self.export_text_in(range.clone());
        if text.trim().is_empty() {
            return None;
        }
        let from = self.paragraphs.get(range.start.para).map(|p| p.id);
        self.delete(range);
        Some(self.push_overflow(text, now, from))
    }

    /// Move a whole paragraph into overflow. The document always keeps at least one paragraph.
    pub fn paragraph_to_overflow(&mut self, para: usize, now: DateTime<Utc>) -> Option<OverflowId> {
        let text = self.paragraphs[para].export_text();
        if text.trim().is_empty() {
            return None;
        }
        let removed = self.paragraphs.remove(para);
        if self.paragraphs.is_empty() {
            let p = self.new_paragraph("");
            self.paragraphs.push(p);
        }
        Some(self.push_overflow(text, now, Some(removed.id)))
    }

    fn push_overflow(
        &mut self,
        text: String,
        now: DateTime<Utc>,
        from: Option<crate::model::ParaId>,
    ) -> OverflowId {
        let id = OverflowId(self.fresh_id());
        self.overflow.push(OverflowItem {
            id,
            text,
            created_at: now,
            from_paragraph: from,
        });
        id
    }

    /// Insert an overflow item's text at `pos`. Returns the position after the inserted text.
    pub fn insert_overflow(&mut self, id: OverflowId, pos: DocPos, remove: bool) -> Option<DocPos> {
        let ix = self.overflow.iter().position(|o| o.id == id)?;
        let text = self.overflow[ix].text.clone();
        if remove {
            self.overflow.remove(ix);
        }
        Some(self.insert(pos, &text))
    }

    pub fn remove_overflow(&mut self, id: OverflowId) -> bool {
        let before = self.overflow.len();
        self.overflow.retain(|o| o.id != id);
        self.overflow.len() != before
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::model::Document;
    use crate::pos::DocPos;

    #[test]
    fn selection_moves_to_overflow() {
        let mut d = Document::from_texts(&["Keep this. Cut this."]);
        let id = d
            .to_overflow(DocPos::new(0, 10)..DocPos::new(0, 20), Utc::now())
            .unwrap();
        assert_eq!(d.paragraphs[0].text(), "Keep this.");
        assert_eq!(d.overflow.len(), 1);
        assert_eq!(d.overflow[0].id, id);
        assert_eq!(d.overflow[0].text, " Cut this.");
        assert_eq!(d.overflow[0].from_paragraph, Some(d.paragraphs[0].id));
    }

    #[test]
    fn overflow_text_excludes_ghosts() {
        let mut d = Document::from_texts(&["abc def"]);
        d.toggle_ghost(0, 0..4).unwrap();
        d.to_overflow(DocPos::new(0, 0)..DocPos::new(0, 7), Utc::now())
            .unwrap();
        assert_eq!(d.overflow[0].text, "def");
    }

    #[test]
    fn whitespace_only_selection_is_not_stashed() {
        let mut d = Document::from_texts(&["a  b"]);
        assert!(
            d.to_overflow(DocPos::new(0, 1)..DocPos::new(0, 3), Utc::now())
                .is_none()
        );
        assert_eq!(d.paragraphs[0].text(), "a  b");
    }

    #[test]
    fn paragraph_moves_to_overflow_and_document_never_empties() {
        let mut d = Document::from_texts(&["one", "two"]);
        d.paragraph_to_overflow(0, Utc::now()).unwrap();
        assert_eq!(d.paragraphs.len(), 1);
        assert_eq!(d.paragraphs[0].text(), "two");
        d.paragraph_to_overflow(0, Utc::now()).unwrap();
        assert_eq!(d.paragraphs.len(), 1);
        assert_eq!(d.paragraphs[0].text(), "");
        assert_eq!(d.overflow.len(), 2);
    }

    #[test]
    fn insert_overflow_inserts_and_optionally_removes() {
        let mut d = Document::from_texts(&["ab"]);
        let id = d.paragraph_to_overflow(0, Utc::now()).unwrap();
        let end = d.insert_overflow(id, DocPos::new(0, 0), false).unwrap();
        assert_eq!(d.paragraphs[0].text(), "ab");
        assert_eq!(end, DocPos::new(0, 2));
        assert_eq!(d.overflow.len(), 1);
        d.insert_overflow(id, end, true).unwrap();
        assert_eq!(d.paragraphs[0].text(), "abab");
        assert!(d.overflow.is_empty());
        assert!(d.insert_overflow(id, end, true).is_none());
    }

    #[test]
    fn remove_overflow() {
        let mut d = Document::from_texts(&["ab"]);
        let id = d.paragraph_to_overflow(0, Utc::now()).unwrap();
        assert!(d.remove_overflow(id));
        assert!(!d.remove_overflow(id));
    }
}
