//! Prompts for AI alternates and AI trim, and parsing of their responses.

use std::ops::Range;

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

pub fn trim_request(paragraph: &str) -> CompletionRequest {
    let system = "You are an editor helping a writer tighten a paragraph. Find words, phrases or \
                  sentences that could be deleted without losing meaning: filler, hedges, \
                  repetition, throat-clearing. Never rewrite or add text; only point at text to \
                  cut. Copy each cut exactly as it appears in the paragraph, including \
                  punctuation, so it can be found verbatim. Reply with only a JSON array of \
                  strings, or [] if nothing should be cut."
        .to_string();
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

/// Map suggested cuts to byte ranges of the paragraph's rendered text. A cut must lie entirely in
/// non-ghost text and takes its first occurrence that doesn't overlap an earlier cut.
pub fn locate_trims(cuts: &[String], paragraph: &Paragraph) -> Vec<Range<usize>> {
    let text = paragraph.text();
    let mut visible: Vec<Range<usize>> = Vec::new();
    for span in paragraph.spans().into_iter().filter(|s| !s.ghost) {
        match visible.last_mut() {
            Some(last) if last.end == span.range.start => last.end = span.range.end,
            _ => visible.push(span.range),
        }
    }
    let mut found: Vec<Range<usize>> = Vec::new();
    for cut in cuts {
        let hit = text
            .match_indices(cut.as_str())
            .map(|(i, _)| i..i + cut.len())
            .find(|r| {
                visible.iter().any(|v| v.start <= r.start && r.end <= v.end)
                    && !found.iter().any(|f| f.start < r.end && r.start < f.end)
            });
        if let Some(range) = hit {
            found.push(range);
        }
    }
    found.sort_by_key(|r| r.start);
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
    fn locate_trims_skips_ghosted_text() {
        let mut d = Document::from_texts(&["I really really love tea"]);
        d.toggle_ghost(0, 2..9).unwrap();
        assert_eq!(
            locate_trims(&["really".to_string()], &d.paragraphs[0]),
            vec![9..15]
        );
    }
}
