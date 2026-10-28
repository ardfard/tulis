//! Cursor motions over document text.

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;

use crate::model::Document;
use crate::pos::DocPos;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Blank,
    Word,
    Punct,
}

fn class(c: char) -> Class {
    if c.is_whitespace() {
        Class::Blank
    } else if c.is_alphanumeric() || c == '_' {
        Class::Word
    } else {
        Class::Punct
    }
}

pub fn prev_grapheme(doc: &Document, pos: DocPos) -> DocPos {
    if pos.offset == 0 {
        return if pos.para == 0 {
            pos
        } else {
            DocPos::new(pos.para - 1, doc.paragraphs[pos.para - 1].len())
        };
    }
    let text = doc.paragraphs[pos.para].text();
    let offset = text
        .grapheme_indices(true)
        .rev()
        .find_map(|(i, _)| (i < pos.offset).then_some(i))
        .unwrap_or(0);
    DocPos::new(pos.para, offset)
}

pub fn next_grapheme(doc: &Document, pos: DocPos) -> DocPos {
    let text = doc.paragraphs[pos.para].text();
    if pos.offset >= text.len() {
        return if pos.para + 1 < doc.paragraphs.len() {
            DocPos::new(pos.para + 1, 0)
        } else {
            pos
        };
    }
    let offset = text
        .grapheme_indices(true)
        .find_map(|(i, _)| (i > pos.offset).then_some(i))
        .unwrap_or(text.len());
    DocPos::new(pos.para, offset)
}

pub fn next_word_start(doc: &Document, pos: DocPos) -> DocPos {
    let text = doc.flat_text();
    let start = doc.flat_offset(pos);
    let mut chars = text[start..]
        .char_indices()
        .map(|(i, c)| (start + i, c))
        .peekable();
    let Some(&(_, first)) = chars.peek() else {
        return doc.end_pos();
    };
    let first_class = class(first);
    if first_class != Class::Blank {
        while chars.next_if(|&(_, c)| class(c) == first_class).is_some() {}
    }
    while chars.next_if(|&(_, c)| class(c) == Class::Blank).is_some() {}
    match chars.peek() {
        Some(&(i, _)) => doc.pos_from_flat(i),
        None => doc.end_pos(),
    }
}

pub fn prev_word_start(doc: &Document, pos: DocPos) -> DocPos {
    let text = doc.flat_text();
    let end = doc.flat_offset(pos);
    let chars: Vec<(usize, char)> = text[..end].char_indices().collect();
    let mut i = chars.len();
    while i > 0 && class(chars[i - 1].1) == Class::Blank {
        i -= 1;
    }
    if i == 0 {
        return DocPos::default();
    }
    let word_class = class(chars[i - 1].1);
    while i > 0 && class(chars[i - 1].1) == word_class {
        i -= 1;
    }
    doc.pos_from_flat(chars[i].0)
}

pub fn word_end(doc: &Document, pos: DocPos) -> DocPos {
    let text = doc.flat_text();
    let start = doc.flat_offset(pos);
    let chars: Vec<(usize, char)> = text[start..]
        .char_indices()
        .map(|(i, c)| (start + i, c))
        .collect();
    let mut i = 1;
    while i < chars.len() && class(chars[i].1) == Class::Blank {
        i += 1;
    }
    if i >= chars.len() {
        return doc.end_pos();
    }
    let word_class = class(chars[i].1);
    while i + 1 < chars.len() && class(chars[i + 1].1) == word_class {
        i += 1;
    }
    doc.pos_from_flat(chars[i].0)
}

pub fn next_paragraph(doc: &Document, pos: DocPos) -> DocPos {
    if pos.para + 1 < doc.paragraphs.len() {
        DocPos::new(pos.para + 1, 0)
    } else {
        doc.end_pos()
    }
}

pub fn prev_paragraph(_doc: &Document, pos: DocPos) -> DocPos {
    if pos.offset > 0 {
        DocPos::new(pos.para, 0)
    } else if pos.para > 0 {
        DocPos::new(pos.para - 1, 0)
    } else {
        pos
    }
}

/// The run of same-class characters at `offset` (the char before it when at the end).
pub fn word_range_at(text: &str, offset: usize) -> Range<usize> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let ix = match chars.iter().position(|&(i, _)| i == offset) {
        Some(ix) => ix,
        None if !chars.is_empty() => chars.len() - 1,
        None => return offset..offset,
    };
    let word_class = class(chars[ix].1);
    let mut start = ix;
    while start > 0 && class(chars[start - 1].1) == word_class {
        start -= 1;
    }
    let mut end = ix;
    while end + 1 < chars.len() && class(chars[end + 1].1) == word_class {
        end += 1;
    }
    chars[start].0..chars[end].0 + chars[end].1.len_utf8()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(texts: &[&str]) -> Document {
        Document::from_texts(texts)
    }

    fn p(para: usize, offset: usize) -> DocPos {
        DocPos::new(para, offset)
    }

    #[test]
    fn graphemes_cross_paragraphs_and_respect_clusters() {
        let doc = d(&["ae\u{301}", "b"]);
        assert_eq!(next_grapheme(&doc, p(0, 1)), p(0, 4));
        assert_eq!(next_grapheme(&doc, p(0, 4)), p(1, 0));
        assert_eq!(next_grapheme(&doc, p(1, 1)), p(1, 1));
        assert_eq!(prev_grapheme(&doc, p(0, 4)), p(0, 1));
        assert_eq!(prev_grapheme(&doc, p(1, 0)), p(0, 4));
        assert_eq!(prev_grapheme(&doc, p(0, 0)), p(0, 0));
    }

    #[test]
    fn next_word_start_like_vim_w() {
        let doc = d(&["hello world", "next"]);
        assert_eq!(next_word_start(&doc, p(0, 0)), p(0, 6));
        assert_eq!(next_word_start(&doc, p(0, 6)), p(1, 0));
        assert_eq!(next_word_start(&doc, p(1, 0)), p(1, 4));
        assert_eq!(next_word_start(&d(&["foo.bar"]), p(0, 0)), p(0, 3));
    }

    #[test]
    fn prev_word_start_like_vim_b() {
        let doc = d(&["hello world", "cd"]);
        assert_eq!(prev_word_start(&doc, p(0, 8)), p(0, 6));
        assert_eq!(prev_word_start(&doc, p(0, 6)), p(0, 0));
        assert_eq!(prev_word_start(&doc, p(1, 0)), p(0, 6));
        assert_eq!(prev_word_start(&doc, p(0, 0)), p(0, 0));
    }

    #[test]
    fn word_end_like_vim_e() {
        let doc = d(&["hello world"]);
        assert_eq!(word_end(&doc, p(0, 0)), p(0, 4));
        assert_eq!(word_end(&doc, p(0, 4)), p(0, 10));
        assert_eq!(word_end(&doc, p(0, 10)), p(0, 11));
    }

    #[test]
    fn paragraph_motions() {
        let doc = d(&["a", "b", "c"]);
        assert_eq!(next_paragraph(&doc, p(0, 1)), p(1, 0));
        assert_eq!(next_paragraph(&doc, p(2, 0)), p(2, 1));
        assert_eq!(prev_paragraph(&doc, p(1, 1)), p(1, 0));
        assert_eq!(prev_paragraph(&doc, p(1, 0)), p(0, 0));
        assert_eq!(prev_paragraph(&doc, p(0, 0)), p(0, 0));
    }

    #[test]
    fn word_range_for_double_click() {
        assert_eq!(word_range_at("hello world", 7), 6..11);
        assert_eq!(word_range_at("hello world", 5), 5..6);
        assert_eq!(word_range_at("hello world", 11), 6..11);
        assert_eq!(word_range_at("", 0), 0..0);
    }
}
