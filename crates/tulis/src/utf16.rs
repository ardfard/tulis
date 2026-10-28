//! UTF-8 byte offsets ↔ UTF-16 code-unit offsets, for the platform input handler.

use std::ops::Range;

pub fn offset_from_utf16(text: &str, utf16: usize) -> usize {
    let mut utf8 = 0;
    let mut units = 0;
    for ch in text.chars() {
        if units >= utf16 {
            break;
        }
        units += ch.len_utf16();
        utf8 += ch.len_utf8();
    }
    utf8
}

pub fn offset_to_utf16(text: &str, offset: usize) -> usize {
    let mut utf16 = 0;
    let mut bytes = 0;
    for ch in text.chars() {
        if bytes >= offset {
            break;
        }
        bytes += ch.len_utf8();
        utf16 += ch.len_utf16();
    }
    utf16
}

pub fn range_from_utf16(text: &str, range: &Range<usize>) -> Range<usize> {
    offset_from_utf16(text, range.start)..offset_from_utf16(text, range.end)
}

pub fn range_to_utf16(text: &str, range: &Range<usize>) -> Range<usize> {
    offset_to_utf16(text, range.start)..offset_to_utf16(text, range.end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_offsets_around_surrogate_pairs() {
        let text = "a😀b";
        assert_eq!(offset_to_utf16(text, 5), 3);
        assert_eq!(offset_from_utf16(text, 3), 5);
        assert_eq!(range_to_utf16(text, &(1..5)), 1..3);
        assert_eq!(range_from_utf16(text, &(1..3)), 1..5);
        assert_eq!(offset_from_utf16(text, 99), text.len());
    }
}
