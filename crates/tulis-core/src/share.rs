//! Share to X or LinkedIn: the text to share and the intent URL.

use std::ops::Range;

use crate::storage::export_markdown;
use crate::{DocPos, Document};

/// X rejects longer posts.
pub const X_LIMIT: usize = 280;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShareTarget {
    X,
    LinkedIn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareUrl {
    pub url: String,
    /// The text was cut to fit the target's limit.
    pub trimmed: bool,
}

/// What to share: the selection (paragraphs separated by a blank line) or, with no selection,
/// the whole document, as exported Markdown without the trailing newline.
pub fn share_text(doc: &Document, selection: Option<Range<DocPos>>) -> String {
    match selection {
        Some(range) if range.start != range.end => doc
            .export_text_in(range)
            .split('\n')
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => export_markdown(doc).trim_end().to_string(),
    }
}

/// Percent-encode everything except RFC 3986 unreserved characters.
pub fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// At most `limit` characters of `text`, without trailing whitespace; true if it was cut.
pub fn trim_to_chars(text: &str, limit: usize) -> (&str, bool) {
    match text.char_indices().nth(limit) {
        Some((end, _)) => (text[..end].trim_end(), true),
        None => (text, false),
    }
}

pub fn share_url(target: ShareTarget, text: &str) -> ShareUrl {
    match target {
        ShareTarget::X => {
            let (text, trimmed) = trim_to_chars(text, X_LIMIT);
            ShareUrl {
                url: format!("https://x.com/intent/post?text={}", percent_encode(text)),
                trimmed,
            }
        }
        ShareTarget::LinkedIn => ShareUrl {
            url: format!(
                "https://www.linkedin.com/feed/?shareActive=true&text={}",
                percent_encode(text)
            ),
            trimmed: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encoding_keeps_unreserved_and_encodes_utf8() {
        assert_eq!(percent_encode("a-b_c.d~e"), "a-b_c.d~e");
        assert_eq!(percent_encode("tea & café\n"), "tea%20%26%20caf%C3%A9%0A");
    }

    #[test]
    fn x_url_trims_to_280_chars() {
        let short = share_url(ShareTarget::X, "Hello world");
        assert_eq!(short.url, "https://x.com/intent/post?text=Hello%20world");
        assert!(!short.trimmed);
        let long = "é".repeat(300);
        let url = share_url(ShareTarget::X, &long);
        assert!(url.trimmed);
        assert_eq!(url.url.matches("%C3%A9").count(), 280);
    }

    #[test]
    fn trimming_drops_trailing_whitespace_and_respects_exact_fit() {
        assert_eq!(trim_to_chars("abc def", 4), ("abc", true));
        assert_eq!(trim_to_chars("abcd", 4), ("abcd", false));
    }

    #[test]
    fn linkedin_url_is_never_trimmed() {
        let url = share_url(ShareTarget::LinkedIn, &"a".repeat(400));
        assert!(!url.trimmed);
        assert!(
            url.url
                .starts_with("https://www.linkedin.com/feed/?shareActive=true&text=aaa")
        );
    }

    #[test]
    fn share_text_uses_selection_or_whole_document_without_ghosts() {
        let mut doc = Document::from_texts(&["I really love tea", "", "Second."]);
        doc.set_ghost(0, 2..9, true).unwrap();
        assert_eq!(share_text(&doc, None), "I love tea\n\nSecond.");
        let range = DocPos::new(0, 9)..DocPos::new(2, 3);
        assert_eq!(share_text(&doc, Some(range)), "love tea\n\nSec");
        let caret = DocPos::new(0, 1)..DocPos::new(0, 1);
        assert_eq!(share_text(&doc, Some(caret)), "I love tea\n\nSecond.");
    }
}
