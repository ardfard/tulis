# Tulis Redesign, Part 4: The Lab

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace v1's trim (red strikethrough, Tab accepts) with Write_On's Lab. LAB / Ctrl+L / vim `gl` open a popover above the LAB button, scoped to the selection's paragraphs or the whole document; Ctrl+Shift+T scopes it to the cursor's paragraph and Ctrl+Shift+Alt+T to the whole document. It lists six guide checks and five trim levels, with "what each idea does" descriptions, and is driven by arrows or `j`/`k`, Enter and Esc. A trim level fades the words it would cut and shows a bar ("<Level> <before> → <after> words · −<pct>%") with **Make the cuts** (one undo step), **Walk through** (`y`/Enter cut, `n`/Backspace keep, Esc stop) and **Done**. Clicking a faded passage (or Enter in it) keeps it; clicking it again fades it again. Mark checks tint passages, with a bar ("<n> marked · <check>", the note under the cursor, **Clear**), Tab / `]m` / `[m` navigation, and Ctrl+Shift+G targeting the mark under the cursor. "Fix punctuation and typos" turns each fix into an AI alternate. Every Lab run shares a cancel flag that is checked before each paragraph's request.

**Architecture:** `tulis-core` gains the prompt builders and parsers (`trim_request(paragraph, target_percent)`, `fix_request`/`parse_fixes`/`locate_fixes`, `MarkKind`/`mark_request`/`parse_marks`/`locate_marks`, one shared exact-substring `locate`), `sentence::long_sentences`, `stats::words_without` and `VimCommand::JumpMark`. The app's new `editor/lab.rs` holds the Lab's view state on the editor (`TrimPreview`, `Mark`, `lab_fixes`, the trim cache) and one runner, `Editor::run_lab`, that sends a list of per-paragraph `LabJob`s one at a time on the background executor, checking an `Arc<AtomicBool>` before each, and hands the results to a closure. `Editor::stop_ai` raises the flag and drops the task; Esc, Done, Clear and closing the popover call it. The popover is a gpui-component `Popover` in the footer whose trigger is a `LabButton` (`Selectable`), controlled by `Workspace::lab_open` with `on_open_change`; its content is the `LabPanel` view (`lab_popover.rs`, key context `Lab`). It stays open while a run waits for the AI and closes on `EditorEvent::LabDone`. The bar is rendered by the workspace above the footer from `Editor::lab_bar()`. Rendering: `build_runs` takes faded ranges (alpha × `fade_alpha`) and tinted ranges (`mark_tint` background); the strikethrough and `Theme::trim` go.

**Tech Stack:** Rust 1.98 (edition 2024), `gpui-pre =0.3.10` (imported as `gpui`), `gpui-component 0.8.1` (`popover::Popover`, `Selectable`, `button::Button`), plus the existing `tulis-core` dependencies (`serde_json` parses the new responses). No new crates.

**Spec:** `docs/superpowers/specs/2026-10-29-tulis-redesign-design.md`, section D (D.1–D.4), the keymap table, Error handling and Testing. Builds on parts 1–3 (`docs/superpowers/plans/2026-10-29-tulis-redesign-1-foundation-chrome.md`, `docs/superpowers/plans/2026-10-29-tulis-redesign-2-alternates-sidebar.md`, `docs/superpowers/plans/2026-10-30-tulis-redesign-3-overflow.md`). Read the spec section before starting a task.

**How the code is given.** The three new files (`editor/lab.rs`, `editor/lab_tests.rs`, `lab_popover.rs`, all in Task 3) are given in full; later tasks change them with diffs. Changes to existing files are unified diffs against the state the previous step leaves (3 lines of context, as `git diff` prints them). Apply them by hand or with `patch -p1`. Each task's Step 1 shows the test hunks and Step 3 the implementation hunks. Every step was run in a scratch copy of the repository, in this order, and the diffs are already formatted by `cargo fmt`.

## Global Constraints

- Version control is **jj**. Commit with `jj describe -m "<message>" && jj new`. Never run `git commit`.
- Every commit message ends with a blank line and then `Co-Authored-By: Claude <model> <noreply@anthropic.com>`, with your model name filled in (for example `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`).
- Run `cargo fmt --all` before every commit. The diffs below are already formatted.
- No new clippy warnings. `cargo clippy --workspace --all-targets` reports exactly: `tulis-core (lib)` 3 warnings, `tulis (bin "tulis")` 3 warnings and `tulis (bin "tulis" test)` 4 warnings (all from older code). No count may go up. Dead-code warnings count too, so don't add an item before the task that uses it (this is why the trim levels arrive with the popover in Task 3).
- Every task ends with `cargo test --workspace` passing. The starting point (HEAD `018969f`) has 162 app tests and 153 core tests. Part 4 ends with 187 and 163.
- Every app task ends with a launch check: `cargo build && timeout 6 ./target/debug/tulis /tmp/smoke.md; echo $?` prints `124`, with nothing on stderr. (Create `/tmp/smoke.md` with a couple of paragraphs first.)
- `tulis-core` must not depend on `gpui`.
- Positions in the Lab (cuts, marks, fixes) are byte ranges of a paragraph's **rendered** text (`Paragraph::text()`), located as exact substrings that lie entirely in non-ghost text. Requests send the **export** text.
- Don't change undo or overflow semantics: Lab edits go through the existing `record_step` / `begin_edit` / `end_edit`.
- Editor tests (`editor/tests.rs`, `editor/lab_tests.rs`) don't initialize gpui-component. Anything that renders the popover or the bar is tested from `workspace.rs` through `root_workspace` (part 2).
- Don't search `.devenv` or `.direnv`.
- **gpui-component facts** (checked against `~/.cargo/registry/src/*/gpui-component-0.8.1` and `gpui-base-0.8.1`):
  - `gpui_component::popover::Popover` (`gpui-component src/popover.rs`): `Popover::new(id).anchor(Anchor::BottomCenter)` opens above the trigger, centered. `.trigger(T)` needs `T: Selectable + IntoElement + 'static`; `Selectable` (`gpui-base src/component_traits.rs:12`) is `selected(self, bool) -> Self` and `is_selected(&self) -> bool`. `.content(Fn(&mut PopoverState, &mut Window, &mut Context<PopoverState>) -> E)` is called on every render (return a stored `Entity`, don't create one). `.open(bool)` makes it controlled; `.on_open_change(Fn(&bool, &mut Window, &mut App))` reports changes the popover makes itself (trigger click, Esc, click outside), not ones from `.open`. `.track_focus(&handle)` focuses that handle when it opens.
  - `gpui-base src/popover.rs`: opening saves the focused handle and focuses the tracked one; closing refocuses the saved handle if focus was inside the popover (`transition_to`, :94). The content has key context `PopoverContent`, where `escape` dispatches gpui-base's `Cancel` (dismiss) and `enter`/`space` are bound to `NoAction` (:18-29). A deeper context (`Lab`) wins over those, so our `enter` reaches the panel and `escape` falls through to the dismiss.
  - `Button::new(id).ghost().xsmall().label(..).on_click(..)` as in the header and footer.
- **gpui-pre facts** (`~/.cargo/registry/src/*/gpui-pre-0.3.10`): `debug_selector` exists in every build and is a no-op outside tests (`src/elements/div.rs:906-922`); tests read it with `VisualTestContext::debug_bounds`. `Entity<V: Render>` is `IntoElement` (`src/view.rs:112`). `VisualTestContext::simulate_keystrokes` runs the executor until parked (`src/app/test_context.rs:622`), so a test that must act while a run is in flight calls methods directly. A popover focuses during render, so tests draw (`window.draw(cx).clear(cx)`) before checking focus.
- Unknown API? Read those source files. Don't guess.

## File Map

```
README.md                                keys, vim line, smoke checklist (Task 8)
crates/tulis-core/src/
  ai/prompts.rs                          trim_request(target_percent); Fix, fix_request, parse_fixes, locate_fixes;
                                         MarkKind, MarkHit, mark_request, parse_marks, locate_marks; shared locate
  sentence.rs                            long_sentences
  stats.rs                               words_without
  vim.rs                                 VimCommand::JumpMark (`]m` / `[m`)
crates/tulis/src/
  actions.rs                             - AiTrimParagraph, AiTrimDocument, AcceptTrim
                                         + LabParagraph, LabDocument, LabNext, LabPrev, LabChoose, NextMark, PrevMark
  commands.rs                            lab_paragraph / lab_document / lab_next / lab_prev / lab_choose /
                                         next_mark / prev_mark rows (accept_trim and ai_trim_* go)
  main.rs                                + mod lab_popover;
  lab_popover.rs           (new)         LabItem, LabEvent, LabPanel (key context `Lab`), LabButton
  chrome.rs                              footer Popover; render_lab_bar
  workspace.rs                           lab / lab_open, open_lab, lab_paragraph, lab_document, show_lab, close_lab,
                                         LabDone; tests
  runs.rs                                faded (fade_alpha) and tinted (mark_tint) ranges; no strikethrough
  theme.rs                               Theme::trim removed
  editor/mod.rs                          trim_preview, marks, lab_fixes, trim_cache, ai_cancel; EditorEvent::LabDone;
                                         v1 `trims` and `open_lab` removed
  editor/lab.rs            (new)         TrimLevel, TrimSuggestion, TrimPreview, Walk, Check, Mark, BarAction, LabBar,
                                         LabJob, run_lab, stop_ai, run_trim, run_check, fixes, walk, navigation
  editor/lab_tests.rs      (new)         the Lab's editor tests
  editor/ai.rs                           v1 trim removed; provider is pub(super); Ctrl+Shift+G inside a mark
  editor/features.rs                     accept_trim removed; Esc order
  editor/actions.rs                      Enter/Backspace/click in a preview or walk
  editor/input.rs                        `y`/`n` captured while walking
  editor/element.rs                      faded cuts, tinted marks
  editor/vim_exec.rs                     `gl` dispatches OpenLab; `]m` / `[m`
  editor/tests.rs                        v1 trim tests removed or moved to the preview
```

---
### Task 1: Trim targets, fix and mark prompts in core

**Files:**
- Modify: `crates/tulis-core/src/ai/prompts.rs`, `crates/tulis/src/editor/ai.rs`

**Interfaces:**
- Consumes: `Paragraph::{text, spans}`, `CompletionRequest`.
- Produces:
  - `prompts::trim_request(paragraph: &str, target_percent: u8) -> CompletionRequest`
  - `pub struct Fix { pub original: String, pub fixed: String }`; `prompts::fix_request(paragraph: &str) -> CompletionRequest`; `prompts::parse_fixes(response: &str) -> Vec<Fix>`; `prompts::locate_fixes(fixes: &[Fix], paragraph: &Paragraph) -> Vec<(Range<usize>, String)>`
  - `pub enum MarkKind { Weakest, Convoluted, Tone, Hedges }`; `pub struct MarkHit { pub text: String, pub note: String }`; `prompts::mark_request(kind: MarkKind, paragraph: &str, context: Option<&str>) -> CompletionRequest`; `prompts::parse_marks(response: &str) -> Vec<MarkHit>`; `prompts::locate_marks(hits: &[MarkHit], paragraph: &Paragraph) -> Vec<(Range<usize>, String)>`

The trim request names the share and the word count it means (`ceil(words × pct / 100)`, at least 1). Fix and mark responses are JSON arrays of objects; items that aren't objects, lack the key, or (for fixes) change nothing are dropped. All three locators share one private `locate`: each needle takes its first occurrence that lies entirely in non-ghost text and doesn't overlap an earlier one, so text the model invented is dropped. The app keeps calling the v1 trim for now, with a 10% target, until Task 3 replaces it.

- [ ] **Step 1: Write the failing tests**

```diff
diff --git a/crates/tulis-core/src/ai/prompts.rs b/crates/tulis-core/src/ai/prompts.rs
index 24e565f..41badc3 100644
--- a/crates/tulis-core/src/ai/prompts.rs
+++ b/crates/tulis-core/src/ai/prompts.rs
@@ -202,6 +202,131 @@ mod tests {
         );
     }
 
+    #[test]
+    fn trim_request_asks_for_the_target_share_of_words() {
+        let paragraph = "one two three four five six seven eight nine ten";
+        let req = trim_request(paragraph, 20);
+        assert!(req.system.contains("20%"), "{}", req.system);
+        assert!(
+            req.system.contains("about 2 of its 10 words"),
+            "{}",
+            req.system
+        );
+        assert_eq!(req.user, paragraph);
+        // A short paragraph still asks for at least one word.
+        assert!(
+            trim_request("Hi there.", 10)
+                .system
+                .contains("about 1 of its 2 words")
+        );
+    }
+
+    fn fix(original: &str, fixed: &str) -> Fix {
+        Fix {
+            original: original.into(),
+            fixed: fixed.into(),
+        }
+    }
+
+    #[test]
+    fn fix_request_asks_for_original_and_fixed_pairs() {
+        let req = fix_request("Its a nice day");
+        assert!(req.system.contains(r#""original""#));
+        assert!(req.system.contains(r#""fixed""#));
+        assert_eq!(req.user, "Its a nice day");
+    }
+
+    #[test]
+    fn parse_fixes_reads_objects_and_drops_no_ops() {
+        let response = r#"Sure: [{"original": "Its", "fixed": "It's"},
+            {"original": "day", "fixed": "day"}, {"original": "", "fixed": "x"},
+            {"fixed": "y"}, "loose string", {"original": "teh", "fixed": "the", "why": "typo"}]"#;
+        assert_eq!(
+            parse_fixes(response),
+            vec![fix("Its", "It's"), fix("teh", "the")]
+        );
+        assert!(parse_fixes("nothing to fix").is_empty());
+        assert!(parse_fixes("[not json]").is_empty());
+    }
+
+    #[test]
+    fn locate_fixes_finds_exact_visible_text_and_rejects_invented_text() {
+        let mut d = Document::from_texts(&["Its a nice day, its true."]);
+        d.toggle_ghost(0, 16..19).unwrap(); // the second "its"
+        let fixes = vec![
+            fix("nice", "lovely"),
+            fix("its", "it's"),         // only in ghosted text
+            fix("a nice dya", "a day"), // invented
+            fix("Its", "It's"),
+        ];
+        assert_eq!(
+            locate_fixes(&fixes, &d.paragraphs[0]),
+            vec![(0..3, "It's".to_string()), (6..10, "lovely".to_string())]
+        );
+    }
+
+    #[test]
+    fn mark_request_names_the_kind_and_adds_tone_context() {
+        let req = mark_request(MarkKind::Hedges, "I think maybe so.", None);
+        assert!(req.system.contains("hedges"), "{}", req.system);
+        assert!(req.system.contains(r#""note""#));
+        assert_eq!(req.user, "I think maybe so.");
+        for kind in [MarkKind::Weakest, MarkKind::Convoluted, MarkKind::Tone] {
+            assert_ne!(mark_request(kind, "x", None).system, req.system);
+        }
+        let tone = mark_request(
+            MarkKind::Tone,
+            "Yo dude.",
+            Some("Dear Sir, I write to you."),
+        );
+        assert!(tone.user.contains("Dear Sir, I write to you."));
+        assert!(tone.user.ends_with("Paragraph:\nYo dude."));
+    }
+
+    #[test]
+    fn parse_marks_reads_text_and_note() {
+        let response = r#"```json
+[{"text": "I think", "note": "hedge"}, {"text": "", "note": "x"}, {"note": "y"},
+ {"text": "maybe", "note": "  "}]
+```"#;
+        assert_eq!(
+            parse_marks(response),
+            vec![
+                MarkHit {
+                    text: "I think".into(),
+                    note: "hedge".into()
+                },
+                MarkHit {
+                    text: "maybe".into(),
+                    note: String::new()
+                },
+            ]
+        );
+    }
+
+    #[test]
+    fn locate_marks_keeps_notes_and_skips_invented_text() {
+        let d = Document::from_texts(&["I think it is maybe fine, I think."]);
+        let hits = vec![
+            MarkHit {
+                text: "maybe".into(),
+                note: "filler".into(),
+            },
+            MarkHit {
+                text: "I guess".into(),
+                note: "invented".into(),
+            },
+            MarkHit {
+                text: "I think".into(),
+                note: "hedge".into(),
+            },
+        ];
+        assert_eq!(
+            locate_marks(&hits, &d.paragraphs[0]),
+            vec![(0..7, "hedge".to_string()), (14..19, "filler".to_string())]
+        );
+    }
+
     #[test]
     fn locate_trims_skips_ghosted_text() {
         let mut d = Document::from_texts(&["I really really love tea"]);
```

- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test -p tulis-core prompts`
Expected: compile errors (`trim_request` takes one argument; `Fix`, `fix_request`, `MarkKind`, … don't exist).

- [ ] **Step 3: Implement**

```diff
diff --git a/crates/tulis-core/src/ai/prompts.rs b/crates/tulis-core/src/ai/prompts.rs
index 41badc3..350fcd6 100644
--- a/crates/tulis-core/src/ai/prompts.rs
+++ b/crates/tulis-core/src/ai/prompts.rs
@@ -1,7 +1,9 @@
-//! Prompts for AI alternates and AI trim, and parsing of their responses.
+//! Prompts for AI alternates, the Lab's trims, fixes and marks, and parsing of their responses.
 
 use std::ops::Range;
 
+use serde_json::{Map, Value};
+
 use super::CompletionRequest;
 use crate::model::Paragraph;
 
@@ -43,14 +45,19 @@ pub fn parse_alternates(response: &str, original: &str) -> Vec<String> {
     out
 }
 
-pub fn trim_request(paragraph: &str) -> CompletionRequest {
-    let system = "You are an editor helping a writer tighten a paragraph. Find words, phrases or \
-                  sentences that could be deleted without losing meaning: filler, hedges, \
-                  repetition, throat-clearing. Never rewrite or add text; only point at text to \
-                  cut. Copy each cut exactly as it appears in the paragraph, including \
-                  punctuation, so it can be found verbatim. Reply with only a JSON array of \
-                  strings, or [] if nothing should be cut."
-        .to_string();
+/// Ask for passages to cut whose words add up to about `target_percent` of the paragraph's.
+pub fn trim_request(paragraph: &str, target_percent: u8) -> CompletionRequest {
+    let words = paragraph.split_whitespace().count();
+    let target = (words * usize::from(target_percent)).div_ceil(100).max(1);
+    let system = format!(
+        "You are an editor helping a writer tighten a paragraph. Find words, phrases or \
+         sentences that could be deleted without losing meaning: filler, hedges, repetition, \
+         throat-clearing. Together they should come to about {target_percent}% of the \
+         paragraph's words (about {target} of its {words} words). Never rewrite or add text; \
+         only point at text to cut. Copy each cut exactly as it appears in the paragraph, \
+         including punctuation, so it can be found verbatim. Reply with only a JSON array of \
+         strings, or [] if nothing should be cut."
+    );
     CompletionRequest {
         system,
         user: paragraph.to_string(),
@@ -66,9 +73,150 @@ pub fn parse_trim(response: &str) -> Vec<String> {
         .collect()
 }
 
-/// Map suggested cuts to byte ranges of the paragraph's rendered text. A cut must lie entirely in
-/// non-ghost text and takes its first occurrence that doesn't overlap an earlier cut.
+/// Map suggested cuts to byte ranges of the paragraph's rendered text (see `locate`).
 pub fn locate_trims(cuts: &[String], paragraph: &Paragraph) -> Vec<Range<usize>> {
+    locate(cuts.iter().map(String::as_str), paragraph)
+        .into_iter()
+        .map(|(_, range)| range)
+        .collect()
+}
+
+/// A correction proposed by the Lab's "Fix punctuation and typos" check.
+#[derive(Clone, Debug, PartialEq, Eq)]
+pub struct Fix {
+    /// The exact text to replace, as it appears in the paragraph.
+    pub original: String,
+    pub fixed: String,
+}
+
+pub fn fix_request(paragraph: &str) -> CompletionRequest {
+    let system = "You are a careful proofreader. Find punctuation mistakes, typos and spelling \
+                  errors in the paragraph. Leave style, word choice and meaning alone. For each \
+                  mistake, copy the shortest stretch of the paragraph that contains it, exactly \
+                  as written, and give its corrected form. Reply with only a JSON array of \
+                  objects like {\"original\": \"teh cat\", \"fixed\": \"the cat\"}, or [] if \
+                  there is nothing to fix."
+        .to_string();
+    CompletionRequest {
+        system,
+        user: paragraph.to_string(),
+        max_tokens: MAX_TOKENS,
+    }
+}
+
+/// Fixes from a response. Items without an original, and fixes that change nothing, are dropped.
+pub fn parse_fixes(response: &str) -> Vec<Fix> {
+    json_objects(response)
+        .iter()
+        .map(|item| Fix {
+            original: string_field(item, "original"),
+            fixed: string_field(item, "fixed"),
+        })
+        .filter(|fix| !fix.original.is_empty() && fix.original != fix.fixed)
+        .collect()
+}
+
+/// Each fix's range in the paragraph's rendered text and its replacement, in text order. Fixes
+/// whose original isn't in the visible text are dropped (see `locate`).
+pub fn locate_fixes(fixes: &[Fix], paragraph: &Paragraph) -> Vec<(Range<usize>, String)> {
+    locate(fixes.iter().map(|f| f.original.as_str()), paragraph)
+        .into_iter()
+        .map(|(i, range)| (range, fixes[i].fixed.clone()))
+        .collect()
+}
+
+/// What a Lab "mark" check looks for.
+#[derive(Clone, Copy, Debug, PartialEq, Eq)]
+pub enum MarkKind {
+    Weakest,
+    Convoluted,
+    Tone,
+    Hedges,
+}
+
+impl MarkKind {
+    fn instructions(self) -> &'static str {
+        match self {
+            MarkKind::Weakest => {
+                "Find the weakest sentences: vague, flat or redundant ones that do the least \
+                 work. Copy each whole sentence."
+            }
+            MarkKind::Convoluted => {
+                "Find convoluted sentences: hard to follow because of nesting, stacked clauses \
+                 or tangled order. Copy each whole sentence."
+            }
+            MarkKind::Tone => {
+                "Find words or short phrases whose tone doesn't fit the rest of the document \
+                 (an excerpt of it is given for reference). Copy only the words."
+            }
+            MarkKind::Hedges => {
+                "Find hedges and filler: words or short phrases such as \"I think\", \
+                 \"maybe\", \"really\" or \"in order to\" that soften or pad without adding \
+                 meaning. Copy only the words."
+            }
+        }
+    }
+}
+
+/// A passage a mark check pointed at, with its reason.
+#[derive(Clone, Debug, PartialEq, Eq)]
+pub struct MarkHit {
+    pub text: String,
+    pub note: String,
+}
+
+/// Ask for passages of `paragraph` to mark. `context` (an excerpt of the whole document) is
+/// sent along for the tone check.
+pub fn mark_request(kind: MarkKind, paragraph: &str, context: Option<&str>) -> CompletionRequest {
+    let system = format!(
+        "You are an editor reviewing a writer's paragraph. {} Never rewrite anything. Copy each \
+         passage exactly as it appears in the paragraph, including punctuation, so it can be \
+         found verbatim, and add a short note (under ten words) saying why. Reply with only a \
+         JSON array of objects like {{\"text\": \"...\", \"note\": \"...\"}}, or [] if \
+         nothing stands out.",
+        kind.instructions()
+    );
+    let user = match context {
+        Some(context) => {
+            format!("The document, for reference (excerpt):\n{context}\n\nParagraph:\n{paragraph}")
+        }
+        None => paragraph.to_string(),
+    };
+    CompletionRequest {
+        system,
+        user,
+        max_tokens: MAX_TOKENS,
+    }
+}
+
+/// Marks from a response; items without text are dropped, and notes are trimmed.
+pub fn parse_marks(response: &str) -> Vec<MarkHit> {
+    json_objects(response)
+        .iter()
+        .map(|item| MarkHit {
+            text: string_field(item, "text"),
+            note: single_line(&string_field(item, "note")),
+        })
+        .filter(|hit| !hit.text.trim().is_empty())
+        .collect()
+}
+
+/// Each mark's range in the paragraph's rendered text and its note, in text order. Marks whose
+/// text isn't in the visible text are dropped (see `locate`).
+pub fn locate_marks(hits: &[MarkHit], paragraph: &Paragraph) -> Vec<(Range<usize>, String)> {
+    locate(hits.iter().map(|h| h.text.as_str()), paragraph)
+        .into_iter()
+        .map(|(i, range)| (range, hits[i].note.clone()))
+        .collect()
+}
+
+/// Find each needle in the paragraph's rendered text, as `(needle index, byte range)` sorted by
+/// position. A needle must lie entirely in non-ghost text and takes its first occurrence that
+/// doesn't overlap an earlier needle's; empty needles and text that isn't there are dropped.
+fn locate<'a>(
+    needles: impl IntoIterator<Item = &'a str>,
+    paragraph: &Paragraph,
+) -> Vec<(usize, Range<usize>)> {
     let text = paragraph.text();
     let mut visible: Vec<Range<usize>> = Vec::new();
     for span in paragraph.spans().into_iter().filter(|s| !s.ghost) {
@@ -77,20 +225,25 @@ pub fn locate_trims(cuts: &[String], paragraph: &Paragraph) -> Vec<Range<usize>>
             _ => visible.push(span.range),
         }
     }
-    let mut found: Vec<Range<usize>> = Vec::new();
-    for cut in cuts {
+    let mut found: Vec<(usize, Range<usize>)> = Vec::new();
+    for (i, needle) in needles.into_iter().enumerate() {
+        if needle.is_empty() {
+            continue;
+        }
         let hit = text
-            .match_indices(cut.as_str())
-            .map(|(i, _)| i..i + cut.len())
+            .match_indices(needle)
+            .map(|(at, _)| at..at + needle.len())
             .find(|r| {
                 visible.iter().any(|v| v.start <= r.start && r.end <= v.end)
-                    && !found.iter().any(|f| f.start < r.end && r.start < f.end)
+                    && !found
+                        .iter()
+                        .any(|(_, f)| f.start < r.end && r.start < f.end)
             });
         if let Some(range) = hit {
-            found.push(range);
+            found.push((i, range));
         }
     }
-    found.sort_by_key(|r| r.start);
+    found.sort_by_key(|(_, r)| r.start);
     found
 }
 
@@ -112,6 +265,31 @@ fn json_array(response: &str) -> Option<Vec<String>> {
     serde_json::from_str(&response[start..=end]).ok()
 }
 
+/// The objects of the first JSON array in `response` (other items are skipped).
+fn json_objects(response: &str) -> Vec<Map<String, Value>> {
+    let (Some(start), Some(end)) = (response.find('['), response.rfind(']')) else {
+        return Vec::new();
+    };
+    if end <= start {
+        return Vec::new();
+    }
+    serde_json::from_str::<Vec<Value>>(&response[start..=end])
+        .unwrap_or_default()
+        .into_iter()
+        .filter_map(|item| match item {
+            Value::Object(map) => Some(map),
+            _ => None,
+        })
+        .collect()
+}
+
+fn string_field(item: &Map<String, Value>, key: &str) -> String {
+    item.get(key)
+        .and_then(Value::as_str)
+        .unwrap_or_default()
+        .to_string()
+}
+
 fn strip_list_marker(line: &str) -> &str {
     let mut text = line.trim();
     for marker in ["- ", "* ", "• "] {
diff --git a/crates/tulis/src/editor/ai.rs b/crates/tulis/src/editor/ai.rs
index b5e1563..d6de768 100644
--- a/crates/tulis/src/editor/ai.rs
+++ b/crates/tulis/src/editor/ai.rs
@@ -240,7 +240,7 @@ impl Editor {
                 let p = &self.doc.paragraphs[i];
                 let export = p.export_text();
                 (!export.trim().is_empty())
-                    .then(|| (p.id, p.text(), prompts::trim_request(&export)))
+                    .then(|| (p.id, p.text(), prompts::trim_request(&export, 10)))
             })
             .collect();
         if jobs.is_empty() {
```

- [ ] **Step 4: Run the tests (expect success)**

Run: `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: 162 app tests and 160 core tests pass; clippy counts match the baseline. Launch check as in the Global Constraints.

- [ ] **Step 5: Commit**

```bash
jj describe -m "feat(core): trim targets, fix and mark prompts with exact-text location

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 2: Long sentences and words after cuts in core

**Files:**
- Modify: `crates/tulis-core/src/sentence.rs`, `crates/tulis-core/src/stats.rs`

**Interfaces:**
- Consumes: `sentence_ranges`, `Counts::of`, `title_marker_len`.
- Produces:
  - `sentence::long_sentences(text: &str, max_words: usize) -> Vec<(Range<usize>, usize)>` (range and word count, for sentences with more than `max_words` words)
  - `stats::words_without(doc: &Document, cuts: &[(usize, Range<usize>)]) -> usize` (the document's word count as `document_counts` gives it, with the cuts left out too)

`words_without` drops ghosted text and the title marker as `document_counts` does, and removes cut bytes without inserting a space, so a cut inside a word leaves the rest of the word. The trim bar uses it for "after".

- [ ] **Step 1: Write the failing tests**

```diff
diff --git a/crates/tulis-core/src/sentence.rs b/crates/tulis-core/src/sentence.rs
index 11d4c8a..2f65f08 100644
--- a/crates/tulis-core/src/sentence.rs
+++ b/crates/tulis-core/src/sentence.rs
@@ -100,6 +100,15 @@ mod tests {
             .collect()
     }
 
+    #[test]
+    fn long_sentences_are_those_over_the_limit() {
+        let text = "One two three. One two three four five! Short.";
+        assert_eq!(long_sentences(text, 3), vec![(15..39, 5)]);
+        assert!(long_sentences(text, 5).is_empty());
+        assert_eq!(long_sentences(text, 0).len(), 3);
+        assert!(long_sentences("", 0).is_empty());
+    }
+
     #[test]
     fn terminators_split_sentences() {
         assert_eq!(
diff --git a/crates/tulis-core/src/stats.rs b/crates/tulis-core/src/stats.rs
index ec41f9b..935435d 100644
--- a/crates/tulis-core/src/stats.rs
+++ b/crates/tulis-core/src/stats.rs
@@ -87,6 +87,17 @@ pub fn selection_counts(doc: &Document, range: Range<DocPos>) -> Counts {
 mod tests {
     use super::*;
 
+    #[test]
+    fn words_without_leaves_out_cuts_ghosts_and_the_title_marker() {
+        let mut d = Document::from_texts(&["# My title", "I really love green tea.", "Cut me."]);
+        d.toggle_ghost(1, 14..20).unwrap(); // "green "
+        assert_eq!(document_counts(&d).words, 8);
+        assert_eq!(words_without(&d, &[]), 8);
+        assert_eq!(words_without(&d, &[(1, 2..9), (2, 0..7)]), 5);
+        // Part of a word leaves the rest of it.
+        assert_eq!(words_without(&d, &[(1, 3..5)]), 8);
+    }
+
     #[test]
     fn counts_words_and_chars() {
         assert_eq!(
```

- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test -p tulis-core`
Expected: `cannot find function long_sentences` / `words_without`.

- [ ] **Step 3: Implement**

```diff
diff --git a/crates/tulis-core/src/sentence.rs b/crates/tulis-core/src/sentence.rs
index 2f65f08..3ef4a57 100644
--- a/crates/tulis-core/src/sentence.rs
+++ b/crates/tulis-core/src/sentence.rs
@@ -75,6 +75,19 @@ pub fn sentence_ranges(text: &str) -> Vec<Range<usize>> {
     out
 }
 
+/// The sentences of `text` with more than `max_words` whitespace-separated words, with their
+/// word counts (the Lab's "Mark sentences that run long" check).
+pub fn long_sentences(text: &str, max_words: usize) -> Vec<(Range<usize>, usize)> {
+    sentence_ranges(text)
+        .into_iter()
+        .map(|range| {
+            let words = text[range.clone()].split_whitespace().count();
+            (range, words)
+        })
+        .filter(|&(_, words)| words > max_words)
+        .collect()
+}
+
 /// The sentence at `offset`. An offset in the whitespace between two sentences belongs to the
 /// one before it; one before the first sentence belongs to the first. Blank text gives an empty
 /// range at `offset`.
diff --git a/crates/tulis-core/src/stats.rs b/crates/tulis-core/src/stats.rs
index 935435d..5886f7e 100644
--- a/crates/tulis-core/src/stats.rs
+++ b/crates/tulis-core/src/stats.rs
@@ -70,6 +70,41 @@ pub fn document_counts(doc: &Document) -> Counts {
         .fold(Counts::default(), Counts::add)
 }
 
+/// The document's word count, as `document_counts` gives it, with `cuts` left out as well. Each
+/// cut is a paragraph index and a byte range of that paragraph's rendered text (the Lab's trim
+/// preview counts the words that would remain).
+pub fn words_without(doc: &Document, cuts: &[(usize, Range<usize>)]) -> usize {
+    doc.paragraphs
+        .iter()
+        .enumerate()
+        .map(|(i, p)| {
+            let text = p.text();
+            let skip = if i == 0 { title_marker_len(&text) } else { 0 };
+            let ghosts: Vec<Range<usize>> = p
+                .spans()
+                .into_iter()
+                .filter(|s| s.ghost)
+                .map(|s| s.range)
+                .collect();
+            let cut_here: Vec<&Range<usize>> = cuts
+                .iter()
+                .filter(|(para, _)| *para == i)
+                .map(|(_, range)| range)
+                .collect();
+            let kept: String = text
+                .char_indices()
+                .filter(|(at, _)| {
+                    *at >= skip
+                        && !ghosts.iter().any(|g| g.contains(at))
+                        && !cut_here.iter().any(|c| c.contains(at))
+                })
+                .map(|(_, c)| c)
+                .collect();
+            Counts::of(&kept).words
+        })
+        .sum()
+}
+
 /// Counts for `range`. Ghosts and the title marker are left out.
 pub fn selection_counts(doc: &Document, range: Range<DocPos>) -> Counts {
     let mut start = range.start;
```

- [ ] **Step 4: Run the tests (expect success)**

Run: `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: 162 app tests and 162 core tests pass; clippy counts match the baseline.

- [ ] **Step 5: Commit**

```bash
jj describe -m "feat(core): long sentences and the word count after cuts

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 3: The Lab popover and trim previews (v1 trim retired)

**Files:**
- Create: `crates/tulis/src/editor/lab.rs`, `crates/tulis/src/editor/lab_tests.rs`, `crates/tulis/src/lab_popover.rs`
- Modify: `crates/tulis-core/src/vim.rs`, `crates/tulis/src/{actions.rs, commands.rs, chrome.rs, main.rs, runs.rs, theme.rs, workspace.rs}`, `crates/tulis/src/editor/{mod.rs, ai.rs, element.rs, features.rs, vim_exec.rs, tests.rs}`

**Interfaces:**
- Consumes: Task 1's `trim_request`, `parse_trim`, `locate_trims`; `Editor::{provider, record_step, set_selection, finish_alt_edit}`.
- Produces:
  - Core: `VimCommand::JumpMark { forward: bool, count: usize }` from `]m` / `[m` (with a count; nothing with a pending operator).
  - `editor::lab`: `enum TrimLevel { Original, Slight, Tighten, Sharper, Half }` with `ALL`, `percent()`, `label()`; `struct TrimSuggestion { para_id, range }` (moved from `editor/mod.rs`); `struct TrimPreview { level, cuts, kept: HashSet<usize> }` with `new`, `is_active`, `active_ranges(id)`, `retain`; `struct LabJob { para_id, text, export, request }`; `type LabResults`.
  - `Editor` fields: `trim_preview: Option<TrimPreview>`, `trim_cache: HashMap<(TrimLevel, ParaId, String), (String, Vec<Range<usize>>)>`, `ai_cancel: Option<Arc<AtomicBool>>`. `EditorEvent::LabDone`.
  - `Editor::run_lab(jobs, working: &str, apply: impl FnOnce(&mut Self, LabResults, usize, &mut Context<Self>) + 'static, cx)`, `Editor::stop_ai() -> bool`, `Editor::lab_running() -> bool`, `Editor::lab_scope() -> Vec<ParaId>`, `Editor::run_trim(level, scope: &[ParaId], cx)`, `Editor::jump_lab_item(forward, count, cx)`, actions `NextMark` (Tab) / `PrevMark` (Shift+Tab).
  - `lab_popover`: `enum LabItem { Trim(TrimLevel) }`, `enum LabEvent { Close }`, `struct LabPanel { focus, scope, highlight, .. }` with `reset(scope, cx)` and `run(item, cx)`, `struct LabButton` (`Selectable`).
  - `Workspace`: `lab: Entity<LabPanel>`, `lab_open: bool`, `open_lab` (selection's paragraphs or all), `lab_paragraph` (Ctrl+Shift+T), `lab_document` (Ctrl+Shift+Alt+T), `close_lab(cx)` (stops a run with "Lab stopped").
  - `build_runs(text_len, spans, faded: &[Range<usize>], marked, title_marker, caret_on_marker, style)`; `RunStyle::fade_alpha` replaces `RunStyle::trim`; `Theme::trim` is removed.

How it fits together:
- **The runner.** `run_lab` stops any running request, makes a fresh cancel flag, and runs the jobs in order on the background executor, checking the flag before each request. When it finishes it applies the results (unless the flag was raised), clears the flag and emits `LabDone`. `stop_ai` raises the flag and drops the task, so the background loop sends nothing more and the result is never applied. `lab_running()` is `ai_cancel.is_some()`, which AI alternates don't set; `request_ai_alternates` now calls `stop_ai` first too.
- **Trims.** `run_trim` skips empty and vanished paragraphs, takes cached answers (keyed by level, paragraph id and the export text sent, valid while the rendered text matches), and runs the rest. Results for paragraphs whose text changed are dropped. If every request failed, the first error is toasted and the old preview stays; if some failed, "N of M paragraphs failed". No cuts at all toasts "No cuts suggested". `Original` clears the preview.
- **Preview state** is view state. `drop_transient` (every edit) drops the cuts in the touched paragraphs, keeping the kept flags of the others; undo, redo and `load` clear the preview.
- **Navigation.** Tab / Shift+Tab / `]m` / `[m` put the caret at the start of the next or previous active cut, wrapping. Esc cancels a run first, then closes the preview ("Trim preview closed").
- **The popover.** The footer's `Popover` is controlled by `lab_open`. Ctrl+L, LAB (via `on_open_change`), `gl` (now `dispatch_action(OpenLab)`), Ctrl+Shift+T and Ctrl+Shift+Alt+T set the panel's scope and open it; the panel takes focus when it renders. `j`/`k`/arrows move the highlight (wrapping) and Enter runs it. Esc, a click outside or LAB close it, and focus returns to the page. A run that finishes without the AI (cache, `Original`) closes it at once (`LabEvent::Close`); one that waits keeps it open with "Working… Esc stops" until `LabDone`. Closing it earlier stops the run.

- [ ] **Step 1: Write the failing tests**

Create `crates/tulis/src/editor/lab_tests.rs`:

```rust
//! The Lab, from the editor's side: trim previews, the runner and its cancel flag.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use gpui::{Entity, TestAppContext, VisualTestContext};
use tulis_core::ai::{AiError, CompletionRequest, LlmProvider, MockProvider};
use tulis_core::vim::Vim;
use tulis_core::{DocPos, ParaId, Selection};

use super::lab::TrimSuggestion;
use super::tests::{editor_with, texts, type_str};
use super::{Editor, TrimLevel, TrimPreview};
use crate::actions::*;

fn mock(
    editor: &Entity<Editor>,
    cx: &mut VisualTestContext,
    responses: &[&str],
) -> Arc<MockProvider> {
    let mock = Arc::new(MockProvider::new(
        responses.iter().map(|r| Ok(r.to_string())).collect(),
    ));
    editor.update(cx, |e, _| e.provider_override = Some(mock.clone()));
    mock
}

fn ids(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> Vec<ParaId> {
    editor.read_with(cx, |e, _| e.doc.paragraphs.iter().map(|p| p.id).collect())
}

fn notice(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> Option<String> {
    editor.read_with(cx, |e, _| e.last_notice().map(str::to_string))
}

fn cut_ranges(
    editor: &Entity<Editor>,
    cx: &mut VisualTestContext,
) -> Vec<(usize, std::ops::Range<usize>)> {
    editor.read_with(cx, |e, _| {
        e.trim_preview
            .iter()
            .flat_map(|t| &t.cuts)
            .map(|c| (e.doc.para_index(c.para_id).unwrap(), c.range.clone()))
            .collect()
    })
}

/// A preview of `cuts` (paragraph index, range) at `Slight trim`, set directly.
fn preview(
    editor: &Entity<Editor>,
    cx: &mut VisualTestContext,
    cuts: &[(usize, std::ops::Range<usize>)],
) {
    editor.update(cx, |e, _| {
        let cuts = cuts
            .iter()
            .map(|(para, range)| TrimSuggestion {
                para_id: e.doc.paragraphs[*para].id,
                range: range.clone(),
            })
            .collect();
        e.trim_preview = Some(TrimPreview::new(TrimLevel::Slight, cuts));
    });
}

#[gpui::test]
fn a_trim_level_previews_cuts_per_paragraph_and_caches_them(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I really love tea.", "", "It is very, very good."]);
    let mock = mock(&editor, cx, &[r#"["really "]"#, r#"["very, "]"#]);
    let all = ids(&editor, cx);
    editor.update(cx, |e, cx| e.run_trim(TrimLevel::Slight, &all, cx));
    cx.run_until_parked();
    // One request per non-empty paragraph, asking for the level's share of words.
    let requests = mock.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].system.contains("10%"));
    assert_eq!(requests[1].user, "It is very, very good.");
    assert_eq!(cut_ranges(&editor, cx), vec![(0, 2..9), (2, 6..12)]);
    let level = editor.read_with(cx, |e, _| e.trim_preview.as_ref().map(|t| t.level));
    assert_eq!(level, Some(TrimLevel::Slight));
    assert!(editor.read_with(cx, |e, _| e.ai_task.is_none()));
    // Original clears the preview.
    editor.update(cx, |e, cx| e.run_trim(TrimLevel::Original, &all, cx));
    assert!(editor.read_with(cx, |e, _| e.trim_preview.is_none()));
    // The same level again comes from the cache, without a request.
    editor.update(cx, |e, cx| e.run_trim(TrimLevel::Slight, &all, cx));
    assert_eq!(cut_ranges(&editor, cx), vec![(0, 2..9), (2, 6..12)]);
    assert_eq!(mock.requests.lock().unwrap().len(), 2);
    // An edited paragraph, or another level, is asked again.
    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(2, 0)));
    type_str(&editor, cx, "So. ");
    editor.update(cx, |e, cx| e.run_trim(TrimLevel::Slight, &all, cx));
    cx.run_until_parked();
    let requests = mock.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 3);
    assert_eq!(requests[2].user, "So. It is very, very good.");
    editor.update(cx, |e, cx| e.run_trim(TrimLevel::Half, &all[..1], cx));
    cx.run_until_parked();
    let requests = mock.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 4);
    assert!(requests[3].system.contains("50%"));
}

#[gpui::test]
fn the_preview_fades_its_active_cuts(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I really love tea."]);
    preview(&editor, cx, &[(0, 2..9)]);
    editor.read_with(cx, |e, _| {
        let preview = e.trim_preview.as_ref().unwrap();
        assert_eq!(preview.active_ranges(e.doc.paragraphs[0].id), vec![2..9]);
    });
}

#[gpui::test]
fn editing_a_paragraph_drops_only_its_cuts(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I really love tea.", "It is very good."]);
    preview(&editor, cx, &[(0, 2..9), (1, 6..11)]);
    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(0, 0)));
    type_str(&editor, cx, "Oh, ");
    assert_eq!(cut_ranges(&editor, cx), vec![(1, 6..11)]);
    // Undo restores the text and leaves no preview.
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert!(editor.read_with(cx, |e, _| e.trim_preview.is_none()));
}

#[gpui::test]
fn tab_and_bracket_m_move_between_active_cuts(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I really love tea.", "It is very good."]);
    preview(&editor, cx, &[(0, 2..9), (1, 6..11)]);
    let caret = |cx: &mut VisualTestContext| editor.read_with(cx, |e, _| e.selection);
    editor.update_in(cx, |e, window, cx| e.next_mark(&NextMark, window, cx));
    assert_eq!(caret(cx), Selection::caret(DocPos::new(0, 2)));
    editor.update_in(cx, |e, window, cx| e.next_mark(&NextMark, window, cx));
    assert_eq!(caret(cx), Selection::caret(DocPos::new(1, 6)));
    // It wraps around, both ways.
    editor.update_in(cx, |e, window, cx| e.next_mark(&NextMark, window, cx));
    assert_eq!(caret(cx), Selection::caret(DocPos::new(0, 2)));
    editor.update_in(cx, |e, window, cx| e.prev_mark(&PrevMark, window, cx));
    assert_eq!(caret(cx), Selection::caret(DocPos::new(1, 6)));
    // Vim `]m` / `[m`, with a count.
    editor.update(cx, |e, _| {
        e.vim = Some(Vim::new());
        e.selection = Selection::caret(DocPos::new(0, 0));
    });
    type_str(&editor, cx, "2]m");
    assert_eq!(caret(cx), Selection::caret(DocPos::new(1, 6)));
    type_str(&editor, cx, "[m");
    assert_eq!(caret(cx), Selection::caret(DocPos::new(0, 2)));
    assert_eq!(
        texts(&editor, cx),
        vec!["I really love tea.", "It is very good."]
    );
}

#[gpui::test]
fn escape_cancels_a_running_trim_then_closes_the_preview(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I really love tea."]);
    let mock = mock(&editor, cx, &[r#"["really "]"#]);
    let all = ids(&editor, cx);
    editor.update(cx, |e, cx| e.run_trim(TrimLevel::Slight, &all, cx));
    editor.update_in(cx, |e, window, cx| e.cancel(&Cancel, window, cx));
    cx.run_until_parked();
    assert_eq!(notice(&editor, cx).as_deref(), Some("AI request cancelled"));
    assert!(mock.requests.lock().unwrap().is_empty());
    assert!(editor.read_with(cx, |e, _| e.trim_preview.is_none()));
    preview(&editor, cx, &[(0, 2..9)]);
    editor.update_in(cx, |e, window, cx| e.cancel(&Cancel, window, cx));
    assert!(editor.read_with(cx, |e, _| e.trim_preview.is_none()));
    assert_eq!(notice(&editor, cx).as_deref(), Some("Trim preview closed"));
}

/// Answers like `MockProvider`, then raises `flag`: as if Esc arrived while the first
/// paragraph's request was out.
struct StopAfterFirst {
    mock: MockProvider,
    flag: Mutex<Option<Arc<AtomicBool>>>,
}

impl LlmProvider for StopAfterFirst {
    fn complete(&self, req: &CompletionRequest) -> Result<String, AiError> {
        let result = self.mock.complete(req);
        if let Some(flag) = self.flag.lock().unwrap().as_ref() {
            flag.store(true, Ordering::SeqCst);
        }
        result
    }

    fn name(&self) -> &str {
        "mock"
    }

    fn model(&self) -> &str {
        "mock-model"
    }
}

#[gpui::test]
fn the_cancel_flag_stops_the_remaining_paragraph_requests(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["One really.", "Two really.", "Three really."]);
    let provider = Arc::new(StopAfterFirst {
        mock: MockProvider::new(vec![Ok(r#"["really"]"#.into()); 3]),
        flag: Mutex::new(None),
    });
    editor.update(cx, |e, _| e.provider_override = Some(provider.clone()));
    let all = ids(&editor, cx);
    editor.update(cx, |e, cx| e.run_trim(TrimLevel::Slight, &all, cx));
    let flag = editor
        .read_with(cx, |e, _| e.ai_cancel.clone())
        .expect("a cancel flag");
    *provider.flag.lock().unwrap() = Some(flag);
    cx.run_until_parked();
    assert_eq!(provider.mock.requests.lock().unwrap().len(), 1);
    assert!(editor.read_with(cx, |e, _| e.trim_preview.is_none()));
}

#[gpui::test]
fn partial_failures_toast_how_many_paragraphs_failed(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I really love tea.", "It is very good."]);
    let mock = Arc::new(MockProvider::new(vec![
        Ok(r#"["really "]"#.into()),
        Err(AiError::Network("down".into())),
    ]));
    editor.update(cx, |e, _| e.provider_override = Some(mock.clone()));
    let all = ids(&editor, cx);
    editor.update(cx, |e, cx| e.run_trim(TrimLevel::Tighten, &all, cx));
    cx.run_until_parked();
    assert_eq!(cut_ranges(&editor, cx), vec![(0, 2..9)]);
    assert_eq!(
        notice(&editor, cx).as_deref(),
        Some("1 of 2 paragraphs failed")
    );
    // When every request fails, the error itself is shown and nothing is previewed.
    let mock = Arc::new(MockProvider::new(vec![Err(AiError::Refused)]));
    editor.update(cx, |e, cx| {
        e.provider_override = Some(mock.clone());
        e.run_trim(TrimLevel::Sharper, &all[..1], cx);
    });
    cx.run_until_parked();
    assert_eq!(
        notice(&editor, cx).as_deref(),
        Some("The model declined this request")
    );
    // The earlier preview stays.
    assert_eq!(cut_ranges(&editor, cx), vec![(0, 2..9)]);
}
```

The other test changes: core `]m`/`[m`, the command table, the faded run, the v1 trim tests (removed, or moved to the preview), and the workspace's popover tests.

```diff
diff --git a/crates/tulis-core/src/vim.rs b/crates/tulis-core/src/vim.rs
index 2d99337..f229d88 100644
--- a/crates/tulis-core/src/vim.rs
+++ b/crates/tulis-core/src/vim.rs
@@ -655,6 +655,27 @@ mod tests {
         );
     }
 
+    #[test]
+    fn bracket_m_jumps_between_marks() {
+        let mut v = Vim::new();
+        assert_eq!(
+            feed(&mut v, "]m"),
+            vec![JumpMark {
+                forward: true,
+                count: 1
+            }]
+        );
+        assert_eq!(
+            feed(&mut v, "2[m"),
+            vec![JumpMark {
+                forward: false,
+                count: 2
+            }]
+        );
+        assert_eq!(feed(&mut v, "d]m"), vec![]);
+        assert!(!v.is_pending());
+    }
+
     #[test]
     fn g_view_commands() {
         let mut v = Vim::new();
diff --git a/crates/tulis/src/commands.rs b/crates/tulis/src/commands.rs
index b4e9f0e..c4e2ea6 100644
--- a/crates/tulis/src/commands.rs
+++ b/crates/tulis/src/commands.rs
@@ -722,6 +722,8 @@ mod tests {
             VimCommand::Redo { .. } => "redo",
             VimCommand::CycleAlt { forward: true, .. } => "cycle_next",
             VimCommand::CycleAlt { forward: false, .. } => "cycle_prev",
+            VimCommand::JumpMark { forward: true, .. } => "next_mark",
+            VimCommand::JumpMark { forward: false, .. } => "prev_mark",
             VimCommand::ToggleGhost => "toggle_ghost",
             VimCommand::CycleTheme => "cycle_theme",
             VimCommand::ToggleMarkers => "toggle_markers",
@@ -772,6 +774,14 @@ mod tests {
                 forward: false,
                 count: 1,
             },
+            VimCommand::JumpMark {
+                forward: true,
+                count: 1,
+            },
+            VimCommand::JumpMark {
+                forward: false,
+                count: 1,
+            },
             VimCommand::ToggleGhost,
             VimCommand::CycleTheme,
             VimCommand::ToggleMarkers,
@@ -819,6 +829,7 @@ mod tests {
         assert_eq!(key_label("ctrl--"), "Ctrl+-");
         assert_eq!(key_label("secondary-/"), format!("{ctrl}+/"));
         assert_eq!(key_label("secondary-shift-a"), format!("{ctrl}+Shift+A"));
+        assert_eq!(key_label("shift-tab"), "Shift+Tab");
     }
 
     #[test]
diff --git a/crates/tulis/src/editor/mod.rs b/crates/tulis/src/editor/mod.rs
index f196e21..6ad980c 100644
--- a/crates/tulis/src/editor/mod.rs
+++ b/crates/tulis/src/editor/mod.rs
@@ -6,6 +6,8 @@ mod alt_session;
 mod element;
 mod features;
 mod input;
+#[cfg(test)]
+mod lab_tests;
 pub mod layout;
 mod markers;
 #[cfg(test)]
diff --git a/crates/tulis/src/editor/tests.rs b/crates/tulis/src/editor/tests.rs
index 9aa1996..a69eaec 100644
--- a/crates/tulis/src/editor/tests.rs
+++ b/crates/tulis/src/editor/tests.rs
@@ -2,7 +2,8 @@ use gpui::{Entity, EntityInputHandler, TestAppContext, VisualTestContext};
 use tulis_core::config::Config;
 use tulis_core::{DocPos, Document, Selection};
 
-use super::{AltEdit, Editor, TrimSuggestion};
+use super::lab::TrimSuggestion;
+use super::{AltEdit, Editor, TrimPreview};
 use crate::actions::*;
 
 pub(crate) fn editor_with<'a>(
@@ -389,33 +390,6 @@ fn dropping_a_block_puts_it_where_it_lands(cx: &mut TestAppContext) {
     assert_eq!(overflow(cx), "Block.\n\nOther.");
 }
 
-#[gpui::test]
-fn accepting_a_trim_ghosts_it_and_escape_dismisses(cx: &mut TestAppContext) {
-    let (editor, cx) = editor_with(cx, &["I really love tea, really."]);
-    editor.update(cx, |e, _| {
-        let id = e.doc.paragraphs[0].id;
-        e.trims = vec![
-            TrimSuggestion {
-                para_id: id,
-                range: 2..9,
-            }, // "really "
-            TrimSuggestion {
-                para_id: id,
-                range: 18..25,
-            }, // " really"
-        ];
-    });
-    // The cursor is at (0,0): the next suggestion at/after it is 2..9.
-    editor.update_in(cx, |e, window, cx| e.accept_trim(&AcceptTrim, window, cx));
-    assert_eq!(
-        editor.read_with(cx, |e, _| e.doc.paragraphs[0].export_text()),
-        "I love tea, really."
-    );
-    assert_eq!(editor.read_with(cx, |e, _| e.trims.len()), 1);
-    editor.update_in(cx, |e, window, cx| e.cancel(&Cancel, window, cx));
-    assert!(editor.read_with(cx, |e, _| e.trims.is_empty()));
-}
-
 use std::sync::Arc;
 use tulis_core::ai::MockProvider;
 
@@ -475,22 +449,6 @@ fn stale_ai_result_is_dropped(cx: &mut TestAppContext) {
     );
 }
 
-#[gpui::test]
-fn ai_trim_suggests_cuts_that_tab_accepts(cx: &mut TestAppContext) {
-    let (editor, cx) = editor_with(cx, &["I really love tea."]);
-    with_mock(&editor, cx, vec![r#"["really "]"#]);
-    editor.update_in(cx, |e, window, cx| {
-        e.ai_trim_paragraph(&AiTrimParagraph, window, cx)
-    });
-    cx.run_until_parked();
-    assert_eq!(editor.read_with(cx, |e, _| e.trims.clone()).len(), 1);
-    editor.update_in(cx, |e, window, cx| e.accept_trim(&AcceptTrim, window, cx));
-    assert_eq!(
-        editor.read_with(cx, |e, _| e.doc.paragraphs[0].export_text()),
-        "I love tea."
-    );
-}
-
 #[gpui::test]
 fn missing_api_key_is_reported(cx: &mut TestAppContext) {
     let (editor, cx) = editor_with(cx, &["I love tea"]);
@@ -802,15 +760,6 @@ fn vim_gm_toggles_markers(cx: &mut TestAppContext) {
     );
 }
 
-#[gpui::test]
-fn vim_gl_runs_the_document_trim_until_the_lab_exists(cx: &mut TestAppContext) {
-    let (editor, cx) = vim_editor(cx, &["I really love tea."]);
-    with_mock(&editor, cx, vec![r#"["really "]"#]);
-    type_str(&editor, cx, "gl");
-    cx.run_until_parked();
-    assert_eq!(editor.read_with(cx, |e, _| e.trims.len()), 1);
-}
-
 #[gpui::test]
 fn cycling_boxes_the_group_until_the_cursor_leaves(cx: &mut TestAppContext) {
     let (editor, cx) = editor_with(cx, &["I love tea"]);
@@ -1309,10 +1258,13 @@ fn every_preview_clears_trims_and_the_cycled_box(cx: &mut TestAppContext) {
     editor.update(cx, |e, cx| e.preview_alternate(1, cx));
     editor.update(cx, |e, cx| {
         let para_id = e.doc.paragraphs[0].id;
-        e.trims = vec![TrimSuggestion {
-            para_id,
-            range: 0..1,
-        }];
+        e.trim_preview = Some(TrimPreview::new(
+            super::TrimLevel::Slight,
+            vec![TrimSuggestion {
+                para_id,
+                range: 0..1,
+            }],
+        ));
         e.cycled = Some(super::CycledGroup {
             para_id,
             range: 2..7,
@@ -1320,7 +1272,7 @@ fn every_preview_clears_trims_and_the_cycled_box(cx: &mut TestAppContext) {
         e.preview_alternate(2, cx);
     });
     editor.read_with(cx, |e, _| {
-        assert!(e.trims.is_empty());
+        assert!(e.trim_preview.as_ref().unwrap().cuts.is_empty());
         assert!(e.cycled.is_none());
     });
 }
@@ -1465,13 +1417,16 @@ fn escape_on_the_page_closes_the_sidebar_when_nothing_else_is_pending(cx: &mut T
     editor.update(cx, |e, cx| e.open_alternates(None, cx));
     escape(cx);
     assert!(!open(cx));
-    // Trim suggestions are dismissed first.
+    // A trim preview is closed first.
     editor.update(cx, |e, cx| {
         let para_id = e.doc.paragraphs[0].id;
-        e.trims = vec![TrimSuggestion {
-            para_id,
-            range: 0..1,
-        }];
+        e.trim_preview = Some(TrimPreview::new(
+            super::TrimLevel::Slight,
+            vec![TrimSuggestion {
+                para_id,
+                range: 0..1,
+            }],
+        ));
         e.open_alternates(None, cx);
     });
     escape(cx);
diff --git a/crates/tulis/src/runs.rs b/crates/tulis/src/runs.rs
index 7745499..7f72cbe 100644
--- a/crates/tulis/src/runs.rs
+++ b/crates/tulis/src/runs.rs
@@ -99,7 +99,7 @@ mod tests {
             font: font("Test"),
             title_font,
             color: hsla(0., 0., 0., 1.),
-            trim: hsla(0., 1., 0.5, 1.),
+            fade_alpha: 0.35,
             marker: hsla(0., 0., 0.5, 0.5),
             ghost_alpha: 0.1,
         }
@@ -131,8 +131,10 @@ mod tests {
         );
         assert!(runs.iter().all(|r| r.underline.is_none()));
         assert!((runs[2].color.a - 0.1).abs() < 1e-6);
-        assert!(runs[3].strikethrough.is_some());
-        assert!(runs[4].strikethrough.is_none());
+        // A trim preview's cut is faded, not struck through.
+        assert!(runs.iter().all(|r| r.strikethrough.is_none()));
+        assert!((runs[3].color.a - 0.1 * 0.35).abs() < 1e-6);
+        assert!((runs[4].color.a - 0.1).abs() < 1e-6);
     }
 
     #[test]
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
index ad08244..f233479 100644
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -1210,6 +1210,162 @@ mod tests {
         assert_eq!(scratchpad_caret(&ws, cx), original.len());
     }
 
+    fn draw(cx: &mut gpui::VisualTestContext) {
+        cx.update(|window, cx| window.draw(cx).clear(cx));
+    }
+
+    fn lab_open(ws: &gpui::Entity<Workspace>, cx: &mut gpui::VisualTestContext) -> bool {
+        ws.read_with(cx, |ws, _| ws.lab_open)
+    }
+
+    fn lab_focused(ws: &gpui::Entity<Workspace>, cx: &mut gpui::VisualTestContext) -> bool {
+        let lab = ws.read_with(cx, |ws, _| ws.lab.clone());
+        cx.update(|window, cx| lab.read(cx).focus.is_focused(window))
+    }
+
+    fn lab_scope(ws: &gpui::Entity<Workspace>, cx: &mut gpui::VisualTestContext) -> Vec<usize> {
+        ws.read_with(cx, |ws, cx| {
+            let doc = &ws.editor.read(cx).doc;
+            let scope = &ws.lab.read(cx).scope;
+            scope.iter().filter_map(|id| doc.para_index(*id)).collect()
+        })
+    }
+
+    fn lab_mock(
+        ws: &gpui::Entity<Workspace>,
+        cx: &mut gpui::VisualTestContext,
+        responses: &[&str],
+    ) -> std::sync::Arc<tulis_core::ai::MockProvider> {
+        let mock = std::sync::Arc::new(tulis_core::ai::MockProvider::new(
+            responses.iter().map(|r| Ok(r.to_string())).collect(),
+        ));
+        ws.update(cx, |ws, cx| {
+            ws.editor
+                .update(cx, |e, _| e.provider_override = Some(mock.clone()))
+        });
+        mock
+    }
+
+    #[gpui::test]
+    fn ctrl_l_opens_the_lab_on_the_document_and_escape_closes_it(cx: &mut TestAppContext) {
+        let (ws, cx) = root_workspace(cx, &["One.", "Two."]);
+        cx.simulate_keystrokes("secondary-l");
+        draw(cx);
+        assert!(lab_open(&ws, cx));
+        assert!(lab_focused(&ws, cx));
+        assert!(cx.debug_bounds("lab-panel").is_some());
+        assert_eq!(lab_scope(&ws, cx), vec![0, 1]);
+        cx.simulate_keystrokes("escape");
+        draw(cx);
+        assert!(!lab_open(&ws, cx));
+        assert!(page_focused(&ws, cx));
+        assert!(cx.debug_bounds("lab-panel").is_none());
+    }
+
+    #[gpui::test]
+    fn lab_keys_pick_a_trim_level_and_the_lab_closes_when_it_is_done(cx: &mut TestAppContext) {
+        let (ws, cx) = root_workspace(cx, &["I really love tea."]);
+        let mock = lab_mock(&ws, cx, &[r#"["really "]"#]);
+        cx.simulate_keystrokes("secondary-l");
+        draw(cx);
+        cx.simulate_keystrokes("j j down k up right");
+        let highlight = ws.read_with(cx, |ws, cx| ws.lab.read(cx).highlight);
+        assert_eq!(highlight, 2);
+        cx.simulate_keystrokes("left enter");
+        draw(cx);
+        assert_eq!(mock.requests.lock().unwrap().len(), 1);
+        let preview = ws.read_with(cx, |ws, cx| ws.editor.read(cx).trim_preview.clone());
+        let preview = preview.expect("a trim preview");
+        assert_eq!(preview.level, crate::editor::TrimLevel::Slight);
+        assert_eq!(preview.cuts.len(), 1);
+        assert!(!lab_open(&ws, cx));
+        assert!(page_focused(&ws, cx));
+        // Nothing is cut until asked.
+        let text = ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.paragraphs[0].text());
+        assert_eq!(text, "I really love tea.");
+    }
+
+    #[gpui::test]
+    fn the_lab_works_on_the_cursor_paragraph_the_selection_or_the_document(
+        cx: &mut TestAppContext,
+    ) {
+        let (ws, cx) = root_workspace(cx, &["One really.", "Two really.", "Three."]);
+        let mock = lab_mock(&ws, cx, &[r#"["really"]"#]);
+        let select = |ws: &gpui::Entity<Workspace>, cx: &mut gpui::VisualTestContext, a, b| {
+            ws.update(cx, |ws, cx| {
+                ws.editor
+                    .update(cx, |e, _| e.selection = Selection { anchor: a, head: b })
+            })
+        };
+        select(&ws, cx, DocPos::new(1, 2), DocPos::new(1, 2));
+        cx.simulate_keystrokes("secondary-shift-t");
+        draw(cx);
+        assert_eq!(lab_scope(&ws, cx), vec![1]);
+        cx.simulate_keystrokes("down enter");
+        draw(cx);
+        let requests = mock.requests.lock().unwrap().clone();
+        assert_eq!(requests.len(), 1);
+        assert_eq!(requests[0].user, "Two really.");
+        cx.simulate_keystrokes("secondary-shift-alt-t");
+        draw(cx);
+        assert_eq!(lab_scope(&ws, cx), vec![0, 1, 2]);
+        cx.simulate_keystrokes("escape");
+        draw(cx);
+        select(&ws, cx, DocPos::new(0, 1), DocPos::new(1, 2));
+        cx.simulate_keystrokes("secondary-l");
+        draw(cx);
+        assert_eq!(lab_scope(&ws, cx), vec![0, 1]);
+    }
+
+    #[gpui::test]
+    fn closing_the_lab_while_it_works_stops_the_remaining_requests(cx: &mut TestAppContext) {
+        let (ws, cx) = root_workspace(cx, &["One really.", "Two really."]);
+        let mock = lab_mock(&ws, cx, &[r#"["really"]"#, r#"["really"]"#]);
+        cx.simulate_keystrokes("secondary-l");
+        draw(cx);
+        let lab = ws.read_with(cx, |ws, _| ws.lab.clone());
+        lab.update(cx, |lab, cx| {
+            lab.run(
+                crate::lab_popover::LabItem::Trim(crate::editor::TrimLevel::Slight),
+                cx,
+            )
+        });
+        // It stays open while the AI works.
+        assert!(lab_open(&ws, cx));
+        assert!(ws.read_with(cx, |ws, cx| ws.editor.read(cx).lab_running()));
+        ws.update(cx, |ws, cx| ws.close_lab(cx));
+        cx.run_until_parked();
+        assert!(mock.requests.lock().unwrap().len() < 2);
+        let editor = ws.read_with(cx, |ws, _| ws.editor.clone());
+        editor.read_with(cx, |e, _| {
+            assert!(e.trim_preview.is_none());
+            assert!(!e.lab_running());
+            assert_eq!(e.last_notice(), Some("Lab stopped"));
+        });
+    }
+
+    #[gpui::test]
+    fn the_lab_button_and_vim_gl_open_the_lab(cx: &mut TestAppContext) {
+        let (ws, cx) = root_workspace(cx, &["One."]);
+        draw(cx);
+        let button = cx.debug_bounds("lab-button").expect("the LAB button");
+        cx.simulate_click(button.center(), gpui::Modifiers::none());
+        draw(cx);
+        assert!(lab_open(&ws, cx));
+        assert!(lab_focused(&ws, cx));
+        cx.simulate_click(button.center(), gpui::Modifiers::none());
+        draw(cx);
+        assert!(!lab_open(&ws, cx));
+        let editor = ws.read_with(cx, |ws, _| ws.editor.clone());
+        editor.update_in(cx, |e, window, cx| {
+            e.vim = Some(tulis_core::vim::Vim::new());
+            e.replace_text_in_range(None, "g", window, cx);
+            e.replace_text_in_range(None, "l", window, cx);
+        });
+        draw(cx);
+        assert!(lab_open(&ws, cx));
+    }
+
     fn sidebar_focused(ws: &gpui::Entity<Workspace>, cx: &mut gpui::VisualTestContext) -> bool {
         let sidebar = ws.read_with(cx, |ws, _| ws.alt_sidebar.clone());
         sidebar.is_some_and(|s| cx.update(|window, cx| s.read(cx).list_focus.is_focused(window)))
```

- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile errors (`JumpMark`, `TrimLevel`, `run_trim`, `lab_open`, … don't exist).

- [ ] **Step 3: Implement**

Create `crates/tulis/src/editor/lab.rs`:

```rust
//! The Lab on the page: trim previews, the runner every Lab request goes through, and its
//! cancel flag.

use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{Context, Window, prelude::*};
use tulis_core::ai::{AiError, CompletionRequest, prompts};
use tulis_core::{DocPos, ParaId, Selection};

use super::{Editor, EditorEvent};
use crate::actions::{NextMark, PrevMark};

/// How much a trim asks the AI to cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrimLevel {
    Original,
    Slight,
    Tighten,
    Sharper,
    Half,
}

impl TrimLevel {
    pub const ALL: [TrimLevel; 5] = [
        TrimLevel::Original,
        TrimLevel::Slight,
        TrimLevel::Tighten,
        TrimLevel::Sharper,
        TrimLevel::Half,
    ];

    /// The share of the words to cut.
    pub fn percent(self) -> u8 {
        match self {
            TrimLevel::Original => 0,
            TrimLevel::Slight => 10,
            TrimLevel::Tighten => 20,
            TrimLevel::Sharper => 30,
            TrimLevel::Half => 50,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TrimLevel::Original => "Original",
            TrimLevel::Slight => "Slight trim",
            TrimLevel::Tighten => "Tighten more",
            TrimLevel::Sharper => "Even sharper",
            TrimLevel::Half => "Cut in half",
        }
    }
}

/// A passage a trim would cut: a byte range of its paragraph's rendered text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrimSuggestion {
    pub para_id: ParaId,
    pub range: Range<usize>,
}

/// The faded preview of a trim level. It is view state: editing a paragraph drops its cuts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrimPreview {
    pub level: TrimLevel,
    /// In document order.
    pub cuts: Vec<TrimSuggestion>,
    /// Indices into `cuts` the writer chose to keep; the others are active.
    pub kept: HashSet<usize>,
}

impl TrimPreview {
    pub fn new(level: TrimLevel, cuts: Vec<TrimSuggestion>) -> Self {
        Self {
            level,
            cuts,
            kept: HashSet::new(),
        }
    }

    pub fn is_active(&self, ix: usize) -> bool {
        ix < self.cuts.len() && !self.kept.contains(&ix)
    }

    /// The active cuts in paragraph `id` (drawn faded).
    pub fn active_ranges(&self, id: ParaId) -> Vec<Range<usize>> {
        self.cuts
            .iter()
            .enumerate()
            .filter(|(ix, cut)| cut.para_id == id && self.is_active(*ix))
            .map(|(_, cut)| cut.range.clone())
            .collect()
    }

    /// Keep only the cuts `keep` accepts; kept cuts stay kept.
    pub(crate) fn retain(&mut self, keep: impl Fn(&TrimSuggestion) -> bool) {
        let cuts = std::mem::take(&mut self.cuts);
        let kept = std::mem::take(&mut self.kept);
        for (ix, cut) in cuts.into_iter().enumerate() {
            if keep(&cut) {
                if kept.contains(&ix) {
                    self.kept.insert(self.cuts.len());
                }
                self.cuts.push(cut);
            }
        }
    }
}

/// One paragraph's Lab request, with the paragraph as it was when asked (results for a
/// paragraph that changed meanwhile are dropped).
pub(crate) struct LabJob {
    pub para_id: ParaId,
    /// The rendered text.
    pub text: String,
    /// The export text (what was sent).
    pub export: String,
    pub request: CompletionRequest,
}

/// Each job that ran, with what came back.
pub(crate) type LabResults = Vec<(LabJob, Result<String, AiError>)>;

/// Key of the trim cache: level, paragraph and the export text that was sent.
pub(crate) type TrimKey = (TrimLevel, ParaId, String);

/// What a finished run found, for one paragraph: its rendered text then, and the ranges.
type Found = Vec<(ParaId, String, Vec<Range<usize>>)>;

/// The toast for a run in which `failed` of `total` requests failed (`None` when none did).
/// When all of them failed, it is the first error.
fn failure_notice(failed: usize, total: usize, first_error: Option<String>) -> Option<String> {
    match failed {
        0 => None,
        n if n == total => first_error,
        n => Some(format!("{n} of {total} paragraphs failed")),
    }
}

impl Editor {
    /// The paragraphs the Lab works on: the selection's, or the whole document's when nothing is
    /// selected.
    pub fn lab_scope(&self) -> Vec<ParaId> {
        let range = self.selection.range();
        let paras = if range.start == range.end {
            0..self.doc.paragraphs.len()
        } else {
            range.start.para..range.end.para + 1
        };
        self.doc.paragraphs[paras].iter().map(|p| p.id).collect()
    }

    /// A Lab run is waiting for the AI.
    pub fn lab_running(&self) -> bool {
        self.ai_cancel.is_some()
    }

    /// Run `jobs` one at a time off the UI thread, then hand what came back to `apply` with the
    /// number of jobs, and emit `LabDone`. The run's cancel flag is checked before each
    /// request; `stop_ai` sets it and drops the task, so a cancelled run sends no more requests
    /// and applies nothing.
    pub(crate) fn run_lab(
        &mut self,
        jobs: Vec<LabJob>,
        working: &str,
        apply: impl FnOnce(&mut Self, LabResults, usize, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(provider) = self.provider(cx) else {
            return;
        };
        self.stop_ai();
        let cancel = Arc::new(AtomicBool::new(false));
        self.ai_cancel = Some(cancel.clone());
        let total = jobs.len();
        let flag = cancel.clone();
        let work = cx.background_spawn(async move {
            let mut results = Vec::new();
            for job in jobs {
                if flag.load(Ordering::SeqCst) {
                    break;
                }
                let result = provider.complete(&job.request);
                results.push((job, result));
            }
            results
        });
        self.ai_task = Some(cx.spawn(async move |this, cx| {
            let results = work.await;
            this.update(cx, |editor, cx| {
                editor.ai_task = None;
                editor.ai_cancel = None;
                if !cancel.load(Ordering::SeqCst) {
                    apply(editor, results, total, cx);
                }
                cx.emit(EditorEvent::LabDone);
                cx.notify();
            })
            .ok();
        }));
        self.set_notice(working, cx);
    }

    /// Stop the running AI request or Lab run: raise its cancel flag, so no further paragraph
    /// is sent, and drop its task. Returns whether one was running.
    pub(crate) fn stop_ai(&mut self) -> bool {
        if let Some(flag) = self.ai_cancel.take() {
            flag.store(true, Ordering::SeqCst);
        }
        self.ai_task.take().is_some()
    }

    /// Preview trim `level` over the paragraphs in `scope` (ids of paragraphs that are gone are
    /// skipped): one request per non-empty paragraph, unless the cache has its answer.
    /// `Original` clears the preview.
    pub fn run_trim(&mut self, level: TrimLevel, scope: &[ParaId], cx: &mut Context<Self>) {
        self.finish_alt_edit();
        if level == TrimLevel::Original {
            self.trim_preview = None;
            cx.notify();
            return;
        }
        let mut cached: Found = Vec::new();
        let mut jobs = Vec::new();
        for &id in scope {
            let Some(para) = self.doc.para_index(id) else {
                continue;
            };
            let p = &self.doc.paragraphs[para];
            let (text, export) = (p.text(), p.export_text());
            if export.trim().is_empty() {
                continue;
            }
            match self.trim_cache.get(&(level, id, export.clone())) {
                Some((cached_text, ranges)) if *cached_text == text => {
                    cached.push((id, text, ranges.clone()))
                }
                _ => jobs.push(LabJob {
                    para_id: id,
                    request: prompts::trim_request(&export, level.percent()),
                    text,
                    export,
                }),
            }
        }
        if jobs.is_empty() {
            if cached.is_empty() {
                return self.set_notice("Nothing to trim", cx);
            }
            return self.show_trim(level, cached, cx);
        }
        self.run_lab(
            jobs,
            "AI is looking for cuts…",
            move |editor, results, total, cx| {
                editor.apply_trim_results(level, cached, results, total, cx)
            },
            cx,
        );
    }

    fn apply_trim_results(
        &mut self,
        level: TrimLevel,
        mut found: Found,
        results: LabResults,
        total: usize,
        cx: &mut Context<Self>,
    ) {
        let mut failed = 0;
        let mut first_error = None;
        for (job, result) in results {
            let response = match result {
                Ok(response) => response,
                Err(e) => {
                    failed += 1;
                    first_error.get_or_insert(e.to_string());
                    continue;
                }
            };
            let Some(para) = self.doc.para_index(job.para_id) else {
                continue;
            };
            let p = &self.doc.paragraphs[para];
            if p.text() != job.text {
                continue;
            }
            let ranges = prompts::locate_trims(&prompts::parse_trim(&response), p);
            self.trim_cache.insert(
                (level, job.para_id, job.export),
                (job.text.clone(), ranges.clone()),
            );
            found.push((job.para_id, job.text, ranges));
        }
        let notice = failure_notice(failed, total, first_error);
        let all_failed = failed == total && found.is_empty();
        if !all_failed {
            self.show_trim(level, found, cx);
        }
        if let Some(notice) = notice {
            self.set_notice(notice, cx);
        }
    }

    /// Show the preview of `found` (paragraphs whose text changed since are left out).
    fn show_trim(&mut self, level: TrimLevel, found: Found, cx: &mut Context<Self>) {
        let mut cuts: Vec<(usize, TrimSuggestion)> = Vec::new();
        for (id, text, ranges) in found {
            let Some(para) = self.doc.para_index(id) else {
                continue;
            };
            if self.doc.paragraphs[para].text() != text {
                continue;
            }
            cuts.extend(
                ranges
                    .into_iter()
                    .map(|range| (para, TrimSuggestion { para_id: id, range })),
            );
        }
        cuts.sort_by_key(|(para, cut)| (*para, cut.range.start));
        if cuts.is_empty() {
            self.trim_preview = None;
            self.set_notice("No cuts suggested", cx);
        } else {
            let cuts = cuts.into_iter().map(|(_, cut)| cut).collect();
            self.trim_preview = Some(TrimPreview::new(level, cuts));
        }
        cx.notify();
    }

    /// The active cuts of the trim preview, as `(paragraph, range)` in document order.
    fn lab_items(&self) -> Vec<(usize, Range<usize>)> {
        let Some(preview) = &self.trim_preview else {
            return Vec::new();
        };
        preview
            .cuts
            .iter()
            .enumerate()
            .filter(|(ix, _)| preview.is_active(*ix))
            .filter_map(|(_, cut)| Some((self.doc.para_index(cut.para_id)?, cut.range.clone())))
            .collect()
    }

    /// Tab, `]m`, `[m`: put the cursor at the start of the next (or previous) active cut,
    /// `count` times, wrapping around.
    pub(crate) fn jump_lab_item(&mut self, forward: bool, count: usize, cx: &mut Context<Self>) {
        let items = self.lab_items();
        if items.is_empty() {
            return;
        }
        let start = self.selection.range().start;
        let mut at = (start.para, start.offset);
        for _ in 0..count.max(1) {
            let next = if forward {
                items
                    .iter()
                    .find(|(p, r)| (*p, r.start) > at)
                    .or(items.first())
            } else {
                items
                    .iter()
                    .rev()
                    .find(|(p, r)| (*p, r.start) < at)
                    .or(items.last())
            };
            if let Some((para, range)) = next {
                at = (*para, range.start);
            }
        }
        self.set_selection(Selection::caret(DocPos::new(at.0, at.1)), cx);
    }

    pub(super) fn next_mark(&mut self, _: &NextMark, _: &mut Window, cx: &mut Context<Self>) {
        self.jump_lab_item(true, 1, cx);
    }

    pub(super) fn prev_mark(&mut self, _: &PrevMark, _: &mut Window, cx: &mut Context<Self>) {
        self.jump_lab_item(false, 1, cx);
    }
}
```

Create `crates/tulis/src/lab_popover.rs`:

```rust
//! The Lab popover, anchored above the footer's LAB button: what the Lab can do to its
//! paragraphs (the selection's, the cursor's or the whole document's).

use gpui::{
    App, ClickEvent, Context, Entity, EventEmitter, FocusHandle, Hsla, IntoElement, ParentElement,
    Render, RenderOnce, Styled, Subscription, Window, div, prelude::*, px,
};
use gpui_component::Selectable;
use tulis_core::ParaId;

use crate::actions::{LabChoose, LabNext, LabPrev};
use crate::editor::{Editor, TrimLevel};

pub const LAB_WIDTH: f32 = 340.;

/// Something the Lab can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabItem {
    Trim(TrimLevel),
}

impl LabItem {
    /// In the order the popover lists them (and the arrow keys walk them).
    pub fn all() -> Vec<LabItem> {
        TrimLevel::ALL.into_iter().map(LabItem::Trim).collect()
    }
}

/// The popover asks the workspace to close it.
pub enum LabEvent {
    Close,
}

pub struct LabPanel {
    editor: Entity<Editor>,
    pub(crate) focus: FocusHandle,
    /// The paragraphs the Lab works on.
    pub(crate) scope: Vec<ParaId>,
    /// Index into `LabItem::all()`.
    pub(crate) highlight: usize,
    _subscription: Subscription,
}

impl EventEmitter<LabEvent> for LabPanel {}

impl LabPanel {
    pub fn new(editor: Entity<Editor>, cx: &mut Context<Self>) -> Self {
        Self {
            _subscription: cx.observe(&editor, |_, _, cx| cx.notify()),
            editor,
            focus: cx.focus_handle(),
            scope: Vec::new(),
            highlight: 0,
        }
    }

    /// Work on `scope` from now on, starting from the first row.
    pub(crate) fn reset(&mut self, scope: Vec<ParaId>, cx: &mut Context<Self>) {
        self.scope = scope;
        self.highlight = 0;
        cx.notify();
    }

    fn move_highlight(&mut self, forward: bool, cx: &mut Context<Self>) {
        let count = LabItem::all().len();
        self.highlight = if forward {
            (self.highlight + 1) % count
        } else {
            (self.highlight + count - 1) % count
        };
        cx.notify();
    }

    fn next(&mut self, _: &LabNext, _: &mut Window, cx: &mut Context<Self>) {
        self.move_highlight(true, cx);
    }

    fn prev(&mut self, _: &LabPrev, _: &mut Window, cx: &mut Context<Self>) {
        self.move_highlight(false, cx);
    }

    fn choose(&mut self, _: &LabChoose, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(item) = LabItem::all().get(self.highlight).copied() {
            self.run(item, cx);
        }
    }

    /// Run `item` on the scope. The popover closes now, or when the AI is done (closing it
    /// before then stops the run).
    pub(crate) fn run(&mut self, item: LabItem, cx: &mut Context<Self>) {
        let scope = self.scope.clone();
        self.editor.update(cx, |e, cx| match item {
            LabItem::Trim(level) => e.run_trim(level, &scope, cx),
        });
        if !self.editor.read(cx).lab_running() {
            cx.emit(LabEvent::Close);
        }
    }
}

impl Render for LabPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let t = editor.theme;
        let running = editor.lab_running();
        let highlight = self.highlight;
        let items = LabItem::all();
        let trims = items.iter().enumerate().map(|(ix, item)| {
            let LabItem::Trim(level) = item;
            (ix, *level)
        });
        div()
            .id("lab-panel")
            .debug_selector(|| "lab-panel".into())
            .key_context("Lab")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::next))
            .on_action(cx.listener(Self::prev))
            .on_action(cx.listener(Self::choose))
            .w(px(LAB_WIDTH))
            .flex()
            .flex_col()
            .gap_2()
            .text_sm()
            .text_color(t.text)
            .child(div().italic().text_color(t.accent).child("The Lab guide…"))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_1()
                    .children(trims.map(|(ix, level)| {
                        div()
                            .id(("lab-trim", ix))
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .border_1()
                            .border_color(t.rule)
                            .text_xs()
                            .cursor_pointer()
                            .when(ix == highlight, |d| d.bg(t.selection))
                            .hover(|d| d.border_color(t.accent))
                            .child(level.label())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.highlight = ix;
                                this.run(LabItem::Trim(level), cx);
                            }))
                    })),
            )
            .when(running, |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child("Working… Esc stops"),
                )
            })
    }
}

/// The footer's dashed LAB button: the Lab popover's trigger.
#[derive(IntoElement)]
pub struct LabButton {
    pub open: bool,
    pub accent: Hsla,
    pub hover: Hsla,
}

impl Selectable for LabButton {
    fn selected(mut self, selected: bool) -> Self {
        self.open = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.open
    }
}

impl RenderOnce for LabButton {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div()
            .id("lab")
            .debug_selector(|| "lab-button".into())
            .px_3()
            .py_0p5()
            .rounded_md()
            .border_1()
            .border_dashed()
            .border_color(self.accent)
            .text_color(self.accent)
            .cursor_pointer()
            .when(self.open, |d| d.bg(self.hover))
            .hover(|d| d.bg(self.hover))
            .child("LAB")
    }
}
```

Then change the existing files:

```diff
diff --git a/crates/tulis-core/src/vim.rs b/crates/tulis-core/src/vim.rs
index f229d88..f7e3a16 100644
--- a/crates/tulis-core/src/vim.rs
+++ b/crates/tulis-core/src/vim.rs
@@ -94,6 +94,11 @@ pub enum VimCommand {
         forward: bool,
         count: usize,
     },
+    /// `]m` / `[m`: the next or previous Lab cut or mark.
+    JumpMark {
+        forward: bool,
+        count: usize,
+    },
     ToggleGhost,
     /// `gt`: next theme preset.
     CycleTheme,
@@ -367,6 +372,14 @@ impl Vim {
                     count,
                 }]
             }
+            (']' | '[', 'm') if self.op.is_none() => {
+                let count = self.count();
+                self.reset();
+                vec![VimCommand::JumpMark {
+                    forward: prefix == ']',
+                    count,
+                }]
+            }
             _ => {
                 self.reset();
                 Vec::new()
diff --git a/crates/tulis/src/actions.rs b/crates/tulis/src/actions.rs
index b94e163..1cb4f27 100644
--- a/crates/tulis/src/actions.rs
+++ b/crates/tulis/src/actions.rs
@@ -52,9 +52,8 @@ gpui::actions!(
         FocusOverflow,
         FocusPage,
         AiAlternates,
-        AiTrimParagraph,
-        AiTrimDocument,
-        AcceptTrim,
+        NextMark,
+        PrevMark,
         Cancel,
         OpenContextMenu,
         ToggleVim,
@@ -63,6 +62,11 @@ gpui::actions!(
         TogglePreview,
         ShowCheatSheet,
         OpenLab,
+        LabParagraph,
+        LabDocument,
+        LabNext,
+        LabPrev,
+        LabChoose,
         ShareX,
         ShareLinkedIn,
         Save,
diff --git a/crates/tulis/src/chrome.rs b/crates/tulis/src/chrome.rs
index e87e24e..f50d0c2 100644
--- a/crates/tulis/src/chrome.rs
+++ b/crates/tulis/src/chrome.rs
@@ -3,10 +3,11 @@
 use std::time::Instant;
 
 use gpui::{
-    Animation, AnimationExt, App, ClickEvent, ClipboardItem, Context, IntoElement, ParentElement,
-    Styled, Window, div, prelude::*, px,
+    Anchor, Animation, AnimationExt, App, ClickEvent, ClipboardItem, Context, IntoElement,
+    ParentElement, Styled, Window, div, prelude::*, px,
 };
 use gpui_component::button::{Button, ButtonVariants};
+use gpui_component::popover::Popover;
 use gpui_component::{IconName, Selectable, Sizable};
 use tulis_core::alternates::AltLevel;
 use tulis_core::share::{self, ShareTarget};
@@ -17,6 +18,7 @@ use crate::actions::{
     Open, OpenLab, Save, ShareLinkedIn, ShareX, ShowCheatSheet, ToggleMarkers, ToggleOverflowPanel,
     TogglePreview,
 };
+use crate::lab_popover::LabButton;
 use crate::toast::TOAST_FADE;
 use crate::workspace::{Workspace, file_name};
 
@@ -327,23 +329,26 @@ impl Workspace {
                             })),
                     ),
             )
-            .child(
-                div()
-                    .id("lab")
-                    .px_3()
-                    .py_0p5()
-                    .rounded_md()
-                    .border_1()
-                    .border_dashed()
-                    .border_color(t.accent)
-                    .text_color(t.accent)
-                    .cursor_pointer()
-                    .hover(|s| s.bg(t.panel))
-                    .child("LAB")
-                    .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
-                        ws.open_lab(&OpenLab, window, cx)
-                    })),
-            )
+            .child({
+                let lab = self.lab.clone();
+                Popover::new("lab-popover")
+                    .anchor(Anchor::BottomCenter)
+                    .trigger(LabButton {
+                        open: self.lab_open,
+                        accent: t.accent,
+                        hover: t.panel,
+                    })
+                    .open(self.lab_open)
+                    .track_focus(&self.lab.read(cx).focus)
+                    .on_open_change(cx.listener(|ws, open: &bool, window, cx| {
+                        if *open {
+                            ws.open_lab(&OpenLab, window, cx)
+                        } else {
+                            ws.close_lab(cx)
+                        }
+                    }))
+                    .content(move |_, _, _| lab.clone())
+            })
             .child(
                 div().flex_1().flex().flex_row().justify_end().child(
                     div()
diff --git a/crates/tulis/src/commands.rs b/crates/tulis/src/commands.rs
index c4e2ea6..168bdd1 100644
--- a/crates/tulis/src/commands.rs
+++ b/crates/tulis/src/commands.rs
@@ -500,31 +500,67 @@ pub static COMMANDS: &[Command] = &[
         OpenLab
     ),
     cmd!(
-        "ai_trim_paragraph",
-        "AI trim: paragraph",
+        "lab_paragraph",
+        "The Lab: this paragraph",
         Lab,
         ["secondary-shift-t"],
         ED,
         None,
-        AiTrimParagraph
+        LabParagraph
     ),
     cmd!(
-        "ai_trim_document",
-        "AI trim: document",
+        "lab_document",
+        "The Lab: whole document",
         Lab,
         ["secondary-shift-alt-t"],
         ED,
         None,
-        AiTrimDocument
+        LabDocument
     ),
     cmd!(
-        "accept_trim",
-        "Accept a trim",
+        "lab_next",
+        "Lab: next row",
+        Hidden,
+        ["down", "j", "right"],
+        Some("Lab"),
+        None,
+        LabNext
+    ),
+    cmd!(
+        "lab_prev",
+        "Lab: previous row",
+        Hidden,
+        ["up", "k", "left"],
+        Some("Lab"),
+        None,
+        LabPrev
+    ),
+    cmd!(
+        "lab_choose",
+        "Lab: run this row",
+        Hidden,
+        ["enter"],
+        Some("Lab"),
+        None,
+        LabChoose
+    ),
+    cmd!(
+        "next_mark",
+        "Next cut or mark",
         Lab,
         ["tab"],
         ED,
-        None,
-        AcceptTrim
+        Some("]m"),
+        NextMark
+    ),
+    cmd!(
+        "prev_mark",
+        "Previous cut or mark",
+        Lab,
+        ["shift-tab"],
+        ED,
+        Some("[m"),
+        PrevMark
     ),
     // View
     cmd!(
diff --git a/crates/tulis/src/editor/ai.rs b/crates/tulis/src/editor/ai.rs
index d6de768..da6725b 100644
--- a/crates/tulis/src/editor/ai.rs
+++ b/crates/tulis/src/editor/ai.rs
@@ -1,16 +1,16 @@
-//! AI alternates and AI trim: build requests, run them off the UI thread, apply results.
+//! AI alternates: build requests, run them off the UI thread, apply results.
 
 use std::ops::Range;
 use std::sync::Arc;
 
 use gpui::{Context, Window, prelude::*};
-use tulis_core::ai::{AiError, CompletionRequest, LlmProvider, prompts, provider_from_config};
+use tulis_core::ai::{AiError, LlmProvider, prompts, provider_from_config};
 use tulis_core::history::EditKind;
 use tulis_core::{AiKind, DocPos, ParaId, Selection, Source};
 
 use super::alt_session::MERGED_NOTICE;
-use super::{Editor, Scope, TrimSuggestion};
-use crate::actions::{AiAlternates, AiTrimDocument, AiTrimParagraph};
+use super::{Editor, Scope};
+use crate::actions::AiAlternates;
 
 pub(crate) const STALE_NOTICE: &str = "Text changed while AI was working; result dropped";
 
@@ -27,7 +27,7 @@ pub(crate) struct AlternatesJob {
 }
 
 impl Editor {
-    fn provider(&mut self, cx: &mut Context<Self>) -> Option<Arc<dyn LlmProvider>> {
+    pub(super) fn provider(&mut self, cx: &mut Context<Self>) -> Option<Arc<dyn LlmProvider>> {
         if let Some(provider) = self.provider_override.clone() {
             return Some(provider);
         }
@@ -127,6 +127,7 @@ impl Editor {
             group,
             source,
         };
+        self.stop_ai();
         let work = cx.background_spawn(async move { provider.complete(&request) });
         self.ai_task = Some(cx.spawn(async move |this, cx| {
             let result = work.await;
@@ -211,103 +212,4 @@ impl Editor {
         }
         (p.text().get(job.range.clone()) == Some(job.original.as_str())).then(|| job.range.clone())
     }
-
-    pub(super) fn ai_trim_paragraph(
-        &mut self,
-        _: &AiTrimParagraph,
-        _: &mut Window,
-        cx: &mut Context<Self>,
-    ) {
-        let para = self.selection.head.para;
-        self.start_trim(vec![para], cx);
-    }
-
-    pub(crate) fn ai_trim_document(
-        &mut self,
-        _: &AiTrimDocument,
-        _: &mut Window,
-        cx: &mut Context<Self>,
-    ) {
-        let all = (0..self.doc.paragraphs.len()).collect();
-        self.start_trim(all, cx);
-    }
-
-    fn start_trim(&mut self, paragraphs: Vec<usize>, cx: &mut Context<Self>) {
-        self.finish_alt_edit();
-        let jobs: Vec<(ParaId, String, CompletionRequest)> = paragraphs
-            .into_iter()
-            .filter_map(|i| {
-                let p = &self.doc.paragraphs[i];
-                let export = p.export_text();
-                (!export.trim().is_empty())
-                    .then(|| (p.id, p.text(), prompts::trim_request(&export, 10)))
-            })
-            .collect();
-        if jobs.is_empty() {
-            return self.set_notice("Nothing to trim", cx);
-        }
-        let Some(provider) = self.provider(cx) else {
-            return;
-        };
-        // One request per paragraph, in sequence, on the background executor.
-        let work = cx.background_spawn(async move {
-            jobs.into_iter()
-                .map(|(id, text, request)| (id, text, provider.complete(&request)))
-                .collect::<Vec<_>>()
-        });
-        self.ai_task = Some(cx.spawn(async move |this, cx| {
-            let results = work.await;
-            this.update(cx, |editor, cx| {
-                editor.ai_task = None;
-                editor.apply_trims(results, cx);
-            })
-            .ok();
-        }));
-        self.set_notice("AI is looking for cuts…", cx);
-    }
-
-    pub(crate) fn apply_trims(
-        &mut self,
-        results: Vec<(ParaId, String, Result<String, AiError>)>,
-        cx: &mut Context<Self>,
-    ) {
-        let mut found = 0;
-        let mut stale = 0;
-        let mut error = None;
-        for (id, text, result) in results {
-            let response = match result {
-                Ok(response) => response,
-                Err(e) => {
-                    error.get_or_insert(e.to_string());
-                    continue;
-                }
-            };
-            let Some(para) = self.doc.para_index(id) else {
-                stale += 1;
-                continue;
-            };
-            let p = &self.doc.paragraphs[para];
-            if p.text() != text {
-                stale += 1;
-                continue;
-            }
-            let ranges = prompts::locate_trims(&prompts::parse_trim(&response), p);
-            found += ranges.len();
-            self.trims.retain(|t| t.para_id != id);
-            self.trims.extend(
-                ranges
-                    .into_iter()
-                    .map(|range| TrimSuggestion { para_id: id, range }),
-            );
-        }
-        let mut notice = match (found, error) {
-            (0, Some(e)) => e,
-            (0, None) => "No cuts suggested".to_string(),
-            (n, _) => format!("{n} suggested cuts — Tab to accept, Esc to dismiss"),
-        };
-        if stale > 0 {
-            notice.push_str(&format!(" ({stale} changed paragraphs skipped)"));
-        }
-        self.set_notice(notice, cx);
-    }
 }
diff --git a/crates/tulis/src/editor/element.rs b/crates/tulis/src/editor/element.rs
index e4a461b..3a86d07 100644
--- a/crates/tulis/src/editor/element.rs
+++ b/crates/tulis/src/editor/element.rs
@@ -77,7 +77,7 @@ fn build_layout(
             font: font(editor.font_family.clone()),
             title_font: title_font.clone(),
             color: faded(editor.theme.text, alpha),
-            trim: faded(editor.theme.trim, alpha),
+            fade_alpha: editor.theme.fade_alpha,
             marker: faded(editor.theme.text_muted, 0.5 * alpha),
             ghost_alpha: editor.theme.ghost_alpha,
         }
@@ -95,12 +95,11 @@ fn build_layout(
     let mut paras = Vec::with_capacity(editor.doc.paragraphs.len());
     for (i, p) in editor.doc.paragraphs.iter().enumerate() {
         let text = p.text();
-        let trims: Vec<_> = editor
-            .trims
-            .iter()
-            .filter(|t| t.para_id == p.id)
-            .map(|t| t.range.clone())
-            .collect();
+        let cuts = editor
+            .trim_preview
+            .as_ref()
+            .map(|t| t.active_ranges(p.id))
+            .unwrap_or_default();
         let marked_here =
             marked.and_then(|(a, b)| (a.para == i && b.para == i).then(|| a.offset..b.offset));
         let text_len = text.len();
@@ -111,7 +110,7 @@ fn build_layout(
         let runs = build_runs(
             text_len,
             &p.spans(),
-            &trims,
+            &cuts,
             marked_here,
             title_marker,
             caret_on_marker,
diff --git a/crates/tulis/src/editor/features.rs b/crates/tulis/src/editor/features.rs
index 63da511..d646b50 100644
--- a/crates/tulis/src/editor/features.rs
+++ b/crates/tulis/src/editor/features.rs
@@ -1,4 +1,4 @@
-//! Alternates, ghosting, overflow and trim-acceptance commands.
+//! Alternates, ghosting and overflow commands, and Esc.
 
 use std::ops::Range;
 
@@ -7,7 +7,7 @@ use tulis_core::alternates::{AltLevel, OpError};
 use tulis_core::history::EditKind;
 use tulis_core::{DocPos, Selection};
 
-use super::{AltEdit, CycledGroup, Editor, TrimSuggestion};
+use super::{AltEdit, CycledGroup, Editor};
 use crate::actions::*;
 
 const STASHED: &str = "Stashed to Overflow";
@@ -298,57 +298,15 @@ impl Editor {
         self.use_overflow(range, self.selection.head, copy, cx)
     }
 
-    /// Accept the trim suggestion under the cursor (or the next one, or the first) by ghosting it.
-    pub(super) fn accept_trim(&mut self, _: &AcceptTrim, _: &mut Window, cx: &mut Context<Self>) {
-        if self.trims.is_empty() {
-            return;
-        }
-        let head = self.selection.head;
-        let head_id = self.doc.paragraphs[head.para].id;
-        let order = |t: &TrimSuggestion| {
-            (
-                self.doc.para_index(t.para_id).unwrap_or(usize::MAX),
-                t.range.start,
-            )
-        };
-        let under_cursor = self.trims.iter().position(|t| {
-            t.para_id == head_id && t.range.start <= head.offset && head.offset <= t.range.end
-        });
-        let ix = under_cursor
-            .or_else(|| {
-                (0..self.trims.len())
-                    .filter(|&i| order(&self.trims[i]) >= (head.para, head.offset))
-                    .min_by_key(|&i| order(&self.trims[i]))
-            })
-            .or_else(|| (0..self.trims.len()).min_by_key(|&i| order(&self.trims[i])));
-        let Some(ix) = ix else { return };
-        let trim = self.trims.remove(ix);
-        let Some(para) = self.doc.para_index(trim.para_id) else {
-            return;
-        };
-        // Not `begin_edit`: ghosting doesn't change rendered text, so the other suggestions stay valid.
-        self.record_step(EditKind::Other);
-        match self.doc.set_ghost(para, trim.range, true) {
-            Ok(range) => {
-                self.selection = Selection::caret(DocPos::new(para, range.end));
-                self.dirty = true;
-                self.autoscroll = true;
-                cx.notify();
-            }
-            Err(e) => self.set_notice(e.to_string(), cx),
-        }
-    }
-
-    /// Esc: cancel AI, else dismiss trims, else stop editing an alternate (and leave vim's
-    /// Insert or Visual mode, or drop its pending keys). With none of that to do, close an open
-    /// sidebar.
+    /// Esc: cancel AI, else close the trim preview, else stop editing an alternate (and leave
+    /// vim's Insert or Visual mode, or drop its pending keys). With none of that to do, close an
+    /// open sidebar.
     pub(super) fn cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
-        if self.ai_task.take().is_some() {
+        if self.stop_ai() {
             return self.set_notice("AI request cancelled", cx);
         }
-        if !self.trims.is_empty() {
-            self.trims.clear();
-            return self.set_notice("Trim suggestions dismissed", cx);
+        if self.trim_preview.take().is_some() {
+            return self.set_notice("Trim preview closed", cx);
         }
         let idle = self.alt_edit.is_none()
             && self
diff --git a/crates/tulis/src/editor/mod.rs b/crates/tulis/src/editor/mod.rs
index 6ad980c..300b753 100644
--- a/crates/tulis/src/editor/mod.rs
+++ b/crates/tulis/src/editor/mod.rs
@@ -6,6 +6,7 @@ mod alt_session;
 mod element;
 mod features;
 mod input;
+mod lab;
 #[cfg(test)]
 mod lab_tests;
 pub mod layout;
@@ -14,8 +15,10 @@ mod markers;
 pub(crate) mod tests;
 mod vim_exec;
 
+use std::collections::HashMap;
 use std::ops::Range;
 use std::sync::Arc;
+use std::sync::atomic::AtomicBool;
 use std::time::Instant;
 
 use gpui::{
@@ -32,6 +35,7 @@ use tulis_core::{DocPos, Document, ParaId, Selection};
 pub use alt_session::{AltSession, Scope};
 use element::EditorElement;
 pub(crate) use element::GUTTER;
+pub use lab::{TrimLevel, TrimPreview};
 use layout::LayoutCache;
 
 use crate::theme::{self, Theme};
@@ -61,16 +65,12 @@ pub enum EditorEvent {
     AlternatesOpened,
     /// The alternates sidebar closed; the workspace returns focus to the page.
     AlternatesClosed,
+    /// A Lab run finished (the popover, kept open while it worked, closes).
+    LabDone,
 }
 
 impl gpui::EventEmitter<EditorEvent> for Editor {}
 
-#[derive(Clone, Debug, PartialEq, Eq)]
-pub struct TrimSuggestion {
-    pub para_id: ParaId,
-    pub range: Range<usize>,
-}
-
 /// The group (or paragraph, for versions) just cycled; boxed until the cursor leaves it.
 #[derive(Clone, Debug, PartialEq, Eq)]
 pub struct CycledGroup {
@@ -91,13 +91,18 @@ pub struct Editor {
     pub config: Config,
     pub dirty: bool,
     pub toasts: ToastQueue,
-    pub trims: Vec<TrimSuggestion>,
+    /// The Lab's trim preview, if one is showing.
+    pub trim_preview: Option<TrimPreview>,
+    /// Located cuts by level, paragraph and the text sent, with the rendered text they index.
+    pub(crate) trim_cache: HashMap<lab::TrimKey, (String, Vec<Range<usize>>)>,
     pub vim: Option<Vim>,
     pub alt_edit: Option<AltEdit>,
     pub cycled: Option<CycledGroup>,
     /// The open alternates sidebar, if any.
     pub alt_session: Option<AltSession>,
     pub ai_task: Option<Task<()>>,
+    /// The running Lab request's cancel flag (see `Editor::stop_ai`).
+    pub ai_cancel: Option<Arc<AtomicBool>>,
     pub scroll: ScrollHandle,
     pub theme: Theme,
     /// Alternate and version markers are drawn (Ctrl+. / vim `gm`).
@@ -143,12 +148,14 @@ impl Editor {
             config,
             dirty: false,
             toasts: ToastQueue::default(),
-            trims: Vec::new(),
+            trim_preview: None,
+            trim_cache: HashMap::new(),
             vim,
             alt_edit: None,
             cycled: None,
             alt_session: None,
             ai_task: None,
+            ai_cancel: None,
             scroll: ScrollHandle::new(),
             theme: theme_id.theme(),
             show_markers: true,
@@ -179,10 +186,11 @@ impl Editor {
         self.selection = Selection::default();
         self.history = History::new(HISTORY_LIMIT);
         self.overflow_changed(false);
-        self.trims.clear();
+        self.trim_preview = None;
+        self.trim_cache.clear();
         self.alt_edit = None;
         self.cycled = None;
-        self.ai_task = None;
+        self.stop_ai();
         self.marked = None;
         self.close_alternates(cx);
         self.goal_x = None;
@@ -237,11 +245,6 @@ impl Editor {
         self.set_notice(state, cx);
     }
 
-    /// The Lab arrives with the Lab plan; until then this runs the v1 document trim.
-    pub fn open_lab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
-        self.ai_trim_document(&crate::actions::AiTrimDocument, window, cx);
-    }
-
     pub fn cursor(&self) -> DocPos {
         self.selection.head
     }
@@ -306,8 +309,8 @@ impl Shift {
 }
 
 impl Editor {
-    /// Record an undo step (call before mutating) and drop trim suggestions in the touched
-    /// paragraphs (`touched` is a range of paragraph indices).
+    /// Record an undo step (call before mutating) and drop the trim preview's cuts in the
+    /// touched paragraphs (`touched` is a range of paragraph indices).
     pub(crate) fn begin_edit(&mut self, kind: EditKind, touched: Range<usize>) {
         self.record_step(kind);
         self.drop_transient(touched);
@@ -334,8 +337,8 @@ impl Editor {
         self.selection = selection;
     }
 
-    /// Drop the cycled box and the trim suggestions in the touched paragraphs (`touched` is a
-    /// range of paragraph indices), as every edit does.
+    /// Drop the cycled box and the trim preview's cuts in the touched paragraphs (`touched` is
+    /// a range of paragraph indices), as every edit does.
     pub(crate) fn drop_transient(&mut self, touched: Range<usize>) {
         self.cycled = None;
         let end = touched.end.min(self.doc.paragraphs.len());
@@ -345,7 +348,9 @@ impl Editor {
             .get(touched.start.min(end)..end)
             .map(|ps| ps.iter().map(|p| p.id).collect())
             .unwrap_or_default();
-        self.trims.retain(|t| !ids.contains(&t.para_id));
+        if let Some(preview) = &mut self.trim_preview {
+            preview.retain(|cut| !ids.contains(&cut.para_id));
+        }
     }
 
     pub(crate) fn end_edit(&mut self, cx: &mut Context<Self>) {
@@ -608,7 +613,7 @@ impl Editor {
             anchor: self.doc.clamp(self.selection.anchor),
             head: self.doc.clamp(self.selection.head),
         };
-        self.trims.clear();
+        self.trim_preview = None;
         self.marked = None;
         self.goal_x = None;
         self.dirty = true;
@@ -662,10 +667,9 @@ impl Render for Editor {
             .on_action(cx.listener(Self::delete_alternate))
             .on_action(cx.listener(Self::toggle_ghost))
             .on_action(cx.listener(Self::send_to_overflow))
-            .on_action(cx.listener(Self::accept_trim))
+            .on_action(cx.listener(Self::next_mark))
+            .on_action(cx.listener(Self::prev_mark))
             .on_action(cx.listener(Self::ai_alternates))
-            .on_action(cx.listener(Self::ai_trim_paragraph))
-            .on_action(cx.listener(Self::ai_trim_document))
             .on_action(cx.listener(Self::cancel))
             .on_action(cx.listener(Self::open_context_menu))
             .on_action(cx.listener(Self::toggle_vim))
diff --git a/crates/tulis/src/editor/vim_exec.rs b/crates/tulis/src/editor/vim_exec.rs
index 601eedb..f5ed290 100644
--- a/crates/tulis/src/editor/vim_exec.rs
+++ b/crates/tulis/src/editor/vim_exec.rs
@@ -10,7 +10,7 @@ use tulis_core::{DocPos, Selection};
 
 use super::Editor;
 use crate::actions::{
-    AiAlternates, FocusOverflow, SendToOverflow, ShowCheatSheet, ToggleOverflowPanel,
+    AiAlternates, FocusOverflow, OpenLab, SendToOverflow, ShowCheatSheet, ToggleOverflowPanel,
     TogglePreview, ToggleVim,
 };
 
@@ -266,7 +266,8 @@ impl Editor {
             VimCommand::ToggleGhost => self.toggle_ghost_here(cx),
             VimCommand::CycleTheme => self.cycle_theme(cx),
             VimCommand::ToggleMarkers => self.toggle_markers(cx),
-            VimCommand::Lab => self.open_lab(window, cx),
+            VimCommand::Lab => window.dispatch_action(Box::new(OpenLab), cx),
+            VimCommand::JumpMark { forward, count } => self.jump_lab_item(forward, count, cx),
             VimCommand::AiAlternatives => self.ai_alternates(&AiAlternates, window, cx),
             VimCommand::Stash => self.send_to_overflow(&SendToOverflow, window, cx),
             // Called directly (not dispatched): in Visual mode the selection must still be there.
diff --git a/crates/tulis/src/main.rs b/crates/tulis/src/main.rs
index 86ffa74..282fd93 100644
--- a/crates/tulis/src/main.rs
+++ b/crates/tulis/src/main.rs
@@ -6,6 +6,7 @@ mod commands;
 mod editor;
 mod header_dim;
 mod keymap;
+mod lab_popover;
 mod runs;
 mod scratchpad;
 mod sidebar;
diff --git a/crates/tulis/src/runs.rs b/crates/tulis/src/runs.rs
index 7f72cbe..0695f08 100644
--- a/crates/tulis/src/runs.rs
+++ b/crates/tulis/src/runs.rs
@@ -1,9 +1,9 @@
-//! Turn a paragraph's spans, trim suggestions, IME marked range and `# ` title marker into GPUI
-//! text runs.
+//! Turn a paragraph's spans, a trim preview's cuts, IME marked range and `# ` title marker into
+//! GPUI text runs.
 
 use std::ops::Range;
 
-use gpui::{Font, Hsla, StrikethroughStyle, TextRun, UnderlineStyle, px};
+use gpui::{Font, Hsla, TextRun, UnderlineStyle, px};
 use tulis_core::Span;
 
 pub struct RunStyle {
@@ -11,16 +11,17 @@ pub struct RunStyle {
     /// The font of a `# ` title paragraph (bold).
     pub title_font: Font,
     pub color: Hsla,
-    pub trim: Hsla,
     /// The `# ` marker's color while the caret sits on it (otherwise it is not drawn).
     pub marker: Hsla,
     pub ghost_alpha: f32,
+    /// Alpha factor of a trim preview's active cuts.
+    pub fade_alpha: f32,
 }
 
 pub fn build_runs(
     text_len: usize,
     spans: &[Span],
-    trims: &[Range<usize>],
+    faded: &[Range<usize>],
     marked: Option<Range<usize>>,
     title_marker: usize,
     caret_on_marker: bool,
@@ -31,7 +32,7 @@ pub fn build_runs(
     for span in spans {
         cuts.extend([span.range.start, span.range.end]);
     }
-    for range in trims.iter().chain(marked.iter()) {
+    for range in faded.iter().chain(marked.iter()) {
         cuts.extend([range.start.min(text_len), range.end.min(text_len)]);
     }
     cuts.sort_unstable();
@@ -45,11 +46,14 @@ pub fn build_runs(
                 .find(|s| s.range.start <= a && b <= s.range.end);
             let ghost = span.is_some_and(|s| s.ghost);
             let in_marked = marked.as_ref().is_some_and(|m| m.start <= a && b <= m.end);
-            let trimmed = trims.iter().any(|t| t.start <= a && b <= t.end);
+            let fade = faded.iter().any(|t| t.start <= a && b <= t.end);
             let mut color = style.color;
             if ghost {
                 color.a *= style.ghost_alpha;
             }
+            if fade {
+                color.a *= style.fade_alpha;
+            }
             // The title's `# ` stays in the text but is not drawn, unless the caret is on it
             // (then the block cursor needs something under it).
             if b <= title_marker {
@@ -65,10 +69,6 @@ pub fn build_runs(
                 thickness: px(1.),
                 wavy: false,
             });
-            let strikethrough = trimmed.then(|| StrikethroughStyle {
-                thickness: px(1.5),
-                color: Some(style.trim),
-            });
             TextRun {
                 len: b - a,
                 font: if title_marker > 0 {
@@ -79,7 +79,7 @@ pub fn build_runs(
                 color,
                 background_color: None,
                 underline,
-                strikethrough,
+                strikethrough: None,
             }
         })
         .collect()
diff --git a/crates/tulis/src/theme.rs b/crates/tulis/src/theme.rs
index faa10ab..ede0816 100644
--- a/crates/tulis/src/theme.rs
+++ b/crates/tulis/src/theme.rs
@@ -26,8 +26,6 @@ pub struct Theme {
     pub fade_alpha: f32,
     /// Background of Lab marks.
     pub mark_tint: Hsla,
-    /// Strikethrough of v1 trim suggestions.
-    pub trim: Hsla,
 }
 
 #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
@@ -98,7 +96,6 @@ impl ThemeId {
                     ghost_alpha: 0.06,
                     fade_alpha: 0.35,
                     mark_tint: with_alpha(hex(0xf2c94c), 0.18),
-                    trim: hex(0xf87171),
                 }
             }
             ThemeId::Paper => Theme {
@@ -117,7 +114,6 @@ impl ThemeId {
                 ghost_alpha: 0.12,
                 fade_alpha: 0.35,
                 mark_tint: hsla(0.14, 0.9, 0.6, 0.3),
-                trim: hsla(0.0, 0.7, 0.5, 0.9),
             },
             ThemeId::Sepia => Theme {
                 id: self,
@@ -135,7 +131,6 @@ impl ThemeId {
                 ghost_alpha: 0.12,
                 fade_alpha: 0.35,
                 mark_tint: with_alpha(hex(0xe0b040), 0.3),
-                trim: hex(0xb03a2e),
             },
             ThemeId::Black => {
                 let accent = hex(0x8b7cf6);
@@ -155,7 +150,6 @@ impl ThemeId {
                     ghost_alpha: 0.08,
                     fade_alpha: 0.35,
                     mark_tint: with_alpha(hex(0xf2c94c), 0.18),
-                    trim: hex(0xf87171),
                 }
             }
         }
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
index f233479..24de3b4 100644
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -14,11 +14,12 @@ use tulis_core::config::Config;
 use tulis_core::storage;
 
 use crate::actions::{
-    AiAlternates, CycleTheme, New, Open, OpenAlternates, OpenLab, Quit, Save, SaveAs,
-    SendToOverflow, ToggleGhost, ToggleMarkers, ToggleOverflowPanel, TogglePreview,
+    AiAlternates, CycleTheme, LabDocument, LabParagraph, New, Open, OpenAlternates, OpenLab, Quit,
+    Save, SaveAs, SendToOverflow, ToggleGhost, ToggleMarkers, ToggleOverflowPanel, TogglePreview,
 };
 use crate::editor::{Editor, EditorEvent, GUTTER};
 use crate::header_dim::HeaderDim;
+use crate::lab_popover::{LabEvent, LabPanel};
 use crate::scratchpad::OverflowDrag;
 use crate::theme;
 
@@ -34,6 +35,9 @@ pub struct Workspace {
     pub(crate) scratchpad: Option<Entity<crate::scratchpad::Scratchpad>>,
     pub(crate) preview: bool,
     preview_focus: FocusHandle,
+    /// The Lab popover's contents, and whether it is open.
+    pub(crate) lab: Entity<LabPanel>,
+    pub(crate) lab_open: bool,
     pub(crate) header_dim: HeaderDim,
     /// Repaints the header when its auto-dim state is due to change.
     pub(crate) dim_timer: Option<Task<()>>,
@@ -56,6 +60,7 @@ impl Workspace {
         cx: &mut Context<Self>,
     ) -> Self {
         let editor = cx.new(|cx| Editor::new(config, Document::new(), cx));
+        let lab = cx.new(|cx| LabPanel::new(editor.clone(), cx));
         let subscriptions =
             vec![
                 cx.observe(&editor, |_, _, cx| cx.notify()),
@@ -64,8 +69,12 @@ impl Workspace {
                         EditorEvent::Typed => ws.typed(cx),
                         EditorEvent::AlternatesOpened => ws.alternates_opened(window, cx),
                         EditorEvent::AlternatesClosed => ws.alternates_closed(window, cx),
+                        EditorEvent::LabDone => ws.close_lab(cx),
                     }
                 }),
+                cx.subscribe(&lab, |ws, _, event: &LabEvent, cx| match event {
+                    LabEvent::Close => ws.close_lab(cx),
+                }),
             ];
         let initial_theme = editor.read(cx).theme;
         theme::apply_to_components(&initial_theme, cx);
@@ -79,6 +88,8 @@ impl Workspace {
             scratchpad: None,
             preview: false,
             preview_focus: cx.focus_handle(),
+            lab,
+            lab_open: false,
             header_dim: HeaderDim::default(),
             dim_timer: None,
             window_title: String::new(),
@@ -151,8 +162,55 @@ impl Workspace {
         self.editor.update(cx, |e, cx| e.toggle_markers(cx));
     }
 
-    pub(crate) fn open_lab(&mut self, _: &OpenLab, window: &mut Window, cx: &mut Context<Self>) {
-        self.editor.update(cx, |e, cx| e.open_lab(window, cx));
+    /// LAB, Ctrl+L, vim `gl`: the Lab on the selection's paragraphs, or the whole document.
+    pub(crate) fn open_lab(&mut self, _: &OpenLab, _: &mut Window, cx: &mut Context<Self>) {
+        let scope = self.editor.read(cx).lab_scope();
+        self.show_lab(scope, cx);
+    }
+
+    /// Ctrl+Shift+T: the Lab on the cursor's paragraph.
+    fn lab_paragraph(&mut self, _: &LabParagraph, _: &mut Window, cx: &mut Context<Self>) {
+        let editor = self.editor.read(cx);
+        let scope = vec![editor.doc.paragraphs[editor.cursor().para].id];
+        self.show_lab(scope, cx);
+    }
+
+    /// Ctrl+Shift+Alt+T: the Lab on the whole document.
+    fn lab_document(&mut self, _: &LabDocument, _: &mut Window, cx: &mut Context<Self>) {
+        let scope = self
+            .editor
+            .read(cx)
+            .doc
+            .paragraphs
+            .iter()
+            .map(|p| p.id)
+            .collect();
+        self.show_lab(scope, cx);
+    }
+
+    /// Open the popover on `scope`. It takes focus when it renders.
+    fn show_lab(&mut self, scope: Vec<tulis_core::ParaId>, cx: &mut Context<Self>) {
+        self.editor.update(cx, |e, _| {
+            e.finish_alt_edit();
+        });
+        self.lab.update(cx, |lab, cx| lab.reset(scope, cx));
+        self.lab_open = true;
+        cx.notify();
+    }
+
+    /// Close the popover (Esc, a click outside, LAB, or its run finishing). A run still waiting
+    /// for the AI is stopped. Focus goes back where it was when the popover opened.
+    pub(crate) fn close_lab(&mut self, cx: &mut Context<Self>) {
+        if !std::mem::take(&mut self.lab_open) {
+            return;
+        }
+        self.editor.update(cx, |e, cx| {
+            if e.lab_running() {
+                e.stop_ai();
+                e.set_notice("Lab stopped", cx);
+            }
+        });
+        cx.notify();
     }
 
     fn cycle_theme(&mut self, _: &CycleTheme, _: &mut Window, cx: &mut Context<Self>) {
@@ -343,6 +401,8 @@ impl Render for Workspace {
             .on_action(cx.listener(Self::cycle_theme))
             .on_action(cx.listener(Self::toggle_markers))
             .on_action(cx.listener(Self::open_lab))
+            .on_action(cx.listener(Self::lab_paragraph))
+            .on_action(cx.listener(Self::lab_document))
             .on_action(cx.listener(Self::toggle_preview))
             .on_action(cx.listener(Self::share_x))
             .on_action(cx.listener(Self::share_linkedin))
```

- [ ] **Step 4: Run the tests (expect success)**

Run: `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: 171 app tests and 163 core tests pass; clippy counts match the baseline. Launch check as in the Global Constraints.

- [ ] **Step 5: Commit**

```bash
jj describe -m "feat: the Lab popover with trim levels previewed as faded cuts; v1 trim retired

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 4: Keep a cut, the Lab bar, Make the cuts and Done

**Files:**
- Modify: `crates/tulis/src/editor/lab.rs`, `crates/tulis/src/editor/actions.rs`, `crates/tulis/src/chrome.rs`, `crates/tulis/src/workspace.rs`, `crates/tulis/src/editor/lab_tests.rs`

**Interfaces:**
- Consumes: `stats::{document_counts, words_without}`, `Document::set_ghost`, `Editor::{record_step, end_edit}`.
- Produces: `enum BarAction { MakeCuts, Done }` with `label()` and `id()`; `struct LabBar { title, hint: Option<String>, actions: Vec<BarAction> }`; `Editor::lab_bar() -> Option<LabBar>`, `Editor::bar_action(action, cx)`, `Editor::toggle_cut_at(pos, cx) -> bool`; `Workspace::render_lab_bar`.

The bar reads `"<Level> <before> → <after> words · −<pct>%"`: `before` is the document's export word count, `after` is `words_without` the active cuts, and `pct` is the rounded real reduction. A click (no Shift) or Enter at a position inside a cut, edges included, toggles it between kept and active. Enter there doesn't split the paragraph. **Make the cuts** ghosts every active cut after one `record_step`, closes the preview and toasts "Cut N passages (ghosted — Ctrl+Z to undo)". **Done** stops any run and clears the preview. The bar sits between the page and the footer; each button is wrapped in a `debug_selector` (`lab-bar-<id>`) and returns focus to the page.

- [ ] **Step 1: Write the failing tests**

```diff
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
index 24de3b4..02600f0 100644
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -1404,6 +1404,30 @@ mod tests {
         });
     }
 
+    #[gpui::test]
+    fn the_bar_shows_a_trim_preview_and_its_buttons_work(cx: &mut TestAppContext) {
+        let (ws, cx) = root_workspace(cx, &["I really love tea."]);
+        draw(cx);
+        assert!(cx.debug_bounds("lab-bar").is_none());
+        let editor = ws.read_with(cx, |ws, _| ws.editor.clone());
+        let mock = lab_mock(&ws, cx, &[r#"["really "]"#]);
+        editor.update(cx, |e, cx| {
+            let scope = e.lab_scope();
+            e.run_trim(crate::editor::TrimLevel::Slight, &scope, cx)
+        });
+        cx.run_until_parked();
+        assert_eq!(mock.requests.lock().unwrap().len(), 1);
+        draw(cx);
+        assert!(cx.debug_bounds("lab-bar").is_some());
+        let button = cx.debug_bounds("lab-bar-make-cuts").expect("Make the cuts");
+        cx.simulate_click(button.center(), gpui::Modifiers::none());
+        draw(cx);
+        assert!(cx.debug_bounds("lab-bar").is_none());
+        let export = editor.read_with(cx, |e, _| e.doc.paragraphs[0].export_text());
+        assert_eq!(export, "I love tea.");
+        assert!(page_focused(&ws, cx));
+    }
+
     #[gpui::test]
     fn the_lab_button_and_vim_gl_open_the_lab(cx: &mut TestAppContext) {
         let (ws, cx) = root_workspace(cx, &["One."]);
diff --git a/crates/tulis/src/editor/lab_tests.rs b/crates/tulis/src/editor/lab_tests.rs
index 674409b..9fc68a1 100644
--- a/crates/tulis/src/editor/lab_tests.rs
+++ b/crates/tulis/src/editor/lab_tests.rs
@@ -8,7 +8,7 @@ use tulis_core::ai::{AiError, CompletionRequest, LlmProvider, MockProvider};
 use tulis_core::vim::Vim;
 use tulis_core::{DocPos, ParaId, Selection};
 
-use super::lab::TrimSuggestion;
+use super::lab::{BarAction, TrimSuggestion};
 use super::tests::{editor_with, texts, type_str};
 use super::{Editor, TrimLevel, TrimPreview};
 use crate::actions::*;
@@ -244,3 +244,100 @@ fn partial_failures_toast_how_many_paragraphs_failed(cx: &mut TestAppContext) {
     // The earlier preview stays.
     assert_eq!(cut_ranges(&editor, cx), vec![(0, 2..9)]);
 }
+
+fn bar_title(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> Option<String> {
+    editor.read_with(cx, |e, _| e.lab_bar().map(|bar| bar.title))
+}
+
+#[gpui::test]
+fn the_bar_counts_words_before_and_after_the_active_cuts(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["I really love tea.", "It is very, very good."]);
+    assert_eq!(bar_title(&editor, cx), None);
+    preview(&editor, cx, &[(0, 2..9), (1, 6..12)]);
+    assert_eq!(
+        bar_title(&editor, cx).as_deref(),
+        Some("Slight trim 9 → 7 words · −22%")
+    );
+    let bar = editor.read_with(cx, |e, _| e.lab_bar()).unwrap();
+    assert_eq!(
+        bar.hint.as_deref(),
+        Some("Faded words would go. Click one to keep it.")
+    );
+    assert_eq!(bar.actions, vec![BarAction::MakeCuts, BarAction::Done]);
+    // A kept cut counts again.
+    editor.update(cx, |e, cx| assert!(e.toggle_cut_at(DocPos::new(0, 4), cx)));
+    assert_eq!(
+        bar_title(&editor, cx).as_deref(),
+        Some("Slight trim 9 → 8 words · −11%")
+    );
+}
+
+#[gpui::test]
+fn clicking_or_enter_keeps_a_cut_and_clicking_again_reactivates_it(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["I really love tea."]);
+    preview(&editor, cx, &[(0, 2..9)]);
+    let active = |cx: &mut VisualTestContext| {
+        editor.read_with(cx, |e, _| e.trim_preview.as_ref().unwrap().is_active(0))
+    };
+    // Enter with the cursor in a cut keeps it, and doesn't split the paragraph.
+    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(0, 5)));
+    editor.update_in(cx, |e, window, cx| e.newline(&Newline, window, cx));
+    assert!(!active(cx));
+    assert_eq!(texts(&editor, cx), vec!["I really love tea."]);
+    // A click on a kept cut makes it active again; a click elsewhere does nothing.
+    editor.update(cx, |e, cx| assert!(e.toggle_cut_at(DocPos::new(0, 3), cx)));
+    assert!(active(cx));
+    editor.update(cx, |e, cx| {
+        assert!(!e.toggle_cut_at(DocPos::new(0, 12), cx))
+    });
+    assert!(active(cx));
+    // Enter outside every cut starts a new paragraph as usual.
+    editor.update(cx, |e, _| {
+        e.selection = Selection::caret(DocPos::new(0, 13))
+    });
+    editor.update_in(cx, |e, window, cx| e.newline(&Newline, window, cx));
+    assert_eq!(texts(&editor, cx), vec!["I really love", " tea."]);
+}
+
+#[gpui::test]
+fn make_the_cuts_ghosts_every_active_cut_in_one_undo_step(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["I really love tea, truly.", "It is very good."]);
+    preview(&editor, cx, &[(0, 2..9), (0, 17..24), (1, 6..11)]);
+    editor.update(cx, |e, cx| assert!(e.toggle_cut_at(DocPos::new(0, 20), cx)));
+    editor.update(cx, |e, cx| e.bar_action(BarAction::MakeCuts, cx));
+    let exports = |cx: &mut VisualTestContext| {
+        editor.read_with(cx, |e, _| {
+            e.doc
+                .paragraphs
+                .iter()
+                .map(|p| p.export_text())
+                .collect::<Vec<_>>()
+        })
+    };
+    assert_eq!(exports(cx), vec!["I love tea, truly.", "It is good."]);
+    assert!(editor.read_with(cx, |e, _| e.trim_preview.is_none()));
+    assert_eq!(
+        notice(&editor, cx).as_deref(),
+        Some("Cut 2 passages (ghosted — Ctrl+Z to undo)")
+    );
+    // The rendered text is unchanged; one undo revives every cut.
+    assert_eq!(
+        texts(&editor, cx),
+        vec!["I really love tea, truly.", "It is very good."]
+    );
+    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
+    assert_eq!(
+        exports(cx),
+        vec!["I really love tea, truly.", "It is very good."]
+    );
+}
+
+#[gpui::test]
+fn done_clears_the_preview_without_changes(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["I really love tea."]);
+    preview(&editor, cx, &[(0, 2..9)]);
+    editor.update(cx, |e, cx| e.bar_action(BarAction::Done, cx));
+    assert!(editor.read_with(cx, |e, _| e.trim_preview.is_none()));
+    assert!(!editor.read_with(cx, |e, _| e.dirty));
+    assert_eq!(texts(&editor, cx), vec!["I really love tea."]);
+}
```

- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile errors (`BarAction`, `lab_bar`, `toggle_cut_at`, `bar_action` don't exist).

- [ ] **Step 3: Implement**

```diff
diff --git a/crates/tulis/src/chrome.rs b/crates/tulis/src/chrome.rs
index f50d0c2..c9fd304 100644
--- a/crates/tulis/src/chrome.rs
+++ b/crates/tulis/src/chrome.rs
@@ -275,6 +275,55 @@ impl Workspace {
         self.share(ShareTarget::LinkedIn, cx);
     }
 
+    /// The Lab bar above the footer: what the Lab is showing, with its buttons.
+    pub(crate) fn render_lab_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
+        let editor = self.editor.read(cx);
+        let t = editor.theme;
+        let bar = editor.lab_bar()?;
+        Some(
+            div()
+                .id("lab-bar")
+                .debug_selector(|| "lab-bar".into())
+                .flex_none()
+                .px_4()
+                .py_1()
+                .flex()
+                .flex_row()
+                .items_center()
+                .gap_3()
+                .border_t_1()
+                .border_color(t.rule)
+                .bg(t.panel)
+                .text_xs()
+                .child(
+                    div()
+                        .flex_1()
+                        .min_w(px(0.))
+                        .flex()
+                        .flex_col()
+                        .child(div().text_color(t.text).child(bar.title))
+                        .children(
+                            bar.hint
+                                .map(|hint| div().text_color(t.text_muted).child(hint)),
+                        ),
+                )
+                .children(bar.actions.into_iter().map(|action| {
+                    div()
+                        .debug_selector(move || format!("lab-bar-{}", action.id()))
+                        .child(
+                            Button::new(action.id())
+                                .ghost()
+                                .xsmall()
+                                .label(action.label())
+                                .on_click(cx.listener(move |ws, _: &ClickEvent, window, cx| {
+                                    ws.editor.update(cx, |e, cx| e.bar_action(action, cx));
+                                    ws.focus_editor(window, cx);
+                                })),
+                        )
+                })),
+        )
+    }
+
     /// Save/Open/Share on the left, LAB in the middle, XYZ (Overflow) on the right.
     pub(crate) fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
         let t = self.editor.read(cx).theme;
diff --git a/crates/tulis/src/editor/actions.rs b/crates/tulis/src/editor/actions.rs
index 762f073..c68501d 100644
--- a/crates/tulis/src/editor/actions.rs
+++ b/crates/tulis/src/editor/actions.rs
@@ -180,6 +180,10 @@ impl Editor {
     }
 
     pub(super) fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
+        // Enter in a trim preview's cut keeps it (or makes a kept one active again).
+        if self.selection.is_empty() && self.toggle_cut_at(self.selection.head, cx) {
+            return;
+        }
         if self.vim_captures_text() {
             return self.vertical_move(1, false, cx);
         }
@@ -284,7 +288,13 @@ impl Editor {
                 };
                 self.set_selection(selection, cx);
             }
-            _ => self.move_to(pos, event.modifiers.shift, cx),
+            _ => {
+                self.move_to(pos, event.modifiers.shift, cx);
+                // A click on a trim preview's cut keeps it, or makes a kept one active again.
+                if !event.modifiers.shift {
+                    self.toggle_cut_at(pos, cx);
+                }
+            }
         }
     }
 
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
index 02600f0..04a02f6 100644
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -432,6 +432,7 @@ impl Render for Workspace {
                         row.children(self.scratchpad.clone())
                     }),
             )
+            .children(self.render_lab_bar(cx))
             .child(self.render_footer(cx))
             .child(self.render_toasts(cx))
     }
diff --git a/crates/tulis/src/editor/lab.rs b/crates/tulis/src/editor/lab.rs
index f765907..9f9d6e8 100644
--- a/crates/tulis/src/editor/lab.rs
+++ b/crates/tulis/src/editor/lab.rs
@@ -8,6 +8,8 @@ use std::sync::atomic::{AtomicBool, Ordering};
 
 use gpui::{Context, Window, prelude::*};
 use tulis_core::ai::{AiError, CompletionRequest, prompts};
+use tulis_core::history::EditKind;
+use tulis_core::stats;
 use tulis_core::{DocPos, ParaId, Selection};
 
 use super::{Editor, EditorEvent};
@@ -109,6 +111,38 @@ impl TrimPreview {
     }
 }
 
+/// A button on the Lab bar.
+#[derive(Clone, Copy, Debug, PartialEq, Eq)]
+pub enum BarAction {
+    MakeCuts,
+    Done,
+}
+
+impl BarAction {
+    pub fn label(self) -> &'static str {
+        match self {
+            BarAction::MakeCuts => "Make the cuts",
+            BarAction::Done => "Done",
+        }
+    }
+
+    /// For element ids and test selectors.
+    pub fn id(self) -> &'static str {
+        match self {
+            BarAction::MakeCuts => "make-cuts",
+            BarAction::Done => "done",
+        }
+    }
+}
+
+/// The bar above the footer while the Lab has something on the page.
+#[derive(Clone, Debug, PartialEq, Eq)]
+pub struct LabBar {
+    pub title: String,
+    pub hint: Option<String>,
+    pub actions: Vec<BarAction>,
+}
+
 /// One paragraph's Lab request, with the paragraph as it was when asked (results for a
 /// paragraph that changed meanwhile are dropped).
 pub(crate) struct LabJob {
@@ -331,6 +365,85 @@ impl Editor {
         cx.notify();
     }
 
+    /// The bar's text and buttons, if the Lab has something showing.
+    pub fn lab_bar(&self) -> Option<LabBar> {
+        let preview = self.trim_preview.as_ref()?;
+        let before = stats::document_counts(&self.doc).words;
+        let after = stats::words_without(&self.doc, &self.lab_items());
+        let pct = if before == 0 {
+            0
+        } else {
+            ((before - after.min(before)) as f32 * 100. / before as f32).round() as usize
+        };
+        Some(LabBar {
+            title: format!(
+                "{} {before} → {after} words · −{pct}%",
+                preview.level.label()
+            ),
+            hint: Some("Faded words would go. Click one to keep it.".into()),
+            actions: vec![BarAction::MakeCuts, BarAction::Done],
+        })
+    }
+
+    /// A click on the bar.
+    pub fn bar_action(&mut self, action: BarAction, cx: &mut Context<Self>) {
+        match action {
+            BarAction::MakeCuts => self.make_cuts(cx),
+            BarAction::Done => {
+                self.stop_ai();
+                self.trim_preview = None;
+                cx.notify();
+            }
+        }
+    }
+
+    /// Ghost every active cut, as one undo step, and close the preview.
+    fn make_cuts(&mut self, cx: &mut Context<Self>) {
+        self.finish_alt_edit();
+        let cuts = self.lab_items();
+        if cuts.is_empty() {
+            return self.set_notice("No cuts to make", cx);
+        }
+        // Not `begin_edit`: ghosting leaves the rendered text (and so the other cuts) as is.
+        self.record_step(EditKind::Other);
+        let mut made = 0;
+        for (para, range) in cuts {
+            if self.doc.set_ghost(para, range, true).is_ok() {
+                made += 1;
+            }
+        }
+        self.trim_preview = None;
+        self.dirty = true;
+        self.end_edit(cx);
+        let passages = if made == 1 { "passage" } else { "passages" };
+        self.set_notice(
+            format!("Cut {made} {passages} (ghosted — Ctrl+Z to undo)"),
+            cx,
+        );
+    }
+
+    /// A click (or Enter) at `pos`: keep the active cut there, or make a kept one active
+    /// again. Returns false when there is no cut at `pos`.
+    pub(crate) fn toggle_cut_at(&mut self, pos: DocPos, cx: &mut Context<Self>) -> bool {
+        let Some(id) = self.doc.paragraphs.get(pos.para).map(|p| p.id) else {
+            return false;
+        };
+        let Some(preview) = &mut self.trim_preview else {
+            return false;
+        };
+        let hit = preview.cuts.iter().position(|cut| {
+            cut.para_id == id && cut.range.start <= pos.offset && pos.offset <= cut.range.end
+        });
+        let Some(ix) = hit else {
+            return false;
+        };
+        if !preview.kept.remove(&ix) {
+            preview.kept.insert(ix);
+        }
+        cx.notify();
+        true
+    }
+
     /// The active cuts of the trim preview, as `(paragraph, range)` in document order.
     fn lab_items(&self) -> Vec<(usize, Range<usize>)> {
         let Some(preview) = &self.trim_preview else {
```

- [ ] **Step 4: Run the tests (expect success)**

Run: `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: 176 app tests and 163 core tests pass; clippy counts match the baseline. Launch check as in the Global Constraints.

- [ ] **Step 5: Commit**

```bash
jj describe -m "feat: keep or restore a cut, the Lab bar, Make the cuts and Done

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 5: Walk through the cuts

**Files:**
- Modify: `crates/tulis/src/editor/lab.rs`, `crates/tulis/src/editor/actions.rs`, `crates/tulis/src/editor/input.rs`, `crates/tulis/src/editor/features.rs`, `crates/tulis/src/editor/lab_tests.rs`

**Interfaces:**
- Consumes: Task 4's bar and `TrimPreview`.
- Produces: `struct Walk { at, cut, kept }`, `TrimPreview::walk: Option<Walk>`; `BarAction::Walk`; `Editor::walking()`, `walk_step(cut: bool, cx)`, `end_walk(cx) -> bool`.

**Walk through** selects the first active cut (so it is highlighted and scrolled to). The bar reads "Cut i of n" (`i` counts decisions so far plus one; `n` adds the active cuts still ahead) over "y or Enter cuts it · n or Backspace keeps it · Esc stops", with **Done**. `y` (typed, with or without vim) and Enter ghost the cut as its own undo step and drop it from the preview; `n` and Backspace keep it. After the last cut the walk and the preview end with "Walked through N cuts: X cut, Y kept". Esc ends the walk and leaves the preview. `y`/`n` are captured only while walking. An edit that drops any cut ends the walk.

- [ ] **Step 1: Write the failing tests**

```diff
diff --git a/crates/tulis/src/editor/lab_tests.rs b/crates/tulis/src/editor/lab_tests.rs
index 9fc68a1..08c3236 100644
--- a/crates/tulis/src/editor/lab_tests.rs
+++ b/crates/tulis/src/editor/lab_tests.rs
@@ -263,7 +263,10 @@ fn the_bar_counts_words_before_and_after_the_active_cuts(cx: &mut TestAppContext
         bar.hint.as_deref(),
         Some("Faded words would go. Click one to keep it.")
     );
-    assert_eq!(bar.actions, vec![BarAction::MakeCuts, BarAction::Done]);
+    assert_eq!(
+        bar.actions,
+        vec![BarAction::MakeCuts, BarAction::Walk, BarAction::Done]
+    );
     // A kept cut counts again.
     editor.update(cx, |e, cx| assert!(e.toggle_cut_at(DocPos::new(0, 4), cx)));
     assert_eq!(
@@ -341,3 +344,86 @@ fn done_clears_the_preview_without_changes(cx: &mut TestAppContext) {
     assert!(!editor.read_with(cx, |e, _| e.dirty));
     assert_eq!(texts(&editor, cx), vec!["I really love tea."]);
 }
+
+fn exports(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> Vec<String> {
+    editor.read_with(cx, |e, _| {
+        e.doc.paragraphs.iter().map(|p| p.export_text()).collect()
+    })
+}
+
+fn selected(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> (DocPos, DocPos) {
+    editor.read_with(cx, |e, _| {
+        let r = e.selection.range();
+        (r.start, r.end)
+    })
+}
+
+#[gpui::test]
+fn walking_through_cuts_or_keeps_each_cut_in_turn(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["I really love tea, truly.", "It is very good."]);
+    preview(&editor, cx, &[(0, 2..9), (0, 17..24), (1, 6..11)]);
+    editor.update(cx, |e, cx| e.bar_action(BarAction::Walk, cx));
+    assert_eq!(bar_title(&editor, cx).as_deref(), Some("Cut 1 of 3"));
+    let bar = editor.read_with(cx, |e, _| e.lab_bar()).unwrap();
+    assert_eq!(
+        bar.hint.as_deref(),
+        Some("y or Enter cuts it · n or Backspace keeps it · Esc stops")
+    );
+    // The current cut is selected (and scrolled to).
+    assert_eq!(
+        selected(&editor, cx),
+        (DocPos::new(0, 2), DocPos::new(0, 9))
+    );
+    // `y` cuts it, without vim and without typing.
+    type_str(&editor, cx, "y");
+    assert_eq!(exports(&editor, cx)[0], "I love tea, truly.");
+    assert_eq!(bar_title(&editor, cx).as_deref(), Some("Cut 2 of 3"));
+    assert_eq!(
+        selected(&editor, cx),
+        (DocPos::new(0, 17), DocPos::new(0, 24))
+    );
+    // Backspace keeps it.
+    editor.update_in(cx, |e, window, cx| e.backspace(&Backspace, window, cx));
+    assert_eq!(bar_title(&editor, cx).as_deref(), Some("Cut 3 of 3"));
+    // Enter cuts the last one, which ends the walk and the preview.
+    editor.update_in(cx, |e, window, cx| e.newline(&Newline, window, cx));
+    assert_eq!(
+        exports(&editor, cx),
+        vec!["I love tea, truly.", "It is good."]
+    );
+    assert!(editor.read_with(cx, |e, _| e.trim_preview.is_none()));
+    assert_eq!(
+        notice(&editor, cx).as_deref(),
+        Some("Walked through 3 cuts: 2 cut, 1 kept")
+    );
+    // Each cut was its own undo step.
+    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
+    assert_eq!(
+        exports(&editor, cx),
+        vec!["I love tea, truly.", "It is very good."]
+    );
+    // Now `y` is typed again.
+    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(1, 0)));
+    type_str(&editor, cx, "y");
+    assert_eq!(texts(&editor, cx)[1], "yIt is very good.");
+}
+
+#[gpui::test]
+fn escape_stops_the_walk_and_leaves_the_rest_of_the_preview(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["I really love tea, truly."]);
+    preview(&editor, cx, &[(0, 2..9), (0, 17..24)]);
+    editor.update(cx, |e, cx| e.bar_action(BarAction::Walk, cx));
+    type_str(&editor, cx, "n");
+    editor.update_in(cx, |e, window, cx| e.cancel(&Cancel, window, cx));
+    let preview = editor.read_with(cx, |e, _| e.trim_preview.clone()).unwrap();
+    assert!(preview.walk.is_none());
+    assert!(!preview.is_active(0) && preview.is_active(1));
+    assert_eq!(
+        bar_title(&editor, cx).as_deref(),
+        Some("Slight trim 5 → 4 words · −20%")
+    );
+    // `y` and `n` are text again.
+    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(0, 0)));
+    type_str(&editor, cx, "n");
+    assert_eq!(texts(&editor, cx), vec!["nI really love tea, truly."]);
+}
```

- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile errors (`BarAction::Walk`, `TrimPreview::walk` don't exist).

- [ ] **Step 3: Implement**

```diff
diff --git a/crates/tulis/src/editor/actions.rs b/crates/tulis/src/editor/actions.rs
index c68501d..847fd1a 100644
--- a/crates/tulis/src/editor/actions.rs
+++ b/crates/tulis/src/editor/actions.rs
@@ -180,6 +180,9 @@ impl Editor {
     }
 
     pub(super) fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
+        if self.walking() {
+            return self.walk_step(true, cx);
+        }
         // Enter in a trim preview's cut keeps it (or makes a kept one active again).
         if self.selection.is_empty() && self.toggle_cut_at(self.selection.head, cx) {
             return;
@@ -191,6 +194,9 @@ impl Editor {
     }
 
     pub(super) fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
+        if self.walking() {
+            return self.walk_step(false, cx);
+        }
         if self.vim_captures_text() {
             return self.horizontal(false, false, cx);
         }
diff --git a/crates/tulis/src/editor/features.rs b/crates/tulis/src/editor/features.rs
index d646b50..9c9fb87 100644
--- a/crates/tulis/src/editor/features.rs
+++ b/crates/tulis/src/editor/features.rs
@@ -298,13 +298,16 @@ impl Editor {
         self.use_overflow(range, self.selection.head, copy, cx)
     }
 
-    /// Esc: cancel AI, else close the trim preview, else stop editing an alternate (and leave
+    /// Esc: cancel AI, else end a walk through cuts, else close the trim preview, else stop editing an alternate (and leave
     /// vim's Insert or Visual mode, or drop its pending keys). With none of that to do, close an
     /// open sidebar.
     pub(super) fn cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
         if self.stop_ai() {
             return self.set_notice("AI request cancelled", cx);
         }
+        if self.end_walk(cx) {
+            return;
+        }
         if self.trim_preview.take().is_some() {
             return self.set_notice("Trim preview closed", cx);
         }
diff --git a/crates/tulis/src/editor/input.rs b/crates/tulis/src/editor/input.rs
index 9ef6e64..e02820f 100644
--- a/crates/tulis/src/editor/input.rs
+++ b/crates/tulis/src/editor/input.rs
@@ -78,6 +78,10 @@ impl EntityInputHandler for Editor {
         window: &mut Window,
         cx: &mut Context<Self>,
     ) {
+        // A walk through a trim preview captures plain `y` and `n`.
+        if self.walking() && (text == "y" || text == "n") {
+            return self.walk_step(text == "y", cx);
+        }
         if self.vim_captures_text() {
             for ch in text.chars() {
                 self.vim_key(tulis_core::vim::VimKey::Char(ch), window, cx);
diff --git a/crates/tulis/src/editor/lab.rs b/crates/tulis/src/editor/lab.rs
index 9f9d6e8..c788253 100644
--- a/crates/tulis/src/editor/lab.rs
+++ b/crates/tulis/src/editor/lab.rs
@@ -71,6 +71,18 @@ pub struct TrimPreview {
     pub cuts: Vec<TrimSuggestion>,
     /// Indices into `cuts` the writer chose to keep; the others are active.
     pub kept: HashSet<usize>,
+    /// Walking through the cuts one at a time.
+    pub walk: Option<Walk>,
+}
+
+/// A walk through a trim preview's cuts.
+#[derive(Clone, Copy, Debug, PartialEq, Eq)]
+pub struct Walk {
+    /// The cut being decided (an index into `cuts`).
+    pub at: usize,
+    /// Cuts ghosted and kept so far.
+    pub cut: usize,
+    pub kept: usize,
 }
 
 impl TrimPreview {
@@ -79,9 +91,26 @@ impl TrimPreview {
             level,
             cuts,
             kept: HashSet::new(),
+            walk: None,
         }
     }
 
+    /// The first active cut at index `from` or later.
+    fn next_active(&self, from: usize) -> Option<usize> {
+        (from..self.cuts.len()).find(|&ix| self.is_active(ix))
+    }
+
+    /// Remove cut `ix` (once it is ghosted); later indices shift down.
+    fn remove(&mut self, ix: usize) {
+        self.cuts.remove(ix);
+        self.kept = self
+            .kept
+            .iter()
+            .filter(|&&k| k != ix)
+            .map(|&k| if k > ix { k - 1 } else { k })
+            .collect();
+    }
+
     pub fn is_active(&self, ix: usize) -> bool {
         ix < self.cuts.len() && !self.kept.contains(&ix)
     }
@@ -96,8 +125,11 @@ impl TrimPreview {
             .collect()
     }
 
-    /// Keep only the cuts `keep` accepts; kept cuts stay kept.
+    /// Keep only the cuts `keep` accepts; kept cuts stay kept. Dropping any ends a walk.
     pub(crate) fn retain(&mut self, keep: impl Fn(&TrimSuggestion) -> bool) {
+        if !self.cuts.iter().all(&keep) {
+            self.walk = None;
+        }
         let cuts = std::mem::take(&mut self.cuts);
         let kept = std::mem::take(&mut self.kept);
         for (ix, cut) in cuts.into_iter().enumerate() {
@@ -115,6 +147,7 @@ impl TrimPreview {
 #[derive(Clone, Copy, Debug, PartialEq, Eq)]
 pub enum BarAction {
     MakeCuts,
+    Walk,
     Done,
 }
 
@@ -122,6 +155,7 @@ impl BarAction {
     pub fn label(self) -> &'static str {
         match self {
             BarAction::MakeCuts => "Make the cuts",
+            BarAction::Walk => "Walk through",
             BarAction::Done => "Done",
         }
     }
@@ -130,6 +164,7 @@ impl BarAction {
     pub fn id(self) -> &'static str {
         match self {
             BarAction::MakeCuts => "make-cuts",
+            BarAction::Walk => "walk",
             BarAction::Done => "done",
         }
     }
@@ -368,6 +403,17 @@ impl Editor {
     /// The bar's text and buttons, if the Lab has something showing.
     pub fn lab_bar(&self) -> Option<LabBar> {
         let preview = self.trim_preview.as_ref()?;
+        if let Some(walk) = preview.walk {
+            let decided = walk.cut + walk.kept;
+            let left = (walk.at..preview.cuts.len())
+                .filter(|&ix| preview.is_active(ix))
+                .count();
+            return Some(LabBar {
+                title: format!("Cut {} of {}", decided + 1, decided + left),
+                hint: Some("y or Enter cuts it · n or Backspace keeps it · Esc stops".into()),
+                actions: vec![BarAction::Done],
+            });
+        }
         let before = stats::document_counts(&self.doc).words;
         let after = stats::words_without(&self.doc, &self.lab_items());
         let pct = if before == 0 {
@@ -381,7 +427,7 @@ impl Editor {
                 preview.level.label()
             ),
             hint: Some("Faded words would go. Click one to keep it.".into()),
-            actions: vec![BarAction::MakeCuts, BarAction::Done],
+            actions: vec![BarAction::MakeCuts, BarAction::Walk, BarAction::Done],
         })
     }
 
@@ -389,6 +435,7 @@ impl Editor {
     pub fn bar_action(&mut self, action: BarAction, cx: &mut Context<Self>) {
         match action {
             BarAction::MakeCuts => self.make_cuts(cx),
+            BarAction::Walk => self.start_walk(cx),
             BarAction::Done => {
                 self.stop_ai();
                 self.trim_preview = None;
@@ -422,6 +469,113 @@ impl Editor {
         );
     }
 
+    /// Walking through the trim preview's cuts (`y`/`n`, Enter/Backspace and Esc are captured).
+    pub fn walking(&self) -> bool {
+        self.trim_preview.as_ref().is_some_and(|t| t.walk.is_some())
+    }
+
+    /// Walk through: select the first active cut and capture the walk keys.
+    fn start_walk(&mut self, cx: &mut Context<Self>) {
+        let Some(preview) = &mut self.trim_preview else {
+            return;
+        };
+        let Some(at) = preview.next_active(0) else {
+            return self.set_notice("No cuts to walk through", cx);
+        };
+        preview.walk = Some(Walk {
+            at,
+            cut: 0,
+            kept: 0,
+        });
+        self.select_walk_cut(cx);
+    }
+
+    /// Select the walk's current cut (scrolling to it).
+    fn select_walk_cut(&mut self, cx: &mut Context<Self>) {
+        let Some(preview) = &self.trim_preview else {
+            return;
+        };
+        let Some(walk) = preview.walk else { return };
+        let cut = &preview.cuts[walk.at];
+        let Some(para) = self.doc.para_index(cut.para_id) else {
+            return;
+        };
+        let range = cut.range.clone();
+        self.set_selection(
+            Selection {
+                anchor: DocPos::new(para, range.start),
+                head: DocPos::new(para, range.end),
+            },
+            cx,
+        );
+    }
+
+    /// `y`/Enter (`cut`) ghosts the walk's current cut, as its own undo step; `n`/Backspace
+    /// keeps it. Either moves on; after the last cut the walk and the preview end.
+    pub(crate) fn walk_step(&mut self, cut: bool, cx: &mut Context<Self>) {
+        let Some(walk) = self.trim_preview.as_ref().and_then(|t| t.walk) else {
+            return;
+        };
+        let target = self.trim_preview.as_ref().and_then(|t| {
+            let c = &t.cuts[walk.at];
+            Some((self.doc.para_index(c.para_id)?, c.range.clone()))
+        });
+        let mut next = walk;
+        if cut {
+            if let Some((para, range)) = target {
+                self.record_step(EditKind::Other);
+                if self.doc.set_ghost(para, range, true).is_ok() {
+                    self.dirty = true;
+                }
+            }
+            next.cut += 1;
+        } else {
+            next.kept += 1;
+        }
+        let Some(preview) = &mut self.trim_preview else {
+            return;
+        };
+        let from = if cut {
+            preview.remove(walk.at);
+            walk.at
+        } else {
+            preview.kept.insert(walk.at);
+            walk.at + 1
+        };
+        match preview.next_active(from) {
+            Some(at) => {
+                preview.walk = Some(Walk { at, ..next });
+                self.select_walk_cut(cx);
+            }
+            None => {
+                self.trim_preview = None;
+                let head = self.selection.head;
+                self.set_selection(Selection::caret(head), cx);
+                let total = next.cut + next.kept;
+                self.set_notice(
+                    format!(
+                        "Walked through {total} cuts: {} cut, {} kept",
+                        next.cut, next.kept
+                    ),
+                    cx,
+                );
+            }
+        }
+    }
+
+    /// Esc during a walk: stop it, leaving the preview. Returns false when not walking.
+    pub(crate) fn end_walk(&mut self, cx: &mut Context<Self>) -> bool {
+        let Some(preview) = &mut self.trim_preview else {
+            return false;
+        };
+        if preview.walk.take().is_none() {
+            return false;
+        }
+        let head = self.selection.head;
+        self.set_selection(Selection::caret(head), cx);
+        true
+    }
+
     /// A click (or Enter) at `pos`: keep the active cut there, or make a kept one active
     /// again. Returns false when there is no cut at `pos`.
     pub(crate) fn toggle_cut_at(&mut self, pos: DocPos, cx: &mut Context<Self>) -> bool {
```

- [ ] **Step 4: Run the tests (expect success)**

Run: `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: 178 app tests and 163 core tests pass; clippy counts match the baseline. Launch check as in the Global Constraints.

- [ ] **Step 5: Commit**

```bash
jj describe -m "feat: walk through a trim preview's cuts (y/Enter cut, n/Backspace keep, Esc stops)

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 6: Mark checks

**Files:**
- Modify: `crates/tulis/src/editor/lab.rs`, `crates/tulis/src/editor/{mod.rs, ai.rs, element.rs, features.rs}`, `crates/tulis/src/runs.rs`, `crates/tulis/src/lab_popover.rs`, `crates/tulis/src/workspace.rs`, `crates/tulis/src/editor/lab_tests.rs`

**Interfaces:**
- Consumes: Task 1's `mark_request`, `parse_marks`, `locate_marks`; Task 2's `long_sentences`; `config.editor.long_sentence_words`.
- Produces: `enum Check { Weakest, Long, Convoluted, Tone, Hedges }` with `ALL`, `label()`, `short_label()`, `description()`; `struct Mark { para_id, range, check, note }`; `Editor::marks: Vec<Mark>`; `Editor::run_check(check, scope, cx)`, `mark_at(pos)`, `mark_ranges(id)`; `BarAction::Clear`; `LabItem::Check(Check)`, `LabPanel::explain`; `build_runs(.., faded, tinted, ..)` with `RunStyle::mark_tint`.

The long-sentence check runs locally: `long_sentences` over each paragraph's rendered text, note "N words". The others send `mark_request` per non-empty paragraph; the tone check adds the first 1500 characters of the document's export text. New marks (sorted) replace any marks or trim preview, and a new trim preview replaces marks; none toasts "Nothing marked". The bar reads "<n> marked · <short label>", its second line is the note of the mark under the cursor, and **Clear** stops any run and removes them. Edits drop the marks in their paragraphs, and undo clears them. Esc clears them after a preview ("Marks cleared"). Tab / `]m` / `[m` move between marks when no preview shows. Ctrl+Shift+G (and `gA`, the menu) with a caret inside a mark asks for alternatives to the mark's range. The popover lists the checks first, then the trim buttons, with a "what each idea does" link that toggles a description under each check. `build_runs` now has eight parameters, so it carries `#[allow(clippy::too_many_arguments)]`.

- [ ] **Step 1: Write the failing tests**

```diff
diff --git a/crates/tulis/src/runs.rs b/crates/tulis/src/runs.rs
index 0695f08..064b58b 100644
--- a/crates/tulis/src/runs.rs
+++ b/crates/tulis/src/runs.rs
@@ -102,6 +102,7 @@ mod tests {
             fade_alpha: 0.35,
             marker: hsla(0., 0., 0.5, 0.5),
             ghost_alpha: 0.1,
+            mark_tint: hsla(0.14, 0.9, 0.6, 0.3),
         }
     }
 
@@ -124,7 +125,7 @@ mod tests {
                 alt: None,
             },
         ];
-        let runs = build_runs(10, &spans, &[7..9], None, 0, false, &style());
+        let runs = build_runs(10, &spans, &[7..9], &[], None, 0, false, &style());
         assert_eq!(
             runs.iter().map(|r| r.len).collect::<Vec<_>>(),
             vec![2, 4, 1, 2, 1]
@@ -144,12 +145,12 @@ mod tests {
             ghost: false,
             alt: None,
         }];
-        let runs = build_runs(7, &spans, &[], None, 2, false, &style());
+        let runs = build_runs(7, &spans, &[], &[], None, 2, false, &style());
         assert_eq!(runs.iter().map(|r| r.len).collect::<Vec<_>>(), vec![2, 5]);
         assert_eq!(runs[0].color.a, 0.);
         assert_eq!(runs[1].color.a, 1.);
         assert!(runs.iter().all(|r| r.font.weight == FontWeight::BOLD));
-        let plain = build_runs(7, &spans, &[], None, 0, false, &style());
+        let plain = build_runs(7, &spans, &[], &[], None, 0, false, &style());
         assert!(plain.iter().all(|r| r.font.weight != FontWeight::BOLD));
     }
 
@@ -160,9 +161,9 @@ mod tests {
             ghost: false,
             alt: None,
         }];
-        let hidden = build_runs(7, &spans, &[], None, 2, false, &style());
+        let hidden = build_runs(7, &spans, &[], &[], None, 2, false, &style());
         assert_eq!(hidden[0].color.a, 0.);
-        let shown = build_runs(7, &spans, &[], None, 2, true, &style());
+        let shown = build_runs(7, &spans, &[], &[], None, 2, true, &style());
         assert_eq!(shown[0].color, style().marker);
         assert!(shown[0].color.a > 0.);
         // The title text itself is untouched.
@@ -176,7 +177,7 @@ mod tests {
             ghost: false,
             alt: None,
         }];
-        let runs = build_runs(5, &spans, &[], Some(1..3), 0, false, &style());
+        let runs = build_runs(5, &spans, &[], &[], Some(1..3), 0, false, &style());
         assert_eq!(
             runs.iter().map(|r| r.len).collect::<Vec<_>>(),
             vec![1, 2, 2]
@@ -185,8 +186,27 @@ mod tests {
         assert!(runs[2].underline.is_none());
     }
 
+    #[test]
+    fn marks_get_a_tinted_background() {
+        let spans = vec![Span {
+            range: 0..6,
+            ghost: false,
+            alt: None,
+        }];
+        let runs = build_runs(6, &spans, &[], &[2..4, 5..6], None, 0, false, &style());
+        assert_eq!(
+            runs.iter().map(|r| r.len).collect::<Vec<_>>(),
+            vec![2, 2, 1, 1]
+        );
+        assert_eq!(runs[1].background_color, Some(style().mark_tint));
+        assert_eq!(runs[3].background_color, Some(style().mark_tint));
+        assert_eq!(runs[0].background_color, None);
+        assert_eq!(runs[2].background_color, None);
+        assert_eq!(runs[1].color, style().color);
+    }
+
     #[test]
     fn empty_text_has_no_runs() {
-        assert!(build_runs(0, &[], &[], None, 0, false, &style()).is_empty());
+        assert!(build_runs(0, &[], &[], &[], None, 0, false, &style()).is_empty());
     }
 }
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
index 04a02f6..7556266 100644
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -1329,9 +1329,14 @@ mod tests {
         let mock = lab_mock(&ws, cx, &[r#"["really "]"#]);
         cx.simulate_keystrokes("secondary-l");
         draw(cx);
+        // Past the checks to the trim buttons.
+        let checks = crate::editor::Check::ALL.len();
+        for _ in 0..checks {
+            cx.simulate_keystrokes("j");
+        }
         cx.simulate_keystrokes("j j down k up right");
         let highlight = ws.read_with(cx, |ws, cx| ws.lab.read(cx).highlight);
-        assert_eq!(highlight, 2);
+        assert_eq!(highlight, checks + 2);
         cx.simulate_keystrokes("left enter");
         draw(cx);
         assert_eq!(mock.requests.lock().unwrap().len(), 1);
@@ -1362,6 +1367,9 @@ mod tests {
         cx.simulate_keystrokes("secondary-shift-t");
         draw(cx);
         assert_eq!(lab_scope(&ws, cx), vec![1]);
+        for _ in 0..crate::editor::Check::ALL.len() {
+            cx.simulate_keystrokes("j");
+        }
         cx.simulate_keystrokes("down enter");
         draw(cx);
         let requests = mock.requests.lock().unwrap().clone();
@@ -1429,6 +1437,30 @@ mod tests {
         assert!(page_focused(&ws, cx));
     }
 
+    #[gpui::test]
+    fn the_lab_lists_checks_then_trims_and_explains_the_checks(cx: &mut TestAppContext) {
+        let (ws, cx) = root_workspace(cx, &["Short. This sentence runs on and on and on."]);
+        ws.update(cx, |ws, cx| {
+            ws.editor
+                .update(cx, |e, _| e.config.editor.long_sentence_words = 4)
+        });
+        cx.simulate_keystrokes("secondary-l");
+        draw(cx);
+        assert!(cx.debug_bounds("lab-check-0").is_some());
+        assert!(cx.debug_bounds("lab-desc-0").is_none());
+        let link = cx.debug_bounds("lab-explain").expect("the explain link");
+        cx.simulate_click(link.center(), gpui::Modifiers::none());
+        draw(cx);
+        assert!(cx.debug_bounds("lab-desc-0").is_some());
+        // The second row is the long-sentence check; it runs locally and closes the Lab.
+        cx.simulate_keystrokes("j enter");
+        draw(cx);
+        assert!(!lab_open(&ws, cx));
+        let marks = ws.read_with(cx, |ws, cx| ws.editor.read(cx).marks.len());
+        assert_eq!(marks, 1);
+        assert!(cx.debug_bounds("lab-bar").is_some());
+    }
+
     #[gpui::test]
     fn the_lab_button_and_vim_gl_open_the_lab(cx: &mut TestAppContext) {
         let (ws, cx) = root_workspace(cx, &["One."]);
diff --git a/crates/tulis/src/editor/lab_tests.rs b/crates/tulis/src/editor/lab_tests.rs
index 08c3236..bec629b 100644
--- a/crates/tulis/src/editor/lab_tests.rs
+++ b/crates/tulis/src/editor/lab_tests.rs
@@ -8,7 +8,7 @@ use tulis_core::ai::{AiError, CompletionRequest, LlmProvider, MockProvider};
 use tulis_core::vim::Vim;
 use tulis_core::{DocPos, ParaId, Selection};
 
-use super::lab::{BarAction, TrimSuggestion};
+use super::lab::{BarAction, Check, TrimSuggestion};
 use super::tests::{editor_with, texts, type_str};
 use super::{Editor, TrimLevel, TrimPreview};
 use crate::actions::*;
@@ -427,3 +427,160 @@ fn escape_stops_the_walk_and_leaves_the_rest_of_the_preview(cx: &mut TestAppCont
     type_str(&editor, cx, "n");
     assert_eq!(texts(&editor, cx), vec!["nI really love tea, truly."]);
 }
+
+fn marks(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> Vec<(usize, String, String)> {
+    editor.read_with(cx, |e, _| {
+        e.marks
+            .iter()
+            .map(|m| {
+                let para = e.doc.para_index(m.para_id).unwrap();
+                let text = e.doc.paragraphs[para].text()[m.range.clone()].to_string();
+                (para, text, m.note.clone())
+            })
+            .collect()
+    })
+}
+
+#[gpui::test]
+fn the_long_sentence_check_marks_locally(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(
+        cx,
+        &[
+            "Short one. This one has more than five words in it.",
+            "Tiny.",
+        ],
+    );
+    let mock = mock(&editor, cx, &[]);
+    editor.update(cx, |e, _| e.config.editor.long_sentence_words = 5);
+    let all = ids(&editor, cx);
+    editor.update(cx, |e, cx| e.run_check(Check::Long, &all, cx));
+    assert!(mock.requests.lock().unwrap().is_empty());
+    assert!(!editor.read_with(cx, |e, _| e.lab_running()));
+    assert_eq!(
+        marks(&editor, cx),
+        vec![(
+            0,
+            "This one has more than five words in it.".to_string(),
+            "9 words".to_string()
+        )]
+    );
+    let bar = editor.read_with(cx, |e, _| e.lab_bar()).unwrap();
+    assert_eq!(bar.title, "1 marked · long sentences");
+    assert_eq!(bar.actions, vec![BarAction::Clear]);
+    // Clear removes the marks.
+    editor.update(cx, |e, cx| e.bar_action(BarAction::Clear, cx));
+    assert!(marks(&editor, cx).is_empty());
+    assert!(editor.read_with(cx, |e, _| e.lab_bar()).is_none());
+}
+
+#[gpui::test]
+fn ai_mark_checks_locate_exact_text_and_drop_invented_text(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["I think it is fine.", "", "Maybe so, really."]);
+    let mock = mock(
+        &editor,
+        cx,
+        &[
+            r#"[{"text": "I think", "note": "hedge"}]"#,
+            r#"[{"text": "really", "note": "filler"}, {"text": "perhaps", "note": "invented"}]"#,
+        ],
+    );
+    let all = ids(&editor, cx);
+    editor.update(cx, |e, cx| e.run_check(Check::Hedges, &all, cx));
+    cx.run_until_parked();
+    let requests = mock.requests.lock().unwrap().clone();
+    assert_eq!(requests.len(), 2);
+    assert!(requests[0].system.contains("hedges"));
+    assert_eq!(
+        marks(&editor, cx),
+        vec![
+            (0, "I think".to_string(), "hedge".to_string()),
+            (2, "really".to_string(), "filler".to_string()),
+        ]
+    );
+    assert_eq!(
+        bar_title(&editor, cx).as_deref(),
+        Some("2 marked · hedges and filler")
+    );
+    // The tone check sends an excerpt of the whole document along.
+    let mock = self::mock(&editor, cx, &["[]"]);
+    editor.update(cx, |e, cx| e.run_check(Check::Tone, &all[2..], cx));
+    cx.run_until_parked();
+    let request = mock.requests.lock().unwrap()[0].clone();
+    assert!(request.user.contains("I think it is fine."));
+    assert!(request.user.ends_with("Paragraph:\nMaybe so, really."));
+    // Nothing found: the marks go, with a toast.
+    assert!(marks(&editor, cx).is_empty());
+    assert_eq!(notice(&editor, cx).as_deref(), Some("Nothing marked"));
+}
+
+#[gpui::test]
+fn tab_moves_between_marks_and_the_bar_shows_the_note_under_the_cursor(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["I think it is fine.", "Maybe so, really."]);
+    mock(
+        &editor,
+        cx,
+        &[
+            r#"[{"text": "I think", "note": "hedge"}]"#,
+            r#"[{"text": "really", "note": "filler"}]"#,
+        ],
+    );
+    let all = ids(&editor, cx);
+    editor.update(cx, |e, cx| e.run_check(Check::Hedges, &all, cx));
+    cx.run_until_parked();
+    let hint = |cx: &mut VisualTestContext| editor.read_with(cx, |e, _| e.lab_bar().unwrap().hint);
+    editor.update(cx, |e, _| {
+        e.selection = Selection::caret(DocPos::new(0, 10))
+    });
+    assert_eq!(hint(cx), None);
+    editor.update_in(cx, |e, window, cx| e.next_mark(&NextMark, window, cx));
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.selection),
+        Selection::caret(DocPos::new(1, 10))
+    );
+    assert_eq!(hint(cx).as_deref(), Some("filler"));
+    editor.update_in(cx, |e, window, cx| e.prev_mark(&PrevMark, window, cx));
+    assert_eq!(hint(cx).as_deref(), Some("hedge"));
+    // Editing a paragraph drops its marks; Esc clears the rest.
+    type_str(&editor, cx, "Oh, ");
+    assert_eq!(marks(&editor, cx).len(), 1);
+    editor.update_in(cx, |e, window, cx| e.cancel(&Cancel, window, cx));
+    assert!(marks(&editor, cx).is_empty());
+    assert_eq!(notice(&editor, cx).as_deref(), Some("Marks cleared"));
+}
+
+#[gpui::test]
+fn ctrl_shift_g_inside_a_mark_targets_the_mark(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["I think it is fine."]);
+    let mock = mock(
+        &editor,
+        cx,
+        &[r#"[{"text": "I think", "note": "hedge"}]"#, r#"["I hold"]"#],
+    );
+    let all = ids(&editor, cx);
+    editor.update(cx, |e, cx| e.run_check(Check::Hedges, &all, cx));
+    cx.run_until_parked();
+    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(0, 3)));
+    editor.update_in(cx, |e, window, cx| {
+        e.ai_alternates(&AiAlternates, window, cx)
+    });
+    cx.run_until_parked();
+    let request = mock.requests.lock().unwrap()[1].clone();
+    assert!(request.user.ends_with("Passage to rewrite:\nI think"));
+    assert_eq!(texts(&editor, cx), vec!["I hold it is fine."]);
+}
+
+#[gpui::test]
+fn a_check_replaces_the_trim_preview_and_a_trim_replaces_the_marks(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["One two three four five six seven."]);
+    preview(&editor, cx, &[(0, 4..8)]);
+    editor.update(cx, |e, _| e.config.editor.long_sentence_words = 3);
+    let all = ids(&editor, cx);
+    editor.update(cx, |e, cx| e.run_check(Check::Long, &all, cx));
+    assert!(editor.read_with(cx, |e, _| e.trim_preview.is_none()));
+    assert_eq!(marks(&editor, cx).len(), 1);
+    mock(&editor, cx, &[r#"["two "]"#]);
+    editor.update(cx, |e, cx| e.run_trim(TrimLevel::Slight, &all, cx));
+    cx.run_until_parked();
+    assert!(marks(&editor, cx).is_empty());
+    assert!(editor.read_with(cx, |e, _| e.trim_preview.is_some()));
+}
```

- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile errors (`Check`, `run_check`, `marks`, `mark_tint` in `RunStyle` don't exist).

- [ ] **Step 3: Implement**

```diff
diff --git a/crates/tulis/src/editor/ai.rs b/crates/tulis/src/editor/ai.rs
index da6725b..83d090f 100644
--- a/crates/tulis/src/editor/ai.rs
+++ b/crates/tulis/src/editor/ai.rs
@@ -77,6 +77,15 @@ impl Editor {
             Ok(target) => target,
             Err(e) => return self.set_notice(e.to_string(), cx),
         };
+        // Inside a Lab mark, the mark is the target.
+        let head = self.selection.head;
+        if self.selection.is_empty()
+            && let Some(ix) = self.mark_at(head)
+        {
+            let range = self.marks[ix].range.clone();
+            self.retarget_alt_session_to(head.para, range.clone(), false, cx);
+            return self.request_ai_alternates(head.para, range, false, cx);
+        }
         let whole_paragraph = self.selection.is_empty();
         self.retarget_alt_session_to(para, range.clone(), whole_paragraph, cx);
         self.request_ai_alternates(para, range, whole_paragraph, cx);
diff --git a/crates/tulis/src/editor/element.rs b/crates/tulis/src/editor/element.rs
index 3a86d07..9c80a2f 100644
--- a/crates/tulis/src/editor/element.rs
+++ b/crates/tulis/src/editor/element.rs
@@ -78,6 +78,7 @@ fn build_layout(
             title_font: title_font.clone(),
             color: faded(editor.theme.text, alpha),
             fade_alpha: editor.theme.fade_alpha,
+            mark_tint: editor.theme.mark_tint,
             marker: faded(editor.theme.text_muted, 0.5 * alpha),
             ghost_alpha: editor.theme.ghost_alpha,
         }
@@ -111,6 +112,7 @@ fn build_layout(
             text_len,
             &p.spans(),
             &cuts,
+            &editor.mark_ranges(p.id),
             marked_here,
             title_marker,
             caret_on_marker,
diff --git a/crates/tulis/src/editor/features.rs b/crates/tulis/src/editor/features.rs
index 9c9fb87..73b326d 100644
--- a/crates/tulis/src/editor/features.rs
+++ b/crates/tulis/src/editor/features.rs
@@ -298,7 +298,8 @@ impl Editor {
         self.use_overflow(range, self.selection.head, copy, cx)
     }
 
-    /// Esc: cancel AI, else end a walk through cuts, else close the trim preview, else stop editing an alternate (and leave
+    /// Esc: cancel AI, else end a walk through cuts, else close the trim preview (or clear the
+    /// marks), else stop editing an alternate (and leave
     /// vim's Insert or Visual mode, or drop its pending keys). With none of that to do, close an
     /// open sidebar.
     pub(super) fn cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
@@ -311,6 +312,10 @@ impl Editor {
         if self.trim_preview.take().is_some() {
             return self.set_notice("Trim preview closed", cx);
         }
+        if !self.marks.is_empty() {
+            self.marks.clear();
+            return self.set_notice("Marks cleared", cx);
+        }
         let idle = self.alt_edit.is_none()
             && self
                 .vim
diff --git a/crates/tulis/src/editor/mod.rs b/crates/tulis/src/editor/mod.rs
index 300b753..2554695 100644
--- a/crates/tulis/src/editor/mod.rs
+++ b/crates/tulis/src/editor/mod.rs
@@ -35,7 +35,7 @@ use tulis_core::{DocPos, Document, ParaId, Selection};
 pub use alt_session::{AltSession, Scope};
 use element::EditorElement;
 pub(crate) use element::GUTTER;
-pub use lab::{TrimLevel, TrimPreview};
+pub use lab::{Check, Mark, TrimLevel, TrimPreview};
 use layout::LayoutCache;
 
 use crate::theme::{self, Theme};
@@ -93,6 +93,8 @@ pub struct Editor {
     pub toasts: ToastQueue,
     /// The Lab's trim preview, if one is showing.
     pub trim_preview: Option<TrimPreview>,
+    /// The Lab's guide-check marks, in document order.
+    pub marks: Vec<Mark>,
     /// Located cuts by level, paragraph and the text sent, with the rendered text they index.
     pub(crate) trim_cache: HashMap<lab::TrimKey, (String, Vec<Range<usize>>)>,
     pub vim: Option<Vim>,
@@ -149,6 +151,7 @@ impl Editor {
             dirty: false,
             toasts: ToastQueue::default(),
             trim_preview: None,
+            marks: Vec::new(),
             trim_cache: HashMap::new(),
             vim,
             alt_edit: None,
@@ -187,6 +190,7 @@ impl Editor {
         self.history = History::new(HISTORY_LIMIT);
         self.overflow_changed(false);
         self.trim_preview = None;
+        self.marks.clear();
         self.trim_cache.clear();
         self.alt_edit = None;
         self.cycled = None;
@@ -351,6 +355,7 @@ impl Editor {
         if let Some(preview) = &mut self.trim_preview {
             preview.retain(|cut| !ids.contains(&cut.para_id));
         }
+        self.marks.retain(|m| !ids.contains(&m.para_id));
     }
 
     pub(crate) fn end_edit(&mut self, cx: &mut Context<Self>) {
@@ -614,6 +619,7 @@ impl Editor {
             head: self.doc.clamp(self.selection.head),
         };
         self.trim_preview = None;
+        self.marks.clear();
         self.marked = None;
         self.goal_x = None;
         self.dirty = true;
diff --git a/crates/tulis/src/runs.rs b/crates/tulis/src/runs.rs
index 064b58b..15a40d1 100644
--- a/crates/tulis/src/runs.rs
+++ b/crates/tulis/src/runs.rs
@@ -1,4 +1,4 @@
-//! Turn a paragraph's spans, a trim preview's cuts, IME marked range and `# ` title marker into
+//! Turn a paragraph's spans, a trim preview's cuts, Lab marks, IME marked range and `# ` title marker into
 //! GPUI text runs.
 
 use std::ops::Range;
@@ -16,12 +16,16 @@ pub struct RunStyle {
     pub ghost_alpha: f32,
     /// Alpha factor of a trim preview's active cuts.
     pub fade_alpha: f32,
+    /// Background of Lab marks.
+    pub mark_tint: Hsla,
 }
 
+#[allow(clippy::too_many_arguments)]
 pub fn build_runs(
     text_len: usize,
     spans: &[Span],
     faded: &[Range<usize>],
+    tinted: &[Range<usize>],
     marked: Option<Range<usize>>,
     title_marker: usize,
     caret_on_marker: bool,
@@ -32,7 +36,7 @@ pub fn build_runs(
     for span in spans {
         cuts.extend([span.range.start, span.range.end]);
     }
-    for range in faded.iter().chain(marked.iter()) {
+    for range in faded.iter().chain(tinted).chain(marked.iter()) {
         cuts.extend([range.start.min(text_len), range.end.min(text_len)]);
     }
     cuts.sort_unstable();
@@ -47,6 +51,7 @@ pub fn build_runs(
             let ghost = span.is_some_and(|s| s.ghost);
             let in_marked = marked.as_ref().is_some_and(|m| m.start <= a && b <= m.end);
             let fade = faded.iter().any(|t| t.start <= a && b <= t.end);
+            let tint = tinted.iter().any(|t| t.start <= a && b <= t.end);
             let mut color = style.color;
             if ghost {
                 color.a *= style.ghost_alpha;
@@ -77,7 +82,7 @@ pub fn build_runs(
                     style.font.clone()
                 },
                 color,
-                background_color: None,
+                background_color: tint.then_some(style.mark_tint),
                 underline,
                 strikethrough: None,
             }
diff --git a/crates/tulis/src/editor/lab.rs b/crates/tulis/src/editor/lab.rs
index c788253..8dee86e 100644
--- a/crates/tulis/src/editor/lab.rs
+++ b/crates/tulis/src/editor/lab.rs
@@ -1,5 +1,5 @@
-//! The Lab on the page: trim previews, the runner every Lab request goes through, and its
-//! cancel flag.
+//! The Lab on the page: trim previews, guide-check marks, the runner every Lab request goes
+//! through, and its cancel flag.
 
 use std::collections::HashSet;
 use std::ops::Range;
@@ -7,8 +7,10 @@ use std::sync::Arc;
 use std::sync::atomic::{AtomicBool, Ordering};
 
 use gpui::{Context, Window, prelude::*};
+use tulis_core::ai::prompts::MarkKind;
 use tulis_core::ai::{AiError, CompletionRequest, prompts};
 use tulis_core::history::EditKind;
+use tulis_core::sentence;
 use tulis_core::stats;
 use tulis_core::{DocPos, ParaId, Selection};
 
@@ -56,6 +58,82 @@ impl TrimLevel {
     }
 }
 
+/// A guide check: it marks passages (or, for typos, proposes fixes).
+#[derive(Clone, Copy, Debug, PartialEq, Eq)]
+pub enum Check {
+    Weakest,
+    Long,
+    Convoluted,
+    Tone,
+    Hedges,
+}
+
+impl Check {
+    /// In the order the Lab lists them.
+    pub const ALL: [Check; 5] = [
+        Check::Weakest,
+        Check::Long,
+        Check::Convoluted,
+        Check::Tone,
+        Check::Hedges,
+    ];
+
+    pub fn label(self) -> &'static str {
+        match self {
+            Check::Weakest => "Mark the weakest sentences",
+            Check::Long => "Mark sentences that run long",
+            Check::Convoluted => "Mark convoluted sentences",
+            Check::Tone => "Mark words that don't fit the tone",
+            Check::Hedges => "Mark hedges and filler",
+        }
+    }
+
+    /// What the bar calls its marks.
+    pub fn short_label(self) -> &'static str {
+        match self {
+            Check::Weakest => "weakest sentences",
+            Check::Long => "long sentences",
+            Check::Convoluted => "convoluted sentences",
+            Check::Tone => "off-tone words",
+            Check::Hedges => "hedges and filler",
+        }
+    }
+
+    /// The one-line description "what each idea does" shows.
+    pub fn description(self) -> &'static str {
+        match self {
+            Check::Weakest => "AI marks the sentences that do the least work.",
+            Check::Long => "Marks sentences over your word limit (long_sentence_words). No AI.",
+            Check::Convoluted => "AI marks sentences that are hard to follow.",
+            Check::Tone => "AI marks words that don't match the rest of the document.",
+            Check::Hedges => "AI marks padding: I think, maybe, really, in order to…",
+        }
+    }
+
+    fn mark_kind(self) -> Option<MarkKind> {
+        match self {
+            Check::Weakest => Some(MarkKind::Weakest),
+            Check::Convoluted => Some(MarkKind::Convoluted),
+            Check::Tone => Some(MarkKind::Tone),
+            Check::Hedges => Some(MarkKind::Hedges),
+            Check::Long => None,
+        }
+    }
+}
+
+/// A passage a check marked. View state, like the trim preview: editing its paragraph drops it.
+#[derive(Clone, Debug, PartialEq, Eq)]
+pub struct Mark {
+    pub para_id: ParaId,
+    /// A byte range of the paragraph's rendered text.
+    pub range: Range<usize>,
+    pub check: Check,
+    pub note: String,
+}
+
+/// How much of the document the tone check sends along.
+const TONE_EXCERPT_CHARS: usize = 1500;
+
 /// A passage a trim would cut: a byte range of its paragraph's rendered text.
 #[derive(Clone, Debug, PartialEq, Eq)]
 pub struct TrimSuggestion {
@@ -149,6 +227,7 @@ pub enum BarAction {
     MakeCuts,
     Walk,
     Done,
+    Clear,
 }
 
 impl BarAction {
@@ -157,6 +236,7 @@ impl BarAction {
             BarAction::MakeCuts => "Make the cuts",
             BarAction::Walk => "Walk through",
             BarAction::Done => "Done",
+            BarAction::Clear => "Clear",
         }
     }
 
@@ -166,6 +246,7 @@ impl BarAction {
             BarAction::MakeCuts => "make-cuts",
             BarAction::Walk => "walk",
             BarAction::Done => "done",
+            BarAction::Clear => "clear",
         }
     }
 }
@@ -396,13 +477,27 @@ impl Editor {
         } else {
             let cuts = cuts.into_iter().map(|(_, cut)| cut).collect();
             self.trim_preview = Some(TrimPreview::new(level, cuts));
+            self.marks.clear();
         }
         cx.notify();
     }
 
     /// The bar's text and buttons, if the Lab has something showing.
     pub fn lab_bar(&self) -> Option<LabBar> {
-        let preview = self.trim_preview.as_ref()?;
+        let Some(preview) = self.trim_preview.as_ref() else {
+            let first = self.marks.first()?;
+            let head = self.selection.head;
+            let note = self.mark_at(head).map(|ix| self.marks[ix].note.clone());
+            return Some(LabBar {
+                title: format!(
+                    "{} marked · {}",
+                    self.marks.len(),
+                    first.check.short_label()
+                ),
+                hint: note.filter(|note| !note.is_empty()),
+                actions: vec![BarAction::Clear],
+            });
+        };
         if let Some(walk) = preview.walk {
             let decided = walk.cut + walk.kept;
             let left = (walk.at..preview.cuts.len())
@@ -441,6 +536,11 @@ impl Editor {
                 self.trim_preview = None;
                 cx.notify();
             }
+            BarAction::Clear => {
+                self.stop_ai();
+                self.marks.clear();
+                cx.notify();
+            }
         }
     }
 
@@ -598,10 +698,153 @@ impl Editor {
         true
     }
 
-    /// The active cuts of the trim preview, as `(paragraph, range)` in document order.
+    /// The mark under `pos` (edges included), as an index into `marks`.
+    pub(crate) fn mark_at(&self, pos: DocPos) -> Option<usize> {
+        let id = self.doc.paragraphs.get(pos.para)?.id;
+        self.marks.iter().position(|m| {
+            m.para_id == id && m.range.start <= pos.offset && pos.offset <= m.range.end
+        })
+    }
+
+    /// The marks in paragraph `id` (drawn tinted).
+    pub(crate) fn mark_ranges(&self, id: ParaId) -> Vec<Range<usize>> {
+        self.marks
+            .iter()
+            .filter(|m| m.para_id == id)
+            .map(|m| m.range.clone())
+            .collect()
+    }
+
+    /// Run `check` over the paragraphs in `scope`. The long-sentence check is local; the others
+    /// send one request per non-empty paragraph. The marks replace any marks or trim preview.
+    pub fn run_check(&mut self, check: Check, scope: &[ParaId], cx: &mut Context<Self>) {
+        self.finish_alt_edit();
+        let paras: Vec<usize> = scope
+            .iter()
+            .filter_map(|id| self.doc.para_index(*id))
+            .filter(|&para| !self.doc.paragraphs[para].export_text().trim().is_empty())
+            .collect();
+        if paras.is_empty() {
+            return self.set_notice("Nothing to check", cx);
+        }
+        let Some(kind) = check.mark_kind() else {
+            let max = self.config.editor.long_sentence_words;
+            let mut marks = Vec::new();
+            for para in paras {
+                let p = &self.doc.paragraphs[para];
+                for (range, words) in sentence::long_sentences(&p.text(), max) {
+                    marks.push(Mark {
+                        para_id: p.id,
+                        range,
+                        check,
+                        note: format!("{words} words"),
+                    });
+                }
+            }
+            return self.show_marks(marks, cx);
+        };
+        let context = (kind == MarkKind::Tone).then(|| self.document_excerpt());
+        let jobs = paras
+            .into_iter()
+            .map(|para| {
+                let p = &self.doc.paragraphs[para];
+                let export = p.export_text();
+                LabJob {
+                    para_id: p.id,
+                    text: p.text(),
+                    request: prompts::mark_request(kind, &export, context.as_deref()),
+                    export,
+                }
+            })
+            .collect();
+        self.run_lab(
+            jobs,
+            "AI is reading…",
+            move |editor, results, total, cx| editor.apply_mark_results(check, results, total, cx),
+            cx,
+        );
+    }
+
+    /// The start of the document's export text, for the tone check.
+    fn document_excerpt(&self) -> String {
+        let text = self
+            .doc
+            .paragraphs
+            .iter()
+            .map(|p| p.export_text())
+            .filter(|t| !t.trim().is_empty())
+            .collect::<Vec<_>>()
+            .join("\n\n");
+        text.chars().take(TONE_EXCERPT_CHARS).collect()
+    }
+
+    fn apply_mark_results(
+        &mut self,
+        check: Check,
+        results: LabResults,
+        total: usize,
+        cx: &mut Context<Self>,
+    ) {
+        let mut failed = 0;
+        let mut first_error = None;
+        let mut marks = Vec::new();
+        for (job, result) in results {
+            let response = match result {
+                Ok(response) => response,
+                Err(e) => {
+                    failed += 1;
+                    first_error.get_or_insert(e.to_string());
+                    continue;
+                }
+            };
+            let Some(para) = self.doc.para_index(job.para_id) else {
+                continue;
+            };
+            let p = &self.doc.paragraphs[para];
+            if p.text() != job.text {
+                continue;
+            }
+            let hits = prompts::parse_marks(&response);
+            marks.extend(
+                prompts::locate_marks(&hits, p)
+                    .into_iter()
+                    .map(|(range, note)| Mark {
+                        para_id: job.para_id,
+                        range,
+                        check,
+                        note,
+                    }),
+            );
+        }
+        if failed < total {
+            self.show_marks(marks, cx);
+        }
+        if let Some(notice) = failure_notice(failed, total, first_error) {
+            self.set_notice(notice, cx);
+        }
+    }
+
+    /// Show `marks` (in document order) in place of any marks or trim preview.
+    fn show_marks(&mut self, mut marks: Vec<Mark>, cx: &mut Context<Self>) {
+        let order = |m: &Mark| (self.doc.para_index(m.para_id), m.range.start);
+        marks.sort_by_key(order);
+        if marks.is_empty() {
+            self.set_notice("Nothing marked", cx);
+        }
+        self.marks = marks;
+        self.trim_preview = None;
+        cx.notify();
+    }
+
+    /// The active cuts of the trim preview, or else the marks, as `(paragraph, range)` in
+    /// document order.
     fn lab_items(&self) -> Vec<(usize, Range<usize>)> {
         let Some(preview) = &self.trim_preview else {
-            return Vec::new();
+            return self
+                .marks
+                .iter()
+                .filter_map(|m| Some((self.doc.para_index(m.para_id)?, m.range.clone())))
+                .collect();
         };
         preview
             .cuts
@@ -612,7 +855,7 @@ impl Editor {
             .collect()
     }
 
-    /// Tab, `]m`, `[m`: put the cursor at the start of the next (or previous) active cut,
+    /// Tab, `]m`, `[m`: put the cursor at the start of the next (or previous) active cut or mark,
     /// `count` times, wrapping around.
     pub(crate) fn jump_lab_item(&mut self, forward: bool, count: usize, cx: &mut Context<Self>) {
         let items = self.lab_items();
diff --git a/crates/tulis/src/lab_popover.rs b/crates/tulis/src/lab_popover.rs
index eecd15e..57dd2ab 100644
--- a/crates/tulis/src/lab_popover.rs
+++ b/crates/tulis/src/lab_popover.rs
@@ -9,20 +9,26 @@ use gpui_component::Selectable;
 use tulis_core::ParaId;
 
 use crate::actions::{LabChoose, LabNext, LabPrev};
-use crate::editor::{Editor, TrimLevel};
+use crate::editor::{Check, Editor, TrimLevel};
 
 pub const LAB_WIDTH: f32 = 340.;
 
 /// Something the Lab can run.
 #[derive(Clone, Copy, Debug, PartialEq, Eq)]
 pub enum LabItem {
+    Check(Check),
     Trim(TrimLevel),
 }
 
 impl LabItem {
-    /// In the order the popover lists them (and the arrow keys walk them).
+    /// In the order the popover lists them (and the arrow keys walk them): the checks, then
+    /// the trim levels.
     pub fn all() -> Vec<LabItem> {
-        TrimLevel::ALL.into_iter().map(LabItem::Trim).collect()
+        Check::ALL
+            .into_iter()
+            .map(LabItem::Check)
+            .chain(TrimLevel::ALL.into_iter().map(LabItem::Trim))
+            .collect()
     }
 }
 
@@ -38,6 +44,8 @@ pub struct LabPanel {
     pub(crate) scope: Vec<ParaId>,
     /// Index into `LabItem::all()`.
     pub(crate) highlight: usize,
+    /// "what each idea does" is expanded.
+    pub(crate) explain: bool,
     _subscription: Subscription,
 }
 
@@ -51,6 +59,7 @@ impl LabPanel {
             focus: cx.focus_handle(),
             scope: Vec::new(),
             highlight: 0,
+            explain: false,
         }
     }
 
@@ -90,6 +99,7 @@ impl LabPanel {
     pub(crate) fn run(&mut self, item: LabItem, cx: &mut Context<Self>) {
         let scope = self.scope.clone();
         self.editor.update(cx, |e, cx| match item {
+            LabItem::Check(check) => e.run_check(check, &scope, cx),
             LabItem::Trim(level) => e.run_trim(level, &scope, cx),
         });
         if !self.editor.read(cx).lab_running() {
@@ -104,11 +114,22 @@ impl Render for LabPanel {
         let t = editor.theme;
         let running = editor.lab_running();
         let highlight = self.highlight;
+        let explain = self.explain;
         let items = LabItem::all();
-        let trims = items.iter().enumerate().map(|(ix, item)| {
-            let LabItem::Trim(level) = item;
-            (ix, *level)
-        });
+        let checks = items
+            .iter()
+            .enumerate()
+            .filter_map(|(ix, item)| match item {
+                LabItem::Check(check) => Some((ix, *check)),
+                LabItem::Trim(_) => None,
+            });
+        let trims = items
+            .iter()
+            .enumerate()
+            .filter_map(|(ix, item)| match item {
+                LabItem::Trim(level) => Some((ix, *level)),
+                LabItem::Check(_) => None,
+            });
         div()
             .id("lab-panel")
             .debug_selector(|| "lab-panel".into())
@@ -123,7 +144,58 @@ impl Render for LabPanel {
             .gap_2()
             .text_sm()
             .text_color(t.text)
-            .child(div().italic().text_color(t.accent).child("The Lab guide…"))
+            .child(
+                div()
+                    .flex()
+                    .flex_row()
+                    .items_baseline()
+                    .child(
+                        div()
+                            .flex_1()
+                            .italic()
+                            .text_color(t.accent)
+                            .child("The Lab guide…"),
+                    )
+                    .child(
+                        div()
+                            .id("lab-explain")
+                            .debug_selector(|| "lab-explain".into())
+                            .text_xs()
+                            .text_color(t.text_muted)
+                            .underline()
+                            .cursor_pointer()
+                            .child("what each idea does")
+                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
+                                this.explain = !this.explain;
+                                cx.notify();
+                            })),
+                    ),
+            )
+            .child(div().flex().flex_col().children(checks.map(|(ix, check)| {
+                div()
+                    .id(("lab-check", ix))
+                    .debug_selector(move || format!("lab-check-{ix}"))
+                    .px_2()
+                    .py_1()
+                    .rounded_md()
+                    .cursor_pointer()
+                    .when(ix == highlight, |d| d.bg(t.selection))
+                    .hover(|d| d.bg(t.selection))
+                    .child(check.label())
+                    .when(explain, |d| {
+                        d.child(
+                            div()
+                                .debug_selector(move || format!("lab-desc-{ix}"))
+                                .text_xs()
+                                .text_color(t.text_muted)
+                                .child(check.description()),
+                        )
+                    })
+                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
+                        this.highlight = ix;
+                        this.run(LabItem::Check(check), cx);
+                    }))
+            })))
             .child(
                 div()
                     .flex()
```

- [ ] **Step 4: Run the tests (expect success)**

Run: `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: 185 app tests and 163 core tests pass; clippy counts match the baseline. Launch check as in the Global Constraints.

- [ ] **Step 5: Commit**

```bash
jj describe -m "feat: Lab mark checks with tinted marks, notes, Tab/]m/[m and Clear

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 7: Fix punctuation and typos

**Files:**
- Modify: `crates/tulis/src/editor/lab.rs`, `crates/tulis/src/editor/mod.rs`, `crates/tulis/src/editor/features.rs`, `crates/tulis/src/workspace.rs`, `crates/tulis/src/editor/lab_tests.rs`

**Interfaces:**
- Consumes: Task 1's `fix_request`, `parse_fixes`, `locate_fixes`; `Document::add_ai_alternates`.
- Produces: `Check::Fix` (first in `Check::ALL`); `Editor::lab_fixes: Option<usize>`.

One request per paragraph. Located fixes become AI alternates (the fix shown, the original one Alt+↑ away), applied last-first within each paragraph so earlier ranges hold, all after one `begin_edit`, so one undo removes them. The bar reads "N fixes proposed — Alt+↑/↓ to compare" with **Clear** (which hides the bar; the alternates stay). No fixes toasts "No fixes needed" and records no undo step. Esc hides the bar after marks.

- [ ] **Step 1: Write the failing tests**

```diff
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
index 7556266..c1492e9 100644
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -1452,8 +1452,8 @@ mod tests {
         cx.simulate_click(link.center(), gpui::Modifiers::none());
         draw(cx);
         assert!(cx.debug_bounds("lab-desc-0").is_some());
-        // The second row is the long-sentence check; it runs locally and closes the Lab.
-        cx.simulate_keystrokes("j enter");
+        // The third row is the long-sentence check; it runs locally and closes the Lab.
+        cx.simulate_keystrokes("j j enter");
         draw(cx);
         assert!(!lab_open(&ws, cx));
         let marks = ws.read_with(cx, |ws, cx| ws.editor.read(cx).marks.len());
diff --git a/crates/tulis/src/editor/lab_tests.rs b/crates/tulis/src/editor/lab_tests.rs
index bec629b..48b5e6a 100644
--- a/crates/tulis/src/editor/lab_tests.rs
+++ b/crates/tulis/src/editor/lab_tests.rs
@@ -584,3 +584,67 @@ fn a_check_replaces_the_trim_preview_and_a_trim_replaces_the_marks(cx: &mut Test
     assert!(marks(&editor, cx).is_empty());
     assert!(editor.read_with(cx, |e, _| e.trim_preview.is_some()));
 }
+
+#[gpui::test]
+fn fixes_become_ai_alternates_in_one_undo_step(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["Its a nice day, its true.", "Teh end."]);
+    let mock = mock(
+        &editor,
+        cx,
+        &[
+            r#"[{"original": "Its", "fixed": "It's"}, {"original": "a nice dya", "fixed": "x"}]"#,
+            r#"[{"original": "Teh", "fixed": "The"}]"#,
+        ],
+    );
+    let all = ids(&editor, cx);
+    editor.update(cx, |e, cx| e.run_check(Check::Fix, &all, cx));
+    cx.run_until_parked();
+    assert!(
+        mock.requests.lock().unwrap()[0]
+            .system
+            .contains("proofreader")
+    );
+    assert_eq!(
+        texts(&editor, cx),
+        vec!["It's a nice day, its true.", "The end."]
+    );
+    editor.read_with(cx, |e, _| {
+        let info = e
+            .doc
+            .alt_info(DocPos::new(0, 1))
+            .expect("an alternate group");
+        assert_eq!(info.count, 2);
+        assert!(e.doc.paragraphs[0].segments()[0].active_is_ai());
+    });
+    assert_eq!(
+        bar_title(&editor, cx).as_deref(),
+        Some("2 fixes proposed — Alt+↑/↓ to compare")
+    );
+    // The original is one Alt+↑ away.
+    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(1, 1)));
+    editor.update_in(cx, |e, window, cx| e.cycle_prev(&CyclePrev, window, cx));
+    assert_eq!(texts(&editor, cx)[1], "Teh end.");
+    // Clear hides the bar; the alternates stay.
+    editor.update(cx, |e, cx| e.bar_action(BarAction::Clear, cx));
+    assert!(editor.read_with(cx, |e, _| e.lab_bar()).is_none());
+    // One undo step for every fix (and one for the cycle).
+    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
+    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
+    assert_eq!(
+        texts(&editor, cx),
+        vec!["Its a nice day, its true.", "Teh end."]
+    );
+    assert!(editor.read_with(cx, |e, _| e.doc.alt_info(DocPos::new(0, 1)).is_none()));
+}
+
+#[gpui::test]
+fn a_fix_check_with_nothing_to_fix_changes_nothing(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["All good."]);
+    mock(&editor, cx, &["[]"]);
+    let all = ids(&editor, cx);
+    editor.update(cx, |e, cx| e.run_check(Check::Fix, &all, cx));
+    cx.run_until_parked();
+    assert_eq!(notice(&editor, cx).as_deref(), Some("No fixes needed"));
+    assert!(!editor.read_with(cx, |e, _| e.dirty));
+    assert!(editor.read_with(cx, |e, _| e.lab_bar()).is_none());
+}
```

- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile error (`Check::Fix` doesn't exist).

- [ ] **Step 3: Implement**

```diff
diff --git a/crates/tulis/src/editor/features.rs b/crates/tulis/src/editor/features.rs
index 73b326d..08c702b 100644
--- a/crates/tulis/src/editor/features.rs
+++ b/crates/tulis/src/editor/features.rs
@@ -316,6 +316,9 @@ impl Editor {
             self.marks.clear();
             return self.set_notice("Marks cleared", cx);
         }
+        if self.lab_fixes.take().is_some() {
+            return cx.notify();
+        }
         let idle = self.alt_edit.is_none()
             && self
                 .vim
diff --git a/crates/tulis/src/editor/mod.rs b/crates/tulis/src/editor/mod.rs
index 2554695..cfe1498 100644
--- a/crates/tulis/src/editor/mod.rs
+++ b/crates/tulis/src/editor/mod.rs
@@ -95,6 +95,8 @@ pub struct Editor {
     pub trim_preview: Option<TrimPreview>,
     /// The Lab's guide-check marks, in document order.
     pub marks: Vec<Mark>,
+    /// How many fixes the last "Fix punctuation and typos" proposed, while its bar shows.
+    pub lab_fixes: Option<usize>,
     /// Located cuts by level, paragraph and the text sent, with the rendered text they index.
     pub(crate) trim_cache: HashMap<lab::TrimKey, (String, Vec<Range<usize>>)>,
     pub vim: Option<Vim>,
@@ -152,6 +154,7 @@ impl Editor {
             toasts: ToastQueue::default(),
             trim_preview: None,
             marks: Vec::new(),
+            lab_fixes: None,
             trim_cache: HashMap::new(),
             vim,
             alt_edit: None,
@@ -191,6 +194,7 @@ impl Editor {
         self.overflow_changed(false);
         self.trim_preview = None;
         self.marks.clear();
+        self.lab_fixes = None;
         self.trim_cache.clear();
         self.alt_edit = None;
         self.cycled = None;
diff --git a/crates/tulis/src/editor/lab.rs b/crates/tulis/src/editor/lab.rs
index 8dee86e..ab52144 100644
--- a/crates/tulis/src/editor/lab.rs
+++ b/crates/tulis/src/editor/lab.rs
@@ -12,7 +12,7 @@ use tulis_core::ai::{AiError, CompletionRequest, prompts};
 use tulis_core::history::EditKind;
 use tulis_core::sentence;
 use tulis_core::stats;
-use tulis_core::{DocPos, ParaId, Selection};
+use tulis_core::{AiKind, DocPos, ParaId, Selection, Source};
 
 use super::{Editor, EditorEvent};
 use crate::actions::{NextMark, PrevMark};
@@ -61,6 +61,7 @@ impl TrimLevel {
 /// A guide check: it marks passages (or, for typos, proposes fixes).
 #[derive(Clone, Copy, Debug, PartialEq, Eq)]
 pub enum Check {
+    Fix,
     Weakest,
     Long,
     Convoluted,
@@ -70,7 +71,8 @@ pub enum Check {
 
 impl Check {
     /// In the order the Lab lists them.
-    pub const ALL: [Check; 5] = [
+    pub const ALL: [Check; 6] = [
+        Check::Fix,
         Check::Weakest,
         Check::Long,
         Check::Convoluted,
@@ -80,6 +82,7 @@ impl Check {
 
     pub fn label(self) -> &'static str {
         match self {
+            Check::Fix => "Fix punctuation and typos",
             Check::Weakest => "Mark the weakest sentences",
             Check::Long => "Mark sentences that run long",
             Check::Convoluted => "Mark convoluted sentences",
@@ -91,6 +94,7 @@ impl Check {
     /// What the bar calls its marks.
     pub fn short_label(self) -> &'static str {
         match self {
+            Check::Fix => "fixes",
             Check::Weakest => "weakest sentences",
             Check::Long => "long sentences",
             Check::Convoluted => "convoluted sentences",
@@ -102,6 +106,7 @@ impl Check {
     /// The one-line description "what each idea does" shows.
     pub fn description(self) -> &'static str {
         match self {
+            Check::Fix => "AI proposes corrections as alternates; Alt+↑/↓ compares them.",
             Check::Weakest => "AI marks the sentences that do the least work.",
             Check::Long => "Marks sentences over your word limit (long_sentence_words). No AI.",
             Check::Convoluted => "AI marks sentences that are hard to follow.",
@@ -116,7 +121,7 @@ impl Check {
             Check::Convoluted => Some(MarkKind::Convoluted),
             Check::Tone => Some(MarkKind::Tone),
             Check::Hedges => Some(MarkKind::Hedges),
-            Check::Long => None,
+            Check::Fix | Check::Long => None,
         }
     }
 }
@@ -276,6 +281,9 @@ pub(crate) type LabResults = Vec<(LabJob, Result<String, AiError>)>;
 /// Key of the trim cache: level, paragraph and the export text that was sent.
 pub(crate) type TrimKey = (TrimLevel, ParaId, String);
 
+/// Located fixes (or marks): ranges of a paragraph's rendered text, each with its text.
+type Located = Vec<(Range<usize>, String)>;
+
 /// What a finished run found, for one paragraph: its rendered text then, and the ranges.
 type Found = Vec<(ParaId, String, Vec<Range<usize>>)>;
 
@@ -478,6 +486,7 @@ impl Editor {
             let cuts = cuts.into_iter().map(|(_, cut)| cut).collect();
             self.trim_preview = Some(TrimPreview::new(level, cuts));
             self.marks.clear();
+            self.lab_fixes = None;
         }
         cx.notify();
     }
@@ -485,7 +494,15 @@ impl Editor {
     /// The bar's text and buttons, if the Lab has something showing.
     pub fn lab_bar(&self) -> Option<LabBar> {
         let Some(preview) = self.trim_preview.as_ref() else {
-            let first = self.marks.first()?;
+            let Some(first) = self.marks.first() else {
+                let fixes = self.lab_fixes?;
+                let s = if fixes == 1 { "" } else { "es" };
+                return Some(LabBar {
+                    title: format!("{fixes} fix{s} proposed — Alt+↑/↓ to compare"),
+                    hint: None,
+                    actions: vec![BarAction::Clear],
+                });
+            };
             let head = self.selection.head;
             let note = self.mark_at(head).map(|ix| self.marks[ix].note.clone());
             return Some(LabBar {
@@ -539,6 +556,7 @@ impl Editor {
             BarAction::Clear => {
                 self.stop_ai();
                 self.marks.clear();
+                self.lab_fixes = None;
                 cx.notify();
             }
         }
@@ -727,6 +745,9 @@ impl Editor {
         if paras.is_empty() {
             return self.set_notice("Nothing to check", cx);
         }
+        if check == Check::Fix {
+            return self.run_fixes(paras, cx);
+        }
         let Some(kind) = check.mark_kind() else {
             let max = self.config.editor.long_sentence_words;
             let mut marks = Vec::new();
@@ -765,6 +786,99 @@ impl Editor {
         );
     }
 
+    /// "Fix punctuation and typos": one request per paragraph; the fixes become AI alternates.
+    fn run_fixes(&mut self, paras: Vec<usize>, cx: &mut Context<Self>) {
+        let Some(provider) = self.provider(cx) else {
+            return;
+        };
+        let source = Source::Ai {
+            provider: provider.name().into(),
+            model: provider.model().into(),
+            kind: AiKind::Rewrite,
+        };
+        let jobs = paras
+            .into_iter()
+            .map(|para| {
+                let p = &self.doc.paragraphs[para];
+                let export = p.export_text();
+                LabJob {
+                    para_id: p.id,
+                    text: p.text(),
+                    request: prompts::fix_request(&export),
+                    export,
+                }
+            })
+            .collect();
+        self.run_lab(
+            jobs,
+            "AI is proofreading…",
+            move |editor, results, total, cx| editor.apply_fix_results(source, results, total, cx),
+            cx,
+        );
+    }
+
+    /// Every located fix becomes an AI alternate (shown), all in one undo step.
+    fn apply_fix_results(
+        &mut self,
+        source: Source,
+        results: LabResults,
+        total: usize,
+        cx: &mut Context<Self>,
+    ) {
+        let mut failed = 0;
+        let mut first_error = None;
+        let mut found: Vec<(usize, Located)> = Vec::new();
+        for (job, result) in results {
+            let response = match result {
+                Ok(response) => response,
+                Err(e) => {
+                    failed += 1;
+                    first_error.get_or_insert(e.to_string());
+                    continue;
+                }
+            };
+            let Some(para) = self.doc.para_index(job.para_id) else {
+                continue;
+            };
+            let p = &self.doc.paragraphs[para];
+            if p.text() != job.text {
+                continue;
+            }
+            let fixes = prompts::locate_fixes(&prompts::parse_fixes(&response), p);
+            if !fixes.is_empty() {
+                found.push((para, fixes));
+            }
+        }
+        let notice = failure_notice(failed, total, first_error);
+        if found.is_empty() {
+            if failed < total {
+                self.set_notice("No fixes needed", cx);
+            }
+        } else {
+            self.finish_alt_edit();
+            self.begin_edit(EditKind::Other, 0..self.doc.paragraphs.len());
+            let mut made = 0;
+            for (para, fixes) in found {
+                // Last first, so the earlier ranges still hold.
+                for (range, fixed) in fixes.into_iter().rev() {
+                    let added =
+                        self.doc
+                            .add_ai_alternates(para, range, vec![fixed], source.clone());
+                    if added.is_ok() {
+                        made += 1;
+                    }
+                }
+            }
+            self.trim_preview = None;
+            self.marks.clear();
+            self.lab_fixes = Some(made);
+            self.end_edit(cx);
+        }
+        if let Some(notice) = notice {
+            self.set_notice(notice, cx);
+        }
+    }
+
     /// The start of the document's export text, for the tone check.
     fn document_excerpt(&self) -> String {
         let text = self
@@ -833,6 +947,7 @@ impl Editor {
         }
         self.marks = marks;
         self.trim_preview = None;
+        self.lab_fixes = None;
         cx.notify();
     }
 
```

- [ ] **Step 4: Run the tests (expect success)**

Run: `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: 187 app tests and 163 core tests pass; clippy counts match the baseline. Launch check as in the Global Constraints.

- [ ] **Step 5: Commit**

```bash
jj describe -m "feat: Lab fixes for punctuation and typos as AI alternates

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 8: README, smoke checklist and final verification

**Files:**
- Modify: `README.md`

**Interfaces:** none (docs only).

- [ ] **Step 1: Update the README**

The new checklist items go at the end of the checklist, before `## License`.

```diff
diff --git a/README.md b/README.md
index 48704b3..9a6f4a0 100644
--- a/README.md
+++ b/README.md
@@ -30,18 +30,19 @@ paragraph that still matches and tells you how many paragraphs lost theirs.
 | `Ctrl+Shift+\` | Move focus between the page and the scratchpad (opens it); `Esc` in the scratchpad returns to the page |
 | `Ctrl+Enter` / `Ctrl+Shift+Enter` (in the scratchpad) | Move / copy its selection, or the block at its cursor, to the page cursor. Or drag a block's `⠿` grip onto the page (hold `Ctrl` to copy) |
 | `Ctrl+Shift+G` (or `Ctrl+J`) | AI alternatives for the selection, else the paragraph (as versions); an open sidebar follows to that target. In the sidebar (or its "✦ Ask AI" row): for the sidebar's target |
-| `Ctrl+Shift+T` / `Ctrl+Shift+Alt+T` | AI trim suggestions for the paragraph / document; `Tab` accepts (ghosts), `Esc` dismisses |
+| `Ctrl+Shift+T` / `Ctrl+Shift+Alt+T` | The Lab on the cursor's paragraph / the whole document |
+| `Tab` / `Shift+Tab` | Next / previous trim cut or Lab mark |
 | `Ctrl+Alt+V` | Toggle vim mode |
 | `Ctrl+K T` | Cycle theme (Midnight, Paper, Sepia, Black) for the session |
 | `Ctrl+.` | Show/hide alternate and version markers |
 | `Ctrl+Shift+M` | Markdown preview (`Esc` closes) |
 | `Ctrl+?` | Cheat sheet |
 | Right-click (or `Shift+F10` / the Menu key, at the caret) | Menu: Alternatives / AI alternatives for the selection (or the word under the pointer), Ghost it / Revive it, Stash this in Overflow |
-| `Ctrl+L` | The Lab (runs the document trim until Part 4) |
+| `Ctrl+L` | The Lab (or click LAB): on the selection's paragraphs, else the whole document. Arrows or `j`/`k` and `Enter` pick a guide check or a trim level; `Esc` closes it (and stops a run in progress) |
 | `Ctrl+S` / `Ctrl+Shift+S` / `Ctrl+O` / `Ctrl+N` / `Ctrl+Q` | Save / Save as / Open / New / Quit |
 
 Vim mode: `h j k l w b e 0 ^ $ gg G { }` with counts; `i a I A o O`; `d c y` + motion, `dd cc yy`,
-`x p P u Ctrl+R`; `v`/`V`; `]a`/`[a` cycle alternates; `gh` toggles ghost; `ga` sidebar, `gA` AI alternatives, `gs` stash (in Visual mode these act on the selection); `gt gm gp g? gl gx gX`
+`x p P u Ctrl+R`; `v`/`V`; `]a`/`[a` cycle alternates; `]m`/`[m` next/previous Lab cut or mark; `gh` toggles ghost; `ga` sidebar, `gA` AI alternatives, `gs` stash (in Visual mode these act on the selection); `gt gm gp g? gl gx gX`
 (theme, markers, preview, cheat sheet, Lab, Overflow, focus Overflow). In `V` (VisualLine) mode `ga gA gs gh` act on whole paragraphs; `Vgs` removes them, like `Vd`.
 
 Sentence tab: sentences end at `.`, `!` or `?` followed by a space or the end of the paragraph.
@@ -61,7 +62,7 @@ vim_mode = false
 font_family = "IBM Plex Mono"   # falls back to IBM Plex Mono, JetBrains Mono, then the platform mono font
 font_size = 18
 theme = "midnight"              # midnight | paper | sepia | black (Ctrl+K T cycles for the session)
-long_sentence_words = 30        # used by the Lab (Part 4)
+long_sentence_words = 30        # the Lab's "Mark sentences that run long" check
 
 [ai]
 provider = "openai"                         # any OpenAI-compatible endpoint, or "anthropic"
@@ -92,7 +93,7 @@ cargo test --workspace
 - [ ] Overflow: stash, panel toggle, type in the scratchpad, Ctrl+Enter / drag back to the page
 - [ ] Save/Open round-trips alternates, ghosts, overflow; external edit shows drift notice
 - [ ] Unsaved-changes prompt on New/Open/Quit/window close
-- [ ] AI alternates (violet underline) and trim (red strike, Tab/Esc) with a real key; missing key notice without
+- [ ] AI alternates (violet underline) and a Lab trim preview (faded cuts) with a real key; missing key notice without
 - [ ] Vim mode: motions, operators, visual, `]a`, `gh`, `ga` (sidebar), `gA` (AI alternatives), `gs` (stash), also from `v`/`V` (`V` acts on whole paragraphs); `vim_mode = true` starts in Normal
 - [ ] A missing font family falls back to a readable font
 - [ ] Midnight theme on first start; Ctrl+K T / `gt` cycles Midnight → Paper → Sepia → Black with a toast; buttons and dialogs follow the theme
@@ -105,7 +106,7 @@ cargo test --workspace
 - [ ] Header fades after ~2s of continuous typing and returns on mouse move or a 2s pause
 - [ ] Ctrl+Shift+M / `gp` shows the Markdown preview without ghosts; Esc returns
 - [ ] Ctrl+? / `g?` / keyboard icon opens the cheat sheet; typing filters; Esc closes
-- [ ] Footer: Save and Open work; Share X/in copy the text and open the browser (X trims to 280 with a toast); LAB runs the trim; XYZ toggles Overflow
+- [ ] Footer: Save and Open work; Share X/in copy the text and open the browser (X trims to 280 with a toast); LAB opens the Lab; XYZ toggles Overflow
 - [ ] Toasts appear centered above the footer, at most two, and fade after ~3s
 - [ ] Vim mode shows a NORMAL/INSERT badge next to the counts
 - [ ] Ctrl+Shift+A / Ctrl+D / `ga` opens a 240px sidebar on the left; while it has focus the page dims except the target paragraph; tabs read Word (Selection with a selection), Sentence, Paragraph in italic accent
@@ -117,7 +118,7 @@ cargo test --workspace
 - [ ] Delete / `dd` removes the highlighted option but never the last; Esc closes and keeps what is shown
 - [ ] Deleting the target paragraph (or typing inside the target) closes the sidebar
 - [ ] Right-click a word: the menu targets it (or the selection); Ghost it turns into Revive it on ghosted text; Stash moves it to Overflow
-- [ ] Ctrl+Z / Ctrl+Shift+Z work while the sidebar has focus; Esc on the page (with no AI request, trims or alternate edit pending) closes the sidebar
+- [ ] Ctrl+Z / Ctrl+Shift+Z work while the sidebar has focus; Esc on the page (with no AI request, Lab preview or marks, or alternate edit pending) closes the sidebar
 - [ ] Shift+F10 or the Menu key opens the right-click menu at the caret
 - [ ] XYZ / Ctrl+\ / `gx` opens a 260px Overflow panel on the right: italic "Overflow" title, a text field, and "ctrl+↵ or drag into the page to use" below it
 - [ ] Ctrl+Shift+X / `gs` / the menu stash appends after a blank line with a "Stashed to Overflow" toast and doesn't open the panel; `Vgs` leaves no empty paragraph
@@ -126,6 +127,19 @@ cargo test --workspace
 - [ ] Hovering a block shows a `⠿` grip next to its first line; dragging it onto the page inserts it where it's dropped (Ctrl held copies)
 - [ ] Ctrl+Shift+\ / `gX` moves focus page ↔ scratchpad; Esc in the scratchpad returns to the page
 - [ ] A file saved by an older Tulis opens with its overflow items joined into the scratchpad; saving writes sidecar version 2
+- [ ] LAB / Ctrl+L / `gl` opens the Lab above the LAB button with focus in it: "The Lab guide…", six checks, five trim buttons; arrows or `j`/`k` move, Enter runs, Esc closes and returns focus to the page
+- [ ] "what each idea does" expands a one-line description under each check
+- [ ] Ctrl+Shift+T scopes the Lab to the cursor's paragraph, Ctrl+Shift+Alt+T to the whole document, a selection to its paragraphs
+- [ ] A trim level fades the words it would cut; the bar reads "<Level> N → M words · −P%" and "Faded words would go. Click one to keep it."
+- [ ] Clicking a faded passage (or Enter in it) keeps it; clicking it again fades it again; Tab / `]m` / `[m` move between active cuts
+- [ ] Make the cuts ghosts every faded passage with a "Cut N passages" toast; one Ctrl+Z brings them all back
+- [ ] Walk through selects each cut in turn ("Cut i of n"): `y`/Enter cuts, `n`/Backspace keeps, Esc stops; afterwards `y`/`n` type again
+- [ ] Done (or Esc) clears the preview with no change; choosing a level again is instant (cached)
+- [ ] Each mark check tints its passages; the bar shows "N marked · <check>" and the note under the cursor; Tab / `]m` / `[m` move between marks; Clear removes them
+- [ ] "Mark sentences that run long" works without an API key, using `long_sentence_words`
+- [ ] Ctrl+Shift+G with the cursor in a mark asks for alternatives to the mark
+- [ ] "Fix punctuation and typos" shows each fix as an AI alternate (Alt+↑/↓ compares) with "N fixes proposed"; one Ctrl+Z undoes them all
+- [ ] Closing the Lab (Esc, click outside) while it works shows "Lab stopped" and sends no further requests; a partial failure toasts "N of M paragraphs failed"
 
 ## License
 
```

- [ ] **Step 2: Full verification**

Run: `cargo fmt --all --check && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: formatting is clean, 187 app tests and 163 core tests pass, and the clippy counts match the baseline.

Run: `cargo build && timeout 6 target/debug/tulis /tmp/smoke.md; echo $?`
Expected: `124`, with nothing on stderr.

Then walk through the new smoke-checklist items by hand. Report any that fail rather than ticking them.

- [ ] **Step 3: Commit**

```bash
jj describe -m "docs: README keys and smoke checklist for redesign part 4

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

## Decisions this plan makes where the spec is silent or ambiguous

- **The popover stays open while a run waits for the AI** ("Working… Esc stops") and closes when it finishes, so "closing the popover while running" has a literal meaning: Esc, a click outside or LAB stop the run with "Lab stopped". Runs that need no AI (cache hits, `Original`, long sentences) close it at once.
- **Ctrl+Shift+T / Ctrl+Shift+Alt+T** become `LabParagraph` / `LabDocument`, handled by the workspace (they bubble from the page). Ctrl+L with no selection is the same as Ctrl+Shift+Alt+T.
- **The trim cache key** is (level, paragraph id, export text sent), and an entry is used only while the paragraph's rendered text still matches. So once cuts are ghosted, asking the same level again asks the AI anew instead of offering the ghosted cuts.
- **One Lab overlay at a time:** a trim preview, marks or the fixes bar replaces the others.
- **Tab / `]m` / `[m`** put the caret at the start of the next or previous active cut (or, with no preview, mark), wrapping. Shift+Tab is the previous one. They don't select. With nothing showing, Tab does nothing, as before.
- **Enter in a cut toggles it** like a click: an active cut is kept, a kept one becomes active again. Edges count as inside. Enter outside every cut starts a paragraph as usual.
- **Esc order on the page:** cancel a run, end a walk, close the trim preview, clear marks, hide the fixes bar, then the older behavior (alternate editing, vim, sidebar).
- **All requests failing** toasts the first error (the v1 message) and leaves any earlier preview or marks alone. Some failing toasts "N of M paragraphs failed", where M counts the paragraphs sent. Stale paragraphs are dropped silently.
- **Walk through** visits active cuts in order. Each `y` is its own undo step and removes that cut from the preview. The walk state carries counts (`Walk { at, cut, kept }`) so the bar can say "Cut i of n". The spec's `walk: Option<usize>` is `walk: Option<Walk>`. When the last cut is decided, the walk and the preview end with a summary toast.
- **The "Make the cuts" toast** says "passage" for one cut.
- **Long sentences** are counted on the rendered text, so ghosted words count; whitespace separates words.
- **Mark bar label:** a short label ("long sentences", "hedges and filler") reads better after "N marked ·" than the row text.
- **The tone context** is the first 1500 characters of the document's export text (non-empty paragraphs joined by blank lines).
- **Fixes** are `AiKind::Rewrite` alternates showing the fix. A fix that `add_ai_alternates` refuses is skipped, and the bar counts only the applied ones.
