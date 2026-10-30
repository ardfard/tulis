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

impl Document {
    /// Move the export text of `range` to the end of the scratchpad, after a blank line, as one
    /// block. An empty range stashes (and removes) the paragraph it is in. Returns false, and
    /// changes nothing, when there is only whitespace to stash.
    pub fn stash(&mut self, range: Range<DocPos>) -> bool {
        if range.start == range.end {
            let para = range.start.para;
            return self.stash_paragraphs(para..para + 1);
        }
        let text = as_block(&self.export_text_in(range.clone()));
        if text.is_empty() {
            return false;
        }
        self.delete(range);
        self.append_overflow(&text);
        true
    }

    /// Move whole paragraphs to the scratchpad as one block, removing them from the page. The
    /// document always keeps at least one paragraph.
    pub fn stash_paragraphs(&mut self, paras: Range<usize>) -> bool {
        let paras = paras.start..paras.end.min(self.paragraphs.len());
        let lines: Vec<String> = self.paragraphs[paras.clone()]
            .iter()
            .map(|p| p.export_text())
            .collect();
        let text = as_block(&lines.join("\n"));
        if text.is_empty() {
            return false;
        }
        self.paragraphs.drain(paras);
        if self.paragraphs.is_empty() {
            let p = self.new_paragraph("");
            self.paragraphs.push(p);
        }
        self.append_overflow(&text);
        true
    }

    fn append_overflow(&mut self, block: &str) {
        let kept = self.overflow.trim_end().len();
        self.overflow.truncate(kept);
        if !self.overflow.is_empty() {
            self.overflow.push_str("\n\n");
        }
        self.overflow.push_str(block);
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

    /// Remove a byte range of the scratchpad and return its text. The line breaks on either side
    /// are joined: a blank line stays one blank line, a line break stays one line break, and
    /// none is left at either end of the scratchpad.
    pub fn take_overflow(&mut self, range: Range<usize>) -> String {
        let len = self.overflow.len();
        let mut start = range.start.min(len);
        let mut end = range.end.clamp(start, len);
        while !self.overflow.is_char_boundary(start) {
            start -= 1;
        }
        while !self.overflow.is_char_boundary(end) {
            end += 1;
        }
        let taken = self.overflow[start..end].to_string();
        let (before, after) = (&self.overflow[..start], &self.overflow[end..]);
        let (cut_before, breaks_before) = trailing_break(before);
        let (cut_after, breaks_after) = leading_break(after);
        if breaks_before + breaks_after == 0 {
            self.overflow.replace_range(start..end, "");
            return taken;
        }
        let before = &before[..before.len() - cut_before];
        let after = &after[cut_after..];
        let separator = if before.is_empty() || after.is_empty() {
            ""
        } else if breaks_before.max(breaks_after) >= 2 {
            "\n\n"
        } else {
            "\n"
        };
        self.overflow = format!("{before}{separator}{after}");
        taken
    }
}

#[cfg(test)]
mod tests {
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
        assert!(d.stash(DocPos::new(0, 10)..DocPos::new(0, 20)));
        assert_eq!(d.paragraphs[0].text(), "Keep this.");
        assert_eq!(d.overflow, "Cut this.");
        d.overflow.push_str("\n\n\n");
        assert!(d.stash(DocPos::new(1, 0)..DocPos::new(1, 5)));
        assert_eq!(d.overflow, "Cut this.\n\nMore.");
    }

    #[test]
    fn stash_leaves_ghosts_out_and_ignores_whitespace() {
        let mut d = Document::from_texts(&["abc def", "a  b"]);
        d.toggle_ghost(0, 0..4).unwrap();
        assert!(d.stash(DocPos::new(0, 0)..DocPos::new(0, 7)));
        assert_eq!(d.overflow, "def");
        assert!(!d.stash(DocPos::new(1, 1)..DocPos::new(1, 3)));
        assert_eq!(d.paragraphs[1].text(), "a  b");
        assert_eq!(d.overflow, "def");
    }

    #[test]
    fn stash_across_paragraphs_is_one_block_with_line_breaks() {
        let mut d = Document::from_texts(&["one two", "", "three four"]);
        assert!(d.stash(DocPos::new(0, 4)..DocPos::new(2, 5)));
        assert_eq!(d.paragraphs.len(), 1);
        assert_eq!(d.paragraphs[0].text(), "one  four");
        assert_eq!(d.overflow, "two\nthree");
        assert_eq!(blocks(&d), vec!["two\nthree"]);
    }

    #[test]
    fn an_empty_range_stashes_its_paragraph_and_the_document_never_empties() {
        let mut d = Document::from_texts(&["one", "two"]);
        assert!(d.stash(DocPos::new(0, 1)..DocPos::new(0, 1)));
        assert_eq!(d.paragraphs.len(), 1);
        assert_eq!(d.paragraphs[0].text(), "two");
        assert!(d.stash_paragraphs(0..1));
        assert_eq!(d.paragraphs.len(), 1);
        assert_eq!(d.paragraphs[0].text(), "");
        assert_eq!(d.overflow, "one\n\ntwo");
        assert!(!d.stash(DocPos::new(0, 0)..DocPos::new(0, 0)));
    }

    #[test]
    fn stash_paragraphs_removes_them_all() {
        let mut d = Document::from_texts(&["a", "b", "c"]);
        assert!(d.stash_paragraphs(0..2));
        let texts: Vec<String> = d.paragraphs.iter().map(|p| p.text()).collect();
        assert_eq!(texts, vec!["c"]);
        assert_eq!(d.overflow, "a\nb");
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
            assert_eq!(taken, what);
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
        assert_eq!(d.take_overflow(4..99), "é");
        assert_eq!(d.overflow, "caf");
    }
}
