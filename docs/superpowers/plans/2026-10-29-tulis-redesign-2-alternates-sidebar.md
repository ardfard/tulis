# Tulis Redesign, Part 2: Alternates Sidebar, Sentence Alternates, Context Menu

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the alternates sidebar from Write_On. It has Word/Selection, Sentence and Paragraph tabs, a list of options with `●`/`·`/`✦` markers, a type-to-add field and an "✦ Ask AI" row. Arrow keys and `j`/`k` preview options live in the page, as one undo step per session. Opening it on plain text creates no group. The page dims around the target, and the sidebar closes when the target disappears. Sentence and Selection groups merge the groups inside them, with a toast. The part also adds sentence detection and option/version operations in core, a right-click context menu (Ghost it / Revive it), Ctrl+Shift+A/G/X and Ctrl+/ (the old keys still work), and vim `ga`, `gA`, `gs` in Normal and Visual mode. Every command goes through `commands.rs`, so the keymap and the cheat sheet stay in sync.

**Architecture:** `tulis-core` gains `sentence.rs` and seven option/version methods in `alternates.rs`. The vim machine gains three `g` selection commands. The sidebar's state is model state on `Editor` (`editor/alt_session.rs`: `AltSession`, `Scope`, `AltRow` and the operations), so its behavior is tested in plain editor tests without gpui-component. The view (`sidebar.rs`, `AltSidebar`) holds the focus handles and the `InputState`, and only calls editor methods. The workspace creates the view the first time the editor emits `EditorEvent::AlternatesOpened`, and renders it as a 240px left panel while `editor.alt_session` is set. Sidebar keys are actions bound in two key contexts: `AltSidebar` (Esc, Ctrl+Shift+G) on the whole panel, and `AltList` (j/k/arrows/Enter/Delete/dd/i/o) on the list's own focus node. The text field is a sibling of the list, so letters typed into it never hit list bindings. The context menu wraps the page in the workspace. The editor's right-button handler picks the target first, then the menu's items dispatch the existing editor actions to the page's focus handle.

**Tech Stack:** Rust 1.98 (edition 2024), `gpui-pre =0.3.10` (imported as `gpui`), `gpui-component 0.8.1` (`tab::{TabBar, Tab}`, `menu::ContextMenuExt`, `input::{Input, InputState, InputEvent}`), plus the existing `tulis-core` dependencies. No new crates.

**Spec:** `docs/superpowers/specs/2026-10-29-tulis-redesign-design.md`, sections B.1–B.6 and the keymap table. Builds on part 1 (`docs/superpowers/plans/2026-10-29-tulis-redesign-1-foundation-chrome.md`). Read both before starting a task.

**How the code is given.** New files are given in full. Changes to existing files are given as unified diffs against the state the previous task leaves (3 lines of context, the same as `git diff`). Apply them by hand or with `patch -p1`. Each task's Step 1 shows the test hunks and Step 3 the implementation hunks. Every task was implemented and run in a scratch copy of the repository, in this order.

## Global Constraints

- Version control is **jj**. Commit with `jj describe -m "<message>" && jj new`. Never run `git commit`.
- Every commit message ends with a blank line and then `Co-Authored-By: Claude <model> <noreply@anthropic.com>`, with your model name filled in (for example `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`).
- Run `cargo fmt --all` before every commit. The diffs below are already formatted.
- No new clippy warnings. `cargo clippy --workspace --all-targets` reports exactly: `tulis-core (lib)` 3 warnings, `tulis (bin "tulis")` 3 warnings and `tulis (bin "tulis" test)` 4 warnings (all from v1 code). No count may go up. Dead-code warnings count too, so don't add an item before the task that uses it. The tasks are ordered so that every item has a user when it lands.
- Every task ends with `cargo test --workspace` passing. The starting point (HEAD `fd99aca`) has 94 app tests and 125 core tests. Part 2 ends with 118 and 137.
- Every app task ends with a launch check: `cargo build && timeout 6 ./target/debug/tulis /tmp/smoke.md; echo $?` prints `124`, with nothing on stderr.
- `tulis-core` must not depend on `gpui`.
- Positions are byte offsets into a paragraph's rendered text (ghosts included), always on char boundaries.
- Editor tests (`editor/tests.rs`) don't initialize gpui-component. Anything that renders `TabBar`, `Input` or a context menu is tested from `workspace.rs` tests, which call `init(cx)` and, where a `Root` is needed, use the `root_workspace` helper added in Task 4.
- Don't search `.devenv` or `.direnv`.
- **gpui-pre API facts** (checked against `~/.cargo/registry/src/*/gpui-pre-0.3.10`):
  - `Context::subscribe_in(&Entity<E>, &Window, FnMut(&mut T, &Entity<E>, &Evt, &mut Window, &mut Context<T>))` (`src/app/context.rs:341`). The workspace uses it so editor events can move focus.
  - Key dispatch tries every matching binding, deepest context first, until a handler doesn't propagate (`src/window.rs`, `dispatch_key_event`). gpui-component's Input `Escape` handler propagates when it has nothing to dismiss, so an `escape` binding on an ancestor context still fires.
  - Mouse listeners bubble from the innermost element out: an element's `on_mouse_down(MouseButton::Right, …)` runs before a wrapping context menu's listener.
  - `Styled::line_clamp(usize)` (`src/styled.rs:145`). `FocusHandle::{is_focused(&Window), contains_focused(&Window, &App)}`.
  - Test helpers: `VisualTestContext::{simulate_keystrokes("a b"), simulate_input(text), simulate_mouse_down/up(point, MouseButton, Modifiers), simulate_click}` (`src/app/test_context.rs:949-980`).
- **gpui-component API facts** (checked against `~/.cargo/registry/src/*/gpui-component-0.8.1` and `gpui-base-0.8.1`):
  - Tabs: `gpui_component::tab::{TabBar, Tab}`. `TabBar::new(id).underline().small().selected_index(ix).children(tabs).on_click(Fn(&usize, &mut Window, &mut App))` (`src/tab/tab_bar.rs:68-187`). `Tab::new().child(..)` (`src/tab/tab.rs:624`, `ParentElement` at :733). `small()` comes from `gpui_component::Sizable`.
  - Context menu: `use gpui_component::menu::ContextMenuExt;` then `element.context_menu(|menu: PopupMenu, window, cx| menu.action_context(focus).menu(label, Box::new(Action)))` (`src/menu/context_menu.rs:17`, `src/menu/popup_menu.rs:373,467`). The builder runs in `window.defer` on every open. In the menu, `down` selects and `enter` confirms (`popup_menu.rs:23-26`). Confirming focuses the `action_context` handle and dispatches the action there.
  - Input: `InputState::new(window, cx).placeholder(..)`, `.value()`, `.set_value(v, window, cx)`, `InputEvent::PressEnter { secondary, shift }` (`gpui-base src/input/base/state.rs:132`), `Input::new(&state).small()`.
- Unknown API? Read those source files. Don't guess.

## File Map

```
README.md                               keys, vim line, smoke checklist (Task 9)
crates/tulis-core/src/
  lib.rs                                + pub mod sentence;
  sentence.rs            (new)          sentence_ranges, sentence_range_at
  alternates.rs                         options_at, version_texts, add_option, set_active_option,
                                        set_active_version, remove_option_at, remove_version, add_version
  vim.rs                                VimCommand::{Alternatives, AiAlternatives, Stash}; ga gA gs
crates/tulis/src/
  actions.rs                            + OpenAlternates, CloseAlternates, AltNext, AltPrev, AltKeep,
                                          AltRemove, AltFocusInput
  commands.rs                           alternatives / ai_alternates / toggle_ghost / stash rows; sidebar rows
  keymap.rs                             binding-count test for the new keys
  main.rs                               + mod sidebar;
  sidebar.rs             (new)          AltSidebar view; Workspace::{alternates_opened, alternates_closed}
  workspace.rs                          sidebar panel, editor events via subscribe_in, page context menu, tests
  editor/mod.rs                         alt_session field + events, sync in end_edit/after_history, handlers
  editor/alt_session.rs  (new)          Scope, AltSession, AltRow, open/retarget/rows/close/sync,
                                        preview/keep/remove, add, merge detection
  editor/ai.rs                          ask_ai_alternates (sidebar target), merge toast, follow the target
  editor/element.rs                     DIM_ALPHA, paragraph_alpha, dimmed RunStyle
  editor/actions.rs                     ⋮ opens the Paragraph tab; right-click target; ghost_menu_label
  editor/features.rs                    next_version removed
  editor/vim_exec.rs                    ga/gA/gs; Visual selection commands get the inclusive end
  editor/tests.rs                       sidebar, preview/undo, add/merge, AI, dimming, handle, menu tests
```

---

### Task 1: Sentence detection in core

**Files:**
- Create: `crates/tulis-core/src/sentence.rs`
- Modify: `crates/tulis-core/src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `tulis_core::sentence::{sentence_ranges(text: &str) -> Vec<Range<usize>>, sentence_range_at(text: &str, offset: usize) -> Range<usize>}`.

- [ ] **Step 1: Write the failing tests**

Create `crates/tulis-core/src/sentence.rs` with only the test module below for now, and register the module in `crates/tulis-core/src/lib.rs`:

```rust
pub mod sentence;
```

(insert it between `pub mod pos;` and `pub mod share;`).

```rust
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
```
- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile errors: `sentence_ranges` and `sentence_range_at` are not defined.

- [ ] **Step 3: Implement**

Put this above the test module in `sentence.rs`:

```rust
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
```
- [ ] **Step 4: Run the sentence tests (expect pass)**

Run: `cargo test -p tulis-core sentence::`
Expected: 6 passed.

- [ ] **Step 5: Run everything (expect pass)**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: all tests pass (core gains 6); the clippy counts match the baseline.
- [ ] **Step 6: Commit**

```bash
cargo fmt --all
jj describe -m "feat(core): sentence detection

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---
### Task 2: Core option and version operations

**Files:**
- Modify: `crates/tulis-core/src/alternates.rs`

**Interfaces:**
- Consumes: `Document::ensure_alt` (private, existing), `normalize_segments`.
- Produces:
  - `Paragraph::options_at(&self, seg: usize) -> Vec<(String, Source)>`
  - `Paragraph::version_texts(&self) -> Vec<(String, Source)>`
  - `Document::add_option(&mut self, para: usize, range: Range<usize>, text: String) -> Result<Range<usize>, OpError>` (empty text is `NothingHere`, an empty range `EmptySelection`)
  - `Document::set_active_option(&mut self, para: usize, seg: usize, index: usize) -> Option<Range<usize>>` (the group's span afterwards)
  - `Document::set_active_version(&mut self, para: usize, index: usize) -> bool`
  - `Document::remove_option_at(&mut self, para: usize, seg: usize, index: usize) -> Option<Range<usize>>` (never the last option; a group left with one option becomes plain text)
  - `Document::remove_version(&mut self, para: usize, index: usize) -> bool` (never the last version)
  - `Document::add_version(&mut self, para: usize, text: String) -> usize` (a human-written plain version, made active; the sidebar's Paragraph tab uses it)

- [ ] **Step 1: Write the failing tests**

Add to the test module of `alternates.rs`, before `fn ai_versions_become_active`:

`crates/tulis-core/src/alternates.rs`:
```diff
--- a/crates/tulis-core/src/alternates.rs
+++ b/crates/tulis-core/src/alternates.rs
@@ -481,6 +609,82 @@ mod tests {
         assert_eq!(options(&d, 1).0.len(), 3);
     }
 
+    #[test]
+    fn add_option_creates_the_group_once_and_shows_the_new_option() {
+        let mut d = doc();
+        assert_eq!(d.add_option(0, 2..6, "adore".into()), Ok(2..7));
+        assert_eq!(d.paragraphs[0].text(), "I adore tea");
+        assert_eq!(options(&d, 1), (vec!["love".into(), "adore".into()], 1));
+        assert_eq!(d.add_option(0, 2..7, "like".into()), Ok(2..6));
+        assert_eq!(options(&d, 1).0.len(), 3);
+        assert_eq!(
+            d.add_option(0, 2..6, String::new()),
+            Err(OpError::NothingHere)
+        );
+        assert_eq!(
+            d.add_option(0, 3..3, "x".into()),
+            Err(OpError::EmptySelection)
+        );
+    }
+
+    #[test]
+    fn options_at_lists_options_in_stored_order() {
+        let mut d = doc();
+        assert!(d.paragraphs[0].options_at(0).is_empty());
+        d.add_ai_alternates(0, 2..6, vec!["adore".into()], ai())
+            .unwrap();
+        let listed = d.paragraphs[0].options_at(1);
+        assert_eq!(listed[0], ("love".to_string(), Source::Human));
+        assert_eq!(listed[1].0, "adore");
+        assert!(listed[1].1.is_ai());
+    }
+
+    #[test]
+    fn set_active_option_and_version() {
+        let mut d = doc();
+        d.add_option(0, 2..6, "adore".into()).unwrap();
+        assert_eq!(d.set_active_option(0, 1, 0), Some(2..6));
+        assert_eq!(d.paragraphs[0].text(), "I love tea");
+        assert_eq!(d.set_active_option(0, 1, 5), None);
+        assert_eq!(d.set_active_option(0, 0, 0), None);
+        d.add_version(0, "Tea, I love.".into());
+        assert!(d.set_active_version(0, 0));
+        assert_eq!(d.paragraphs[0].text(), "I love tea");
+        assert!(!d.set_active_version(0, 2));
+    }
+
+    #[test]
+    fn remove_option_at_keeps_the_shown_option_and_collapses_at_one() {
+        let mut d = doc();
+        d.add_option(0, 2..6, "adore".into()).unwrap();
+        d.add_option(0, 2..7, "like".into()).unwrap();
+        // Showing "like" (2); removing "love" (0) keeps "like" shown.
+        assert_eq!(d.remove_option_at(0, 1, 0), Some(2..6));
+        assert_eq!(options(&d, 1), (vec!["adore".into(), "like".into()], 1));
+        // Removing the shown option shows the one before it.
+        assert_eq!(d.remove_option_at(0, 1, 1), Some(2..7));
+        assert_eq!(d.paragraphs[0].text(), "I adore tea");
+        assert_eq!(
+            d.paragraphs[0].segments(),
+            &[Segment::plain("I adore tea", false)]
+        );
+        assert_eq!(d.remove_option_at(0, 0, 0), None);
+    }
+
+    #[test]
+    fn versions_are_listed_added_and_removed_but_never_the_last() {
+        let mut d = doc();
+        assert_eq!(d.add_version(0, "Tea, I love.".into()), 1);
+        assert_eq!(d.paragraphs[0].text(), "Tea, I love.");
+        let listed = d.paragraphs[0].version_texts();
+        assert_eq!(listed.len(), 2);
+        assert_eq!(listed[0], ("I love tea".to_string(), Source::Human));
+        assert!(d.remove_version(0, 0));
+        assert_eq!(d.paragraphs[0].active, 0);
+        assert_eq!(d.paragraphs[0].text(), "Tea, I love.");
+        assert!(!d.remove_version(0, 0));
+    }
+
     #[test]
     fn ai_versions_become_active() {
         let mut d = doc();
```
- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile errors for the new methods.

- [ ] **Step 3: Implement**

Add the `impl Paragraph` block before `impl Document`, and the `Document` methods before `make_para_version`:

`crates/tulis-core/src/alternates.rs`:
```diff
--- a/crates/tulis-core/src/alternates.rs
+++ b/crates/tulis-core/src/alternates.rs
@@ -81,6 +81,33 @@ impl Paragraph {
     }
 }
 
+impl Paragraph {
+    /// The options of alternate `seg` in the active version, in stored order (option 0 is the
+    /// original). Empty when `seg` is not an alternate.
+    pub fn options_at(&self, seg: usize) -> Vec<(String, Source)> {
+        match self.segments().get(seg).map(|s| &s.kind) {
+            Some(SegKind::Alt { options, .. }) => options
+                .iter()
+                .map(|o| (o.text.clone(), o.source.clone()))
+                .collect(),
+            _ => Vec::new(),
+        }
+    }
+
+    /// The rendered text (ghosts included) and source of every version, in stored order.
+    pub fn version_texts(&self) -> Vec<(String, Source)> {
+        self.versions
+            .iter()
+            .map(|v| {
+                (
+                    v.segments.iter().map(Segment::text).collect(),
+                    v.source.clone(),
+                )
+            })
+            .collect()
+    }
+}
+
 impl Document {
     /// Find the alternate exactly covering `range`, or turn `range` into a one-option alternate.
     /// Returns the segment index.
@@ -143,6 +170,107 @@ impl Document {
         Ok(p.segment_ranges()[seg].clone())
     }
 
+    /// Append a human-written option to the group at `range` (creating the group if needed) and
+    /// show it. Returns the group's span.
+    pub fn add_option(
+        &mut self,
+        para: usize,
+        range: Range<usize>,
+        text: String,
+    ) -> Result<Range<usize>, OpError> {
+        if text.is_empty() {
+            return Err(OpError::NothingHere);
+        }
+        let seg = self.ensure_alt(para, range)?;
+        let p = &mut self.paragraphs[para];
+        if let SegKind::Alt { options, active } = &mut p.segments_mut()[seg].kind {
+            options.push(AltOption::human(text));
+            *active = options.len() - 1;
+        }
+        Ok(p.segment_ranges()[seg].clone())
+    }
+
+    /// Show option `index` of alternate `seg`. Returns the group's span afterwards.
+    pub fn set_active_option(
+        &mut self,
+        para: usize,
+        seg: usize,
+        index: usize,
+    ) -> Option<Range<usize>> {
+        let p = self.paragraphs.get_mut(para)?;
+        match &mut p.segments_mut().get_mut(seg)?.kind {
+            SegKind::Alt { options, active } if index < options.len() => *active = index,
+            _ => return None,
+        }
+        Some(p.segment_ranges()[seg].clone())
+    }
+
+    /// Show version `index` of paragraph `para`. Returns false when there is no such version.
+    pub fn set_active_version(&mut self, para: usize, index: usize) -> bool {
+        match self.paragraphs.get_mut(para) {
+            Some(p) if index < p.versions.len() => {
+                p.active = index;
+                true
+            }
+            _ => false,
+        }
+    }
+
+    /// Remove option `index` of alternate `seg`, never the last one. A group left with one
+    /// option becomes plain text. Returns the span of the group (or its text) afterwards.
+    pub fn remove_option_at(
+        &mut self,
+        para: usize,
+        seg: usize,
+        index: usize,
+    ) -> Option<Range<usize>> {
+        let p = self.paragraphs.get_mut(para)?;
+        let start = p.segment_ranges().get(seg)?.start;
+        let segment = p.segments_mut().get_mut(seg)?;
+        let remaining = match &mut segment.kind {
+            SegKind::Alt { options, active } if options.len() > 1 && index < options.len() => {
+                options.remove(index);
+                if index < *active || *active >= options.len() {
+                    *active -= 1;
+                }
+                (options.len() == 1).then(|| options[0].text.clone())
+            }
+            _ => return None,
+        };
+        if let Some(text) = remaining {
+            segment.kind = SegKind::Plain(text);
+        }
+        let len = segment.text().len();
+        normalize_segments(p.segments_mut());
+        Some(start..start + len)
+    }
+
+    /// Remove version `index` of paragraph `para`, never the last one.
+    pub fn remove_version(&mut self, para: usize, index: usize) -> bool {
+        let Some(p) = self.paragraphs.get_mut(para) else {
+            return false;
+        };
+        if p.versions.len() < 2 || index >= p.versions.len() {
+            return false;
+        }
+        p.versions.remove(index);
+        if index < p.active || p.active >= p.versions.len() {
+            p.active -= 1;
+        }
+        true
+    }
+
+    /// Append a human-written version with `text` and show it. Returns its index.
+    pub fn add_version(&mut self, para: usize, text: String) -> usize {
+        let p = &mut self.paragraphs[para];
+        p.versions.push(ParaVersion {
+            segments: vec![Segment::plain(text, false)],
+            source: Source::Human,
+        });
+        p.active = p.versions.len() - 1;
+        p.active
+    }
+
     /// Clone the active version as a new, active version. Returns its index.
     pub fn make_para_version(&mut self, para: usize) -> usize {
         let p = &mut self.paragraphs[para];
```
- [ ] **Step 4: Run the alternates tests (expect pass)**

Run: `cargo test -p tulis-core alternates::tests`
Expected: 20 passed.

- [ ] **Step 5: Run everything (expect pass)**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: all tests pass (core gains 5); the clippy counts match the baseline.
- [ ] **Step 6: Commit**

```bash
cargo fmt --all
jj describe -m "feat(core): option and version operations for the sidebar

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---
### Task 3: Vim `ga` / `gA` / `gs`, and the new keys in the command table

The sidebar doesn't exist yet. Ctrl+Shift+A and Ctrl+D are bound to the new `OpenAlternates` action, which nothing handles until Task 4 (part 1 did the same with its placeholder actions), and vim `ga` dispatches it. `gA` and `gs` already work, through `ai_alternates` and `send_to_overflow`. From Visual mode, all four selection commands (`gh`, `ga`, `gA`, `gs`) now see the selection with its inclusive end, as `d` and `y` do.

**Files:**
- Modify: `crates/tulis-core/src/vim.rs`, `crates/tulis/src/actions.rs`, `crates/tulis/src/commands.rs`, `crates/tulis/src/keymap.rs`, `crates/tulis/src/editor/vim_exec.rs`, `crates/tulis/src/workspace.rs`, `crates/tulis/src/editor/tests.rs`

**Interfaces:**
- Consumes: `Editor::ai_alternates`, `Editor::send_to_overflow` (existing handlers).
- Produces:
  - `VimCommand::{Alternatives, AiAlternatives, Stash}` from `ga`, `gA`, `gs`. In Visual mode each is followed by `EndVisual`, as `gh` is.
  - Action `OpenAlternates`.
  - Command-table changes:
    - The `make_alternate` row becomes `alternatives`: "Alternatives sidebar", keys `secondary-shift-a` and `secondary-d`, vim `ga`. `MakeAlternate` stays an editor action with no key.
    - `ai_alternates` becomes "AI alternatives", keys `secondary-shift-g` and `secondary-j`, vim `gA`.
    - `toggle_ghost` gets keys `secondary-/` and `secondary-g`.
    - `send_to_overflow` becomes `stash`: "Stash in Overflow", keys `secondary-shift-x` and `secondary-shift-o`, vim `gs`.

- [ ] **Step 1: Write the failing tests**

`crates/tulis-core/src/vim.rs`:
```diff
--- a/crates/tulis-core/src/vim.rs
+++ b/crates/tulis-core/src/vim.rs
@@ -640,6 +659,21 @@ mod tests {
         assert_eq!(v.mode(), Mode::Normal);
     }
 
+    #[test]
+    fn g_selection_commands_end_visual_mode() {
+        let mut v = Vim::new();
+        assert_eq!(feed(&mut v, "ga"), vec![Alternatives]);
+        assert_eq!(feed(&mut v, "gA"), vec![AiAlternatives]);
+        assert_eq!(feed(&mut v, "gs"), vec![Stash]);
+        assert_eq!(v.mode(), Mode::Normal);
+        for (keys, command) in [("ga", Alternatives), ("gA", AiAlternatives), ("gs", Stash)] {
+            feed(&mut v, "v");
+            assert_eq!(feed(&mut v, keys), vec![command, EndVisual]);
+            assert_eq!(v.mode(), Mode::Normal);
+        }
+        assert_eq!(feed(&mut v, "dga"), vec![]);
+    }
+
     #[test]
     fn g_view_commands_need_no_pending_operator_and_keep_visual_mode() {
         let mut v = Vim::new();
```
`crates/tulis/src/commands.rs`:
```diff
--- a/crates/tulis/src/commands.rs
+++ b/crates/tulis/src/commands.rs
@@ -600,6 +600,9 @@ mod tests {
             VimCommand::CheatSheet => "cheat_sheet",
             VimCommand::Lab => "open_lab",
             VimCommand::ToggleOverflow => "toggle_overflow",
+            VimCommand::Alternatives => "alternatives",
+            VimCommand::AiAlternatives => "ai_alternates",
+            VimCommand::Stash => "stash",
             // Pure motions and editing grammar: no command-table row.
             VimCommand::Move { .. }
             | VimCommand::Operate { .. }
@@ -646,6 +649,9 @@ mod tests {
             VimCommand::CheatSheet,
             VimCommand::Lab,
             VimCommand::ToggleOverflow,
+            VimCommand::Alternatives,
+            VimCommand::AiAlternatives,
+            VimCommand::Stash,
         ];
         for command in &commands {
             let id = vim_id(command).expect("user-facing command has an id");
@@ -680,6 +686,8 @@ mod tests {
         assert_eq!(key_label("secondary-\\"), format!("{ctrl}+\\"));
         assert_eq!(key_label("escape"), "Esc");
         assert_eq!(key_label("ctrl--"), "Ctrl+-");
+        assert_eq!(key_label("secondary-/"), format!("{ctrl}+/"));
+        assert_eq!(key_label("secondary-shift-a"), format!("{ctrl}+Shift+A"));
     }
 
     #[test]
@@ -693,6 +701,7 @@ mod tests {
         assert_eq!(theme[0].1[0].vim, "gt");
         assert_eq!(cheat_sheet("gx")[0].1[0].label, "Show / hide Overflow");
         assert_eq!(cheat_sheet("linkedin")[0].1[0].key, "—");
+        assert_eq!(cheat_sheet("ai alt")[0].1[0].label, "AI alternatives");
         assert!(cheat_sheet("zzzz").is_empty());
     }
 }
```
`crates/tulis/src/keymap.rs`:
```diff
--- a/crates/tulis/src/keymap.rs
+++ b/crates/tulis/src/keymap.rs
@@ -11,7 +11,7 @@ pub fn bind(cx: &mut App) {
 mod tests {
     use gpui::TestAppContext;
 
-    use crate::actions::{CycleTheme, ToggleMarkers};
+    use crate::actions::{CycleTheme, OpenAlternates, SendToOverflow, ToggleGhost, ToggleMarkers};
     use crate::commands::COMMANDS;
 
     #[gpui::test]
@@ -24,6 +24,10 @@ mod tests {
             assert_eq!(keymap.bindings().len(), expected);
             assert_eq!(keymap.bindings_for_action(&CycleTheme).count(), 1);
             assert_eq!(keymap.bindings_for_action(&ToggleMarkers).count(), 1);
+            // New keys plus the old ones they replace.
+            assert_eq!(keymap.bindings_for_action(&OpenAlternates).count(), 2);
+            assert_eq!(keymap.bindings_for_action(&ToggleGhost).count(), 2);
+            assert_eq!(keymap.bindings_for_action(&SendToOverflow).count(), 2);
         });
     }
 }
```
`crates/tulis/src/workspace.rs`:
```diff
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -802,6 +802,41 @@ mod tests {
         assert!(ws.read_with(cx, |ws, _| ws.show_overflow));
     }
 
+    #[gpui::test]
+    fn new_ghost_and_stash_keys_work_on_the_page(cx: &mut TestAppContext) {
+        init(cx);
+        let (ws, cx) = cx.add_window_view(|window, cx| {
+            Workspace::new(Config::default(), None, None, window, cx)
+        });
+        ws.update_in(cx, |ws, window, cx| {
+            ws.editor.update(cx, |e, cx| {
+                e.replace_text_in_range(None, "One two", window, cx);
+                e.selection = Selection {
+                    anchor: DocPos::new(0, 0),
+                    head: DocPos::new(0, 3),
+                };
+            });
+            ws.focus_editor(window, cx);
+        });
+        cx.run_until_parked();
+        cx.simulate_keystrokes("secondary-/");
+        let export = ws.read_with(cx, |ws, cx| {
+            ws.editor.read(cx).doc.paragraphs[0].export_text()
+        });
+        assert_eq!(export, " two");
+        ws.update(cx, |ws, cx| {
+            ws.editor.update(cx, |e, _| {
+                e.selection = Selection {
+                    anchor: DocPos::new(0, 4),
+                    head: DocPos::new(0, 7),
+                }
+            })
+        });
+        cx.simulate_keystrokes("secondary-shift-x");
+        let stashed = ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.overflow.len());
+        assert_eq!(stashed, 1);
+    }
+
     #[gpui::test]
     fn toasts_stack_two_and_expire(cx: &mut TestAppContext) {
         init(cx);
```
`crates/tulis/src/editor/tests.rs`:
```diff
--- a/crates/tulis/src/editor/tests.rs
+++ b/crates/tulis/src/editor/tests.rs
@@ -731,3 +731,30 @@ fn hidden_markers_record_no_handles(cx: &mut TestAppContext) {
     let handles = editor.read_with(cx, |e, _| e.layout.as_ref().unwrap().version_handles.len());
     assert_eq!(handles, 0);
 }
+
+#[gpui::test]
+fn vim_visual_gs_stashes_the_selection_and_returns_to_normal(cx: &mut TestAppContext) {
+    let (editor, cx) = vim_editor(cx, &["Keep this. Stash that."]);
+    editor.update(cx, |e, _| {
+        e.selection = Selection::caret(DocPos::new(0, 11))
+    });
+    type_str(&editor, cx, "v$gs");
+    assert_eq!(texts(&editor, cx), vec!["Keep this. "]);
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.doc.overflow[0].text.clone()),
+        "Stash that."
+    );
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.vim_mode()),
+        Some(Mode::Normal)
+    );
+}
+
+#[gpui::test]
+fn vim_g_shift_a_asks_ai_for_the_paragraph(cx: &mut TestAppContext) {
+    let (editor, cx) = vim_editor(cx, &["Tea is nice."]);
+    with_mock(&editor, cx, vec![r#"["Tea is lovely."]"#]);
+    type_str(&editor, cx, "gA");
+    cx.run_until_parked();
+    assert_eq!(texts(&editor, cx), vec!["Tea is lovely."]);
+}
```
- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile errors: `VimCommand::Alternatives` (and the others) and `OpenAlternates` don't exist.

- [ ] **Step 3: Implement**

`crates/tulis-core/src/vim.rs`:
```diff
--- a/crates/tulis-core/src/vim.rs
+++ b/crates/tulis-core/src/vim.rs
@@ -107,6 +107,12 @@ pub enum VimCommand {
     Lab,
     /// `gx`: show or hide the Overflow panel.
     ToggleOverflow,
+    /// `ga`: the alternates sidebar for the selection or the word under the cursor.
+    Alternatives,
+    /// `gA`: AI alternatives for the selection or the cursor's paragraph.
+    AiAlternatives,
+    /// `gs`: stash the selection or the cursor's paragraph in Overflow.
+    Stash,
 }
 
 /// The view command for `g` + `c`, if there is one.
@@ -122,6 +128,18 @@ fn g_view_command(c: char) -> Option<VimCommand> {
     })
 }
 
+/// The selection command for `g` + `c`, if there is one. In Visual mode these act on the
+/// selection and return to Normal.
+fn g_selection_command(c: char) -> Option<VimCommand> {
+    Some(match c {
+        'h' => VimCommand::ToggleGhost,
+        'a' => VimCommand::Alternatives,
+        'A' => VimCommand::AiAlternatives,
+        's' => VimCommand::Stash,
+        _ => return None,
+    })
+}
+
 #[derive(Debug, Default)]
 pub struct Vim {
     mode: Mode,
@@ -319,13 +337,14 @@ impl Vim {
     fn feed_prefixed(&mut self, prefix: char, c: char) -> Vec<VimCommand> {
         match (prefix, c) {
             ('g', 'g') => self.motion(Motion::DocStart),
-            ('g', 'h') if self.op.is_none() => {
+            ('g', c) if self.op.is_none() && g_selection_command(c).is_some() => {
                 self.reset();
+                let command = g_selection_command(c).expect("checked above");
                 if self.is_visual() {
                     self.mode = Mode::Normal;
-                    vec![VimCommand::ToggleGhost, VimCommand::EndVisual]
+                    vec![command, VimCommand::EndVisual]
                 } else {
-                    vec![VimCommand::ToggleGhost]
+                    vec![command]
                 }
             }
             ('g', c) if self.op.is_none() && g_view_command(c).is_some() => {
```
`crates/tulis/src/actions.rs`:
```diff
--- a/crates/tulis/src/actions.rs
+++ b/crates/tulis/src/actions.rs
@@ -33,6 +33,7 @@ gpui::actions!(
         Undo,
         Redo,
         MakeAlternate,
+        OpenAlternates,
         CycleNext,
         CyclePrev,
         AcceptAlternate,
```
`crates/tulis/src/commands.rs`:
```diff
--- a/crates/tulis/src/commands.rs
+++ b/crates/tulis/src/commands.rs
@@ -279,13 +279,13 @@ pub static COMMANDS: &[Command] = &[
     ),
     // Alternates
     cmd!(
-        "make_alternate",
-        "Make an alternate",
+        "alternatives",
+        "Alternatives sidebar",
         Alternates,
-        ["secondary-d"],
+        ["secondary-shift-a", "secondary-d"],
         ED,
-        None,
-        MakeAlternate
+        Some("ga"),
+        OpenAlternates
     ),
     cmd!(
         "cycle_next",
@@ -325,11 +325,11 @@ pub static COMMANDS: &[Command] = &[
     ),
     cmd!(
         "ai_alternates",
-        "AI alternates",
+        "AI alternatives",
         Alternates,
-        ["secondary-j"],
+        ["secondary-shift-g", "secondary-j"],
         ED,
-        None,
+        Some("gA"),
         AiAlternates
     ),
     // Ghost
@@ -337,19 +337,19 @@ pub static COMMANDS: &[Command] = &[
         "toggle_ghost",
         "Ghost / revive",
         Ghost,
-        ["secondary-g"],
+        ["secondary-/", "secondary-g"],
         ED,
         Some("gh"),
         ToggleGhost
     ),
     // Overflow
     cmd!(
-        "send_to_overflow",
-        "Send to Overflow",
+        "stash",
+        "Stash in Overflow",
         Overflow,
-        ["secondary-shift-o"],
+        ["secondary-shift-x", "secondary-shift-o"],
         ED,
-        None,
+        Some("gs"),
         SendToOverflow
     ),
     cmd!(
```
`crates/tulis/src/editor/vim_exec.rs`:
```diff
--- a/crates/tulis/src/editor/vim_exec.rs
+++ b/crates/tulis/src/editor/vim_exec.rs
@@ -9,13 +9,31 @@ use tulis_core::vim::{InsertAt, Mode, Motion, Operator, Vim, VimCommand, VimKey}
 use tulis_core::{DocPos, Selection};
 
 use super::Editor;
-use crate::actions::{ShowCheatSheet, ToggleOverflowPanel, TogglePreview, ToggleVim};
+use crate::actions::{
+    AiAlternates, OpenAlternates, SendToOverflow, ShowCheatSheet, ToggleOverflowPanel,
+    TogglePreview, ToggleVim,
+};
+
+/// Commands that act on the selection; from Visual mode they get its inclusive end.
+fn is_selection_command(command: VimCommand) -> bool {
+    matches!(
+        command,
+        VimCommand::ToggleGhost
+            | VimCommand::Alternatives
+            | VimCommand::AiAlternatives
+            | VimCommand::Stash
+    )
+}
 
 impl Editor {
     pub(crate) fn vim_key(&mut self, key: VimKey, window: &mut Window, cx: &mut Context<Self>) {
         let Some(vim) = self.vim.as_mut() else { return };
+        let was_visual = vim.is_visual();
         let commands = vim.feed(key);
         for command in commands {
+            if was_visual && is_selection_command(command) {
+                self.include_visual_end();
+            }
             self.exec_vim(command, window, cx);
         }
         self.clamp_for_normal_mode();
@@ -50,6 +68,15 @@ impl Editor {
         }
     }
 
+    /// Visual mode includes the character under the cursor: make the selection cover it.
+    fn include_visual_end(&mut self) {
+        let range = self.selection.range();
+        self.selection = Selection {
+            anchor: range.start,
+            head: self.next_in_para(range.end),
+        };
+    }
+
     /// The next grapheme, but never across a paragraph boundary.
     fn next_in_para(&self, pos: DocPos) -> DocPos {
         let next = mv::next_grapheme(&self.doc, pos);
@@ -221,6 +248,10 @@ impl Editor {
             VimCommand::CycleTheme => self.cycle_theme(cx),
             VimCommand::ToggleMarkers => self.toggle_markers(cx),
             VimCommand::Lab => self.open_lab(window, cx),
+            VimCommand::AiAlternatives => self.ai_alternates(&AiAlternates, window, cx),
+            VimCommand::Stash => self.send_to_overflow(&SendToOverflow, window, cx),
+            // The sidebar arrives in Task 4, which calls the editor directly instead.
+            VimCommand::Alternatives => window.dispatch_action(Box::new(OpenAlternates), cx),
             // Window-level commands: the workspace handles these actions.
             VimCommand::TogglePreview => window.dispatch_action(Box::new(TogglePreview), cx),
             VimCommand::CheatSheet => window.dispatch_action(Box::new(ShowCheatSheet), cx),
```
`crates/tulis/src/workspace.rs`:
```diff
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -216,7 +216,7 @@ impl Workspace {
                             div()
                                 .text_xs()
                                 .text_color(t.text_muted)
-                                .child("Ctrl+Shift+O moves the selection or paragraph here."),
+                                .child("Ctrl+Shift+X stashes the selection or paragraph here."),
                         )
                     })
                     .children(items.into_iter().rev().map(|item| {
```
- [ ] **Step 4: Run the targeted tests (expect pass)**

Run: `cargo test --workspace vim` and `cargo test -p tulis keys`
Expected: pass.

- [ ] **Step 5: Run everything (expect pass)**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: all tests pass (app 97, core 137); the clippy counts match the baseline.
Run: `cargo build && timeout 6 ./target/debug/tulis /tmp/smoke.md; echo $?`
Expected: `124` and nothing on stderr.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
jj describe -m "feat: vim ga/gA/gs and Ctrl+Shift+A/G/X, Ctrl+/ in the command table

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---
### Task 4: The sidebar model and view: scopes, tabs, rows, Esc

**Files:**
- Create: `crates/tulis/src/editor/alt_session.rs`, `crates/tulis/src/sidebar.rs`
- Modify: `crates/tulis/src/editor/mod.rs`, `crates/tulis/src/editor/vim_exec.rs`, `crates/tulis/src/actions.rs`, `crates/tulis/src/commands.rs`, `crates/tulis/src/main.rs`, `crates/tulis/src/workspace.rs`, `crates/tulis/src/editor/tests.rs`

**Interfaces:**
- Consumes: `sentence_range_at` (Task 1), `options_at`/`version_texts` (Task 2), `OpenAlternates` (Task 3).
- Produces:
  - `editor::Scope::{Word, Sentence, Paragraph}`, `Scope::ALL`, `Scope::index(self) -> usize`
  - `editor::AltSession { para_id, scope, origin: Range<usize>, selection: bool, range: Range<usize>, text: String, highlight: usize, previewed: bool }` and `Editor.alt_session: Option<AltSession>`
  - `alt_session::AltRow { text: String, shown: bool, ai: bool }`
  - `Editor::scope_range(&self, para, scope, origin: &Range<usize>, selection: bool) -> Range<usize>`
  - `Editor::open_alternates(&mut self, scope: Option<Scope>, cx)`, `Editor::set_alt_scope(&mut self, scope, cx)`, `Editor::alt_rows(&self) -> Vec<AltRow>`, `Editor::close_alternates(&mut self, cx)`, `Editor::alt_session_target(&self) -> Option<(usize, Option<usize>)>`, `Editor::highlight_shown_row(&mut self)`, `Editor::sync_alt_session(&mut self, cx)` (called from `end_edit` and `after_history`)
  - `EditorEvent::{AlternatesOpened, AlternatesClosed}`
  - Action `CloseAlternates` (Esc in the `AltSidebar` key context, a Hidden table row)
  - `sidebar::AltSidebar { list_focus: FocusHandle, .. }`, `AltSidebar::new(Entity<Editor>, &mut Window, &mut Context<Self>)`, `sidebar::SIDEBAR_WIDTH = 240.`
  - `Workspace.alt_sidebar: Option<Entity<AltSidebar>>`, `Workspace::{alternates_opened, alternates_closed}(&mut self, &mut Window, &mut Context<Self>)`

How the target works: `AltSession.range` is the target's current byte range (the group's span once there is one), and `text` is its text when last seen. A target counts as an existing group when an alternate's span equals `range` exactly. `sync_alt_session` closes the sidebar when the paragraph is gone, or (below Paragraph scope) when `text` is no longer at `range`. That covers deletion, undo, and typing inside the target.

- [ ] **Step 1: Write the failing tests**

`crates/tulis/src/editor/tests.rs`:
```diff
--- a/crates/tulis/src/editor/tests.rs
+++ b/crates/tulis/src/editor/tests.rs
@@ -758,3 +758,132 @@ fn vim_g_shift_a_asks_ai_for_the_paragraph(cx: &mut TestAppContext) {
     cx.run_until_parked();
     assert_eq!(texts(&editor, cx), vec!["Tea is lovely."]);
 }
+
+use super::Scope;
+use super::alt_session::AltRow;
+
+fn rows(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> Vec<(String, bool, bool)> {
+    editor.read_with(cx, |e, _| {
+        e.alt_rows()
+            .into_iter()
+            .map(|AltRow { text, shown, ai }| (text, shown, ai))
+            .collect()
+    })
+}
+
+fn row(text: &str, shown: bool) -> (String, bool, bool) {
+    (text.to_string(), shown, false)
+}
+
+#[gpui::test]
+fn opening_the_sidebar_lists_the_word_and_creates_nothing(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["I love tea"]);
+    select(&editor, cx, DocPos::new(0, 6), DocPos::new(0, 6)); // just after "love"
+    editor.update(cx, |e, cx| e.open_alternates(None, cx));
+    let session = editor.read_with(cx, |e, _| e.alt_session.clone()).unwrap();
+    assert_eq!(
+        (session.scope, session.selection, session.range),
+        (Scope::Word, false, 2..6)
+    );
+    assert_eq!(rows(&editor, cx), vec![row("love", true)]);
+    editor.update(cx, |e, cx| e.close_alternates(cx));
+    editor.read_with(cx, |e, _| {
+        assert!(e.alt_session.is_none());
+        assert!(!e.doc.paragraphs[0].has_metadata());
+        assert!(!e.dirty);
+    });
+}
+
+#[gpui::test]
+fn word_tab_lists_a_groups_options_in_stored_order(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["I love tea"]);
+    editor.update(cx, |e, _| {
+        e.doc.add_option(0, 2..6, "adore".into()).unwrap();
+        e.selection = Selection::caret(DocPos::new(0, 3));
+    });
+    editor.update(cx, |e, cx| e.open_alternates(None, cx));
+    assert_eq!(
+        rows(&editor, cx),
+        vec![row("love", false), row("adore", true)]
+    );
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.alt_session.as_ref().unwrap().highlight),
+        1
+    );
+}
+
+#[gpui::test]
+fn selection_sentence_and_paragraph_tabs_retarget(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["One two. Three four."]);
+    select(&editor, cx, DocPos::new(0, 9), DocPos::new(0, 14));
+    editor.update(cx, |e, cx| e.open_alternates(None, cx));
+    let session = editor.read_with(cx, |e, _| e.alt_session.clone()).unwrap();
+    assert!(session.selection);
+    assert_eq!(rows(&editor, cx), vec![row("Three", true)]);
+    editor.update(cx, |e, cx| e.set_alt_scope(Scope::Sentence, cx));
+    assert_eq!(rows(&editor, cx), vec![row("Three four.", true)]);
+    editor.update(cx, |e, cx| {
+        e.doc.add_version(0, "Two versions.".into());
+        e.doc.set_active_version(0, 0);
+        e.set_alt_scope(Scope::Paragraph, cx);
+    });
+    assert_eq!(
+        rows(&editor, cx),
+        vec![
+            row("One two. Three four.", true),
+            row("Two versions.", false)
+        ]
+    );
+}
+
+#[gpui::test]
+fn a_multi_paragraph_selection_does_not_open_the_sidebar(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["a", "b"]);
+    select(&editor, cx, DocPos::new(0, 0), DocPos::new(1, 1));
+    editor.update(cx, |e, cx| e.open_alternates(None, cx));
+    assert!(editor.read_with(cx, |e, _| e.alt_session.is_none()));
+    assert_eq!(
+        editor
+            .read_with(cx, |e, _| e.last_notice().map(str::to_string))
+            .as_deref(),
+        Some("selection must be within one paragraph")
+    );
+}
+
+#[gpui::test]
+fn the_sidebar_closes_when_its_target_disappears(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["I love tea", "Next"]);
+    select(&editor, cx, DocPos::new(0, 3), DocPos::new(0, 3));
+    editor.update(cx, |e, cx| e.open_alternates(None, cx));
+    // Editing another paragraph leaves it open.
+    select(&editor, cx, DocPos::new(1, 4), DocPos::new(1, 4));
+    type_str(&editor, cx, "!");
+    assert!(editor.read_with(cx, |e, _| e.alt_session.is_some()));
+    // Typing inside the target closes it.
+    select(&editor, cx, DocPos::new(0, 4), DocPos::new(0, 4));
+    type_str(&editor, cx, "x");
+    assert!(editor.read_with(cx, |e, _| e.alt_session.is_none()));
+    // Deleting its paragraph closes the Paragraph tab.
+    select(&editor, cx, DocPos::new(1, 0), DocPos::new(1, 0));
+    editor.update(cx, |e, cx| e.open_alternates(Some(Scope::Paragraph), cx));
+    let end = editor.read_with(cx, |e, _| e.doc.end_pos());
+    select(&editor, cx, DocPos::new(0, 11), end);
+    editor.update_in(cx, |e, window, cx| e.backspace(&Backspace, window, cx));
+    assert_eq!(texts(&editor, cx), vec!["I loxve tea"]);
+    assert!(editor.read_with(cx, |e, _| e.alt_session.is_none()));
+}
+
+#[gpui::test]
+fn vim_visual_ga_opens_the_selection_tab(cx: &mut TestAppContext) {
+    let (editor, cx) = vim_editor(cx, &["I love tea"]);
+    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(0, 2)));
+    type_str(&editor, cx, "vlllga");
+    let session = editor.read_with(cx, |e, _| e.alt_session.clone()).unwrap();
+    assert!(session.selection);
+    assert_eq!(session.range, 2..6); // Visual mode includes the character under the cursor
+    assert_eq!(rows(&editor, cx), vec![row("love", true)]);
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.vim_mode()),
+        Some(Mode::Normal)
+    );
+}
```
`crates/tulis/src/workspace.rs`:
```diff
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -837,6 +848,48 @@ mod tests {
         assert_eq!(stashed, 1);
     }
 
+    /// A workspace in a `Root` window (tabs and inputs need it), with the page focused.
+    fn root_workspace<'a>(
+        cx: &'a mut TestAppContext,
+        texts: &[&str],
+    ) -> (gpui::Entity<Workspace>, &'a mut gpui::VisualTestContext) {
+        use gpui::AppContext as _;
+        init(cx);
+        let doc = tulis_core::Document::from_texts(texts);
+        let (root, cx) = cx.add_window_view(|window, cx| {
+            let ws = cx.new(|cx| Workspace::new(Config::default(), None, None, window, cx));
+            ws.update(cx, |ws, cx| ws.editor.update(cx, |e, cx| e.load(doc, cx)));
+            gpui_component::Root::new(ws, window, cx)
+        });
+        let ws = root.read_with(cx, |root, _| {
+            root.view().clone().downcast::<Workspace>().unwrap()
+        });
+        ws.update_in(cx, |ws, window, cx| ws.focus_editor(window, cx));
+        cx.run_until_parked();
+        (ws, cx)
+    }
+
+    fn sidebar_focused(ws: &gpui::Entity<Workspace>, cx: &mut gpui::VisualTestContext) -> bool {
+        let sidebar = ws.read_with(cx, |ws, _| ws.alt_sidebar.clone());
+        sidebar.is_some_and(|s| cx.update(|window, cx| s.read(cx).list_focus.is_focused(window)))
+    }
+
+    #[gpui::test]
+    fn ctrl_shift_a_opens_the_sidebar_and_escape_closes_it(cx: &mut TestAppContext) {
+        let (ws, cx) = root_workspace(cx, &["I love tea"]);
+        cx.simulate_keystrokes("secondary-shift-a");
+        cx.run_until_parked();
+        assert!(sidebar_focused(&ws, cx));
+        assert!(ws.read_with(cx, |ws, cx| ws.editor.read(cx).alt_session.is_some()));
+        cx.simulate_keystrokes("escape");
+        cx.run_until_parked();
+        assert!(ws.read_with(cx, |ws, cx| ws.editor.read(cx).alt_session.is_none()));
+        let page_focused = ws.update_in(cx, |ws, window, cx| {
+            gpui::Focusable::focus_handle(ws.editor.read(cx), cx).is_focused(window)
+        });
+        assert!(page_focused);
+    }
+
     #[gpui::test]
     fn toasts_stack_two_and_expire(cx: &mut TestAppContext) {
         init(cx);
```
- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile errors: `open_alternates`, `alt_session`, `Scope`, `alt_rows` and `alt_sidebar` don't exist.

- [ ] **Step 3: Implement**

Create `crates/tulis/src/editor/alt_session.rs`:

```rust
//! The alternates sidebar's model: its target (Word/Selection, Sentence or Paragraph), the rows
//! it lists, and the operations behind its keys. Kept on `Editor`, so it is tested without
//! gpui-component.

use std::ops::Range;

use gpui::{Context, Window};
use tulis_core::alternates::OpError;
use tulis_core::motion::word_range_at;
use tulis_core::sentence::sentence_range_at;
use tulis_core::{ParaId, SegKind};

use super::{Editor, EditorEvent};
use crate::actions::OpenAlternates;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// The group or word under the cursor, or exactly the selection ("Selection").
    Word,
    Sentence,
    Paragraph,
}

impl Scope {
    /// Tab order.
    pub const ALL: [Scope; 3] = [Scope::Word, Scope::Sentence, Scope::Paragraph];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&s| s == self).unwrap_or(0)
    }
}

/// An open sidebar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AltSession {
    pub para_id: ParaId,
    pub scope: Scope,
    /// The selection (or caret) the sidebar was opened on, local to the paragraph.
    pub origin: Range<usize>,
    /// The sidebar was opened on a selection: the Word tab reads "Selection" and targets exactly
    /// `origin`.
    pub selection: bool,
    /// The target's byte range (the group's span once there is a group); `0..len` for Paragraph.
    pub range: Range<usize>,
    /// The target's text when last seen. The sidebar closes when it is no longer at `range`.
    pub text: String,
    /// The highlighted row.
    pub highlight: usize,
    /// A preview has recorded its undo step since the last recorded operation.
    pub(crate) previewed: bool,
}

/// One option (or version) listed in the sidebar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AltRow {
    pub text: String,
    /// Shown in the page (`●`).
    pub shown: bool,
    /// AI-written (`✦`).
    pub ai: bool,
}

/// The word at a caret: the run under it, or the word just before it when the caret sits on
/// whitespace right after a word.
fn word_at_caret(text: &str, offset: usize) -> Range<usize> {
    let range = word_range_at(text, offset);
    if !text[range.clone()].trim().is_empty() || offset == 0 {
        return range;
    }
    let before = text[..offset].chars().next_back().map_or(0, char::len_utf8);
    let previous = word_range_at(text, offset - before);
    if text[previous.clone()].trim().is_empty() {
        range
    } else {
        previous
    }
}

impl Editor {
    /// The target range of `scope` in paragraph `para`, for a sidebar opened on `origin`.
    pub(crate) fn scope_range(
        &self,
        para: usize,
        scope: Scope,
        origin: &Range<usize>,
        selection: bool,
    ) -> Range<usize> {
        let p = &self.doc.paragraphs[para];
        let text = p.text();
        let clamp = |offset: usize| text.floor_char_boundary(offset.min(text.len()));
        let origin = clamp(origin.start)..clamp(origin.end);
        match scope {
            Scope::Paragraph => 0..text.len(),
            Scope::Word if selection => origin,
            Scope::Word => match p.alt_at(origin.start) {
                Some(seg) => p.segment_ranges()[seg].clone(),
                None => word_at_caret(&text, origin.start),
            },
            Scope::Sentence => {
                let first = sentence_range_at(&text, origin.start);
                let last = sentence_range_at(&text, origin.end);
                p.expand_to_alt_boundaries(first.start.min(last.start)..first.end.max(last.end))
            }
        }
    }

    /// Open the sidebar on the selection (one paragraph) or the cursor. `None` opens the Word
    /// (or Selection) tab. Creates nothing in the document.
    pub fn open_alternates(&mut self, scope: Option<Scope>, cx: &mut Context<Self>) {
        self.finish_alt_edit();
        let sel = self.selection.range();
        if sel.start.para != sel.end.para {
            return self.set_notice(OpError::MultiParagraph.to_string(), cx);
        }
        let origin = sel.start.offset..sel.end.offset;
        self.alt_session = Some(AltSession {
            para_id: self.doc.paragraphs[sel.start.para].id,
            scope: scope.unwrap_or(Scope::Word),
            selection: !origin.is_empty(),
            origin,
            range: 0..0,
            text: String::new(),
            highlight: 0,
            previewed: false,
        });
        self.retarget_alt_session();
        cx.emit(EditorEvent::AlternatesOpened);
        cx.notify();
    }

    pub(super) fn open_alternates_action(
        &mut self,
        _: &OpenAlternates,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_alternates(None, cx);
    }

    /// Switch the sidebar's tab; the list follows the new target.
    pub fn set_alt_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        if let Some(session) = self.alt_session.as_mut() {
            session.scope = scope;
            self.retarget_alt_session();
            cx.notify();
        }
    }

    /// Recompute the target from the scope and origin, and highlight the shown row.
    fn retarget_alt_session(&mut self) {
        let Some(session) = self.alt_session.as_ref() else {
            return;
        };
        let Some(para) = self.doc.para_index(session.para_id) else {
            self.alt_session = None;
            return;
        };
        let range = self.scope_range(para, session.scope, &session.origin, session.selection);
        let text = self.doc.paragraphs[para].text()[range.clone()].to_string();
        if let Some(session) = self.alt_session.as_mut() {
            session.range = range;
            session.text = text;
        }
        self.highlight_shown_row();
    }

    pub(crate) fn highlight_shown_row(&mut self) {
        let shown = self.alt_rows().iter().position(|r| r.shown).unwrap_or(0);
        if let Some(session) = self.alt_session.as_mut() {
            session.highlight = shown;
        }
    }

    /// The target's paragraph and, when the target is an existing group, its segment.
    pub(crate) fn alt_session_target(&self) -> Option<(usize, Option<usize>)> {
        let session = self.alt_session.as_ref()?;
        let para = self.doc.para_index(session.para_id)?;
        if session.scope == Scope::Paragraph {
            return Some((para, None));
        }
        let p = &self.doc.paragraphs[para];
        let ranges = p.segment_ranges();
        let seg =
            (0..ranges.len()).find(|&i| p.segments()[i].is_alt() && ranges[i] == session.range);
        Some((para, seg))
    }

    /// The rows the sidebar lists: one per option (or version) in stored order, or a single row
    /// with the target's text when there is no group (or only one version).
    pub fn alt_rows(&self) -> Vec<AltRow> {
        let Some(session) = self.alt_session.as_ref() else {
            return Vec::new();
        };
        let Some((para, seg)) = self.alt_session_target() else {
            return Vec::new();
        };
        let p = &self.doc.paragraphs[para];
        let listed = match (session.scope, seg) {
            (Scope::Paragraph, _) if p.versions.len() > 1 => Some((p.version_texts(), p.active)),
            (_, Some(seg)) => match &p.segments()[seg].kind {
                SegKind::Alt { active, .. } => Some((p.options_at(seg), *active)),
                SegKind::Plain(_) => None,
            },
            _ => None,
        };
        match listed {
            Some((items, active)) => items
                .into_iter()
                .enumerate()
                .map(|(i, (text, source))| AltRow {
                    text,
                    shown: i == active,
                    ai: source.is_ai(),
                })
                .collect(),
            None => {
                let text = match session.scope {
                    Scope::Paragraph => p.text(),
                    _ => session.text.clone(),
                };
                vec![AltRow {
                    text,
                    shown: true,
                    ai: false,
                }]
            }
        }
    }

    /// Close the sidebar, keeping whatever is shown.
    pub fn close_alternates(&mut self, cx: &mut Context<Self>) {
        if self.alt_session.take().is_some() {
            cx.emit(EditorEvent::AlternatesClosed);
            cx.notify();
        }
    }

    /// Close the sidebar if its target is gone: the paragraph was deleted, or (below Paragraph
    /// scope) the target's text is no longer at its range.
    pub(crate) fn sync_alt_session(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.alt_session.as_ref() else {
            return;
        };
        let alive = match self.doc.para_index(session.para_id) {
            None => false,
            Some(_) if session.scope == Scope::Paragraph => true,
            Some(para) => {
                self.doc.paragraphs[para].text().get(session.range.clone())
                    == Some(session.text.as_str())
            }
        };
        if !alive {
            self.close_alternates(cx);
        }
    }
}
```
`TabBar::on_click` gets the tab index (`gpui-component src/tab/tab_bar.rs:187`). The tab labels are child divs, so they keep the italic accent style that `Tab::label` would override.

Create `crates/tulis/src/sidebar.rs`:

```rust
//! The alternates sidebar: Word/Selection, Sentence and Paragraph tabs over the list of options
//! the editor's `AltSession` describes.

use gpui::{
    App, Context, Entity, FocusHandle, IntoElement, ParentElement, Render, Styled, Subscription,
    Window, div, prelude::*, px,
};
use gpui_component::Sizable;
use gpui_component::tab::{Tab, TabBar};

use crate::actions::CloseAlternates;
use crate::editor::{Editor, Scope};
use crate::workspace::Workspace;

pub const SIDEBAR_WIDTH: f32 = 240.;

pub struct AltSidebar {
    editor: Entity<Editor>,
    /// Focus of the option list (its keys: arrows, j/k, Enter, Delete, dd, i/o).
    pub(crate) list_focus: FocusHandle,
    _subscriptions: Vec<Subscription>,
}

impl AltSidebar {
    pub fn new(editor: Entity<Editor>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.observe(&editor, |_, _, cx| cx.notify())];
        Self {
            editor,
            list_focus: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    /// Esc: close the sidebar, keeping the shown option (the workspace refocuses the page).
    fn close(&mut self, _: &CloseAlternates, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| e.close_alternates(cx));
    }

    fn tab_label(scope: Scope, selection: bool) -> &'static str {
        match scope {
            Scope::Word if selection => "Selection",
            Scope::Word => "Word",
            Scope::Sentence => "Sentence",
            Scope::Paragraph => "Paragraph",
        }
    }
}

impl Render for AltSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let t = editor.theme;
        let Some(session) = editor.alt_session.clone() else {
            return div().into_any_element();
        };
        let rows = editor.alt_rows();
        let editor_handle = self.editor.clone();
        let tabs = TabBar::new("alt-tabs")
            .underline()
            .small()
            .selected_index(session.scope.index())
            .children(Scope::ALL.map(|scope| {
                Tab::new().child(
                    div()
                        .italic()
                        .text_color(t.accent)
                        .child(Self::tab_label(scope, session.selection)),
                )
            }))
            .on_click(move |ix: &usize, _, cx: &mut App| {
                editor_handle.update(cx, |e, cx| e.set_alt_scope(Scope::ALL[*ix], cx));
            });
        div()
            .id("alt-sidebar")
            .key_context("AltSidebar")
            .on_action(cx.listener(Self::close))
            .w(px(SIDEBAR_WIDTH))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(t.rule)
            .bg(t.panel)
            .text_sm()
            .child(div().px_2().pt_2().child(tabs))
            .child(
                div()
                    .id("alt-list")
                    .key_context("AltList")
                    .track_focus(&self.list_focus)
                    .flex_1()
                    .overflow_y_scroll()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(rows.into_iter().enumerate().map(|(ix, row)| {
                        let highlighted = ix == session.highlight;
                        div()
                            .id(("alt-row", ix))
                            .flex()
                            .flex_row()
                            .gap_2()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .when(highlighted, |d| d.bg(t.selection))
                            .child(
                                div()
                                    .w(px(10.))
                                    .flex_none()
                                    .text_color(if row.shown { t.accent } else { t.text_muted })
                                    .child(if row.shown { "●" } else { "·" }),
                            )
                            .child(div().flex_1().min_w(px(0.)).line_clamp(3).child(row.text))
                            .when(row.ai, |d| {
                                d.child(div().flex_none().text_color(t.ai_accent).child("✦"))
                            })
                    })),
            )
            .into_any_element()
    }
}

impl Workspace {
    /// The editor opened the sidebar: create its view on first use and focus its list.
    pub(crate) fn alternates_opened(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let editor = self.editor.clone();
        let sidebar = self
            .alt_sidebar
            .get_or_insert_with(|| cx.new(|cx| AltSidebar::new(editor, window, cx)))
            .clone();
        let focus = sidebar.read(cx).list_focus.clone();
        window.focus(&focus, cx);
        cx.notify();
    }

    /// The sidebar closed (Esc, or its target disappeared): the page gets focus back.
    pub(crate) fn alternates_closed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.preview {
            self.focus_editor(window, cx);
        }
        cx.notify();
    }
}
```
Wire it up:

`crates/tulis/src/editor/mod.rs`:
```diff
--- a/crates/tulis/src/editor/mod.rs
+++ b/crates/tulis/src/editor/mod.rs
@@ -2,6 +2,7 @@
 
 mod actions;
 pub(crate) mod ai;
+mod alt_session;
 mod element;
 mod features;
 mod input;
@@ -25,6 +26,7 @@ use tulis_core::history::{EditKind, History};
 use tulis_core::vim::{Mode, Vim};
 use tulis_core::{DocPos, Document, ParaId, Selection};
 
+pub use alt_session::{AltSession, Scope};
 use element::EditorElement;
 pub(crate) use element::GUTTER;
 use layout::LayoutCache;
@@ -39,6 +41,10 @@ pub type Snapshot = (Document, Selection);
 pub enum EditorEvent {
     /// A character was typed into the page (drives the header's auto-dim).
     Typed,
+    /// The alternates sidebar opened (or retargeted); the workspace focuses it.
+    AlternatesOpened,
+    /// The alternates sidebar closed; the workspace returns focus to the page.
+    AlternatesClosed,
 }
 
 impl gpui::EventEmitter<EditorEvent> for Editor {}
@@ -73,6 +79,8 @@ pub struct Editor {
     pub vim: Option<Vim>,
     pub alt_edit: Option<AltEdit>,
     pub cycled: Option<CycledGroup>,
+    /// The open alternates sidebar, if any.
+    pub alt_session: Option<AltSession>,
     pub ai_task: Option<Task<()>>,
     pub scroll: ScrollHandle,
     pub theme: Theme,
@@ -117,6 +125,7 @@ impl Editor {
             vim,
             alt_edit: None,
             cycled: None,
+            alt_session: None,
             ai_task: None,
             scroll: ScrollHandle::new(),
             theme: theme_id.theme(),
@@ -149,6 +158,7 @@ impl Editor {
         self.cycled = None;
         self.ai_task = None;
         self.marked = None;
+        self.close_alternates(cx);
         self.goal_x = None;
         self.dirty = false;
         self.autoscroll = true;
@@ -290,6 +300,7 @@ impl Editor {
         self.dirty = true;
         self.autoscroll = true;
         self.goal_x = None;
+        self.sync_alt_session(cx);
         cx.notify();
     }
 
@@ -518,6 +529,7 @@ impl Editor {
         self.goal_x = None;
         self.dirty = true;
         self.autoscroll = true;
+        self.sync_alt_session(cx);
         cx.notify();
     }
 }
@@ -559,6 +571,7 @@ impl Render for Editor {
             .on_action(cx.listener(Self::undo))
             .on_action(cx.listener(Self::redo))
             .on_action(cx.listener(Self::make_alternate))
+            .on_action(cx.listener(Self::open_alternates_action))
             .on_action(cx.listener(Self::cycle_next))
             .on_action(cx.listener(Self::cycle_prev))
             .on_action(cx.listener(Self::accept_alternate))
```
`crates/tulis/src/editor/vim_exec.rs`:
```diff
--- a/crates/tulis/src/editor/vim_exec.rs
+++ b/crates/tulis/src/editor/vim_exec.rs
@@ -10,8 +10,7 @@ use tulis_core::{DocPos, Selection};
 
 use super::Editor;
 use crate::actions::{
-    AiAlternates, OpenAlternates, SendToOverflow, ShowCheatSheet, ToggleOverflowPanel,
-    TogglePreview, ToggleVim,
+    AiAlternates, SendToOverflow, ShowCheatSheet, ToggleOverflowPanel, TogglePreview, ToggleVim,
 };
 
 /// Commands that act on the selection; from Visual mode they get its inclusive end.
@@ -250,8 +249,8 @@ impl Editor {
             VimCommand::Lab => self.open_lab(window, cx),
             VimCommand::AiAlternatives => self.ai_alternates(&AiAlternates, window, cx),
             VimCommand::Stash => self.send_to_overflow(&SendToOverflow, window, cx),
-            // The sidebar arrives in Task 4, which calls the editor directly instead.
-            VimCommand::Alternatives => window.dispatch_action(Box::new(OpenAlternates), cx),
+            // Called directly (not dispatched): in Visual mode the selection must still be there.
+            VimCommand::Alternatives => self.open_alternates(None, cx),
             // Window-level commands: the workspace handles these actions.
             VimCommand::TogglePreview => window.dispatch_action(Box::new(TogglePreview), cx),
             VimCommand::CheatSheet => window.dispatch_action(Box::new(ShowCheatSheet), cx),
```
`crates/tulis/src/actions.rs`:
```diff
--- a/crates/tulis/src/actions.rs
+++ b/crates/tulis/src/actions.rs
@@ -34,6 +34,7 @@ gpui::actions!(
         Redo,
         MakeAlternate,
         OpenAlternates,
+        CloseAlternates,
         CycleNext,
         CyclePrev,
         AcceptAlternate,
```
`crates/tulis/src/commands.rs`:
```diff
--- a/crates/tulis/src/commands.rs
+++ b/crates/tulis/src/commands.rs
@@ -287,6 +287,15 @@ pub static COMMANDS: &[Command] = &[
         Some("ga"),
         OpenAlternates
     ),
+    cmd!(
+        "close_alternates",
+        "Close the sidebar",
+        Hidden,
+        ["escape"],
+        Some("AltSidebar"),
+        None,
+        CloseAlternates
+    ),
     cmd!(
         "cycle_next",
         "Next alternate",
```
`crates/tulis/src/main.rs`:
```diff
--- a/crates/tulis/src/main.rs
+++ b/crates/tulis/src/main.rs
@@ -7,6 +7,7 @@ mod editor;
 mod header_dim;
 mod keymap;
 mod runs;
+mod sidebar;
 mod theme;
 mod toast;
 mod utf16;
```
`crates/tulis/src/workspace.rs`:
```diff
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -27,6 +27,8 @@ pub struct Workspace {
     pub(crate) keep_in_overflow: bool,
     pub(crate) banner: Option<String>,
     pub(crate) cheat_sheet: Option<Entity<crate::cheat_sheet::CheatSheet>>,
+    /// The alternates sidebar's view, created the first time it opens.
+    pub(crate) alt_sidebar: Option<Entity<crate::sidebar::AltSidebar>>,
     pub(crate) preview: bool,
     preview_focus: FocusHandle,
     pub(crate) header_dim: HeaderDim,
@@ -51,12 +53,17 @@ impl Workspace {
         cx: &mut Context<Self>,
     ) -> Self {
         let editor = cx.new(|cx| Editor::new(config, Document::new(), cx));
-        let subscriptions = vec![
-            cx.observe(&editor, |_, _, cx| cx.notify()),
-            cx.subscribe(&editor, |ws, _, event: &EditorEvent, cx| match event {
-                EditorEvent::Typed => ws.typed(cx),
-            }),
-        ];
+        let subscriptions =
+            vec![
+                cx.observe(&editor, |_, _, cx| cx.notify()),
+                cx.subscribe_in(&editor, window, |ws, _, event: &EditorEvent, window, cx| {
+                    match event {
+                        EditorEvent::Typed => ws.typed(cx),
+                        EditorEvent::AlternatesOpened => ws.alternates_opened(window, cx),
+                        EditorEvent::AlternatesClosed => ws.alternates_closed(window, cx),
+                    }
+                }),
+            ];
         let initial_theme = editor.read(cx).theme;
         theme::apply_to_components(&initial_theme, cx);
         let mut workspace = Self {
@@ -66,6 +73,7 @@ impl Workspace {
             keep_in_overflow: false,
             banner: None,
             cheat_sheet: None,
+            alt_sidebar: None,
             preview: false,
             preview_focus: cx.focus_handle(),
             header_dim: HeaderDim::default(),
@@ -413,6 +421,9 @@ impl Render for Workspace {
                     .min_h(px(0.))
                     .flex()
                     .flex_row()
+                    .when(self.editor.read(cx).alt_session.is_some(), |row| {
+                        row.children(self.alt_sidebar.clone())
+                    })
                     .child(self.render_main(cx))
                     .when(self.show_overflow, |row| {
                         row.child(self.render_overflow(cx))
```
- [ ] **Step 4: Run the sidebar tests (expect pass)**

Run: `cargo test -p tulis sidebar` and `cargo test -p tulis tab`
Expected: pass.

- [ ] **Step 5: Run everything (expect pass)**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: all tests pass (app 104); the clippy counts match the baseline.
Run: `cargo build && timeout 6 ./target/debug/tulis /tmp/smoke.md; echo $?`
Expected: `124` and nothing on stderr.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
jj describe -m "feat: alternates sidebar with Word/Selection, Sentence and Paragraph tabs

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---
### Task 5: Live preview as one undo step, keep, remove, and the list keys

**Files:**
- Modify: `crates/tulis/src/editor/alt_session.rs`, `crates/tulis/src/sidebar.rs`, `crates/tulis/src/actions.rs`, `crates/tulis/src/commands.rs`, `crates/tulis/src/editor/tests.rs`, `crates/tulis/src/workspace.rs`

**Interfaces:**
- Consumes: Task 2's `set_active_option`, `set_active_version`, `remove_option_at` and `remove_version`, and Task 4's session.
- Produces:
  - `Editor::preview_alternate(&mut self, index: usize, cx)`: highlights the row and shows it. The first preview of a session calls `begin_edit`; later previews mutate without recording. A recorded operation (remove or add) resets `previewed`.
  - `Editor::move_alt_highlight(&mut self, delta: isize, cx)` (clamped, no wrap)
  - `Editor::keep_alternate(&mut self, index: usize, cx)`: previews, then puts the caret at the target's end. The sidebar stays open.
  - `Editor::remove_alternate(&mut self, index: usize, cx)`: never the last row.
  - Actions `AltNext` (`down`, `j`), `AltPrev` (`up`, `k`), `AltKeep` (`enter`) and `AltRemove` (`delete`, `d d`), in the `AltList` key context. These are Hidden table rows. `AltList` is the list's own focus node, so the text field (a sibling, added in Task 6) never sees these letter keys.
  - Clicking a row keeps it and focuses the page.

- [ ] **Step 1: Write the failing tests**

`crates/tulis/src/editor/tests.rs`:
```diff
--- a/crates/tulis/src/editor/tests.rs
+++ b/crates/tulis/src/editor/tests.rs
@@ -887,3 +887,89 @@ fn vim_visual_ga_opens_the_selection_tab(cx: &mut TestAppContext) {
         Some(Mode::Normal)
     );
 }
+
+/// "I love tea" with options love / adore / like on 2..6, showing "love", cursor in the group.
+fn three_options(cx: &mut TestAppContext) -> (Entity<Editor>, &mut VisualTestContext) {
+    let (editor, cx) = editor_with(cx, &["I love tea"]);
+    editor.update(cx, |e, _| {
+        e.doc.add_option(0, 2..6, "adore".into()).unwrap();
+        e.doc.add_option(0, 2..7, "like".into()).unwrap();
+        e.doc.set_active_option(0, 1, 0);
+        e.selection = Selection::caret(DocPos::new(0, 3));
+    });
+    editor.update(cx, |e, cx| e.open_alternates(None, cx));
+    (editor, cx)
+}
+
+#[gpui::test]
+fn previewing_shows_options_live_as_one_undo_step(cx: &mut TestAppContext) {
+    let (editor, cx) = three_options(cx);
+    editor.update(cx, |e, cx| e.move_alt_highlight(1, cx));
+    assert_eq!(texts(&editor, cx), vec!["I adore tea"]);
+    editor.update(cx, |e, cx| e.move_alt_highlight(1, cx));
+    assert_eq!(texts(&editor, cx), vec!["I like tea"]);
+    editor.update(cx, |e, cx| e.move_alt_highlight(1, cx)); // clamps at the last row
+    editor.update(cx, |e, cx| e.move_alt_highlight(-2, cx));
+    assert_eq!(texts(&editor, cx), vec!["I love tea"]);
+    editor.update(cx, |e, cx| e.move_alt_highlight(2, cx));
+    assert_eq!(
+        rows(&editor, cx),
+        vec![row("love", false), row("adore", false), row("like", true)]
+    );
+    // One undo undoes the whole preview session.
+    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
+    assert_eq!(texts(&editor, cx), vec!["I love tea"]);
+    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
+    assert_eq!(texts(&editor, cx), vec!["I love tea"]);
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.doc.alt_info(DocPos::new(0, 3)).unwrap().count),
+        3
+    );
+}
+
+#[gpui::test]
+fn keeping_an_option_shows_it_and_moves_the_cursor_after_it(cx: &mut TestAppContext) {
+    let (editor, cx) = three_options(cx);
+    editor.update(cx, |e, cx| e.keep_alternate(1, cx));
+    assert_eq!(texts(&editor, cx), vec!["I adore tea"]);
+    editor.read_with(cx, |e, _| {
+        assert_eq!(e.cursor(), DocPos::new(0, 7));
+        assert!(e.alt_session.is_some(), "Enter keeps the sidebar open");
+    });
+}
+
+#[gpui::test]
+fn removing_options_never_removes_the_last_one(cx: &mut TestAppContext) {
+    let (editor, cx) = three_options(cx);
+    editor.update(cx, |e, cx| e.remove_alternate(1, cx));
+    assert_eq!(
+        rows(&editor, cx),
+        vec![row("love", true), row("like", false)]
+    );
+    editor.update(cx, |e, cx| e.remove_alternate(0, cx));
+    assert_eq!(texts(&editor, cx), vec!["I like tea"]);
+    assert_eq!(rows(&editor, cx), vec![row("like", true)]);
+    editor.update(cx, |e, cx| e.remove_alternate(0, cx));
+    assert_eq!(texts(&editor, cx), vec!["I like tea"]);
+    assert!(editor.read_with(cx, |e, _| !e.doc.paragraphs[0].has_metadata()));
+    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
+    // Undo changes the target's text, so the sidebar closes; the option is back.
+    assert_eq!(texts(&editor, cx), vec!["I love tea"]);
+}
+
+#[gpui::test]
+fn paragraph_tab_previews_and_removes_versions(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["Draft one."]);
+    editor.update(cx, |e, cx| {
+        e.doc.add_version(0, "Draft two.".into());
+        e.open_alternates(Some(Scope::Paragraph), cx);
+    });
+    assert_eq!(
+        rows(&editor, cx),
+        vec![row("Draft one.", false), row("Draft two.", true)]
+    );
+    editor.update(cx, |e, cx| e.move_alt_highlight(-1, cx));
+    assert_eq!(texts(&editor, cx), vec!["Draft one."]);
+    editor.update(cx, |e, cx| e.remove_alternate(1, cx));
+    assert_eq!(rows(&editor, cx), vec![row("Draft one.", true)]);
+}
```
`crates/tulis/src/workspace.rs`:
```diff
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -890,6 +890,35 @@ mod tests {
         assert!(page_focused);
     }
 
+    #[gpui::test]
+    fn sidebar_keys_preview_keep_and_remove(cx: &mut TestAppContext) {
+        let (ws, cx) = root_workspace(cx, &["I love tea"]);
+        ws.update(cx, |ws, cx| {
+            ws.editor.update(cx, |e, cx| {
+                e.doc.add_option(0, 2..6, "adore".into()).unwrap();
+                e.doc.add_option(0, 2..7, "like".into()).unwrap();
+                e.doc.set_active_option(0, 1, 0);
+                e.selection = Selection::caret(DocPos::new(0, 3));
+                e.open_alternates(None, cx);
+            })
+        });
+        cx.run_until_parked();
+        let text = |cx: &mut gpui::VisualTestContext| {
+            ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.paragraphs[0].text())
+        };
+        cx.simulate_keystrokes("j");
+        assert_eq!(text(cx), "I adore tea");
+        cx.simulate_keystrokes("down k j");
+        assert_eq!(text(cx), "I like tea");
+        cx.simulate_keystrokes("d d");
+        assert_eq!(text(cx), "I adore tea");
+        cx.simulate_keystrokes("up enter");
+        cx.run_until_parked();
+        assert_eq!(text(cx), "I love tea");
+        assert!(!sidebar_focused(&ws, cx));
+        assert!(ws.read_with(cx, |ws, cx| ws.editor.read(cx).alt_session.is_some()));
+    }
+
     #[gpui::test]
     fn toasts_stack_two_and_expire(cx: &mut TestAppContext) {
         init(cx);
```
- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile errors: `move_alt_highlight`, `keep_alternate` and `remove_alternate` don't exist.

- [ ] **Step 3: Implement**

`crates/tulis/src/editor/alt_session.rs`:
```diff
--- a/crates/tulis/src/editor/alt_session.rs
+++ b/crates/tulis/src/editor/alt_session.rs
@@ -6,9 +6,10 @@ use std::ops::Range;
 
 use gpui::{Context, Window};
 use tulis_core::alternates::OpError;
+use tulis_core::history::EditKind;
 use tulis_core::motion::word_range_at;
 use tulis_core::sentence::sentence_range_at;
-use tulis_core::{ParaId, SegKind};
+use tulis_core::{DocPos, ParaId, SegKind, Selection};
 
 use super::{Editor, EditorEvent};
 use crate::actions::OpenAlternates;
@@ -254,3 +255,115 @@ impl Editor {
         }
     }
 }
+
+impl Editor {
+    /// Record the target's new range (and text) after an operation changed it.
+    fn update_alt_target(&mut self, para: usize, range: Range<usize>) {
+        let text = self.doc.paragraphs[para]
+            .text()
+            .get(range.clone())
+            .unwrap_or_default()
+            .to_string();
+        if let Some(session) = self.alt_session.as_mut() {
+            if session.scope == Scope::Word {
+                session.origin = if session.selection {
+                    range.clone()
+                } else {
+                    range.start..range.start
+                };
+            }
+            session.range = range;
+            session.text = text;
+        }
+    }
+
+    /// Highlight row `index` and show it in the page. A session's previews share one undo step
+    /// (recorded by the first; another recorded operation starts a new one).
+    pub fn preview_alternate(&mut self, index: usize, cx: &mut Context<Self>) {
+        let Some((para, seg)) = self.alt_session_target() else {
+            return;
+        };
+        let rows = self.alt_rows();
+        let Some(row) = rows.get(index) else {
+            return;
+        };
+        let Some(session) = self.alt_session.as_mut() else {
+            return;
+        };
+        session.highlight = index;
+        if row.shown {
+            return cx.notify();
+        }
+        let (scope, first_preview) = (session.scope, !session.previewed);
+        session.previewed = true;
+        if first_preview {
+            self.begin_edit(EditKind::Other, para..para + 1);
+        }
+        let range = match (scope, seg) {
+            (Scope::Paragraph, _) => {
+                self.doc.set_active_version(para, index);
+                0..self.doc.paragraphs[para].len()
+            }
+            (_, Some(seg)) => match self.doc.set_active_option(para, seg, index) {
+                Some(span) => span,
+                None => return,
+            },
+            _ => return,
+        };
+        self.update_alt_target(para, range.clone());
+        self.selection = Selection::caret(DocPos::new(para, range.end));
+        self.end_edit(cx);
+    }
+
+    /// Move the highlight by `delta` rows (clamped) and preview that row.
+    pub fn move_alt_highlight(&mut self, delta: isize, cx: &mut Context<Self>) {
+        let Some(session) = self.alt_session.as_ref() else {
+            return;
+        };
+        let last = self.alt_rows().len().saturating_sub(1);
+        let index = session.highlight.saturating_add_signed(delta).min(last);
+        self.preview_alternate(index, cx);
+    }
+
+    /// Keep row `index`: show it and put the cursor at the end of the target.
+    pub fn keep_alternate(&mut self, index: usize, cx: &mut Context<Self>) {
+        self.preview_alternate(index, cx);
+        if let Some((para, _)) = self.alt_session_target() {
+            let end = self.alt_session.as_ref().map_or(0, |s| s.range.end);
+            self.set_selection(Selection::caret(DocPos::new(para, end)), cx);
+        }
+    }
+
+    /// Remove row `index` (never the last one). A group left with one option becomes plain text.
+    pub fn remove_alternate(&mut self, index: usize, cx: &mut Context<Self>) {
+        let Some((para, seg)) = self.alt_session_target() else {
+            return;
+        };
+        if self.alt_rows().len() < 2 {
+            return;
+        }
+        let Some(scope) = self.alt_session.as_ref().map(|s| s.scope) else {
+            return;
+        };
+        self.begin_edit(EditKind::Other, para..para + 1);
+        let range = match (scope, seg) {
+            (Scope::Paragraph, _) if self.doc.remove_version(para, index) => {
+                0..self.doc.paragraphs[para].len()
+            }
+            (Scope::Word | Scope::Sentence, Some(seg)) => {
+                match self.doc.remove_option_at(para, seg, index) {
+                    Some(span) => span,
+                    None => return,
+                }
+            }
+            _ => return,
+        };
+        self.update_alt_target(para, range.clone());
+        if let Some(session) = self.alt_session.as_mut() {
+            session.previewed = false;
+        }
+        self.highlight_shown_row();
+        self.selection = Selection::caret(DocPos::new(para, range.end));
+        self.end_edit(cx);
+    }
+}
```
`crates/tulis/src/sidebar.rs`:
```diff
--- a/crates/tulis/src/sidebar.rs
+++ b/crates/tulis/src/sidebar.rs
@@ -2,13 +2,13 @@
 //! the editor's `AltSession` describes.
 
 use gpui::{
-    App, Context, Entity, FocusHandle, IntoElement, ParentElement, Render, Styled, Subscription,
-    Window, div, prelude::*, px,
+    App, ClickEvent, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement, Render,
+    Styled, Subscription, Window, div, prelude::*, px,
 };
 use gpui_component::Sizable;
 use gpui_component::tab::{Tab, TabBar};
 
-use crate::actions::CloseAlternates;
+use crate::actions::{AltKeep, AltNext, AltPrev, AltRemove, CloseAlternates};
 use crate::editor::{Editor, Scope};
 use crate::workspace::Workspace;
 
@@ -36,6 +36,41 @@ impl AltSidebar {
         self.editor.update(cx, |e, cx| e.close_alternates(cx));
     }
 
+    fn next(&mut self, _: &AltNext, _: &mut Window, cx: &mut Context<Self>) {
+        self.editor.update(cx, |e, cx| e.move_alt_highlight(1, cx));
+    }
+
+    fn prev(&mut self, _: &AltPrev, _: &mut Window, cx: &mut Context<Self>) {
+        self.editor.update(cx, |e, cx| e.move_alt_highlight(-1, cx));
+    }
+
+    /// Enter (or a click): keep the highlighted option and return to the page. The sidebar
+    /// stays open.
+    fn keep(&mut self, _: &AltKeep, window: &mut Window, cx: &mut Context<Self>) {
+        let index = self.highlight(cx);
+        self.keep_row(index, window, cx);
+    }
+
+    fn keep_row(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
+        self.editor.update(cx, |e, cx| e.keep_alternate(index, cx));
+        let page = self.editor.focus_handle(cx);
+        window.focus(&page, cx);
+    }
+
+    fn remove(&mut self, _: &AltRemove, _: &mut Window, cx: &mut Context<Self>) {
+        let index = self.highlight(cx);
+        self.editor
+            .update(cx, |e, cx| e.remove_alternate(index, cx));
+    }
+
+    fn highlight(&self, cx: &App) -> usize {
+        self.editor
+            .read(cx)
+            .alt_session
+            .as_ref()
+            .map_or(0, |s| s.highlight)
+    }
+
     fn tab_label(scope: Scope, selection: bool) -> &'static str {
         match scope {
             Scope::Word if selection => "Selection",
@@ -89,6 +124,10 @@ impl Render for AltSidebar {
                     .id("alt-list")
                     .key_context("AltList")
                     .track_focus(&self.list_focus)
+                    .on_action(cx.listener(Self::next))
+                    .on_action(cx.listener(Self::prev))
+                    .on_action(cx.listener(Self::keep))
+                    .on_action(cx.listener(Self::remove))
                     .flex_1()
                     .overflow_y_scroll()
                     .p_2()
@@ -105,7 +144,11 @@ impl Render for AltSidebar {
                             .px_2()
                             .py_1()
                             .rounded_md()
+                            .cursor_pointer()
                             .when(highlighted, |d| d.bg(t.selection))
+                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
+                                this.keep_row(ix, window, cx)
+                            }))
                             .child(
                                 div()
                                     .w(px(10.))
```
`crates/tulis/src/actions.rs`:
```diff
--- a/crates/tulis/src/actions.rs
+++ b/crates/tulis/src/actions.rs
@@ -35,6 +35,10 @@ gpui::actions!(
         MakeAlternate,
         OpenAlternates,
         CloseAlternates,
+        AltNext,
+        AltPrev,
+        AltKeep,
+        AltRemove,
         CycleNext,
         CyclePrev,
         AcceptAlternate,
```
`crates/tulis/src/commands.rs`:
```diff
--- a/crates/tulis/src/commands.rs
+++ b/crates/tulis/src/commands.rs
@@ -296,6 +296,42 @@ pub static COMMANDS: &[Command] = &[
         None,
         CloseAlternates
     ),
+    cmd!(
+        "alt_next",
+        "Sidebar: next option",
+        Hidden,
+        ["down", "j"],
+        Some("AltList"),
+        None,
+        AltNext
+    ),
+    cmd!(
+        "alt_prev",
+        "Sidebar: previous option",
+        Hidden,
+        ["up", "k"],
+        Some("AltList"),
+        None,
+        AltPrev
+    ),
+    cmd!(
+        "alt_keep",
+        "Sidebar: keep this option",
+        Hidden,
+        ["enter"],
+        Some("AltList"),
+        None,
+        AltKeep
+    ),
+    cmd!(
+        "alt_remove",
+        "Sidebar: remove this option",
+        Hidden,
+        ["delete", "d d"],
+        Some("AltList"),
+        None,
+        AltRemove
+    ),
     cmd!(
         "cycle_next",
         "Next alternate",
```
- [ ] **Step 4: Run the targeted tests (expect pass)**

Run: `cargo test -p tulis preview` and `cargo test -p tulis sidebar_keys`
Expected: pass.

- [ ] **Step 5: Run everything (expect pass)**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: all tests pass (app 109); the clippy counts match the baseline.
Run: `cargo build && timeout 6 ./target/debug/tulis /tmp/smoke.md; echo $?`
Expected: `124` and nothing on stderr.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
jj describe -m "feat: sidebar preview (one undo step), keep and remove

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---
### Task 6: Type-to-add, the merge toast, and "✦ Ask AI"

**Files:**
- Modify: `crates/tulis/src/editor/alt_session.rs`, `crates/tulis/src/editor/ai.rs`, `crates/tulis/src/sidebar.rs`, `crates/tulis/src/actions.rs`, `crates/tulis/src/commands.rs`, `crates/tulis/src/editor/tests.rs`, `crates/tulis/src/workspace.rs`

**Interfaces:**
- Consumes: Task 2's `add_option` and `add_version`. `gpui_component::input::{Input, InputState, InputEvent::PressEnter}` (`gpui-base src/input/base/state.rs:132`). The Input's `Escape` handler ends in `cx.propagate()` (`state.rs:2454`), so Esc in the field reaches the sidebar's `CloseAlternates` binding.
- Produces:
  - `alt_session::MERGED_NOTICE = "Inner alternates merged — Ctrl+Z to restore"`
  - `Editor::merges_inner_groups(&self, para, range) -> bool` (any alternate strictly inside the range, after growing it as `make_alt` does)
  - `Editor::add_alternate_text(&mut self, text: &str, cx)`: trims the text and ignores it when blank. It adds an option (creating the group on the first add) or, on the Paragraph tab, a version. It toasts `MERGED_NOTICE` when inner groups merge.
  - `Editor::alt_session_added(&mut self, para, range)` (shared by typed and AI adds)
  - `Editor::ask_ai_alternates(&mut self, cx)`: the sidebar's target while it is open, else v1's behavior. `ai_alternates` (the action) calls it. `apply_ai_alternates` follows the sidebar's target and toasts the merge too.
  - `AltSidebar.input: Entity<InputState>` (placeholder "type another way to say it…"). Enter adds, clears the field and refocuses the list.
  - Actions `AltFocusInput` (`i`, `o` in `AltList`). `AiAlternates` is also bound to `secondary-shift-g` and `secondary-j` in `AltSidebar`. Both are Hidden rows. Plus the "✦ Ask AI" row.

- [ ] **Step 1: Write the failing tests**

`crates/tulis/src/editor/tests.rs`:
```diff
--- a/crates/tulis/src/editor/tests.rs
+++ b/crates/tulis/src/editor/tests.rs
@@ -973,3 +973,91 @@ fn paragraph_tab_previews_and_removes_versions(cx: &mut TestAppContext) {
     editor.update(cx, |e, cx| e.remove_alternate(1, cx));
     assert_eq!(rows(&editor, cx), vec![row("Draft one.", true)]);
 }
+
+#[gpui::test]
+fn the_first_add_creates_the_group_and_shows_the_new_option(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["I love tea"]);
+    select(&editor, cx, DocPos::new(0, 3), DocPos::new(0, 3));
+    editor.update(cx, |e, cx| e.open_alternates(None, cx));
+    editor.update(cx, |e, cx| e.add_alternate_text("  adore ", cx));
+    assert_eq!(texts(&editor, cx), vec!["I adore tea"]);
+    assert_eq!(
+        rows(&editor, cx),
+        vec![row("love", false), row("adore", true)]
+    );
+    editor.update(cx, |e, cx| e.add_alternate_text("like", cx));
+    assert_eq!(rows(&editor, cx).len(), 3);
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.alt_session.as_ref().unwrap().highlight),
+        2
+    );
+    editor.update(cx, |e, cx| e.add_alternate_text("   ", cx)); // blank: ignored
+    assert_eq!(rows(&editor, cx).len(), 3);
+    editor.update_in(cx, |e, window, cx| {
+        e.undo(&Undo, window, cx);
+        e.undo(&Undo, window, cx);
+    });
+    assert_eq!(texts(&editor, cx), vec!["I love tea"]);
+    assert!(editor.read_with(cx, |e, _| !e.doc.paragraphs[0].has_metadata()));
+}
+
+#[gpui::test]
+fn adding_on_the_paragraph_tab_adds_a_version(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["Draft one."]);
+    editor.update(cx, |e, cx| {
+        e.open_alternates(Some(Scope::Paragraph), cx);
+        e.add_alternate_text("Draft two.", cx);
+    });
+    assert_eq!(texts(&editor, cx), vec!["Draft two."]);
+    assert_eq!(
+        rows(&editor, cx),
+        vec![row("Draft one.", false), row("Draft two.", true)]
+    );
+}
+
+#[gpui::test]
+fn a_sentence_option_merges_inner_groups_with_a_toast_and_undo_restores_them(
+    cx: &mut TestAppContext,
+) {
+    let (editor, cx) = editor_with(cx, &["I love tea. Bye."]);
+    editor.update(cx, |e, _| {
+        e.doc.add_option(0, 2..6, "adore".into()).unwrap();
+        e.selection = Selection::caret(DocPos::new(0, 0));
+    });
+    editor.update(cx, |e, cx| {
+        e.open_alternates(Some(Scope::Sentence), cx);
+        e.add_alternate_text("Tea is love.", cx);
+    });
+    assert_eq!(texts(&editor, cx), vec!["Tea is love. Bye."]);
+    assert_eq!(
+        rows(&editor, cx),
+        vec![row("I adore tea.", false), row("Tea is love.", true)]
+    );
+    assert_eq!(
+        editor
+            .read_with(cx, |e, _| e.last_notice().map(str::to_string))
+            .as_deref(),
+        Some(super::alt_session::MERGED_NOTICE)
+    );
+    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
+    assert_eq!(texts(&editor, cx), vec!["I adore tea. Bye."]);
+    let inner = editor.read_with(cx, |e, _| e.doc.alt_info(DocPos::new(0, 3)).unwrap());
+    assert_eq!((inner.span, inner.count), (2..7, 2));
+}
+
+#[gpui::test]
+fn asking_ai_from_the_sidebar_appends_ai_options_to_its_target(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["I love tea. Bye."]);
+    with_mock(&editor, cx, vec![r#"["Tea is love.", "I adore tea."]"#]);
+    editor.update(cx, |e, cx| {
+        e.open_alternates(Some(Scope::Sentence), cx);
+        e.ask_ai_alternates(cx);
+    });
+    cx.run_until_parked();
+    assert_eq!(texts(&editor, cx), vec!["Tea is love. Bye."]);
+    let listed = rows(&editor, cx);
+    assert_eq!(listed.len(), 3);
+    assert_eq!(listed[0], row("I love tea.", false));
+    assert_eq!(listed[1], ("Tea is love.".to_string(), true, true));
+    assert_eq!(listed[2], ("I adore tea.".to_string(), false, true));
+}
```
`crates/tulis/src/workspace.rs`:
```diff
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -919,6 +919,41 @@ mod tests {
         assert!(ws.read_with(cx, |ws, cx| ws.editor.read(cx).alt_session.is_some()));
     }
 
+    #[gpui::test]
+    fn typing_in_the_sidebar_field_adds_an_option(cx: &mut TestAppContext) {
+        let (ws, cx) = root_workspace(cx, &["I love tea"]);
+        ws.update(cx, |ws, cx| {
+            ws.editor.update(cx, |e, cx| {
+                e.selection = Selection::caret(DocPos::new(0, 3));
+                e.open_alternates(None, cx);
+            })
+        });
+        cx.run_until_parked();
+        cx.simulate_keystrokes("i");
+        assert!(!sidebar_focused(&ws, cx));
+        cx.simulate_input("adore");
+        cx.simulate_keystrokes("enter");
+        cx.run_until_parked();
+        let text = ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.paragraphs[0].text());
+        assert_eq!(text, "I adore tea");
+        assert!(sidebar_focused(&ws, cx));
+        let field = ws.read_with(cx, |ws, cx| {
+            ws.alt_sidebar
+                .as_ref()
+                .unwrap()
+                .read(cx)
+                .input
+                .read(cx)
+                .value()
+        });
+        assert_eq!(field.as_ref(), "");
+        // Esc in the field closes the sidebar too.
+        cx.simulate_keystrokes("o");
+        cx.simulate_keystrokes("escape");
+        cx.run_until_parked();
+        assert!(ws.read_with(cx, |ws, cx| ws.editor.read(cx).alt_session.is_none()));
+    }
+
     #[gpui::test]
     fn toasts_stack_two_and_expire(cx: &mut TestAppContext) {
         init(cx);
```
- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile errors: `add_alternate_text`, `ask_ai_alternates`, `MERGED_NOTICE` and `input` don't exist.

- [ ] **Step 3: Implement**

`crates/tulis/src/editor/alt_session.rs`:
```diff
--- a/crates/tulis/src/editor/alt_session.rs
+++ b/crates/tulis/src/editor/alt_session.rs
@@ -367,3 +367,67 @@ impl Editor {
         self.end_edit(cx);
     }
 }
+
+/// Shown when a Sentence or Selection group swallows the groups inside it.
+pub(crate) const MERGED_NOTICE: &str = "Inner alternates merged — Ctrl+Z to restore";
+
+impl Editor {
+    /// After an add (typed or AI) changed the target to `range`: follow it and highlight the
+    /// new shown row. The next preview records a fresh undo step.
+    pub(crate) fn alt_session_added(&mut self, para: usize, range: Range<usize>) {
+        self.update_alt_target(para, range);
+        if let Some(session) = self.alt_session.as_mut() {
+            session.previewed = false;
+        }
+        self.highlight_shown_row();
+    }
+
+    /// Whether making a group over `range` (grown around alternates, as `make_alt` does) would
+    /// merge alternates strictly inside it.
+    pub(crate) fn merges_inner_groups(&self, para: usize, range: Range<usize>) -> bool {
+        let p = &self.doc.paragraphs[para];
+        let range = p.expand_to_alt_boundaries(range);
+        p.segments().iter().zip(p.segment_ranges()).any(|(seg, r)| {
+            seg.is_alt() && r != range && range.start <= r.start && r.end <= range.end
+        })
+    }
+
+    /// Add `text` as a new option of the target (creating its group on the first add), or as a
+    /// new version on the Paragraph tab, and show it.
+    pub fn add_alternate_text(&mut self, text: &str, cx: &mut Context<Self>) {
+        let text = text.trim();
+        if text.is_empty() {
+            return;
+        }
+        let Some((para, _)) = self.alt_session_target() else {
+            return;
+        };
+        let Some((scope, range)) = self
+            .alt_session
+            .as_ref()
+            .map(|s| (s.scope, s.range.clone()))
+        else {
+            return;
+        };
+        if scope != Scope::Paragraph && range.is_empty() {
+            return self.set_notice("Nothing to rewrite here", cx);
+        }
+        let merged = scope != Scope::Paragraph && self.merges_inner_groups(para, range.clone());
+        self.begin_edit(EditKind::Other, para..para + 1);
+        let range = if scope == Scope::Paragraph {
+            self.doc.add_version(para, text.to_string());
+            0..self.doc.paragraphs[para].len()
+        } else {
+            match self.doc.add_option(para, range, text.to_string()) {
+                Ok(span) => span,
+                Err(e) => return self.set_notice(e.to_string(), cx),
+            }
+        };
+        self.alt_session_added(para, range.clone());
+        self.selection = Selection::caret(DocPos::new(para, range.end));
+        self.end_edit(cx);
+        if merged {
+            self.set_notice(MERGED_NOTICE, cx);
+        }
+    }
+}
```
`crates/tulis/src/editor/ai.rs`:
```diff
--- a/crates/tulis/src/editor/ai.rs
+++ b/crates/tulis/src/editor/ai.rs
@@ -8,7 +8,8 @@ use tulis_core::ai::{AiError, CompletionRequest, LlmProvider, prompts, provider_
 use tulis_core::history::EditKind;
 use tulis_core::{AiKind, DocPos, ParaId, Selection, Source};
 
-use super::{Editor, TrimSuggestion};
+use super::alt_session::MERGED_NOTICE;
+use super::{Editor, Scope, TrimSuggestion};
 use crate::actions::{AiAlternates, AiTrimDocument, AiTrimParagraph};
 
 pub(crate) const STALE_NOTICE: &str = "Text changed while AI was working; result dropped";
@@ -42,14 +43,30 @@ impl Editor {
         _: &mut Window,
         cx: &mut Context<Self>,
     ) {
+        self.ask_ai_alternates(cx);
+    }
+
+    /// AI alternatives for the sidebar's target while it is open, else for the selection (or the
+    /// cursor's paragraph, as versions).
+    pub fn ask_ai_alternates(&mut self, cx: &mut Context<Self>) {
         self.finish_alt_edit();
-        let (para, range) = match self.target_range() {
-            Ok(target) => target,
-            Err(e) => return self.set_notice(e.to_string(), cx),
+        let session = self.alt_session_target().zip(
+            self.alt_session
+                .as_ref()
+                .map(|s| (s.scope, s.range.clone())),
+        );
+        let (para, range, whole_paragraph) = match session {
+            Some(((para, _), (Scope::Paragraph, _))) => {
+                (para, 0..self.doc.paragraphs[para].len(), true)
+            }
+            Some(((para, _), (_, range))) => (para, range, false),
+            None => match self.target_range() {
+                Ok((para, range)) => (para, range, self.selection.is_empty()),
+                Err(e) => return self.set_notice(e.to_string(), cx),
+            },
         };
         // Rewrite whole groups: part of an alternate stands for all of it.
         let range = self.doc.paragraphs[para].expand_to_alt_boundaries(range);
-        let whole_paragraph = self.selection.is_empty();
         let (original, export, para_id) = {
             let p = &self.doc.paragraphs[para];
             (p.text()[range.clone()].to_string(), p.export_text(), p.id)
@@ -108,22 +125,40 @@ impl Editor {
             return self.set_notice(STALE_NOTICE, cx);
         }
         let count = texts.len();
+        let for_session = self.alt_session.as_ref().is_some_and(|s| {
+            s.para_id == job.para_id
+                && (s.scope == Scope::Paragraph) == job.whole_paragraph
+                && (job.whole_paragraph
+                    || self.doc.paragraphs[para].expand_to_alt_boundaries(s.range.clone())
+                        == job.range)
+        });
+        let merged = !job.whole_paragraph && self.merges_inner_groups(para, job.range.clone());
         self.begin_edit(EditKind::Other, para..para + 1);
-        if job.whole_paragraph {
+        let span = if job.whole_paragraph {
             if let Err(e) = self.doc.add_ai_versions(para, texts, job.source) {
                 return self.set_notice(e.to_string(), cx);
             }
             self.selection = Selection::caret(DocPos::new(para, 0));
+            0..self.doc.paragraphs[para].len()
         } else {
             match self
                 .doc
                 .add_ai_alternates(para, job.range, texts, job.source)
             {
-                Ok(span) => self.selection = Selection::caret(DocPos::new(para, span.end)),
+                Ok(span) => {
+                    self.selection = Selection::caret(DocPos::new(para, span.end));
+                    span
+                }
                 Err(e) => return self.set_notice(e.to_string(), cx),
             }
+        };
+        if for_session {
+            self.alt_session_added(para, span);
         }
         self.end_edit(cx);
+        if merged {
+            self.set_notice(MERGED_NOTICE, cx);
+        }
         self.set_notice(format!("{count} AI alternatives — Alt+↑/↓ to cycle"), cx);
     }
 
```
`crates/tulis/src/sidebar.rs`:
```diff
--- a/crates/tulis/src/sidebar.rs
+++ b/crates/tulis/src/sidebar.rs
@@ -6,9 +6,12 @@ use gpui::{
     Styled, Subscription, Window, div, prelude::*, px,
 };
 use gpui_component::Sizable;
+use gpui_component::input::{Input, InputEvent, InputState};
 use gpui_component::tab::{Tab, TabBar};
 
-use crate::actions::{AltKeep, AltNext, AltPrev, AltRemove, CloseAlternates};
+use crate::actions::{
+    AiAlternates, AltFocusInput, AltKeep, AltNext, AltPrev, AltRemove, CloseAlternates,
+};
 use crate::editor::{Editor, Scope};
 use crate::workspace::Workspace;
 
@@ -18,19 +21,56 @@ pub struct AltSidebar {
     editor: Entity<Editor>,
     /// Focus of the option list (its keys: arrows, j/k, Enter, Delete, dd, i/o).
     pub(crate) list_focus: FocusHandle,
+    /// The type-to-add row.
+    pub(crate) input: Entity<InputState>,
     _subscriptions: Vec<Subscription>,
 }
 
 impl AltSidebar {
-    pub fn new(editor: Entity<Editor>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
-        let subscriptions = vec![cx.observe(&editor, |_, _, cx| cx.notify())];
+    pub fn new(editor: Entity<Editor>, window: &mut Window, cx: &mut Context<Self>) -> Self {
+        let input =
+            cx.new(|cx| InputState::new(window, cx).placeholder("type another way to say it…"));
+        let subscriptions = vec![
+            cx.observe(&editor, |_, _, cx| cx.notify()),
+            cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| {
+                if let InputEvent::PressEnter { .. } = event {
+                    this.add_typed(window, cx);
+                }
+            }),
+        ];
         Self {
             editor,
             list_focus: cx.focus_handle(),
+            input,
             _subscriptions: subscriptions,
         }
     }
 
+    /// Enter in the text field: add its text as an option (or version), show it, clear the
+    /// field and go back to the list.
+    fn add_typed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
+        let text = self.input.read(cx).value().to_string();
+        if text.trim().is_empty() {
+            return;
+        }
+        self.editor
+            .update(cx, |e, cx| e.add_alternate_text(&text, cx));
+        self.input
+            .update(cx, |input, cx| input.set_value("", window, cx));
+        window.focus(&self.list_focus, cx);
+    }
+
+    /// `i` / `o`: focus the text field.
+    fn focus_input(&mut self, _: &AltFocusInput, window: &mut Window, cx: &mut Context<Self>) {
+        let input = self.input.focus_handle(cx);
+        window.focus(&input, cx);
+    }
+
+    /// Ctrl+Shift+G or the "✦ Ask AI" row: AI alternatives for this target.
+    fn ask_ai(&mut self, _: &AiAlternates, _: &mut Window, cx: &mut Context<Self>) {
+        self.editor.update(cx, |e, cx| e.ask_ai_alternates(cx));
+    }
+
     /// Esc: close the sidebar, keeping the shown option (the workspace refocuses the page).
     fn close(&mut self, _: &CloseAlternates, _: &mut Window, cx: &mut Context<Self>) {
         self.editor.update(cx, |e, cx| e.close_alternates(cx));
@@ -109,6 +149,7 @@ impl Render for AltSidebar {
             .id("alt-sidebar")
             .key_context("AltSidebar")
             .on_action(cx.listener(Self::close))
+            .on_action(cx.listener(Self::ask_ai))
             .w(px(SIDEBAR_WIDTH))
             .flex_none()
             .h_full()
@@ -128,6 +169,7 @@ impl Render for AltSidebar {
                     .on_action(cx.listener(Self::prev))
                     .on_action(cx.listener(Self::keep))
                     .on_action(cx.listener(Self::remove))
+                    .on_action(cx.listener(Self::focus_input))
                     .flex_1()
                     .overflow_y_scroll()
                     .p_2()
@@ -162,6 +204,22 @@ impl Render for AltSidebar {
                             })
                     })),
             )
+            .child(div().px_2().child(Input::new(&self.input).small()))
+            .child(
+                div()
+                    .id("alt-ask-ai")
+                    .m_2()
+                    .px_2()
+                    .py_1()
+                    .rounded_md()
+                    .cursor_pointer()
+                    .text_color(t.ai_accent)
+                    .hover(|d| d.bg(t.selection))
+                    .child("✦ Ask AI")
+                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
+                        this.ask_ai(&AiAlternates, window, cx)
+                    })),
+            )
             .into_any_element()
     }
 }
```
`crates/tulis/src/actions.rs`:
```diff
--- a/crates/tulis/src/actions.rs
+++ b/crates/tulis/src/actions.rs
@@ -39,6 +39,7 @@ gpui::actions!(
         AltPrev,
         AltKeep,
         AltRemove,
+        AltFocusInput,
         CycleNext,
         CyclePrev,
         AcceptAlternate,
```
`crates/tulis/src/commands.rs`:
```diff
--- a/crates/tulis/src/commands.rs
+++ b/crates/tulis/src/commands.rs
@@ -332,6 +332,24 @@ pub static COMMANDS: &[Command] = &[
         None,
         AltRemove
     ),
+    cmd!(
+        "alt_focus_input",
+        "Sidebar: type a new option",
+        Hidden,
+        ["i", "o"],
+        Some("AltList"),
+        None,
+        AltFocusInput
+    ),
+    cmd!(
+        "alt_ask_ai",
+        "Sidebar: AI alternatives",
+        Hidden,
+        ["secondary-shift-g", "secondary-j"],
+        Some("AltSidebar"),
+        None,
+        AiAlternates
+    ),
     cmd!(
         "cycle_next",
         "Next alternate",
```
- [ ] **Step 4: Run the targeted tests (expect pass)**

Run: `cargo test -p tulis add` and `cargo test -p tulis ai`
Expected: pass.

- [ ] **Step 5: Run everything (expect pass)**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: all tests pass (app 114); the clippy counts match the baseline.
Run: `cargo build && timeout 6 ./target/debug/tulis /tmp/smoke.md; echo $?`
Expected: `124` and nothing on stderr.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
jj describe -m "feat: type-to-add, AI options and the inner-alternates merge toast

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---
### Task 7: Page dimming, and the `⋮` handle opens the Paragraph tab

**Files:**
- Modify: `crates/tulis/src/editor/element.rs`, `crates/tulis/src/editor/actions.rs`, `crates/tulis/src/editor/features.rs`, `crates/tulis/src/editor/tests.rs`

**Interfaces:**
- Consumes: `Editor.alt_session` (Task 4).
- Produces:
  - `element::DIM_ALPHA = 0.35` and `element::paragraph_alpha(&Editor, ParaId) -> f32`. While the sidebar is open, `build_layout` shapes every other paragraph with a `RunStyle` whose text, trim and marker colors are scaled by `DIM_ALPHA`. Ghosts are scaled on top of that.
  - Clicking a version handle moves the caret to that paragraph's start and calls `open_alternates(Some(Scope::Paragraph))`. `Editor::next_version` is removed (nothing else uses it).

- [ ] **Step 1: Write the failing tests**

Replace `clicking_the_version_handle_shows_the_next_version` and add the dimming test:

`crates/tulis/src/editor/tests.rs`:
```diff
--- a/crates/tulis/src/editor/tests.rs
+++ b/crates/tulis/src/editor/tests.rs
@@ -699,11 +699,10 @@ fn cycling_boxes_the_group_until_the_cursor_leaves(cx: &mut TestAppContext) {
 }
 
 #[gpui::test]
-fn clicking_the_version_handle_shows_the_next_version(cx: &mut TestAppContext) {
-    let (editor, cx) = editor_with(cx, &["Draft one."]);
-    editor.update_in(cx, |e, window, cx| {
-        e.make_alternate(&MakeAlternate, window, cx);
-        e.replace_text_in_range(None, " Two.", window, cx);
+fn clicking_the_version_handle_opens_the_paragraph_tab(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["Intro.", "Draft one."]);
+    editor.update(cx, |e, _| {
+        e.doc.add_version(1, "Draft two.".into());
     });
     cx.simulate_resize(gpui::size(gpui::px(800.), gpui::px(600.)));
     cx.run_until_parked();
@@ -712,11 +711,17 @@ fn clicking_the_version_handle_shows_the_next_version(cx: &mut TestAppContext) {
         assert_eq!(layout.version_handles.len(), 1);
         layout.version_handles[0].1
     });
-    let before = editor.read_with(cx, |e, _| e.doc.paragraphs[0].active);
     cx.simulate_click(handle.center(), gpui::Modifiers::none());
-    let after = editor.read_with(cx, |e, _| e.doc.paragraphs[0].active);
-    assert_ne!(before, after);
-    assert!(editor.read_with(cx, |e, _| e.cycled.is_some()));
+    let session = editor.read_with(cx, |e, _| e.alt_session.clone()).unwrap();
+    assert_eq!(session.scope, Scope::Paragraph);
+    assert_eq!(
+        session.para_id,
+        editor.read_with(cx, |e, _| e.doc.paragraphs[1].id)
+    );
+    assert_eq!(
+        rows(&editor, cx),
+        vec![row("Draft one.", false), row("Draft two.", true)]
+    );
 }
 
 #[gpui::test]
@@ -1061,3 +1066,24 @@ fn asking_ai_from_the_sidebar_appends_ai_options_to_its_target(cx: &mut TestAppC
     assert_eq!(listed[1], ("Tea is love.".to_string(), true, true));
     assert_eq!(listed[2], ("I adore tea.".to_string(), false, true));
 }
+
+#[gpui::test]
+fn the_page_dims_except_the_target_paragraph_while_the_sidebar_is_open(cx: &mut TestAppContext) {
+    use super::element::{DIM_ALPHA, paragraph_alpha};
+    let (editor, cx) = editor_with(cx, &["One.", "Two."]);
+    let alphas = |cx: &mut VisualTestContext| {
+        editor.read_with(cx, |e, _| {
+            e.doc
+                .paragraphs
+                .iter()
+                .map(|p| paragraph_alpha(e, p.id))
+                .collect::<Vec<_>>()
+        })
+    };
+    assert_eq!(alphas(cx), vec![1., 1.]);
+    select(&editor, cx, DocPos::new(1, 1), DocPos::new(1, 1));
+    editor.update(cx, |e, cx| e.open_alternates(None, cx));
+    assert_eq!(alphas(cx), vec![DIM_ALPHA, 1.]);
+    editor.update(cx, |e, cx| e.close_alternates(cx));
+    assert_eq!(alphas(cx), vec![1., 1.]);
+}
```
- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile errors: `DIM_ALPHA` and `paragraph_alpha` don't exist. The handle test fails (no session).

- [ ] **Step 3: Implement**

`crates/tulis/src/editor/element.rs`:
```diff
--- a/crates/tulis/src/editor/element.rs
+++ b/crates/tulis/src/editor/element.rs
@@ -10,6 +10,7 @@ use super::Editor;
 use super::layout::{LayoutCache, ParaLayout, clamp_px};
 use super::markers::{self, DOT};
 use crate::runs::{RunStyle, build_runs};
+use tulis_core::ParaId;
 use tulis_core::stats::title_marker_len;
 
 pub struct EditorElement {
@@ -40,6 +41,17 @@ impl IntoElement for EditorElement {
 /// Horizontal space kept free on each side of the text, for the paragraph-version bar.
 pub(crate) const GUTTER: Pixels = px(24.);
 
+/// Text alpha of the paragraphs around the alternates sidebar's target while it is open.
+pub(crate) const DIM_ALPHA: f32 = 0.35;
+
+/// The alpha factor for paragraph `id`'s text: dimmed while the sidebar targets another one.
+pub(crate) fn paragraph_alpha(editor: &Editor, id: ParaId) -> f32 {
+    match &editor.alt_session {
+        Some(session) if session.para_id != id => DIM_ALPHA,
+        _ => 1.,
+    }
+}
+
 pub(crate) fn line_height(editor: &Editor) -> Pixels {
     px(editor.config.editor.effective_font_size() * 1.8)
 }
@@ -55,18 +67,22 @@ fn build_layout(
     let lh = line_height(editor);
     let mut title_font = font(editor.font_family.clone());
     title_font.weight = FontWeight::BOLD;
-    let style = RunStyle {
-        font: font(editor.font_family.clone()),
-        title_font,
-        color: editor.theme.text,
-        trim: editor.theme.trim,
-        marker: {
-            let mut c = editor.theme.text_muted;
-            c.a *= 0.5;
+    let style_with_alpha = |alpha: f32| {
+        let faded = |mut c: gpui::Hsla, a: f32| {
+            c.a *= a;
             c
-        },
-        ghost_alpha: editor.theme.ghost_alpha,
+        };
+        RunStyle {
+            font: font(editor.font_family.clone()),
+            title_font: title_font.clone(),
+            color: faded(editor.theme.text, alpha),
+            trim: faded(editor.theme.trim, alpha),
+            marker: faded(editor.theme.text_muted, 0.5 * alpha),
+            ghost_alpha: editor.theme.ghost_alpha,
+        }
     };
+    let style = style_with_alpha(1.);
+    let dimmed = style_with_alpha(DIM_ALPHA);
     let marked = editor.marked.clone().map(|r| {
         (
             editor.doc.pos_from_flat(r.start),
@@ -97,7 +113,11 @@ fn build_layout(
             marked_here,
             title_marker,
             caret_on_marker,
-            &style,
+            if paragraph_alpha(editor, p.id) < 1. {
+                &dimmed
+            } else {
+                &style
+            },
         );
         let line = window
             .text_system()
```
`crates/tulis/src/editor/actions.rs`:
```diff
--- a/crates/tulis/src/editor/actions.rs
+++ b/crates/tulis/src/editor/actions.rs
@@ -257,7 +257,8 @@ impl Editor {
         if let Some(para) = super::markers::handle_at(&layout.version_handles, event.position)
             && para < self.doc.paragraphs.len()
         {
-            self.next_version(para, cx);
+            self.set_selection(Selection::caret(DocPos::new(para, 0)), cx);
+            self.open_alternates(Some(super::Scope::Paragraph), cx);
             return;
         }
         let pos = self.doc.clamp(layout.pos_for_window_point(event.position));
```
`crates/tulis/src/editor/features.rs`:
```diff
--- a/crates/tulis/src/editor/features.rs
+++ b/crates/tulis/src/editor/features.rs
@@ -98,25 +98,6 @@ impl Editor {
         });
     }
 
-    /// Show the next version of paragraph `para` (a click on its `⋮` handle).
-    pub(crate) fn next_version(&mut self, para: usize, cx: &mut Context<Self>) {
-        self.finish_alt_edit();
-        let count = self.doc.paragraphs[para].versions.len();
-        if count < 2 {
-            return;
-        }
-        self.begin_edit(EditKind::Other, para..para + 1);
-        let p = &mut self.doc.paragraphs[para];
-        p.active = (p.active + 1) % count;
-        let (id, len) = (p.id, p.len());
-        self.selection = Selection::caret(DocPos::new(para, 0));
-        self.end_edit(cx);
-        self.cycled = Some(CycledGroup {
-            para_id: id,
-            range: 0..len,
-        });
-    }
-
     pub(super) fn cycle_next(&mut self, _: &CycleNext, _: &mut Window, cx: &mut Context<Self>) {
         self.cycle(true, cx);
     }
```
- [ ] **Step 4: Run the targeted tests (expect pass)**

Run: `cargo test -p tulis handle` and `cargo test -p tulis dims`
Expected: pass.

- [ ] **Step 5: Run everything (expect pass)**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: all tests pass (app 115); the clippy counts match the baseline.
Run: `cargo build && timeout 6 ./target/debug/tulis /tmp/smoke.md; echo $?`
Expected: `124` and nothing on stderr.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
jj describe -m "feat: dim the page around the sidebar target; ⋮ opens the Paragraph tab

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---
### Task 8: The right-click context menu

**Files:**
- Modify: `crates/tulis/src/editor/actions.rs`, `crates/tulis/src/editor/mod.rs`, `crates/tulis/src/workspace.rs`, `crates/tulis/src/editor/tests.rs`

**Interfaces:**
- Consumes: `gpui_component::menu::ContextMenuExt::context_menu(self, Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu)` (`src/menu/context_menu.rs:17`). `PopupMenu::{action_context(FocusHandle), menu(label, Box<dyn Action>)}` (`src/menu/popup_menu.rs:373, 467`). The menu's right-click listener is registered before the children paint, so the editor's own `MouseButton::Right` handler runs first in the bubble phase. The menu is then built in `window.defer`, after the editor has picked the target, so the Ghost/Revive label is current. A confirmed item dispatches its action to the `action_context` handle (the page), so the editor's existing handlers run.
- Produces:
  - `Editor::target_for_context_menu(&mut self, pos: DocPos, cx)`: keeps a non-empty selection. Otherwise it selects the Word-scope range at `pos` (the group, the word, or the word just before a caret on whitespace). With no word there, it just moves the caret.
  - `Editor::ghost_menu_label(&self) -> &'static str`: `"Revive it"` when every span the target covers is ghosted, else `"Ghost it"`.
  - `Editor::on_right_mouse_down`.
  - The page (`div().id("page")`) gets a `.context_menu` with "Alternatives for selection" (`OpenAlternates`), "AI alternatives for selection" (`AiAlternates`), Ghost it / Revive it (`ToggleGhost`) and "Stash this in Overflow" (`SendToOverflow`). The menu shows each action's key from the keymap.

- [ ] **Step 1: Write the failing tests**

`crates/tulis/src/editor/tests.rs`:
```diff
--- a/crates/tulis/src/editor/tests.rs
+++ b/crates/tulis/src/editor/tests.rs
@@ -1087,3 +1087,71 @@ fn the_page_dims_except_the_target_paragraph_while_the_sidebar_is_open(cx: &mut
     editor.update(cx, |e, cx| e.close_alternates(cx));
     assert_eq!(alphas(cx), vec![1., 1.]);
 }
+
+#[gpui::test]
+fn right_click_targets_the_selection_or_the_word_under_the_pointer(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["I love green tea", ""]);
+    editor.update(cx, |e, cx| e.target_for_context_menu(DocPos::new(0, 8), cx));
+    let sel = editor.read_with(cx, |e, _| e.selection.range());
+    assert_eq!(sel, DocPos::new(0, 7)..DocPos::new(0, 12));
+    // An existing selection is kept.
+    select(&editor, cx, DocPos::new(0, 2), DocPos::new(0, 12));
+    editor.update(cx, |e, cx| {
+        e.target_for_context_menu(DocPos::new(0, 14), cx)
+    });
+    let sel = editor.read_with(cx, |e, _| e.selection.range());
+    assert_eq!(sel, DocPos::new(0, 2)..DocPos::new(0, 12));
+    // Just after a word, that word; with no word there, the cursor just moves.
+    select(&editor, cx, DocPos::new(0, 0), DocPos::new(0, 0));
+    editor.update(cx, |e, cx| e.target_for_context_menu(DocPos::new(0, 1), cx));
+    let sel = editor.read_with(cx, |e, _| e.selection.range());
+    assert_eq!(sel, DocPos::new(0, 0)..DocPos::new(0, 1));
+    select(&editor, cx, DocPos::new(0, 0), DocPos::new(0, 0));
+    editor.update(cx, |e, cx| e.target_for_context_menu(DocPos::new(1, 0), cx));
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.selection),
+        Selection::caret(DocPos::new(1, 0))
+    );
+}
+
+#[gpui::test]
+fn context_menu_commands_act_on_the_word_and_the_ghost_label_follows(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["I love green tea"]);
+    editor.update(cx, |e, cx| e.target_for_context_menu(DocPos::new(0, 8), cx));
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.ghost_menu_label()),
+        "Ghost it"
+    );
+    // Alternatives for selection.
+    editor.update(cx, |e, cx| e.open_alternates(None, cx));
+    let session = editor.read_with(cx, |e, _| e.alt_session.clone()).unwrap();
+    assert!(session.selection);
+    assert_eq!(rows(&editor, cx), vec![row("green", true)]);
+    editor.update(cx, |e, cx| e.close_alternates(cx));
+    // Ghost it, then the label reads Revive it.
+    editor.update(cx, |e, cx| e.toggle_ghost_here(cx));
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.doc.paragraphs[0].export_text()),
+        "I love  tea"
+    );
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.ghost_menu_label()),
+        "Revive it"
+    );
+    editor.update(cx, |e, cx| e.toggle_ghost_here(cx));
+    // AI alternatives for selection.
+    with_mock(&editor, cx, vec![r#"["verdant"]"#]);
+    editor.update(cx, |e, cx| e.ask_ai_alternates(cx));
+    cx.run_until_parked();
+    assert_eq!(texts(&editor, cx), vec!["I love verdant tea"]);
+    // Stash this in Overflow.
+    editor.update(cx, |e, cx| e.target_for_context_menu(DocPos::new(0, 3), cx));
+    editor.update_in(cx, |e, window, cx| {
+        e.send_to_overflow(&SendToOverflow, window, cx)
+    });
+    assert_eq!(texts(&editor, cx), vec!["I  verdant tea"]);
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.doc.overflow[0].text.clone()),
+        "love"
+    );
+}
```
`crates/tulis/src/workspace.rs`:
```diff
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -954,6 +967,29 @@ mod tests {
         assert!(ws.read_with(cx, |ws, cx| ws.editor.read(cx).alt_session.is_none()));
     }
 
+    #[gpui::test]
+    fn right_click_menu_opens_the_sidebar_on_the_word(cx: &mut TestAppContext) {
+        let (ws, cx) = root_workspace(cx, &["I love green tea"]);
+        cx.simulate_resize(gpui::size(gpui::px(1000.), gpui::px(700.)));
+        cx.run_until_parked();
+        let point = ws.read_with(cx, |ws, cx| {
+            let layout = ws.editor.read(cx).layout.as_ref().expect("painted");
+            let p = layout.window_point(DocPos::new(0, 8)).unwrap();
+            gpui::point(p.x + gpui::px(2.), p.y + layout.line_height / 2.)
+        });
+        cx.simulate_mouse_down(point, gpui::MouseButton::Right, gpui::Modifiers::none());
+        cx.simulate_mouse_up(point, gpui::MouseButton::Right, gpui::Modifiers::none());
+        cx.run_until_parked();
+        let sel = ws.read_with(cx, |ws, cx| ws.editor.read(cx).selection.range());
+        assert_eq!(sel, DocPos::new(0, 7)..DocPos::new(0, 12));
+        // First item: Alternatives for selection.
+        cx.simulate_keystrokes("down enter");
+        cx.run_until_parked();
+        let session = ws.read_with(cx, |ws, cx| ws.editor.read(cx).alt_session.clone());
+        assert_eq!(session.map(|s| s.range), Some(7..12));
+        assert!(sidebar_focused(&ws, cx));
+    }
+
     #[gpui::test]
     fn toasts_stack_two_and_expire(cx: &mut TestAppContext) {
         init(cx);
```
- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile errors: `target_for_context_menu` and `ghost_menu_label` don't exist.

- [ ] **Step 3: Implement**

`crates/tulis/src/editor/actions.rs`:
```diff
--- a/crates/tulis/src/editor/actions.rs
+++ b/crates/tulis/src/editor/actions.rs
@@ -285,6 +285,64 @@ impl Editor {
         }
     }
 
+    /// Right-click (before the context menu opens): keep a selection; otherwise select the group
+    /// or word under the pointer, so the menu's commands act on it.
+    pub(super) fn on_right_mouse_down(
+        &mut self,
+        event: &MouseDownEvent,
+        window: &mut Window,
+        cx: &mut Context<Self>,
+    ) {
+        window.focus(&self.focus_handle, cx);
+        let Some(layout) = self.layout.as_ref() else {
+            return;
+        };
+        let pos = self.doc.clamp(layout.pos_for_window_point(event.position));
+        self.target_for_context_menu(pos, cx);
+    }
+
+    pub(crate) fn target_for_context_menu(&mut self, pos: DocPos, cx: &mut Context<Self>) {
+        self.finish_alt_edit();
+        if !self.selection.is_empty() {
+            return;
+        }
+        let range = self.scope_range(
+            pos.para,
+            super::Scope::Word,
+            &(pos.offset..pos.offset),
+            false,
+        );
+        let text = self.doc.paragraphs[pos.para].text();
+        let selection = if text[range.clone()].trim().is_empty() {
+            Selection::caret(pos)
+        } else {
+            Selection {
+                anchor: DocPos::new(pos.para, range.start),
+                head: DocPos::new(pos.para, range.end),
+            }
+        };
+        self.set_selection(selection, cx);
+    }
+
+    /// The context menu's ghost item: "Revive it" when its target is entirely ghosted.
+    pub fn ghost_menu_label(&self) -> &'static str {
+        let Ok((para, range)) = self.target_range() else {
+            return "Ghost it";
+        };
+        let p = &self.doc.paragraphs[para];
+        let range = p.expand_to_alt_boundaries(range);
+        let spans: Vec<_> = p
+            .spans()
+            .into_iter()
+            .filter(|s| s.range.start < range.end && range.start < s.range.end)
+            .collect();
+        if !spans.is_empty() && spans.iter().all(|s| s.ghost) {
+            "Revive it"
+        } else {
+            "Ghost it"
+        }
+    }
+
     pub(super) fn on_mouse_move(
         &mut self,
         event: &MouseMoveEvent,
```
`crates/tulis/src/editor/mod.rs`:
```diff
--- a/crates/tulis/src/editor/mod.rs
+++ b/crates/tulis/src/editor/mod.rs
@@ -586,6 +586,7 @@ impl Render for Editor {
             .on_action(cx.listener(Self::toggle_vim))
             .on_key_down(cx.listener(Self::on_key_down))
             .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
+            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_right_mouse_down))
             .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
             .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
             .on_mouse_move(cx.listener(Self::on_mouse_move))
```
`crates/tulis/src/workspace.rs`:
```diff
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -7,14 +7,15 @@ use gpui::{
     PathPromptOptions, PromptLevel, SharedString, Subscription, Task, Window, div,
     linear_color_stop, linear_gradient, prelude::*, px,
 };
+use gpui_component::menu::ContextMenuExt;
 use gpui_component::text::TextView;
 use tulis_core::config::Config;
 use tulis_core::storage;
 use tulis_core::{Document, OverflowId};
 
 use crate::actions::{
-    CycleTheme, New, Open, OpenLab, Quit, Save, SaveAs, ToggleMarkers, ToggleOverflowPanel,
-    TogglePreview,
+    AiAlternates, CycleTheme, New, Open, OpenAlternates, OpenLab, Quit, Save, SaveAs,
+    SendToOverflow, ToggleGhost, ToggleMarkers, ToggleOverflowPanel, TogglePreview,
 };
 use crate::editor::{Editor, EditorEvent, GUTTER};
 use crate::header_dim::HeaderDim;
@@ -349,7 +350,10 @@ impl Workspace {
     fn render_page(&self, cx: &App) -> impl IntoElement {
         let editor = self.editor.read(cx);
         let (scroll, background) = (editor.scroll.clone(), editor.theme.background);
+        let page_focus = editor.focus_handle(cx);
+        let menu_editor = self.editor.clone();
         div()
+            .id("page")
             .relative()
             .flex_1()
             .min_w(px(0.))
@@ -377,6 +381,15 @@ impl Workspace {
             )
             .child(edge_fade(true, background))
             .child(edge_fade(false, background))
+            // The editor's own right-click handler runs first and picks the target.
+            .context_menu(move |menu, _, cx| {
+                let ghost = menu_editor.read(cx).ghost_menu_label();
+                menu.action_context(page_focus.clone())
+                    .menu("Alternatives for selection", Box::new(OpenAlternates))
+                    .menu("AI alternatives for selection", Box::new(AiAlternates))
+                    .menu(ghost, Box::new(ToggleGhost))
+                    .menu("Stash this in Overflow", Box::new(SendToOverflow))
+            })
     }
 }
 
```
- [ ] **Step 4: Run the targeted tests (expect pass)**

Run: `cargo test -p tulis right_click` and `cargo test -p tulis context_menu`
Expected: pass.

- [ ] **Step 5: Run everything (expect pass)**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: all tests pass (app 118, core 137); the clippy counts match the baseline.
Run: `cargo build && timeout 6 ./target/debug/tulis /tmp/smoke.md; echo $?`
Expected: `124` and nothing on stderr.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
jj describe -m "feat: right-click context menu (alternatives, AI, ghost/revive, stash)

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---
### Task 9: README, smoke checklist and final verification

**Files:**
- Modify: `README.md`

**Interfaces:** none (docs only).

- [ ] **Step 1: Update the README**

`README.md`:
```diff
--- a/README.md
+++ b/README.md
@@ -20,25 +20,26 @@ paragraph that still matches and tells you how many paragraphs lost theirs.
 
 | Key | Action |
 |---|---|
-| `Ctrl+D` | Make an alternate of the selection (then type the new wording), or a new version of the paragraph |
+| `Ctrl+Shift+A` (or `Ctrl+D`) | Alternatives sidebar for the selection or the word under the cursor: Word/Selection, Sentence and Paragraph tabs; `↑/↓` or `j/k` preview, `Enter` keeps, `Delete` or `dd` removes, `i`/`o` type a new option, `Esc` closes |
 | `Alt+↑` / `Alt+↓` | Cycle alternates or paragraph versions |
 | `Ctrl+Enter` | Accept: keep only the shown alternate/version |
 | `Ctrl+Shift+Backspace` | Delete the shown alternate/version |
-| `Ctrl+G` | Ghost / revive the selection or paragraph |
-| `Ctrl+Shift+O` | Move the selection or paragraph to overflow |
+| `Ctrl+/` (or `Ctrl+G`) | Ghost / revive the selection or paragraph |
+| `Ctrl+Shift+X` (or `Ctrl+Shift+O`) | Stash the selection or paragraph in overflow |
 | `Ctrl+\` | Show/hide the overflow panel |
-| `Ctrl+J` | AI alternates for the selection or paragraph |
+| `Ctrl+Shift+G` (or `Ctrl+J`) | AI alternatives for the sidebar's target, else the selection or paragraph |
 | `Ctrl+Shift+T` / `Ctrl+Shift+Alt+T` | AI trim suggestions for the paragraph / document; `Tab` accepts (ghosts), `Esc` dismisses |
 | `Ctrl+Alt+V` | Toggle vim mode |
 | `Ctrl+K T` | Cycle theme (Midnight, Paper, Sepia, Black) for the session |
 | `Ctrl+.` | Show/hide alternate and version markers |
 | `Ctrl+Shift+M` | Markdown preview (`Esc` closes) |
 | `Ctrl+?` | Cheat sheet |
+| Right-click | Menu: Alternatives / AI alternatives for the selection (or the word under the pointer), Ghost it / Revive it, Stash this in Overflow |
 | `Ctrl+L` | The Lab (runs the document trim until Part 4) |
 | `Ctrl+S` / `Ctrl+Shift+S` / `Ctrl+O` / `Ctrl+N` / `Ctrl+Q` | Save / Save as / Open / New / Quit |
 
 Vim mode: `h j k l w b e 0 ^ $ gg G { }` with counts; `i a I A o O`; `d c y` + motion, `dd cc yy`,
-`x p P u Ctrl+R`; `v`/`V`; `]a`/`[a` cycle alternates; `gh` toggles ghost; `gt gm gp g? gl gx`
+`x p P u Ctrl+R`; `v`/`V`; `]a`/`[a` cycle alternates; `gh` toggles ghost; `ga` sidebar, `gA` AI alternatives, `gs` stash (in Visual mode these act on the selection); `gt gm gp g? gl gx`
 (theme, markers, preview, cheat sheet, Lab, Overflow).
 
 The footer has Save, Open, Share to X and LinkedIn, LAB and XYZ (Overflow) buttons.
@@ -79,8 +80,8 @@ cargo test --workspace
 - [ ] Copy/cut/paste, undo/redo (typing undoes in chunks)
 - [ ] IME composition (e.g. Japanese/Chinese input method) shows underlined marked text and commits
 - [ ] Caret stays visible when typing past the bottom; scrolling with the wheel works
-- [ ] `Ctrl+D` → type → `Alt+↑/↓` swaps; dotted underline on alternates; `alt i/n` badge in the header
-- [ ] `Ctrl+G` ghosts to ~10% and back; ghosted text absent from saved `.md`
+- [ ] `Alt+↑/↓` swaps a group's options; dotted underline on alternates; `alt i/n` badge in the header
+- [ ] `Ctrl+/` (and `Ctrl+G`) ghosts to ~10% and back; ghosted text absent from saved `.md`
 - [ ] Overflow: send, panel toggle, click to insert, × to remove, "keep after insert"
 - [ ] Save/Open round-trips alternates, ghosts, overflow; external edit shows drift notice
 - [ ] Unsaved-changes prompt on New/Open/Quit/window close
@@ -91,7 +92,7 @@ cargo test --workspace
 - [ ] Unknown `theme` in config shows a toast and uses Midnight
 - [ ] Text column is 640px and centered; text fades out under the top and bottom edges while scrolling
 - [ ] A first paragraph starting with `# ` is bold and its `# ` is invisible (but the cursor still moves over it)
-- [ ] Alternates show a dotted underline (accent when AI-written); paragraphs with versions show a bar left of the column with a `⋮` handle; clicking the handle shows the next version
+- [ ] Alternates show a dotted underline (accent when AI-written); paragraphs with versions show a bar left of the column with a `⋮` handle; clicking the handle opens the sidebar on its Paragraph tab
 - [ ] Alt+↓ draws a faint box around the cycled group until the cursor leaves it
 - [ ] Ctrl+. / `gm` hides and shows all markers
 - [ ] Header fades after ~2s of continuous typing and returns on mouse move or a 2s pause
@@ -104,3 +105,12 @@ cargo test --workspace
 ## License
 
 [MIT](LICENSE).
+- [ ] Ctrl+Shift+A / Ctrl+D / `ga` opens a 240px sidebar on the left; the page dims except the target paragraph; tabs read Word (Selection with a selection), Sentence, Paragraph in italic accent
+- [ ] Rows show `●` for the shown option, `·` for the others and `✦` for AI-written ones; long options clip at 3 lines
+- [ ] j/k or arrows preview options in the page; one Ctrl+Z after closing undoes the whole preview session
+- [ ] Typing in "type another way to say it…" + Enter adds and shows the option; opening and closing without adding leaves no group behind
+- [ ] Adding on the Sentence tab over a word group toasts "Inner alternates merged — Ctrl+Z to restore"
+- [ ] "✦ Ask AI" (or Ctrl+Shift+G in the sidebar) appends AI options to the tab's target
+- [ ] Delete / `dd` removes the highlighted option but never the last; Esc closes and keeps what is shown
+- [ ] Deleting the target paragraph (or typing inside the target) closes the sidebar
+- [ ] Right-click a word: the menu targets it (or the selection); Ghost it turns into Revive it on ghosted text; Stash moves it to Overflow
```
- [ ] **Step 2: Full verification**

Run: `cargo fmt --all --check && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: formatting is clean, 118 app tests and 137 core tests pass, and the clippy counts match the baseline.

Run: `cargo build && timeout 6 target/debug/tulis /tmp/smoke.md; echo $?`
Expected: `124`, with nothing on stderr.

Then walk through the new smoke-checklist items by hand. Report any that fail rather than ticking them.

- [ ] **Step 3: Commit**

```bash
jj describe -m "docs: README keys and smoke checklist for redesign part 2

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

## Decisions this plan makes where the spec is silent or ambiguous

- **Ctrl+D opens the sidebar.** The keymap table lists it as the old key for "Alternatives sidebar", so v1's inline Make Alternate loses its key. `MakeAlternate` stays an editor action (its tests still run), but it has no key and no cheat-sheet row. Typing in the sidebar's field replaces that workflow.
- **Enter keeps the sidebar open.** Enter (or a click) shows the option, puts the caret after the target and focuses the page. Only Esc closes the sidebar. Ctrl+Shift+A on the page retargets it.
- **"One undo step per session"** means previews coalesce into one step, recorded by the first preview that changes anything. A recorded operation (add or remove) ends that run, so a later preview records a fresh step. Removing is its own step.
- **Target identity.** The target is `(paragraph id, byte range, text)`. A group is "the target's" only when its span equals the range exactly. "Target disappears" means the paragraph is gone, or (below Paragraph scope) the text at the range changed, which includes undo and typing inside it. Edits elsewhere in the paragraph that leave the target's text in place keep the sidebar open.
- **Word scope at a caret** uses the group under it, else the word under it. On whitespace right after a word, it uses that word, since that is where the caret sits after typing. A Sentence over a selection covers every sentence the selection touches, grown around alternates.
- **Selection scope** uses exactly the selection. When the group is created, `ensure_alt` may grow it around overlapping groups; the sidebar then follows the returned span.
- **Visual mode.** `gh`, `ga`, `gA` and `gs` act on the selection including the character under the cursor (as `d`/`y` do), then return to Normal. This also fixes v1's exclusive `gh` in Visual mode. `ga` calls the editor directly: a dispatched action would run after `EndVisual` had cleared the selection.
- **List keys** (`j`/`k`, `dd`, `i`/`o`) work whether or not vim mode is on. The list has no text entry, so there is nothing to conflict with. `dd` is a two-keystroke binding (`"d d"`).
- **Removing** never touches the last row. With two left, removing one collapses the group to plain text (or leaves one version). Removing the shown option shows the one before it.
- **Esc in the text field** closes the sidebar, like Esc on the list.
- **AI from the sidebar** targets the tab's target: options for Word/Selection/Sentence, versions for Paragraph. The result follows the target, highlights the first new option, and toasts the merge when a Sentence or Selection AI group swallows inner groups.
- **Dimming** scales text, trim strikes and the title marker of the other paragraphs by 0.35. Markers aren't dimmed.
- **Right-click** keeps a non-empty selection. Otherwise it selects the group or word under the pointer, using the Word-scope rule above; with no word there, it only moves the caret. The menu shows each item's key from the keymap. The ghost label reads "Revive it" when everything the target covers is already ghosted.
- **Stash** (`gs`, Ctrl+Shift+X, the menu) is v1's send-to-overflow, unchanged. Part 3 replaces it with the scratchpad.
- **`⋮` handle** moves the caret to the start of its paragraph and opens the Paragraph tab. `next_version` is removed.
