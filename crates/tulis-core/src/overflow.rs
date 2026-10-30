//! The Overflow scratchpad: free text kept outside the page, in blocks separated by blank lines.

use std::ops::Range;

use crate::model::Document;
use crate::pos::DocPos;

fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

/// The non-blank lines of `text`, trimmed at both ends and joined by `\n`: a stash is always
/// one block.
fn as_block(text: &str) -> String {
    text.lines()
        .filter(|line| !is_blank(line))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// The whitespace at the end of `text` if it holds a line break: (its bytes, its line breaks).
fn trailing_break(text: &str) -> (usize, usize) {
    let tail = &text[text.trim_end().len()..];
    match tail.matches('\n').count() {
        0 => (0, 0),
        breaks => (tail.len(), breaks),
    }
}

/// The whitespace at the start of `text` if it holds a line break: (its bytes, its line breaks).
fn leading_break(text: &str) -> (usize, usize) {
    let head = &text[..text.len() - text.trim_start().len()];
    match head.matches('\n').count() {
        0 => (0, 0),
        breaks => (head.len(), breaks),
    }
}

/// Bytes of text kept on each side of a take, to tell later whether the text there changed.
const CONTEXT: usize = 32;

/// Up to `CONTEXT` bytes of `text` ending at byte `at` (a char boundary).
fn context_before(text: &str, at: usize) -> &str {
    let mut start = at.saturating_sub(CONTEXT);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..at]
}

/// Up to `CONTEXT` bytes of `text` starting at byte `at` (a char boundary).
fn context_after(text: &str, at: usize) -> &str {
    let mut end = (at + CONTEXT).min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[at..end]
}

/// A change a page command made to the scratchpad, kept with its page undo step. The
/// scratchpad's own edits aren't undo steps, so undo and redo replay these on the scratchpad as
/// it is then, rather than restoring a saved copy of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OverflowOp {
    /// `text` was appended as a block (a stash).
    Stash { text: String },
    /// `text` was taken out (Ctrl+Enter, a drop): at byte `offset`, `removed` (the text and the
    /// line breaks joined around it) became `joined`. `before` and `after` are the text on
    /// either side then.
    Take {
        offset: usize,
        text: String,
        removed: String,
        joined: String,
        before: String,
        after: String,
    },
}

impl OverflowOp {
    /// The text stashed or taken.
    pub fn text(&self) -> &str {
        match self {
            OverflowOp::Stash { text } | OverflowOp::Take { text, .. } => text,
        }
    }
}

/// How replaying an op changed the scratchpad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Replayed {
    /// As recorded.
    Exact,
    /// The text went in as a new block at the end.
    Appended,
    /// The text came out of a block found elsewhere.
    Moved,
    /// Nothing: the text there had been edited.
    Kept,
}

impl Document {
    /// Move the export text of `range` to the end of the scratchpad, after a blank line, as one
    /// block. An empty range stashes (and removes) the paragraph it is in. Returns what it did
    /// to the scratchpad, or None, changing nothing, when there is only whitespace to stash.
    pub fn stash(&mut self, range: Range<DocPos>) -> Option<OverflowOp> {
        if range.start == range.end {
            let para = range.start.para;
            return self.stash_paragraphs(para..para + 1);
        }
        let text = as_block(&self.export_text_in(range.clone()));
        if text.is_empty() {
            return None;
        }
        self.delete(range);
        self.append_overflow(&text);
        Some(OverflowOp::Stash { text })
    }

    /// Move whole paragraphs to the scratchpad as one block, removing them from the page. The
    /// document always keeps at least one paragraph.
    pub fn stash_paragraphs(&mut self, paras: Range<usize>) -> Option<OverflowOp> {
        let end = paras.end.min(self.paragraphs.len());
        let paras = paras.start.min(end)..end;
        let lines: Vec<String> = self.paragraphs[paras.clone()]
            .iter()
            .map(|p| p.export_text())
            .collect();
        let text = as_block(&lines.join("\n"));
        if text.is_empty() {
            return None;
        }
        self.paragraphs.drain(paras);
        if self.paragraphs.is_empty() {
            let p = self.new_paragraph("");
            self.paragraphs.push(p);
        }
        self.append_overflow(&text);
        Some(OverflowOp::Stash { text })
    }

    /// Append `block` after a blank line. Returns the take that removes it again.
    fn append_overflow(&mut self, block: &str) -> OverflowOp {
        let kept = self.overflow.trim_end().len();
        self.overflow.truncate(kept);
        let separator = if self.overflow.is_empty() { "" } else { "\n\n" };
        let offset = self.overflow.len();
        let before = context_before(&self.overflow, offset).to_string();
        self.overflow.push_str(separator);
        self.overflow.push_str(block);
        OverflowOp::Take {
            offset,
            text: block.to_string(),
            removed: format!("{separator}{block}"),
            joined: String::new(),
            before,
            after: String::new(),
        }
    }

    /// The scratchpad's blocks: runs of non-blank lines, as byte ranges without the whitespace
    /// around them.
    pub fn overflow_blocks(&self) -> Vec<Range<usize>> {
        let mut blocks = Vec::new();
        let mut current: Option<Range<usize>> = None;
        let mut start = 0;
        for line in self.overflow.split_inclusive('\n') {
            let content = line.trim_end_matches(['\n', '\r']);
            if is_blank(content) {
                blocks.extend(current.take());
            } else {
                let first = start + content.len() - content.trim_start().len();
                let end = start + content.trim_end().len();
                match &mut current {
                    Some(block) => block.end = end,
                    None => current = Some(first..end),
                }
            }
            start += line.len();
        }
        blocks.extend(current);
        blocks
    }

    /// The block containing byte `offset`, its edges included.
    pub fn overflow_block_at(&self, offset: usize) -> Option<Range<usize>> {
        self.overflow_blocks()
            .into_iter()
            .find(|b| b.start <= offset && offset <= b.end)
    }

    /// Remove a byte range of the scratchpad; the op returned holds its text. The line breaks on
    /// either side are joined: a blank line stays one blank line, a line break stays one line
    /// break, and none is left at either end of the scratchpad.
    pub fn take_overflow(&mut self, range: Range<usize>) -> OverflowOp {
        let len = self.overflow.len();
        let mut start = range.start.min(len);
        let mut end = range.end.clamp(start, len);
        while !self.overflow.is_char_boundary(start) {
            start -= 1;
        }
        while !self.overflow.is_char_boundary(end) {
            end += 1;
        }
        let text = self.overflow[start..end].to_string();
        let (before, after) = (&self.overflow[..start], &self.overflow[end..]);
        let (cut_before, breaks_before) = trailing_break(before);
        let (cut_after, breaks_after) = leading_break(after);
        if breaks_before + breaks_after == 0 {
            return self.replace_overflow(start..end, "", text);
        }
        let gap = start - cut_before..end + cut_after;
        let joined = if gap.start == 0 || gap.end == len {
            ""
        } else if breaks_before.max(breaks_after) >= 2 {
            "\n\n"
        } else {
            "\n"
        };
        self.replace_overflow(gap, joined, text)
    }

    /// Replace `gap` with `joined`, for taking `text` out.
    fn replace_overflow(&mut self, gap: Range<usize>, joined: &str, text: String) -> OverflowOp {
        let op = OverflowOp::Take {
            offset: gap.start,
            text,
            removed: self.overflow[gap.clone()].to_string(),
            joined: joined.to_string(),
            before: context_before(&self.overflow, gap.start).to_string(),
            after: context_after(&self.overflow, gap.end).to_string(),
        };
        self.overflow.replace_range(gap, joined);
        op
    }

    /// Whether the scratchpad holds `here` at byte `offset`, with `before` and `after` around it.
    fn overflow_matches(&self, offset: usize, here: &str, before: &str, after: &str) -> bool {
        let text = &self.overflow;
        let end = offset + here.len();
        text.get(offset..end) == Some(here)
            && context_before(text, offset) == before
            && context_after(text, end) == after
    }

    /// The one block whose text is `text`, if there is exactly one.
    fn only_block(&self, text: &str) -> Option<Range<usize>> {
        let mut found = self
            .overflow_blocks()
            .into_iter()
            .filter(|r| self.overflow[r.clone()] == *text);
        let block = found.next()?;
        found.next().is_none().then_some(block)
    }

    /// Undo a page command's `op` on the scratchpad as it is now. A take goes back where it was
    /// if the text around there is unchanged, else at the end as a new block; a stash comes out
    /// if its block is still there, once, else it stays. Returns how, and the op that redoes it
    /// (None when nothing changed).
    pub fn undo_overflow(&mut self, op: &OverflowOp) -> (Replayed, Option<OverflowOp>) {
        match op {
            OverflowOp::Stash { text } => match self.only_block(text) {
                Some(block) => {
                    self.take_overflow(block);
                    (Replayed::Exact, Some(op.clone()))
                }
                None => (Replayed::Kept, None),
            },
            OverflowOp::Take {
                offset,
                text,
                removed,
                joined,
                before,
                after,
            } => {
                if self.overflow_matches(*offset, joined, before, after) {
                    self.overflow
                        .replace_range(*offset..*offset + joined.len(), removed);
                    (Replayed::Exact, Some(op.clone()))
                } else {
                    let redo = self.append_overflow(text.trim());
                    (Replayed::Appended, Some(redo))
                }
            }
        }
    }

    /// Redo a page command's `op` on the scratchpad as it is now. A stash appends its block
    /// again; a take removes its text where it was if the text around there is unchanged, else
    /// the one block holding just that text, else nothing. Returns how, and the op that undoes it
    /// (None when nothing changed).
    pub fn redo_overflow(&mut self, op: &OverflowOp) -> (Replayed, Option<OverflowOp>) {
        match op {
            OverflowOp::Stash { text } => {
                self.append_overflow(text);
                (Replayed::Appended, Some(op.clone()))
            }
            OverflowOp::Take {
                offset,
                text,
                removed,
                joined,
                before,
                after,
            } => {
                if self.overflow_matches(*offset, removed, before, after) {
                    self.overflow
                        .replace_range(*offset..*offset + removed.len(), joined);
                    return (Replayed::Exact, Some(op.clone()));
                }
                match self.only_block(text.trim()) {
                    Some(block) => (Replayed::Moved, Some(self.take_overflow(block))),
                    None => (Replayed::Kept, None),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{OverflowOp, Replayed};
    use crate::model::Document;
    use crate::pos::DocPos;

    fn blocks(d: &Document) -> Vec<&str> {
        d.overflow_blocks()
            .into_iter()
            .map(|r| &d.overflow[r])
            .collect()
    }

    #[test]
    fn stash_appends_the_selection_after_a_blank_line() {
        let mut d = Document::from_texts(&["Keep this. Cut this.", "More."]);
        assert!(d.stash(DocPos::new(0, 10)..DocPos::new(0, 20)).is_some());
        assert_eq!(d.paragraphs[0].text(), "Keep this.");
        assert_eq!(d.overflow, "Cut this.");
        d.overflow.push_str("\n\n\n");
        assert!(d.stash(DocPos::new(1, 0)..DocPos::new(1, 5)).is_some());
        assert_eq!(d.overflow, "Cut this.\n\nMore.");
    }

    #[test]
    fn stash_leaves_ghosts_out_and_ignores_whitespace() {
        let mut d = Document::from_texts(&["abc def", "a  b"]);
        d.toggle_ghost(0, 0..4).unwrap();
        assert!(d.stash(DocPos::new(0, 0)..DocPos::new(0, 7)).is_some());
        assert_eq!(d.overflow, "def");
        assert!(d.stash(DocPos::new(1, 1)..DocPos::new(1, 3)).is_none());
        assert_eq!(d.paragraphs[1].text(), "a  b");
        assert_eq!(d.overflow, "def");
    }

    #[test]
    fn stash_across_paragraphs_is_one_block_with_line_breaks() {
        let mut d = Document::from_texts(&["one two", "", "three four"]);
        assert!(d.stash(DocPos::new(0, 4)..DocPos::new(2, 5)).is_some());
        assert_eq!(d.paragraphs.len(), 1);
        assert_eq!(d.paragraphs[0].text(), "one  four");
        assert_eq!(d.overflow, "two\nthree");
        assert_eq!(blocks(&d), vec!["two\nthree"]);
    }

    #[test]
    fn an_empty_range_stashes_its_paragraph_and_the_document_never_empties() {
        let mut d = Document::from_texts(&["one", "two"]);
        assert!(d.stash(DocPos::new(0, 1)..DocPos::new(0, 1)).is_some());
        assert_eq!(d.paragraphs.len(), 1);
        assert_eq!(d.paragraphs[0].text(), "two");
        assert!(d.stash_paragraphs(0..1).is_some());
        assert_eq!(d.paragraphs.len(), 1);
        assert_eq!(d.paragraphs[0].text(), "");
        assert_eq!(d.overflow, "one\n\ntwo");
        assert!(d.stash(DocPos::new(0, 0)..DocPos::new(0, 0)).is_none());
    }

    #[test]
    fn stash_paragraphs_removes_them_all() {
        let mut d = Document::from_texts(&["a", "b", "c"]);
        assert!(d.stash_paragraphs(0..2).is_some());
        let texts: Vec<String> = d.paragraphs.iter().map(|p| p.text()).collect();
        assert_eq!(texts, vec!["c"]);
        assert_eq!(d.overflow, "a\nb");
    }

    #[test]
    fn stash_paragraphs_past_the_end_stashes_nothing() {
        let mut d = Document::from_texts(&["a", "b"]);
        assert!(d.stash_paragraphs(5..7).is_none());
        let (start, end) = (3, 1);
        assert!(d.stash_paragraphs(start..end).is_none());
        assert_eq!(d.paragraphs.len(), 2);
        assert_eq!(d.overflow, "");
        assert!(d.stash_paragraphs(1..9).is_some());
        assert_eq!(d.overflow, "b");
    }

    #[test]
    fn blocks_split_on_blank_lines() {
        let mut d = Document::new();
        d.overflow = "\n  first line\nsecond  \n \t\n\nthird\r\n\r\nfourth".into();
        assert_eq!(blocks(&d), vec!["first line\nsecond", "third", "fourth"]);
        assert_eq!(d.overflow_block_at(0), None);
        assert_eq!(d.overflow_block_at(3), Some(3..20));
        assert_eq!(d.overflow_block_at(20), Some(3..20));
        let fourth = d.overflow.find("fourth").unwrap();
        assert_eq!(
            d.overflow_block_at(d.overflow.len()),
            Some(fourth..d.overflow.len())
        );
        d.overflow.clear();
        assert!(d.overflow_blocks().is_empty());
    }

    #[test]
    fn take_joins_the_blank_lines_around_the_gap() {
        let take = |text: &str, what: &str| {
            let mut d = Document::new();
            d.overflow = text.into();
            let start = text.find(what).unwrap();
            let taken = d.take_overflow(start..start + what.len());
            assert_eq!(taken.text(), what);
            d.overflow
        };
        assert_eq!(take("A\n\nB\n\nC", "B"), "A\n\nC");
        assert_eq!(take("A\n\nB", "B"), "A");
        assert_eq!(take("A\n\nB", "A"), "B");
        assert_eq!(take("x\ny\nz", "y"), "x\nz");
        assert_eq!(take("bar foo baz", "foo"), "bar  baz");
        assert_eq!(take("bar foo\nbaz", "foo"), "bar \nbaz");
        assert_eq!(take("A\n\n\nB\n  \nC", "B"), "A\n\nC");
        assert_eq!(take("only", "only"), "");
    }

    #[test]
    fn take_clamps_to_the_text_and_char_boundaries() {
        let mut d = Document::new();
        d.overflow = "café".into();
        assert_eq!(d.take_overflow(4..99).text(), "é");
        assert_eq!(d.overflow, "caf");
    }

    fn with_overflow(text: &str) -> Document {
        let mut d = Document::new();
        d.overflow = text.into();
        d
    }

    fn take(d: &mut Document, what: &str) -> OverflowOp {
        let start = d.overflow.find(what).unwrap();
        d.take_overflow(start..start + what.len())
    }

    #[test]
    fn an_undone_take_goes_back_where_it_was() {
        for (text, what) in [
            ("A\n\nB\n\nC", "B"),
            ("A\n\nB", "B"),
            ("A\n\nB", "A"),
            ("x\ny\nz", "y"),
            ("bar foo baz", "foo"),
            ("A\n\n\nB\n  \nC", "B"),
            ("only", "only"),
            ("café\n\nthé", "thé"),
        ] {
            let mut d = with_overflow(text);
            let op = take(&mut d, what);
            let taken = d.overflow.clone();
            let (how, redo) = d.undo_overflow(&op);
            assert_eq!(how, Replayed::Exact, "{text:?}");
            assert_eq!(d.overflow, text);
            let (how, _) = d.redo_overflow(&redo.unwrap());
            assert_eq!(
                (how, d.overflow.as_str()),
                (Replayed::Exact, taken.as_str())
            );
        }
    }

    #[test]
    fn an_undone_take_after_an_edit_nearby_goes_to_the_end() {
        let mut d = with_overflow("Keep.\n\nPrecious block.");
        let op = take(&mut d, "Precious block.");
        assert_eq!(d.overflow, "Keep.");
        d.overflow = "Keep. more".into();
        let (how, redo) = d.undo_overflow(&op);
        assert_eq!(how, Replayed::Appended);
        assert_eq!(d.overflow, "Keep. more\n\nPrecious block.");
        // Redo takes the appended block out again.
        let (how, undo) = d.redo_overflow(&redo.unwrap());
        assert_eq!((how, d.overflow.as_str()), (Replayed::Exact, "Keep. more"));
        assert!(undo.is_some());
        // An edit before the gap moves it: the block can't go back at the old offset.
        let mut d = with_overflow("A.\n\nB.\n\nC.");
        let op = take(&mut d, "B.");
        d.overflow.insert_str(0, "Zero.\n\n");
        let (how, _) = d.undo_overflow(&op);
        assert_eq!(how, Replayed::Appended);
        assert_eq!(d.overflow, "Zero.\n\nA.\n\nC.\n\nB.");
    }

    #[test]
    fn an_undone_take_checks_char_boundaries() {
        let mut d = with_overflow("ab\n\nc");
        let op = take(&mut d, "c");
        assert_eq!(d.overflow, "ab");
        d.overflow = "é".into(); // byte 2 is now the end, but the text before differs
        let (how, _) = d.undo_overflow(&op);
        assert_eq!(how, Replayed::Appended);
        assert_eq!(d.overflow, "é\n\nc");
        let mut d = with_overflow("xé\n\nc");
        let op = take(&mut d, "c");
        d.overflow = "éé".into(); // byte 3 falls inside a character
        let (how, _) = d.undo_overflow(&op);
        assert_eq!((how, d.overflow.as_str()), (Replayed::Appended, "éé\n\nc"));
    }

    #[test]
    fn a_redone_take_after_an_edit_takes_the_matching_block_or_nothing() {
        let mut d = with_overflow("A.\n\nB.");
        let op = take(&mut d, "B.");
        d.undo_overflow(&op);
        d.overflow = "New.\n\nA.\n\nB.".into();
        let (how, undo) = d.redo_overflow(&op);
        assert_eq!((how, d.overflow.as_str()), (Replayed::Moved, "New.\n\nA."));
        assert!(undo.is_some());
        // Edited, or there twice: it stays.
        for edited in ["A.\n\nB, edited.", "B.\n\nB."] {
            d.overflow = format!("Lead.\n\n{edited}");
            let (how, undo) = d.redo_overflow(&op);
            assert_eq!(how, Replayed::Kept);
            assert!(undo.is_none());
            assert_eq!(d.overflow, format!("Lead.\n\n{edited}"));
        }
    }

    #[test]
    fn an_undone_stash_comes_out_only_if_its_block_is_there_once() {
        let mut d = Document::from_texts(&["Stash me.", "Keep."]);
        d.overflow = "older".into();
        let op = d.stash_paragraphs(0..1).unwrap();
        assert_eq!(
            op,
            OverflowOp::Stash {
                text: "Stash me.".into()
            }
        );
        assert_eq!(d.overflow, "older\n\nStash me.");
        let (how, redo) = d.undo_overflow(&op);
        assert_eq!((how, d.overflow.as_str()), (Replayed::Exact, "older"));
        let (how, undo) = d.redo_overflow(&redo.unwrap());
        assert_eq!(
            (how, d.overflow.as_str()),
            (Replayed::Appended, "older\n\nStash me.")
        );
        // Moved up by hand: still found.
        d.overflow = "Stash me.\n\nolder".into();
        let (how, _) = d.undo_overflow(&undo.unwrap());
        assert_eq!((how, d.overflow.as_str()), (Replayed::Exact, "older"));
        for kept in ["older\n\nStash me, edited.", "Stash me.\n\nStash me."] {
            d.overflow = kept.into();
            let (how, redo) = d.undo_overflow(&op);
            assert_eq!((how, d.overflow.as_str()), (Replayed::Kept, kept));
            assert!(redo.is_none());
        }
    }
}
