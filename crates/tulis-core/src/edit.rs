//! Text editing on the document: insert, delete, split and join.

use std::ops::Range;

use crate::model::{Document, Paragraph, Segment, normalize_segments};
use crate::pos::DocPos;

enum InsertAt {
    Into { seg: usize, at: usize },
    NewPlainBefore(usize),
}

impl Paragraph {
    fn insertion_point(&self, offset: usize) -> InsertAt {
        let ranges = self.segment_ranges();
        let segs = self.segments();
        // Strictly inside an alternate or a ghost: edit it.
        for (i, r) in ranges.iter().enumerate() {
            if r.start < offset && offset < r.end && (segs[i].is_alt() || segs[i].ghost) {
                return InsertAt::Into {
                    seg: i,
                    at: offset - r.start,
                };
            }
        }
        // Touching a normal plain segment (inside or at an edge).
        for (i, r) in ranges.iter().enumerate() {
            if !segs[i].is_alt() && !segs[i].ghost && r.start <= offset && offset <= r.end {
                return InsertAt::Into {
                    seg: i,
                    at: offset - r.start,
                };
            }
        }
        // Only alternates or ghosts touch this offset: start a new plain segment here.
        let before = ranges
            .iter()
            .position(|r| r.start >= offset && !r.is_empty())
            .unwrap_or(ranges.len());
        InsertAt::NewPlainBefore(before)
    }

    /// Insert text that contains no newline at `offset`.
    pub(crate) fn insert_text(&mut self, offset: usize, text: &str) {
        debug_assert!(!text.contains('\n'));
        if text.is_empty() {
            return;
        }
        match self.insertion_point(offset) {
            InsertAt::Into { seg, at } => self.segments_mut()[seg].text_mut().insert_str(at, text),
            InsertAt::NewPlainBefore(ix) => {
                self.segments_mut().insert(ix, Segment::plain(text, false))
            }
        }
        normalize_segments(self.segments_mut());
    }

    /// Delete a byte range of the rendered text. Alternates whose active text becomes empty are removed.
    pub(crate) fn delete_text(&mut self, range: Range<usize>) {
        if range.is_empty() {
            return;
        }
        let ranges = self.segment_ranges();
        let segs = self.segments_mut();
        for (i, r) in ranges.iter().enumerate() {
            let start = range.start.max(r.start);
            let end = range.end.min(r.end);
            if start < end {
                segs[i]
                    .text_mut()
                    .replace_range(start - r.start..end - r.start, "");
            }
        }
        segs.retain(|s| !(s.is_alt() && s.text().is_empty()));
        normalize_segments(segs);
    }

    /// Make sure a segment boundary exists at `offset`. A segment strictly containing it is split;
    /// an alternate strictly containing it is collapsed to its active text first.
    pub(crate) fn split_segments_at(&mut self, offset: usize) {
        let ranges = self.segment_ranges();
        if let Some(i) = ranges
            .iter()
            .position(|r| r.start < offset && offset < r.end)
        {
            let start = ranges[i].start;
            let segs = self.segments_mut();
            let ghost = segs[i].ghost;
            let text = segs[i].text().to_string();
            let (left, right) = text.split_at(offset - start);
            segs.splice(
                i..=i,
                [Segment::plain(left, ghost), Segment::plain(right, ghost)],
            );
        }
    }
}

impl Document {
    pub fn end_pos(&self) -> DocPos {
        let last = self.paragraphs.len() - 1;
        DocPos::new(last, self.paragraphs[last].len())
    }

    /// Clamp to an existing paragraph and a char boundary at or before the offset.
    pub fn clamp(&self, pos: DocPos) -> DocPos {
        let para = pos.para.min(self.paragraphs.len() - 1);
        let text = self.paragraphs[para].text();
        let mut offset = pos.offset.min(text.len());
        while !text.is_char_boundary(offset) {
            offset -= 1;
        }
        DocPos::new(para, offset)
    }

    /// Insert text at `pos` (newlines split paragraphs). Returns the position after the text.
    pub fn insert(&mut self, pos: DocPos, text: &str) -> DocPos {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let mut lines = text.split('\n');
        let first = lines.next().unwrap_or("");
        let rest: Vec<&str> = lines.collect();
        if rest.is_empty() {
            self.paragraphs[pos.para].insert_text(pos.offset, first);
            return DocPos::new(pos.para, pos.offset + first.len());
        }
        let mut cur = self.split_paragraph(pos);
        let left = cur.para - 1;
        let left_len = self.paragraphs[left].len();
        self.paragraphs[left].insert_text(left_len, first);
        let (last, middle) = rest.split_last().expect("rest is non-empty");
        for line in middle {
            let p = self.new_paragraph(*line);
            self.paragraphs.insert(cur.para, p);
            cur.para += 1;
        }
        self.paragraphs[cur.para].insert_text(0, last);
        DocPos::new(cur.para, last.len())
    }

    /// Split the paragraph at `pos`. Returns the start of the second paragraph.
    pub fn split_paragraph(&mut self, pos: DocPos) -> DocPos {
        let len = self.paragraphs[pos.para].len();
        if pos.offset == 0 && len > 0 {
            let p = self.new_paragraph("");
            self.paragraphs.insert(pos.para, p);
            return DocPos::new(pos.para + 1, 0);
        }
        if pos.offset >= len {
            let p = self.new_paragraph("");
            self.paragraphs.insert(pos.para + 1, p);
            return DocPos::new(pos.para + 1, 0);
        }
        let right = {
            let para = &mut self.paragraphs[pos.para];
            para.keep_only_active_version();
            para.split_segments_at(pos.offset);
            let ix = para
                .segment_ranges()
                .iter()
                .position(|r| r.start >= pos.offset)
                .expect("a segment starts at the split offset");
            let right = para.segments_mut().split_off(ix);
            normalize_segments(para.segments_mut());
            right
        };
        let mut new_para = self.new_paragraph("");
        *new_para.segments_mut() = right;
        normalize_segments(new_para.segments_mut());
        self.paragraphs.insert(pos.para + 1, new_para);
        DocPos::new(pos.para + 1, 0)
    }

    /// Delete `range` and return its start.
    pub fn delete(&mut self, range: Range<DocPos>) -> DocPos {
        let Range { start, end } = range;
        if start >= end {
            return start;
        }
        if start.para == end.para {
            self.paragraphs[start.para].delete_text(start.offset..end.offset);
            return start;
        }
        let start_len = self.paragraphs[start.para].len();
        self.paragraphs[start.para].delete_text(start.offset..start_len);
        self.paragraphs[end.para].delete_text(0..end.offset);
        let tail = self.paragraphs.remove(end.para);
        self.paragraphs.drain(start.para + 1..end.para);
        let left = &mut self.paragraphs[start.para];
        left.keep_only_active_version();
        left.segments_mut().extend(tail.segments().iter().cloned());
        normalize_segments(left.segments_mut());
        start
    }

    /// Rendered text in `range`, paragraphs joined by `\n`.
    pub fn text_in(&self, range: Range<DocPos>) -> String {
        self.collect_in(range, |p, s, e| p.text()[s..e].to_string())
    }

    /// Like `text_in`, but ghosted text is left out.
    pub fn export_text_in(&self, range: Range<DocPos>) -> String {
        self.collect_in(range, |p, s, e| {
            let text = p.text();
            p.spans()
                .iter()
                .filter(|span| !span.ghost)
                .filter_map(|span| {
                    let a = span.range.start.max(s);
                    let b = span.range.end.min(e);
                    (a < b).then(|| &text[a..b])
                })
                .collect()
        })
    }

    fn collect_in(
        &self,
        range: Range<DocPos>,
        slice: impl Fn(&Paragraph, usize, usize) -> String,
    ) -> String {
        let mut out = String::new();
        for i in range.start.para..=range.end.para {
            let p = &self.paragraphs[i];
            let start = if i == range.start.para {
                range.start.offset
            } else {
                0
            };
            let end = if i == range.end.para {
                range.end.offset
            } else {
                p.len()
            };
            if i > range.start.para {
                out.push('\n');
            }
            out.push_str(&slice(p, start, end));
        }
        out
    }

    /// The whole document's rendered text, paragraphs joined by `\n`.
    pub fn flat_text(&self) -> String {
        self.paragraphs
            .iter()
            .map(Paragraph::text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn flat_offset(&self, pos: DocPos) -> usize {
        self.paragraphs[..pos.para]
            .iter()
            .map(|p| p.len() + 1)
            .sum::<usize>()
            + pos.offset
    }

    pub fn pos_from_flat(&self, flat: usize) -> DocPos {
        let mut rest = flat;
        for (i, p) in self.paragraphs.iter().enumerate() {
            let len = p.len();
            if rest <= len {
                return self.clamp(DocPos::new(i, rest));
            }
            rest -= len + 1;
        }
        self.end_pos()
    }
}

#[cfg(test)]
mod tests {
    use crate::model::test_util::*;
    use crate::model::*;
    use crate::pos::DocPos;

    fn doc_with(segments: Vec<Segment>) -> Document {
        let mut d = Document::new();
        d.paragraphs[0] = para_with(segments);
        d
    }

    fn texts(d: &Document) -> Vec<String> {
        d.paragraphs.iter().map(|p| p.text()).collect()
    }

    #[test]
    fn insert_into_plain() {
        let mut d = Document::from_texts(&["helo"]);
        let p = d.insert(DocPos::new(0, 3), "l");
        assert_eq!(d.paragraphs[0].text(), "hello");
        assert_eq!(p, DocPos::new(0, 4));
    }

    #[test]
    fn insert_at_alt_edge_goes_to_plain() {
        let mut d = doc_with(vec![
            Segment::plain("I ", false),
            alt_seg(&["love"], 0),
            Segment::plain(" tea", false),
        ]);
        d.insert(DocPos::new(0, 6), "!");
        let segs = d.paragraphs[0].segments();
        assert_eq!(segs[1].text(), "love");
        assert_eq!(segs[2], Segment::plain("! tea", false));
    }

    #[test]
    fn insert_at_alt_edge_without_plain_creates_plain() {
        let mut d = doc_with(vec![alt_seg(&["love"], 0)]);
        d.insert(DocPos::new(0, 4), "!");
        assert_eq!(
            d.paragraphs[0].segments(),
            &[alt_seg(&["love"], 0), Segment::plain("!", false)]
        );
        d.insert(DocPos::new(0, 0), "I ");
        assert_eq!(d.paragraphs[0].segments()[0], Segment::plain("I ", false));
    }

    #[test]
    fn insert_strictly_inside_alt_edits_active_option() {
        let mut d = doc_with(vec![alt_seg(&["love", "adore"], 1)]);
        d.insert(DocPos::new(0, 2), "-");
        match &d.paragraphs[0].segments()[0].kind {
            SegKind::Alt { options, active } => {
                assert_eq!(*active, 1);
                assert_eq!(options[0].text, "love");
                assert_eq!(options[1].text, "ad-ore");
            }
            other => panic!("expected alt, got {other:?}"),
        }
    }

    #[test]
    fn typing_after_a_ghost_is_not_ghosted() {
        let mut d = doc_with(vec![Segment::plain("a", false), Segment::plain("b", true)]);
        d.insert(DocPos::new(0, 2), "c");
        assert_eq!(
            d.paragraphs[0].segments(),
            &[
                Segment::plain("a", false),
                Segment::plain("b", true),
                Segment::plain("c", false)
            ]
        );
    }

    #[test]
    fn insert_newline_splits_paragraph() {
        let mut d = Document::from_texts(&["hello world"]);
        let p = d.insert(DocPos::new(0, 5), "\n");
        assert_eq!(texts(&d), vec!["hello", " world"]);
        assert_eq!(p, DocPos::new(1, 0));
    }

    #[test]
    fn insert_multiline_text() {
        let mut d = Document::from_texts(&["ab"]);
        let p = d.insert(DocPos::new(0, 1), "1\n2\n3");
        assert_eq!(texts(&d), vec!["a1", "2", "3b"]);
        assert_eq!(p, DocPos::new(2, 1));
    }

    #[test]
    fn insert_normalizes_crlf() {
        let mut d = Document::from_texts(&[""]);
        d.insert(DocPos::new(0, 0), "a\r\nb");
        assert_eq!(texts(&d), vec!["a", "b"]);
    }

    #[test]
    fn enter_at_start_and_end_keeps_original_paragraph_id() {
        let mut d = Document::from_texts(&["abc"]);
        let id = d.paragraphs[0].id;
        assert_eq!(d.split_paragraph(DocPos::new(0, 0)), DocPos::new(1, 0));
        assert_eq!(d.paragraphs[1].id, id);
        assert_eq!(texts(&d), vec!["", "abc"]);
        assert_eq!(d.split_paragraph(DocPos::new(1, 3)), DocPos::new(2, 0));
        assert_eq!(d.paragraphs[1].id, id);
        assert_eq!(texts(&d), vec!["", "abc", ""]);
    }

    #[test]
    fn split_inside_alt_collapses_it() {
        let mut d = doc_with(vec![alt_seg(&["love", "adore"], 1)]);
        d.split_paragraph(DocPos::new(0, 2));
        assert_eq!(texts(&d), vec!["ad", "ore"]);
        assert!(!d.paragraphs[0].has_metadata());
        assert!(!d.paragraphs[1].has_metadata());
    }

    #[test]
    fn delete_within_paragraph_removes_emptied_alt() {
        let mut d = doc_with(vec![
            Segment::plain("I ", false),
            alt_seg(&["love"], 0),
            Segment::plain(" tea", false),
        ]);
        d.delete(DocPos::new(0, 2)..DocPos::new(0, 6));
        assert_eq!(
            d.paragraphs[0].segments(),
            &[Segment::plain("I  tea", false)]
        );
    }

    #[test]
    fn delete_part_of_alt_edits_active_option() {
        let mut d = doc_with(vec![alt_seg(&["love", "adore"], 1)]);
        d.delete(DocPos::new(0, 0)..DocPos::new(0, 2));
        assert_eq!(d.paragraphs[0].text(), "ore");
        assert!(d.paragraphs[0].segments()[0].is_alt());
    }

    #[test]
    fn delete_across_paragraphs_joins() {
        let mut d = Document::from_texts(&["hello", "big", "world"]);
        let p = d.delete(DocPos::new(0, 4)..DocPos::new(2, 1));
        assert_eq!(texts(&d), vec!["hellorld"]);
        assert_eq!(p, DocPos::new(0, 4));
    }

    #[test]
    fn text_in_and_export_text_in() {
        let mut d = Document::from_texts(&["ab", "cd"]);
        d.paragraphs[1]
            .segments_mut()
            .insert(0, Segment::plain("X", true));
        assert_eq!(d.text_in(DocPos::new(0, 1)..DocPos::new(1, 2)), "b\nXc");
        assert_eq!(
            d.export_text_in(DocPos::new(0, 1)..DocPos::new(1, 2)),
            "b\nc"
        );
    }

    #[test]
    fn flat_offsets_round_trip_with_multibyte() {
        let d = Document::from_texts(&["héllo", "wörld"]);
        assert_eq!(d.flat_text(), "héllo\nwörld");
        let pos = DocPos::new(1, 3);
        let flat = d.flat_offset(pos);
        assert_eq!(flat, 7 + 3);
        assert_eq!(d.pos_from_flat(flat), pos);
        assert_eq!(d.pos_from_flat(6), DocPos::new(0, 6));
        assert_eq!(d.pos_from_flat(999), d.end_pos());
    }

    #[test]
    fn clamp_floors_to_char_boundary_and_valid_paragraph() {
        let d = Document::from_texts(&["héllo"]);
        assert_eq!(d.clamp(DocPos::new(0, 2)), DocPos::new(0, 1));
        assert_eq!(d.clamp(DocPos::new(5, 99)), DocPos::new(0, 6));
    }
}
