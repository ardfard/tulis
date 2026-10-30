//! Prompts for AI alternates, the Lab's trims, fixes and marks, and parsing of their responses.

use std::ops::Range;

use serde_json::{Map, Value};

use super::CompletionRequest;
use crate::model::Paragraph;

const MAX_TOKENS: u32 = 4096;

pub fn alternates_request(paragraph: &str, target: &str, n: usize) -> CompletionRequest {
    let system = format!(
        "You are a writing assistant inside a distraction-free editor. The writer has marked a \
         passage and wants alternative phrasings of it. Write {n} alternatives that fit naturally \
         where the passage sits in its paragraph, keep its meaning, and match the writer's voice \
         and tense. Each alternative replaces only the marked passage, so do not repeat the \
         surrounding text. Reply with only a JSON array of {n} strings."
    );
    let user = format!("Paragraph:\n{paragraph}\n\nPassage to rewrite:\n{target}");
    CompletionRequest {
        system,
        user,
        max_tokens: MAX_TOKENS,
    }
}

/// Alternatives from a response: a JSON array if present, otherwise one per line. Empty items,
/// repeats and copies of the original are dropped.
pub fn parse_alternates(response: &str, original: &str) -> Vec<String> {
    let raw = json_array(response).unwrap_or_else(|| {
        response
            .lines()
            .filter(|line| !line.trim_start().starts_with("```"))
            .map(|line| strip_list_marker(line).to_string())
            .collect()
    });
    let mut out: Vec<String> = Vec::new();
    for item in raw {
        let item = single_line(&item);
        if !item.is_empty() && item != original.trim() && !out.contains(&item) {
            out.push(item);
        }
    }
    out
}

/// Ask for passages to cut whose words add up to about `target_percent` of the paragraph's.
pub fn trim_request(paragraph: &str, target_percent: u8) -> CompletionRequest {
    let words = paragraph.split_whitespace().count();
    let target = (words * usize::from(target_percent)).div_ceil(100).max(1);
    let system = format!(
        "You are an editor helping a writer tighten a paragraph. Find words, phrases or \
         sentences that could be deleted without losing meaning: filler, hedges, repetition, \
         throat-clearing. Together they should come to about {target_percent}% of the \
         paragraph's words (about {target} of its {words} words). Never rewrite or add text; \
         only point at text to cut. Copy each cut exactly as it appears in the paragraph, \
         including punctuation, so it can be found verbatim. Reply with only a JSON array of \
         strings, or [] if nothing should be cut."
    );
    CompletionRequest {
        system,
        user: paragraph.to_string(),
        max_tokens: MAX_TOKENS,
    }
}

pub fn parse_trim(response: &str) -> Vec<String> {
    json_array(response)
        .unwrap_or_default()
        .into_iter()
        .filter(|cut| !cut.trim().is_empty())
        .collect()
}

/// Map suggested cuts to byte ranges of the paragraph's rendered text (see `locate`).
pub fn locate_trims(cuts: &[String], paragraph: &Paragraph) -> Vec<Range<usize>> {
    locate(cuts.iter().map(String::as_str), paragraph)
        .into_iter()
        .map(|(_, range)| range)
        .collect()
}

/// A correction proposed by the Lab's "Fix punctuation and typos" check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fix {
    /// The exact text to replace, as it appears in the paragraph.
    pub original: String,
    pub fixed: String,
}

pub fn fix_request(paragraph: &str) -> CompletionRequest {
    let system = "You are a careful proofreader. Find punctuation mistakes, typos and spelling \
                  errors in the paragraph. Leave style, word choice and meaning alone. For each \
                  mistake, copy the shortest stretch of the paragraph that contains it, exactly \
                  as written, and give its corrected form. Reply with only a JSON array of \
                  objects like {\"original\": \"teh cat\", \"fixed\": \"the cat\"}, or [] if \
                  there is nothing to fix."
        .to_string();
    CompletionRequest {
        system,
        user: paragraph.to_string(),
        max_tokens: MAX_TOKENS,
    }
}

/// Fixes from a response. Items without an original, and fixes that change nothing, are dropped.
pub fn parse_fixes(response: &str) -> Vec<Fix> {
    json_objects(response)
        .iter()
        .map(|item| Fix {
            original: string_field(item, "original"),
            fixed: string_field(item, "fixed"),
        })
        .filter(|fix| !fix.original.is_empty() && fix.original != fix.fixed)
        .collect()
}

/// Each fix's range in the paragraph's rendered text and its replacement, in text order. Fixes
/// whose original isn't in the visible text are dropped (see `locate`).
pub fn locate_fixes(fixes: &[Fix], paragraph: &Paragraph) -> Vec<(Range<usize>, String)> {
    locate(fixes.iter().map(|f| f.original.as_str()), paragraph)
        .into_iter()
        .map(|(i, range)| (range, fixes[i].fixed.clone()))
        .collect()
}

/// What a Lab "mark" check looks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkKind {
    Weakest,
    Convoluted,
    Tone,
    Hedges,
}

impl MarkKind {
    fn instructions(self) -> &'static str {
        match self {
            MarkKind::Weakest => {
                "Find the weakest sentences: vague, flat or redundant ones that do the least \
                 work. Copy each whole sentence."
            }
            MarkKind::Convoluted => {
                "Find convoluted sentences: hard to follow because of nesting, stacked clauses \
                 or tangled order. Copy each whole sentence."
            }
            MarkKind::Tone => {
                "Find words or short phrases whose tone doesn't fit the rest of the document \
                 (an excerpt of it is given for reference). Copy only the words."
            }
            MarkKind::Hedges => {
                "Find hedges and filler: words or short phrases such as \"I think\", \
                 \"maybe\", \"really\" or \"in order to\" that soften or pad without adding \
                 meaning. Copy only the words."
            }
        }
    }
}

/// A passage a mark check pointed at, with its reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkHit {
    pub text: String,
    pub note: String,
}

/// Ask for passages of `paragraph` to mark. `context` (an excerpt of the whole document) is
/// sent along for the tone check.
pub fn mark_request(kind: MarkKind, paragraph: &str, context: Option<&str>) -> CompletionRequest {
    let system = format!(
        "You are an editor reviewing a writer's paragraph. {} Never rewrite anything. Copy each \
         passage exactly as it appears in the paragraph, including punctuation, so it can be \
         found verbatim, and add a short note (under ten words) saying why. Reply with only a \
         JSON array of objects like {{\"text\": \"...\", \"note\": \"...\"}}, or [] if \
         nothing stands out.",
        kind.instructions()
    );
    let user = match context {
        Some(context) => {
            format!("The document, for reference (excerpt):\n{context}\n\nParagraph:\n{paragraph}")
        }
        None => paragraph.to_string(),
    };
    CompletionRequest {
        system,
        user,
        max_tokens: MAX_TOKENS,
    }
}

/// Marks from a response; items without text are dropped, and notes are trimmed.
pub fn parse_marks(response: &str) -> Vec<MarkHit> {
    json_objects(response)
        .iter()
        .map(|item| MarkHit {
            text: string_field(item, "text"),
            note: single_line(&string_field(item, "note")),
        })
        .filter(|hit| !hit.text.trim().is_empty())
        .collect()
}

/// Each mark's range in the paragraph's rendered text and its note, in text order. Marks whose
/// text isn't in the visible text are dropped (see `locate`).
pub fn locate_marks(hits: &[MarkHit], paragraph: &Paragraph) -> Vec<(Range<usize>, String)> {
    locate(hits.iter().map(|h| h.text.as_str()), paragraph)
        .into_iter()
        .map(|(i, range)| (range, hits[i].note.clone()))
        .collect()
}

/// Find each needle in the paragraph's rendered text, as `(needle index, byte range)` sorted by
/// position. A needle must lie entirely in non-ghost text and takes its first occurrence that
/// doesn't overlap an earlier needle's; empty needles and text that isn't there are dropped.
fn locate<'a>(
    needles: impl IntoIterator<Item = &'a str>,
    paragraph: &Paragraph,
) -> Vec<(usize, Range<usize>)> {
    let text = paragraph.text();
    let mut visible: Vec<Range<usize>> = Vec::new();
    for span in paragraph.spans().into_iter().filter(|s| !s.ghost) {
        match visible.last_mut() {
            Some(last) if last.end == span.range.start => last.end = span.range.end,
            _ => visible.push(span.range),
        }
    }
    let mut found: Vec<(usize, Range<usize>)> = Vec::new();
    for (i, needle) in needles.into_iter().enumerate() {
        if needle.is_empty() {
            continue;
        }
        let hit = text
            .match_indices(needle)
            .map(|(at, _)| at..at + needle.len())
            .find(|r| {
                visible.iter().any(|v| v.start <= r.start && r.end <= v.end)
                    && !found
                        .iter()
                        .any(|(_, f)| f.start < r.end && r.start < f.end)
            });
        if let Some(range) = hit {
            found.push((i, range));
        }
    }
    found.sort_by_key(|(_, r)| r.start);
    found
}

/// Collapse each run of line breaks (with the whitespace around it) to one space, then trim.
fn single_line(text: &str) -> String {
    text.split(['\r', '\n'])
        .map(str::trim)
        .filter(|piece| !piece.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn json_array(response: &str) -> Option<Vec<String>> {
    let start = response.find('[')?;
    let end = response.rfind(']')?;
    if end <= start {
        return None;
    }
    serde_json::from_str(&response[start..=end]).ok()
}

/// The objects of the first JSON array in `response` (other items are skipped).
fn json_objects(response: &str) -> Vec<Map<String, Value>> {
    let (Some(start), Some(end)) = (response.find('['), response.rfind(']')) else {
        return Vec::new();
    };
    if end <= start {
        return Vec::new();
    }
    serde_json::from_str::<Vec<Value>>(&response[start..=end])
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| match item {
            Value::Object(map) => Some(map),
            _ => None,
        })
        .collect()
}

fn string_field(item: &Map<String, Value>, key: &str) -> String {
    item.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn strip_list_marker(line: &str) -> &str {
    let mut text = line.trim();
    for marker in ["- ", "* ", "• "] {
        if let Some(rest) = text.strip_prefix(marker) {
            text = rest;
        }
    }
    let digits = text.len() - text.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    if digits > 0 {
        let rest = &text[digits..];
        if let Some(rest) = rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")) {
            text = rest;
        }
    }
    text.trim().trim_matches('"').trim()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Document;

    #[test]
    fn alternates_request_mentions_paragraph_target_and_count() {
        let req = alternates_request("I love tea.", "love", 3);
        assert!(req.system.contains("3"));
        assert!(req.user.contains("I love tea."));
        assert!(req.user.contains("love"));
    }

    #[test]
    fn parse_alternates_from_fenced_json() {
        assert_eq!(
            parse_alternates("```json\n[\"adore\", \"like\"]\n```", "love"),
            vec!["adore", "like"]
        );
    }

    #[test]
    fn parse_alternates_falls_back_to_lines() {
        let resp = "1. first\n2) second\n- third\n\"fourth\"\n```";
        assert_eq!(
            parse_alternates(resp, "x"),
            vec!["first", "second", "third", "fourth"]
        );
    }

    #[test]
    fn parse_alternates_drops_original_duplicates_and_empties() {
        assert_eq!(
            parse_alternates(r#"["love", " adore ", "adore", ""]"#, "love"),
            vec!["adore"]
        );
    }

    #[test]
    fn parse_alternates_collapses_line_breaks_to_spaces() {
        assert_eq!(
            parse_alternates(
                "[\"one\\n two\", \"a \\r\\n\\r\\n b\\n\", \"\\rc\\rd\"]",
                "x"
            ),
            vec!["one two", "a b", "c d"]
        );
    }

    #[test]
    fn parse_trim_reads_json_array_only() {
        assert_eq!(
            parse_trim(r#"Here you go: ["really", "very"]"#),
            vec!["really", "very"]
        );
        assert!(parse_trim("nothing to cut").is_empty());
    }

    #[test]
    fn locate_trims_finds_non_overlapping_occurrences() {
        let d = Document::from_texts(&["I really really love tea"]);
        let cuts = vec![
            "really ".to_string(),
            "really ".to_string(),
            "tea".to_string(),
            "missing".to_string(),
        ];
        assert_eq!(
            locate_trims(&cuts, &d.paragraphs[0]),
            vec![2..9, 9..16, 21..24]
        );
    }

    #[test]
    fn trim_request_asks_for_the_target_share_of_words() {
        let paragraph = "one two three four five six seven eight nine ten";
        let req = trim_request(paragraph, 20);
        assert!(req.system.contains("20%"), "{}", req.system);
        assert!(
            req.system.contains("about 2 of its 10 words"),
            "{}",
            req.system
        );
        assert_eq!(req.user, paragraph);
        // A short paragraph still asks for at least one word.
        assert!(
            trim_request("Hi there.", 10)
                .system
                .contains("about 1 of its 2 words")
        );
    }

    fn fix(original: &str, fixed: &str) -> Fix {
        Fix {
            original: original.into(),
            fixed: fixed.into(),
        }
    }

    #[test]
    fn fix_request_asks_for_original_and_fixed_pairs() {
        let req = fix_request("Its a nice day");
        assert!(req.system.contains(r#""original""#));
        assert!(req.system.contains(r#""fixed""#));
        assert_eq!(req.user, "Its a nice day");
    }

    #[test]
    fn parse_fixes_reads_objects_and_drops_no_ops() {
        let response = r#"Sure: [{"original": "Its", "fixed": "It's"},
            {"original": "day", "fixed": "day"}, {"original": "", "fixed": "x"},
            {"fixed": "y"}, "loose string", {"original": "teh", "fixed": "the", "why": "typo"}]"#;
        assert_eq!(
            parse_fixes(response),
            vec![fix("Its", "It's"), fix("teh", "the")]
        );
        assert!(parse_fixes("nothing to fix").is_empty());
        assert!(parse_fixes("[not json]").is_empty());
    }

    #[test]
    fn locate_fixes_finds_exact_visible_text_and_rejects_invented_text() {
        let mut d = Document::from_texts(&["Its a nice day, its true."]);
        d.toggle_ghost(0, 16..19).unwrap(); // the second "its"
        let fixes = vec![
            fix("nice", "lovely"),
            fix("its", "it's"),         // only in ghosted text
            fix("a nice dya", "a day"), // invented
            fix("Its", "It's"),
        ];
        assert_eq!(
            locate_fixes(&fixes, &d.paragraphs[0]),
            vec![(0..3, "It's".to_string()), (6..10, "lovely".to_string())]
        );
    }

    #[test]
    fn mark_request_names_the_kind_and_adds_tone_context() {
        let req = mark_request(MarkKind::Hedges, "I think maybe so.", None);
        assert!(req.system.contains("hedges"), "{}", req.system);
        assert!(req.system.contains(r#""note""#));
        assert_eq!(req.user, "I think maybe so.");
        for kind in [MarkKind::Weakest, MarkKind::Convoluted, MarkKind::Tone] {
            assert_ne!(mark_request(kind, "x", None).system, req.system);
        }
        let tone = mark_request(
            MarkKind::Tone,
            "Yo dude.",
            Some("Dear Sir, I write to you."),
        );
        assert!(tone.user.contains("Dear Sir, I write to you."));
        assert!(tone.user.ends_with("Paragraph:\nYo dude."));
    }

    #[test]
    fn parse_marks_reads_text_and_note() {
        let response = r#"```json
[{"text": "I think", "note": "hedge"}, {"text": "", "note": "x"}, {"note": "y"},
 {"text": "maybe", "note": "  "}]
```"#;
        assert_eq!(
            parse_marks(response),
            vec![
                MarkHit {
                    text: "I think".into(),
                    note: "hedge".into()
                },
                MarkHit {
                    text: "maybe".into(),
                    note: String::new()
                },
            ]
        );
    }

    #[test]
    fn locate_marks_keeps_notes_and_skips_invented_text() {
        let d = Document::from_texts(&["I think it is maybe fine, I think."]);
        let hits = vec![
            MarkHit {
                text: "maybe".into(),
                note: "filler".into(),
            },
            MarkHit {
                text: "I guess".into(),
                note: "invented".into(),
            },
            MarkHit {
                text: "I think".into(),
                note: "hedge".into(),
            },
        ];
        assert_eq!(
            locate_marks(&hits, &d.paragraphs[0]),
            vec![(0..7, "hedge".to_string()), (14..19, "filler".to_string())]
        );
    }

    #[test]
    fn locate_trims_skips_ghosted_text() {
        let mut d = Document::from_texts(&["I really really love tea"]);
        d.toggle_ghost(0, 2..9).unwrap();
        assert_eq!(
            locate_trims(&["really".to_string()], &d.paragraphs[0]),
            vec![9..15]
        );
    }
}
