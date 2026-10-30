//! Sentence detection over a paragraph's text, for sentence-scope alternates and the Lab.

use std::ops::Range;

/// Words ending in `.` that don't end a sentence (compared case-insensitively).
const ABBREVIATIONS: [&str; 10] = [
    "e.g.", "i.e.", "etc.", "vs.", "mr.", "mrs.", "ms.", "dr.", "st.", "no.",
];

const TERMINATORS: [char; 3] = ['.', '!', '?'];
const CLOSERS: [char; 6] = ['"', '\'', '”', '’', ')', ']'];
const OPENERS: [char; 6] = ['"', '\'', '“', '‘', '(', '['];

/// Whether the word ending at byte `end` (exclusive; it ends with `.`) is an abbreviation.
fn is_abbreviation(text: &str, end: usize) -> bool {
    let start = text[..end]
        .char_indices()
        .rev()
        .find(|(_, c)| c.is_whitespace())
        .map_or(0, |(i, c)| i + c.len_utf8());
    let word = text[start..end].trim_start_matches(OPENERS).to_lowercase();
    ABBREVIATIONS.contains(&word.as_str())
}

/// The sentences of `text`, in order, without surrounding whitespace. A sentence ends after
/// `.`, `!` or `?` (and any closing quotes or brackets) followed by whitespace or the end of the
/// text, unless the word is a known abbreviation. Text with no terminator is one sentence.
pub fn sentence_ranges(text: &str) -> Vec<Range<usize>> {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let mut out = Vec::new();
    let mut start: Option<usize> = None;
    let mut i = 0;
    while i < chars.len() {
        let (at, c) = chars[i];
        if start.is_none() {
            if c.is_whitespace() {
                i += 1;
                continue;
            }
            start = Some(at);
        }
        if !TERMINATORS.contains(&c) {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < chars.len() && TERMINATORS.contains(&chars[j].1) {
            j += 1;
        }
        let terminators_end = chars.get(j).map_or(text.len(), |&(b, _)| b);
        while j < chars.len() && CLOSERS.contains(&chars[j].1) {
            j += 1;
        }
        let end = chars.get(j).map_or(text.len(), |&(b, _)| b);
        let at_break = chars.get(j).is_none_or(|&(_, c)| c.is_whitespace());
        let abbreviation = text[..terminators_end].ends_with('.')
            && terminators_end - at == 1
            && is_abbreviation(text, terminators_end);
        if at_break && !abbreviation {
            out.push(start.take().unwrap_or(at)..end);
        }
        i = j;
    }
    if let Some(start) = start {
        let end = start + text[start..].trim_end().len();
        if end > start {
            out.push(start..end);
        }
    }
    out
}

/// The sentence at `offset`. An offset in the whitespace between two sentences belongs to the
/// one before it; one before the first sentence belongs to the first. Blank text gives an empty
/// range at `offset`.
pub fn sentence_range_at(text: &str, offset: usize) -> Range<usize> {
    let ranges = sentence_ranges(text);
    ranges
        .iter()
        .rev()
        .find(|r| r.start <= offset)
        .or(ranges.first())
        .cloned()
        .unwrap_or(offset..offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sentences(text: &str) -> Vec<&str> {
        sentence_ranges(text)
            .into_iter()
            .map(|r| &text[r])
            .collect()
    }

    #[test]
    fn terminators_split_sentences() {
        assert_eq!(
            sentences("One. Two! Three? Four"),
            vec!["One.", "Two!", "Three?", "Four"]
        );
        assert_eq!(sentences("Wait?! Yes..."), vec!["Wait?!", "Yes..."]);
        assert_eq!(sentences("3.14 is pi. Ok."), vec!["3.14 is pi.", "Ok."]);
    }

    #[test]
    fn closing_quotes_and_brackets_stay_with_the_sentence() {
        assert_eq!(
            sentences("He said \"stop.\" Then (quietly.) left’s end.’ Done"),
            vec![
                "He said \"stop.\"",
                "Then (quietly.)",
                "left’s end.’",
                "Done"
            ]
        );
    }

    #[test]
    fn abbreviations_do_not_end_sentences() {
        assert_eq!(
            sentences("Tea, e.g. green, etc. is fine. Ask Dr. Who vs. MR. X. No. 5 wins."),
            vec![
                "Tea, e.g. green, etc. is fine.",
                "Ask Dr. Who vs. MR. X.",
                "No. 5 wins."
            ]
        );
        assert_eq!(
            sentences("See (e.g. this) now."),
            vec!["See (e.g. this) now."]
        );
    }

    #[test]
    fn whitespace_is_excluded_and_blank_text_has_no_sentences() {
        assert_eq!(sentence_ranges("  Hi there.   Bye.  "), vec![2..11, 14..18]);
        assert_eq!(sentence_ranges("No terminator here  "), vec![0..18]);
        assert!(sentence_ranges("   ").is_empty());
        assert!(sentence_ranges("").is_empty());
    }

    #[test]
    fn multibyte_text() {
        let text = "Café ouvert. Ça va? Größe!";
        assert_eq!(sentences(text), vec!["Café ouvert.", "Ça va?", "Größe!"]);
        let second = sentence_range_at(text, text.find("va").unwrap());
        assert_eq!(&text[second], "Ça va?");
    }

    #[test]
    fn offsets_pick_the_enclosing_or_previous_sentence() {
        let text = "  One. Two.";
        assert_eq!(sentence_range_at(text, 0), 2..6); // before the first
        assert_eq!(sentence_range_at(text, 4), 2..6);
        assert_eq!(sentence_range_at(text, 6), 2..6); // at its end
        assert_eq!(sentence_range_at(text, 7), 7..11); // start of the next
        assert_eq!(sentence_range_at(text, 11), 7..11); // end of the text
        assert_eq!(sentence_range_at("One.  Two.", 5), 0..4); // in the gap
        assert_eq!(sentence_range_at("   ", 1), 1..1);
    }
}
