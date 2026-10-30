# Tulis Redesign, Part 3: The Overflow Scratchpad

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace v1's list of overflow items with Write_On's scratchpad: free text in a 260px right panel, kept in the document as one string. Ctrl+Shift+X / `gs` / the context menu stash text into it without opening it, with a "Stashed to Overflow" toast. Ctrl+Enter in the scratchpad moves its selection, or the block at its cursor, to the page cursor; Ctrl+Shift+Enter copies instead. Each block has a `⠿` grip that can be dragged onto the page (Ctrl held copies). Ctrl+Shift+\\ / vim `gX` move focus between the page and the scratchpad, and Esc in the scratchpad returns to the page. Sidecars move to format version 2, and version 1 files are migrated on open. Vim `Vgs` now removes the stashed paragraphs, like `Vd`.

**Architecture:** `tulis-core` holds the scratchpad as `Document.overflow: String` and does the text work: `stash`, `stash_paragraphs`, `overflow_blocks`, `overflow_block_at` and `take_overflow` (`overflow.rs`). `storage.rs` writes version 2 and rewrites a version 1 sidecar's item list into the string before parsing it. The editor owns every change to the document. `Editor::set_overflow` takes the scratchpad's own edits: it marks the document dirty and writes the new text into every stored undo step (`History::for_each_state`), so page undo never reverts scratchpad typing. `Editor::use_overflow` puts scratchpad text into the page as one undo step; Ctrl+Enter (`use_overflow_selection`) and drops (`drop_overflow`) both go through it. The view (`scratchpad.rs`, `Scratchpad`) holds a gpui-component `TextareaState` and keeps it in sync both ways: field `Change` events call `set_overflow`, and an `observe_in` on the editor calls `set_value` when `doc.overflow` changed elsewhere (stash, undo, open). Its key context is `Scratchpad`. Ctrl+Enter is bound in `Scratchpad > Input`, which matches at the field's own depth and is bound later than the field's Ctrl+Enter (a line break), so it wins. Grips are positioned in window coordinates (`anchored()`) from the field's last layout (`text_bounds`, `line_height`, `display_map`). The page is the drop target (`on_drop::<OverflowDrag>`). The workspace creates the view the first time the panel opens.

**Tech Stack:** Rust 1.98 (edition 2024), `gpui-pre =0.3.10` (imported as `gpui`), `gpui-component 0.8.1` (`input::{Textarea, TextareaState, InputEvent}`), plus the existing `tulis-core` dependencies. No new crates; `chrono` is dropped from both crates.

**Spec:** `docs/superpowers/specs/2026-10-29-tulis-redesign-design.md`, section C, the keymap table and Testing. Builds on parts 1 and 2 (`docs/superpowers/plans/2026-10-29-tulis-redesign-1-foundation-chrome.md`, `docs/superpowers/plans/2026-10-29-tulis-redesign-2-alternates-sidebar.md`). Read the spec section before starting a task.

**How the code is given.** The one new file (`scratchpad.rs`, Task 4) is given in full. Changes to existing files are unified diffs against the state the previous step leaves (3 lines of context, as `git diff` prints them). Apply them by hand or with `patch -p1`. Each task's Step 1 shows the test hunks and Step 3 the implementation hunks. Every step was run in a scratch copy of the repository, in this order. Step 1 diffs are already formatted by `cargo fmt`.

## Global Constraints

- Version control is **jj**. Commit with `jj describe -m "<message>" && jj new`. Never run `git commit`.
- Every commit message ends with a blank line and then `Co-Authored-By: Claude <model> <noreply@anthropic.com>`, with your model name filled in (for example `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`).
- Run `cargo fmt --all` before every commit. The diffs below are already formatted.
- No new clippy warnings. `cargo clippy --workspace --all-targets` reports exactly: `tulis-core (lib)` 3 warnings, `tulis (bin "tulis")` 3 warnings and `tulis (bin "tulis" test)` 4 warnings (all from v1 code). No count may go up. Dead-code warnings count too, so don't add an item before the task that uses it.
- Every task ends with `cargo test --workspace` passing. The starting point (HEAD `cbd3aed`) has 137 app tests and 140 core tests. Part 3 ends with 150 and 145.
- Every app task ends with a launch check: `cargo build && timeout 6 ./target/debug/tulis /tmp/smoke.md; echo $?` prints `124`, with nothing on stderr. (Create `/tmp/smoke.md` with a couple of paragraphs first.)
- `tulis-core` must not depend on `gpui`.
- Overflow positions are byte offsets into `doc.overflow`, always on char boundaries. Page positions are byte offsets into a paragraph's rendered text, as before.
- Editor tests (`editor/tests.rs`) don't initialize gpui-component. Anything that renders the scratchpad is tested from `workspace.rs` tests through the `root_workspace` helper from part 2.
- Don't search `.devenv` or `.direnv`.
- **gpui-component facts** (checked against `~/.cargo/registry/src/*/gpui-component-0.8.1` and `gpui-base-0.8.1`):
  - The multi-line field is `gpui_component::input::{Textarea, TextareaState}` (`gpui-base src/input/textarea/mod.rs`; `TextareaState = InputBaseState<TextareaMode>`). `InputState` is single-line. `TextareaState::new(window, cx)` (`gpui-base src/input/base/state.rs:12194`), `.placeholder(..)` (:1052). `Textarea::new(&state).appearance(false).h(relative(1.))` (`gpui-component src/input/textarea.rs`).
  - `value() -> SharedString` (:1593). `set_value(text, window, cx)` (:1226) clears the field's undo history and emits **no** `InputEvent::Change`. Typing emits `Change`.
  - `selected_range() -> Range<usize>` (:3480) is a byte range; it's empty at the cursor. `text_bounds() -> Option<Bounds>` (:739) is the text's origin, scroll offset included. `input_bounds()` (:735) is the unscrolled viewport. `line_height() -> Option<Pixels>` and `display_map().buffer_line_to_display_row_range(line)` give wrapped rows.
  - Key bindings (`state.rs:141-316`): the field's key context is `Input`. It binds `ctrl-enter` (`cmd-enter` on macOS) to `Enter { secondary: true }`, which inserts a line break in a multi-line field and *then* emits `PressEnter`, so a `PressEnter` subscriber is too late. `ctrl-shift-enter` is not bound. `escape` propagates when there is nothing to dismiss (:2454). `ctrl-z`/`ctrl-y`/`cmd-z` are the field's own undo/redo.
- **gpui-pre facts** (checked against `~/.cargo/registry/src/*/gpui-pre-0.3.10`):
  - Key precedence (`src/keymap.rs:150-260`): deeper context first; at the same depth, the binding added later wins. A `Parent > Child` predicate matches at the child's depth (`src/keymap/context.rs:303`). Bindings without a context count as the deepest. `keymap::bind` runs after `gpui_component::init`, so table bindings come later.
  - Drag and drop (`src/elements/div.rs`): `StatefulInteractiveElement::on_drag(value: T, Fn(&T, Point<Pixels>, &mut Window, &mut App) -> Entity<W>)` (:1667) starts a drag after a left-button move of more than 2px from a mouse-down on the element's hitbox (:3041). `InteractiveElement::on_drop::<T>(Fn(&T, &mut Window, &mut App))` fires on mouse-up over the element (:2962). The drop listener reads the position and modifiers from `window.mouse_position()` and `window.modifiers()`, which mouse-up events update (`src/window.rs:5669`). `Modifiers::secondary(&self, cx)` is Ctrl on Linux/Windows and Cmd on macOS.
  - `anchored().position(point)` places its child at window coordinates, absolutely positioned (`src/elements/anchored.rs`).
  - `Context::observe_in(&Entity, &Window, FnMut(&mut T, Entity<V>, &mut Window, &mut Context<T>))` (`src/app/context.rs:317`). `Styled::cursor_grab()`. `StatefulInteractiveElement::on_hover(Fn(&bool, &mut Window, &mut App))`.
  - Test helpers: `VisualTestContext::{simulate_keystrokes, simulate_input, simulate_mouse_move(point, button, modifiers), simulate_mouse_down, simulate_mouse_up, simulate_resize}` (`src/app/test_context.rs:935-1072`).
- Unknown API? Read those source files. Don't guess.

## File Map

```
README.md                               keys, vim line, smoke checklist (Task 8)
Cargo.lock, crates/*/Cargo.toml         chrono removed (Task 1)
crates/tulis-core/src/
  model.rs                              Document.overflow: String; OverflowItem, OverflowId and their validate check removed
  overflow.rs                           stash, stash_paragraphs, overflow_blocks, overflow_block_at, take_overflow
  storage.rs                            FORMAT_VERSION 2, migrate_v1
  history.rs                            History::for_each_state
  vim.rs                                VimCommand::FocusOverflow (`gX`)
crates/tulis/src/
  actions.rs                            + OverflowUse, OverflowCopy, FocusOverflow, FocusPage
  commands.rs                           overflow_use / overflow_copy / focus_overflow / scratchpad_escape rows
  main.rs                               + mod scratchpad;
  scratchpad.rs          (new)          Scratchpad view, OverflowDrag, grips, DragPreview;
                                        Workspace::{ensure_scratchpad, toggle_overflow_focus, scratchpad_focused}
  workspace.rs                          panel uses the scratchpad, toggle_overflow refocuses, page on_drop, tests
  editor/features.rs                    stash toast, stash_paragraphs, set_overflow, use_overflow,
                                        use_overflow_selection, drop_overflow; insert/remove_overflow removed
  editor/vim_exec.rs                    `Vgs` stashes whole paragraphs; `gX` dispatches FocusOverflow
  editor/tests.rs                       stash, Vgs, set_overflow, use/copy, drop tests
```

---

### Task 1: Overflow becomes a string in core

**Files:**
- Modify: `crates/tulis-core/src/overflow.rs`, `crates/tulis-core/src/model.rs`, `crates/tulis-core/src/storage.rs` (tests), `crates/tulis-core/Cargo.toml`, `crates/tulis/Cargo.toml`, `Cargo.lock`, `crates/tulis/src/editor/features.rs`, `crates/tulis/src/workspace.rs`, `crates/tulis/src/editor/tests.rs`

**Interfaces:**
- Consumes: `Document::{export_text_in, delete, new_paragraph}`.
- Produces:
  - `Document.overflow: String` (`OverflowItem`, `OverflowId`, `to_overflow`, `paragraph_to_overflow`, `insert_overflow`, `remove_overflow` are gone)
  - `Document::stash(&mut self, range: Range<DocPos>) -> bool`
  - `Document::stash_paragraphs(&mut self, paras: Range<usize>) -> bool`
  - `Document::overflow_blocks(&self) -> Vec<Range<usize>>`, `Document::overflow_block_at(&self, offset: usize) -> Option<Range<usize>>`
  - `Document::take_overflow(&mut self, range: Range<usize>) -> String`

A stash appends one block: the stashed lines without blank ones, trimmed, after a blank line. A block is a run of non-blank lines; its range leaves out the whitespace around it. `take_overflow` joins the line breaks on either side of the gap: a blank line stays one blank line, a single line break stays one, and none is left at either end. The app keeps working in this task: `send_to_overflow` calls `stash`, and the panel shows the text read-only until Task 4 replaces it. The sidecar still says version 1 until Task 2.

- [ ] **Step 1: Write the failing tests**

The old overflow tests are replaced. The model's validate test loses its overflow-id check, and the app tests read `doc.overflow` as a string.

```diff
diff --git a/crates/tulis-core/src/model.rs b/crates/tulis-core/src/model.rs
--- a/crates/tulis-core/src/model.rs
+++ b/crates/tulis-core/src/model.rs
@@ -443,17 +443,6 @@ mod tests {
             "line break",
         );
         check(&|d| d.next_id = d.paragraphs[1].id.0, "next_id");
-        check(
-            &|d| {
-                d.overflow.push(OverflowItem {
-                    id: OverflowId(d.next_id),
-                    text: "x".into(),
-                    created_at: Utc::now(),
-                    from_paragraph: None,
-                })
-            },
-            "next_id",
-        );
     }
 
     #[test]
@@ -558,7 +547,8 @@ mod tests {
 
     #[test]
     fn document_round_trips_through_json() {
-        let d = Document::from_texts(&["hello"]);
+        let mut d = Document::from_texts(&["hello"]);
+        d.overflow = "stashed\n\nlines".into();
         let json = serde_json::to_string(&d).unwrap();
         let back: Document = serde_json::from_str(&json).unwrap();
         assert_eq!(back, d);
diff --git a/crates/tulis-core/src/overflow.rs b/crates/tulis-core/src/overflow.rs
--- a/crates/tulis-core/src/overflow.rs
+++ b/crates/tulis-core/src/overflow.rs
@@ -68,74 +68,112 @@ impl Document {
 
 #[cfg(test)]
 mod tests {
-    use chrono::Utc;
-
     use crate::model::Document;
     use crate::pos::DocPos;
 
+    fn blocks(d: &Document) -> Vec<&str> {
+        d.overflow_blocks()
+            .into_iter()
+            .map(|r| &d.overflow[r])
+            .collect()
+    }
+
     #[test]
-    fn selection_moves_to_overflow() {
-        let mut d = Document::from_texts(&["Keep this. Cut this."]);
-        let id = d
-            .to_overflow(DocPos::new(0, 10)..DocPos::new(0, 20), Utc::now())
-            .unwrap();
+    fn stash_appends_the_selection_after_a_blank_line() {
+        let mut d = Document::from_texts(&["Keep this. Cut this.", "More."]);
+        assert!(d.stash(DocPos::new(0, 10)..DocPos::new(0, 20)));
         assert_eq!(d.paragraphs[0].text(), "Keep this.");
-        assert_eq!(d.overflow.len(), 1);
-        assert_eq!(d.overflow[0].id, id);
-        assert_eq!(d.overflow[0].text, " Cut this.");
-        assert_eq!(d.overflow[0].from_paragraph, Some(d.paragraphs[0].id));
+        assert_eq!(d.overflow, "Cut this.");
+        d.overflow.push_str("\n\n\n");
+        assert!(d.stash(DocPos::new(1, 0)..DocPos::new(1, 5)));
+        assert_eq!(d.overflow, "Cut this.\n\nMore.");
     }
 
     #[test]
-    fn overflow_text_excludes_ghosts() {
-        let mut d = Document::from_texts(&["abc def"]);
+    fn stash_leaves_ghosts_out_and_ignores_whitespace() {
+        let mut d = Document::from_texts(&["abc def", "a  b"]);
         d.toggle_ghost(0, 0..4).unwrap();
-        d.to_overflow(DocPos::new(0, 0)..DocPos::new(0, 7), Utc::now())
-            .unwrap();
-        assert_eq!(d.overflow[0].text, "def");
+        assert!(d.stash(DocPos::new(0, 0)..DocPos::new(0, 7)));
+        assert_eq!(d.overflow, "def");
+        assert!(!d.stash(DocPos::new(1, 1)..DocPos::new(1, 3)));
+        assert_eq!(d.paragraphs[1].text(), "a  b");
+        assert_eq!(d.overflow, "def");
     }
 
     #[test]
-    fn whitespace_only_selection_is_not_stashed() {
-        let mut d = Document::from_texts(&["a  b"]);
-        assert!(
-            d.to_overflow(DocPos::new(0, 1)..DocPos::new(0, 3), Utc::now())
-                .is_none()
-        );
-        assert_eq!(d.paragraphs[0].text(), "a  b");
+    fn stash_across_paragraphs_is_one_block_with_line_breaks() {
+        let mut d = Document::from_texts(&["one two", "", "three four"]);
+        assert!(d.stash(DocPos::new(0, 4)..DocPos::new(2, 5)));
+        assert_eq!(d.paragraphs.len(), 1);
+        assert_eq!(d.paragraphs[0].text(), "one  four");
+        assert_eq!(d.overflow, "two\nthree");
+        assert_eq!(blocks(&d), vec!["two\nthree"]);
     }
 
     #[test]
-    fn paragraph_moves_to_overflow_and_document_never_empties() {
+    fn an_empty_range_stashes_its_paragraph_and_the_document_never_empties() {
         let mut d = Document::from_texts(&["one", "two"]);
-        d.paragraph_to_overflow(0, Utc::now()).unwrap();
+        assert!(d.stash(DocPos::new(0, 1)..DocPos::new(0, 1)));
         assert_eq!(d.paragraphs.len(), 1);
         assert_eq!(d.paragraphs[0].text(), "two");
-        d.paragraph_to_overflow(0, Utc::now()).unwrap();
+        assert!(d.stash_paragraphs(0..1));
         assert_eq!(d.paragraphs.len(), 1);
         assert_eq!(d.paragraphs[0].text(), "");
-        assert_eq!(d.overflow.len(), 2);
+        assert_eq!(d.overflow, "one\n\ntwo");
+        assert!(!d.stash(DocPos::new(0, 0)..DocPos::new(0, 0)));
+    }
+
+    #[test]
+    fn stash_paragraphs_removes_them_all() {
+        let mut d = Document::from_texts(&["a", "b", "c"]);
+        assert!(d.stash_paragraphs(0..2));
+        let texts: Vec<String> = d.paragraphs.iter().map(|p| p.text()).collect();
+        assert_eq!(texts, vec!["c"]);
+        assert_eq!(d.overflow, "a\nb");
+    }
+
+    #[test]
+    fn blocks_split_on_blank_lines() {
+        let mut d = Document::new();
+        d.overflow = "\n  first line\nsecond  \n \t\n\nthird\r\n\r\nfourth".into();
+        assert_eq!(blocks(&d), vec!["first line\nsecond", "third", "fourth"]);
+        assert_eq!(d.overflow_block_at(0), None);
+        assert_eq!(d.overflow_block_at(3), Some(3..20));
+        assert_eq!(d.overflow_block_at(20), Some(3..20));
+        let fourth = d.overflow.find("fourth").unwrap();
+        assert_eq!(
+            d.overflow_block_at(d.overflow.len()),
+            Some(fourth..d.overflow.len())
+        );
+        d.overflow.clear();
+        assert!(d.overflow_blocks().is_empty());
     }
 
     #[test]
-    fn insert_overflow_inserts_and_optionally_removes() {
-        let mut d = Document::from_texts(&["ab"]);
-        let id = d.paragraph_to_overflow(0, Utc::now()).unwrap();
-        let end = d.insert_overflow(id, DocPos::new(0, 0), false).unwrap();
-        assert_eq!(d.paragraphs[0].text(), "ab");
-        assert_eq!(end, DocPos::new(0, 2));
-        assert_eq!(d.overflow.len(), 1);
-        d.insert_overflow(id, end, true).unwrap();
-        assert_eq!(d.paragraphs[0].text(), "abab");
-        assert!(d.overflow.is_empty());
-        assert!(d.insert_overflow(id, end, true).is_none());
+    fn take_joins_the_blank_lines_around_the_gap() {
+        let take = |text: &str, what: &str| {
+            let mut d = Document::new();
+            d.overflow = text.into();
+            let start = text.find(what).unwrap();
+            let taken = d.take_overflow(start..start + what.len());
+            assert_eq!(taken, what);
+            d.overflow
+        };
+        assert_eq!(take("A\n\nB\n\nC", "B"), "A\n\nC");
+        assert_eq!(take("A\n\nB", "B"), "A");
+        assert_eq!(take("A\n\nB", "A"), "B");
+        assert_eq!(take("x\ny\nz", "y"), "x\nz");
+        assert_eq!(take("bar foo baz", "foo"), "bar  baz");
+        assert_eq!(take("bar foo\nbaz", "foo"), "bar \nbaz");
+        assert_eq!(take("A\n\n\nB\n  \nC", "B"), "A\n\nC");
+        assert_eq!(take("only", "only"), "");
     }
 
     #[test]
-    fn remove_overflow() {
-        let mut d = Document::from_texts(&["ab"]);
-        let id = d.paragraph_to_overflow(0, Utc::now()).unwrap();
-        assert!(d.remove_overflow(id));
-        assert!(!d.remove_overflow(id));
+    fn take_clamps_to_the_text_and_char_boundaries() {
+        let mut d = Document::new();
+        d.overflow = "café".into();
+        assert_eq!(d.take_overflow(4..99), "é");
+        assert_eq!(d.overflow, "caf");
     }
 }
diff --git a/crates/tulis-core/src/storage.rs b/crates/tulis-core/src/storage.rs
--- a/crates/tulis-core/src/storage.rs
+++ b/crates/tulis-core/src/storage.rs
@@ -215,8 +215,6 @@ pub fn reconcile(texts: &[String], mut sidecar: Document) -> (Document, usize) {
 mod tests {
     use std::fs;
 
-    use chrono::Utc;
-
     use super::*;
     use crate::pos::DocPos;
 
@@ -225,8 +223,7 @@ mod tests {
         d.make_alt(0, 2..6).unwrap();
         d.edit_alt(0, 1, 0..4, "adore");
         d.toggle_ghost(2, 0..9).unwrap();
-        d.to_overflow(DocPos::new(1, 6)..DocPos::new(1, 16), Utc::now())
-            .unwrap();
+        assert!(d.stash(DocPos::new(1, 6)..DocPos::new(1, 16)));
         d
     }
 
@@ -301,7 +298,7 @@ mod tests {
         assert!(doc.paragraphs[0].has_metadata());
         assert_eq!(doc.paragraphs[1].text(), "Something new.");
         assert_eq!(doc.paragraphs.len(), 2);
-        assert_eq!(doc.overflow.len(), 1);
+        assert_eq!(doc.overflow, "paragraph");
         assert_eq!(
             opened.notices,
             vec!["essay.md changed outside Tulis; 1 paragraphs lost alternates/ghosts".to_string()]
diff --git a/crates/tulis/src/editor/tests.rs b/crates/tulis/src/editor/tests.rs
--- a/crates/tulis/src/editor/tests.rs
+++ b/crates/tulis/src/editor/tests.rs
@@ -4,7 +4,6 @@ use tulis_core::{DocPos, Document, Selection};
 
 use super::{AltEdit, Editor, TrimSuggestion};
 use crate::actions::*;
-use tulis_core::OverflowId;
 
 pub(crate) fn editor_with<'a>(
     cx: &'a mut TestAppContext,
@@ -247,16 +246,20 @@ fn ghost_toggle_on_paragraph_and_selection(cx: &mut TestAppContext) {
 }
 
 #[gpui::test]
-fn overflow_round_trip(cx: &mut TestAppContext) {
+fn stashing_the_paragraph_appends_it_and_undo_takes_it_back(cx: &mut TestAppContext) {
     let (editor, cx) = editor_with(cx, &["a", "b"]);
+    editor.update(cx, |e, _| e.doc.overflow = "older".into());
     editor.update_in(cx, |e, window, cx| {
         e.send_to_overflow(&SendToOverflow, window, cx)
     });
     assert_eq!(texts(&editor, cx), vec!["b"]);
-    let id: OverflowId = editor.read_with(cx, |e, _| e.doc.overflow[0].id);
-    editor.update(cx, |e, cx| e.insert_overflow(id, true, cx));
-    assert_eq!(texts(&editor, cx), vec!["ab"]);
-    assert!(editor.read_with(cx, |e, _| e.doc.overflow.is_empty()));
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.doc.overflow.clone()),
+        "older\n\na"
+    );
+    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
+    assert_eq!(texts(&editor, cx), vec!["a", "b"]);
+    assert_eq!(editor.read_with(cx, |e, _| e.doc.overflow.clone()), "older");
 }
 
 #[gpui::test]
@@ -746,7 +749,7 @@ fn vim_visual_gs_stashes_the_selection_and_returns_to_normal(cx: &mut TestAppCon
     type_str(&editor, cx, "v$gs");
     assert_eq!(texts(&editor, cx), vec!["Keep this. "]);
     assert_eq!(
-        editor.read_with(cx, |e, _| e.doc.overflow[0].text.clone()),
+        editor.read_with(cx, |e, _| e.doc.overflow.clone()),
         "Stash that."
     );
     assert_eq!(
@@ -1150,10 +1153,7 @@ fn context_menu_commands_act_on_the_word_and_the_ghost_label_follows(cx: &mut Te
         e.send_to_overflow(&SendToOverflow, window, cx)
     });
     assert_eq!(texts(&editor, cx), vec!["I  verdant tea"]);
-    assert_eq!(
-        editor.read_with(cx, |e, _| e.doc.overflow[0].text.clone()),
-        "love"
-    );
+    assert_eq!(editor.read_with(cx, |e, _| e.doc.overflow.clone()), "love");
 }
 
 #[gpui::test]
@@ -1396,7 +1396,7 @@ fn visual_line_g_commands_act_on_whole_paragraphs(cx: &mut TestAppContext) {
     editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(1, 3)));
     type_str(&editor, cx, "Vgs");
     assert_eq!(
-        editor.read_with(cx, |e, _| e.doc.overflow[0].text.clone()),
+        editor.read_with(cx, |e, _| e.doc.overflow.clone()),
         "Stash this paragraph."
     );
     assert_eq!(
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -857,8 +857,8 @@ mod tests {
             })
         });
         cx.simulate_keystrokes("secondary-shift-x");
-        let stashed = ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.overflow.len());
-        assert_eq!(stashed, 1);
+        let stashed = ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.overflow.clone());
+        assert_eq!(stashed, "two");
     }
 
     /// A workspace in a `Root` window (tabs and inputs need it), with the page focused.
```

- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile errors in `tulis-core` such as ``can't compare `Vec<OverflowItem>` with `&str` `` and ``no method named `overflow_blocks` ``.

- [ ] **Step 3: Implement**

```diff
diff --git a/Cargo.lock b/Cargo.lock
--- a/Cargo.lock
+++ b/Cargo.lock
@@ -6736,7 +6736,6 @@ dependencies = [
 name = "tulis"
 version = "0.1.0"
 dependencies = [
- "chrono",
  "gpui-component",
  "gpui-kit-assets",
  "gpui-pre",
@@ -6750,7 +6749,6 @@ dependencies = [
 name = "tulis-core"
 version = "0.1.0"
 dependencies = [
- "chrono",
  "dirs 7.0.0",
  "hex",
  "serde",
diff --git a/crates/tulis-core/Cargo.toml b/crates/tulis-core/Cargo.toml
--- a/crates/tulis-core/Cargo.toml
+++ b/crates/tulis-core/Cargo.toml
@@ -5,7 +5,6 @@ version.workspace = true
 license.workspace = true
 
 [dependencies]
-chrono = { version = "0.4.45", features = ["serde"] }
 dirs = "7.0.0"
 hex = "0.4.3"
 serde = { version = "1.0.229", features = ["derive"] }
diff --git a/crates/tulis-core/src/model.rs b/crates/tulis-core/src/model.rs
--- a/crates/tulis-core/src/model.rs
+++ b/crates/tulis-core/src/model.rs
@@ -2,15 +2,11 @@
 
 use std::ops::Range;
 
-use chrono::{DateTime, Utc};
 use serde::{Deserialize, Serialize};
 
 #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
 pub struct ParaId(pub u64);
 
-#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
-pub struct OverflowId(pub u64);
-
 #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
 pub enum AiKind {
     Rewrite,
@@ -248,18 +244,11 @@ pub fn normalize_segments(segments: &mut Vec<Segment>) {
     *segments = out;
 }
 
-#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
-pub struct OverflowItem {
-    pub id: OverflowId,
-    pub text: String,
-    pub created_at: DateTime<Utc>,
-    pub from_paragraph: Option<ParaId>,
-}
-
 #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
 pub struct Document {
     pub paragraphs: Vec<Paragraph>,
-    pub overflow: Vec<OverflowItem>,
+    /// The Overflow scratchpad's text (blocks separated by blank lines).
+    pub overflow: String,
     next_id: u64,
 }
 
@@ -277,7 +266,7 @@ impl Document {
     pub fn from_texts<S: AsRef<str>>(texts: &[S]) -> Self {
         let mut doc = Self {
             paragraphs: Vec::new(),
-            overflow: Vec::new(),
+            overflow: String::new(),
             next_id: 1,
         };
         for text in texts {
@@ -367,9 +356,6 @@ impl Document {
                 }
             }
         }
-        if self.overflow.iter().any(|o| o.id.0 >= self.next_id) {
-            return Err("an overflow item has an id not below next_id".into());
-        }
         Ok(())
     }
 }
diff --git a/crates/tulis-core/src/overflow.rs b/crates/tulis-core/src/overflow.rs
--- a/crates/tulis-core/src/overflow.rs
+++ b/crates/tulis-core/src/overflow.rs
@@ -1,68 +1,154 @@
-//! The overflow panel's storage: text stashed outside the document.
+//! The Overflow scratchpad: free text kept outside the page, in blocks separated by blank lines.
 
 use std::ops::Range;
 
-use chrono::{DateTime, Utc};
-
-use crate::model::{Document, OverflowId, OverflowItem};
+use crate::model::Document;
 use crate::pos::DocPos;
 
+fn is_blank(line: &str) -> bool {
+    line.trim().is_empty()
+}
+
+/// The non-blank lines of `text`, trimmed at both ends and joined by `\n`: a stash is always
+/// one block.
+fn as_block(text: &str) -> String {
+    text.lines()
+        .filter(|line| !is_blank(line))
+        .collect::<Vec<_>>()
+        .join("\n")
+        .trim()
+        .to_string()
+}
+
+/// The whitespace at the end of `text` if it holds a line break: (its bytes, its line breaks).
+fn trailing_break(text: &str) -> (usize, usize) {
+    let tail = &text[text.trim_end().len()..];
+    match tail.matches('\n').count() {
+        0 => (0, 0),
+        breaks => (tail.len(), breaks),
+    }
+}
+
+/// The whitespace at the start of `text` if it holds a line break: (its bytes, its line breaks).
+fn leading_break(text: &str) -> (usize, usize) {
+    let head = &text[..text.len() - text.trim_start().len()];
+    match head.matches('\n').count() {
+        0 => (0, 0),
+        breaks => (head.len(), breaks),
+    }
+}
+
 impl Document {
-    /// Move the export text of `range` into overflow. Returns `None` if there is nothing to stash.
-    pub fn to_overflow(&mut self, range: Range<DocPos>, now: DateTime<Utc>) -> Option<OverflowId> {
-        let text = self.export_text_in(range.clone());
-        if text.trim().is_empty() {
-            return None;
+    /// Move the export text of `range` to the end of the scratchpad, after a blank line, as one
+    /// block. An empty range stashes (and removes) the paragraph it is in. Returns false, and
+    /// changes nothing, when there is only whitespace to stash.
+    pub fn stash(&mut self, range: Range<DocPos>) -> bool {
+        if range.start == range.end {
+            let para = range.start.para;
+            return self.stash_paragraphs(para..para + 1);
+        }
+        let text = as_block(&self.export_text_in(range.clone()));
+        if text.is_empty() {
+            return false;
         }
-        let from = self.paragraphs.get(range.start.para).map(|p| p.id);
         self.delete(range);
-        Some(self.push_overflow(text, now, from))
+        self.append_overflow(&text);
+        true
     }
 
-    /// Move a whole paragraph into overflow. The document always keeps at least one paragraph.
-    pub fn paragraph_to_overflow(&mut self, para: usize, now: DateTime<Utc>) -> Option<OverflowId> {
-        let text = self.paragraphs[para].export_text();
-        if text.trim().is_empty() {
-            return None;
+    /// Move whole paragraphs to the scratchpad as one block, removing them from the page. The
+    /// document always keeps at least one paragraph.
+    pub fn stash_paragraphs(&mut self, paras: Range<usize>) -> bool {
+        let paras = paras.start..paras.end.min(self.paragraphs.len());
+        let lines: Vec<String> = self.paragraphs[paras.clone()]
+            .iter()
+            .map(|p| p.export_text())
+            .collect();
+        let text = as_block(&lines.join("\n"));
+        if text.is_empty() {
+            return false;
         }
-        let removed = self.paragraphs.remove(para);
+        self.paragraphs.drain(paras);
         if self.paragraphs.is_empty() {
             let p = self.new_paragraph("");
             self.paragraphs.push(p);
         }
-        Some(self.push_overflow(text, now, Some(removed.id)))
-    }
-
-    fn push_overflow(
-        &mut self,
-        text: String,
-        now: DateTime<Utc>,
-        from: Option<crate::model::ParaId>,
-    ) -> OverflowId {
-        let id = OverflowId(self.fresh_id());
-        self.overflow.push(OverflowItem {
-            id,
-            text,
-            created_at: now,
-            from_paragraph: from,
-        });
-        id
-    }
-
-    /// Insert an overflow item's text at `pos`. Returns the position after the inserted text.
-    pub fn insert_overflow(&mut self, id: OverflowId, pos: DocPos, remove: bool) -> Option<DocPos> {
-        let ix = self.overflow.iter().position(|o| o.id == id)?;
-        let text = self.overflow[ix].text.clone();
-        if remove {
-            self.overflow.remove(ix);
+        self.append_overflow(&text);
+        true
+    }
+
+    fn append_overflow(&mut self, block: &str) {
+        let kept = self.overflow.trim_end().len();
+        self.overflow.truncate(kept);
+        if !self.overflow.is_empty() {
+            self.overflow.push_str("\n\n");
+        }
+        self.overflow.push_str(block);
+    }
+
+    /// The scratchpad's blocks: runs of non-blank lines, as byte ranges without the whitespace
+    /// around them.
+    pub fn overflow_blocks(&self) -> Vec<Range<usize>> {
+        let mut blocks = Vec::new();
+        let mut current: Option<Range<usize>> = None;
+        let mut start = 0;
+        for line in self.overflow.split_inclusive('\n') {
+            let content = line.trim_end_matches(['\n', '\r']);
+            if is_blank(content) {
+                blocks.extend(current.take());
+            } else {
+                let first = start + content.len() - content.trim_start().len();
+                let end = start + content.trim_end().len();
+                match &mut current {
+                    Some(block) => block.end = end,
+                    None => current = Some(first..end),
+                }
+            }
+            start += line.len();
         }
-        Some(self.insert(pos, &text))
+        blocks.extend(current);
+        blocks
+    }
+
+    /// The block containing byte `offset`, its edges included.
+    pub fn overflow_block_at(&self, offset: usize) -> Option<Range<usize>> {
+        self.overflow_blocks()
+            .into_iter()
+            .find(|b| b.start <= offset && offset <= b.end)
     }
 
-    pub fn remove_overflow(&mut self, id: OverflowId) -> bool {
-        let before = self.overflow.len();
-        self.overflow.retain(|o| o.id != id);
-        self.overflow.len() != before
+    /// Remove a byte range of the scratchpad and return its text. The line breaks on either side
+    /// are joined: a blank line stays one blank line, a line break stays one line break, and
+    /// none is left at either end of the scratchpad.
+    pub fn take_overflow(&mut self, range: Range<usize>) -> String {
+        let len = self.overflow.len();
+        let mut start = range.start.min(len);
+        let mut end = range.end.clamp(start, len);
+        while !self.overflow.is_char_boundary(start) {
+            start -= 1;
+        }
+        while !self.overflow.is_char_boundary(end) {
+            end += 1;
+        }
+        let taken = self.overflow[start..end].to_string();
+        let (before, after) = (&self.overflow[..start], &self.overflow[end..]);
+        let (cut_before, breaks_before) = trailing_break(before);
+        let (cut_after, breaks_after) = leading_break(after);
+        if breaks_before + breaks_after == 0 {
+            self.overflow.replace_range(start..end, "");
+            return taken;
+        }
+        let before = &before[..before.len() - cut_before];
+        let after = &after[cut_after..];
+        let separator = if before.is_empty() || after.is_empty() {
+            ""
+        } else if breaks_before.max(breaks_after) >= 2 {
+            "\n\n"
+        } else {
+            "\n"
+        };
+        self.overflow = format!("{before}{separator}{after}");
+        taken
     }
 }
 
diff --git a/crates/tulis/Cargo.toml b/crates/tulis/Cargo.toml
--- a/crates/tulis/Cargo.toml
+++ b/crates/tulis/Cargo.toml
@@ -5,7 +5,6 @@ version.workspace = true
 license.workspace = true
 
 [dependencies]
-chrono = "0.4.45"
 gpui = { package = "gpui-pre", version = "=0.3.10" }
 gpui-component = "0.8.1"
 gpui-kit-assets = "0.8.1"
diff --git a/crates/tulis/src/editor/features.rs b/crates/tulis/src/editor/features.rs
--- a/crates/tulis/src/editor/features.rs
+++ b/crates/tulis/src/editor/features.rs
@@ -6,7 +6,7 @@ use std::time::Instant;
 use gpui::{Context, Window};
 use tulis_core::alternates::{AltLevel, OpError};
 use tulis_core::history::EditKind;
-use tulis_core::{DocPos, OverflowId, Selection};
+use tulis_core::{DocPos, Selection};
 
 use super::{AltEdit, CycledGroup, Editor, TrimSuggestion};
 use crate::actions::*;
@@ -180,36 +180,16 @@ impl Editor {
             return self.set_notice("Nothing to move to overflow", cx);
         }
         self.begin_edit(EditKind::Other, sel.start.para..sel.end.para + 1);
-        let now = chrono::Utc::now();
-        if whole {
-            self.doc.paragraph_to_overflow(sel.start.para, now);
+        self.doc.stash(sel.clone());
+        self.selection = if whole {
             let para = sel.start.para.min(self.doc.paragraphs.len() - 1);
-            self.selection = Selection::caret(DocPos::new(para, 0));
+            Selection::caret(DocPos::new(para, 0))
         } else {
-            self.doc.to_overflow(sel.clone(), now);
-            self.selection = Selection::caret(sel.start);
-        }
-        self.end_edit(cx);
-    }
-
-    pub fn insert_overflow(&mut self, id: OverflowId, remove: bool, cx: &mut Context<Self>) {
-        self.finish_alt_edit();
-        let sel = self.selection.range();
-        self.begin_edit(EditKind::Other, sel.start.para..sel.end.para + 1);
-        let start = self.doc.delete(sel);
-        if let Some(end) = self.doc.insert_overflow(id, start, remove) {
-            self.selection = Selection::caret(end);
-        }
+            Selection::caret(sel.start)
+        };
         self.end_edit(cx);
     }
 
-    pub fn remove_overflow(&mut self, id: OverflowId, cx: &mut Context<Self>) {
-        self.begin_edit(EditKind::Other, 0..0);
-        if self.doc.remove_overflow(id) {
-            self.end_edit(cx);
-        }
-    }
-
     /// Accept the trim suggestion under the cursor (or the next one, or the first) by ghosting it.
     pub(super) fn accept_trim(&mut self, _: &AcceptTrim, _: &mut Window, cx: &mut Context<Self>) {
         if self.trims.is_empty() {
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -3,15 +3,15 @@
 use std::path::{Path, PathBuf};
 
 use gpui::{
-    AnyElement, App, ClickEvent, Context, Entity, FocusHandle, Focusable, Hsla, MouseMoveEvent,
+    AnyElement, App, Context, Entity, FocusHandle, Focusable, Hsla, MouseMoveEvent,
     PathPromptOptions, PromptLevel, SharedString, Subscription, Task, Window, div,
     linear_color_stop, linear_gradient, prelude::*, px,
 };
 use gpui_component::menu::ContextMenuExt;
 use gpui_component::text::TextView;
+use tulis_core::Document;
 use tulis_core::config::Config;
 use tulis_core::storage;
-use tulis_core::{Document, OverflowId};
 
 use crate::actions::{
     AiAlternates, CycleTheme, New, Open, OpenAlternates, OpenLab, Quit, Save, SaveAs,
@@ -25,7 +25,6 @@ pub struct Workspace {
     pub editor: Entity<Editor>,
     pub path: Option<PathBuf>,
     pub(crate) show_overflow: bool,
-    pub(crate) keep_in_overflow: bool,
     pub(crate) banner: Option<String>,
     pub(crate) cheat_sheet: Option<Entity<crate::cheat_sheet::CheatSheet>>,
     /// The alternates sidebar's view, created the first time it opens.
@@ -71,7 +70,6 @@ impl Workspace {
             editor,
             path: None,
             show_overflow: false,
-            keep_in_overflow: false,
             banner: None,
             cheat_sheet: None,
             alt_sidebar: None,
@@ -130,15 +128,6 @@ impl Workspace {
     }
 }
 
-fn preview(text: &str) -> String {
-    const MAX: usize = 240;
-    if text.chars().count() <= MAX {
-        text.to_string()
-    } else {
-        format!("{}…", text.chars().take(MAX).collect::<String>())
-    }
-}
-
 impl Workspace {
     pub(crate) fn toggle_markers(
         &mut self,
@@ -167,17 +156,10 @@ impl Workspace {
         cx.notify();
     }
 
-    fn insert_overflow(&mut self, id: OverflowId, window: &mut Window, cx: &mut Context<Self>) {
-        let remove = !self.keep_in_overflow;
-        self.editor
-            .update(cx, |e, cx| e.insert_overflow(id, remove, cx));
-        self.focus_editor(window, cx);
-    }
-
+    /// The Overflow panel: the scratchpad's text (read-only until the scratchpad view lands).
     fn render_overflow(&self, cx: &mut Context<Self>) -> impl IntoElement {
         let t = self.editor.read(cx).theme;
-        let items = self.editor.read(cx).doc.overflow.clone();
-        let keep = self.keep_in_overflow;
+        let text = self.editor.read(cx).doc.overflow.clone();
         div()
             .w(px(280.))
             .h_full()
@@ -186,75 +168,15 @@ impl Workspace {
             .border_l_1()
             .border_color(t.rule)
             .bg(t.panel)
+            .child(div().px_3().py_2().text_sm().child("Overflow"))
             .child(
                 div()
-                    .px_3()
-                    .py_2()
-                    .flex()
-                    .justify_between()
-                    .text_sm()
-                    .child("Overflow")
-                    .child(
-                        div()
-                            .id("keep-toggle")
-                            .cursor_pointer()
-                            .text_xs()
-                            .text_color(t.text_muted)
-                            .child(if keep {
-                                "☑ keep after insert"
-                            } else {
-                                "☐ keep after insert"
-                            })
-                            .on_click(cx.listener(|ws, _: &ClickEvent, _, cx| {
-                                ws.keep_in_overflow = !ws.keep_in_overflow;
-                                cx.notify();
-                            })),
-                    ),
-            )
-            .child(
-                div()
-                    .id("overflow-list")
+                    .id("overflow-text")
                     .flex_1()
                     .overflow_y_scroll()
                     .px_3()
-                    .flex()
-                    .flex_col()
-                    .gap_2()
-                    .when(items.is_empty(), |list| {
-                        list.child(
-                            div()
-                                .text_xs()
-                                .text_color(t.text_muted)
-                                .child("Ctrl+Shift+X stashes the selection or paragraph here."),
-                        )
-                    })
-                    .children(items.into_iter().rev().map(|item| {
-                        let id = item.id;
-                        div()
-                            .id(("overflow", id.0))
-                            .p_2()
-                            .rounded_md()
-                            .bg(t.background)
-                            .cursor_pointer()
-                            .text_sm()
-                            .flex()
-                            .flex_row()
-                            .gap_2()
-                            .on_click(cx.listener(move |ws, _: &ClickEvent, window, cx| {
-                                ws.insert_overflow(id, window, cx)
-                            }))
-                            .child(div().flex_1().child(preview(&item.text)))
-                            .child(
-                                div()
-                                    .id(("overflow-remove", id.0))
-                                    .text_color(t.text_muted)
-                                    .child("×")
-                                    .on_click(cx.listener(move |ws, _: &ClickEvent, _, cx| {
-                                        cx.stop_propagation();
-                                        ws.editor.update(cx, |e, cx| e.remove_overflow(id, cx));
-                                    })),
-                            )
-                    })),
+                    .text_sm()
+                    .child(text),
             )
     }
 }
```

- [ ] **Step 4: Run the tests (expect success)**

Run: `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: 137 app tests and 142 core tests pass; clippy counts match the baseline.
Launch check: `cargo build && timeout 6 ./target/debug/tulis /tmp/smoke.md; echo $?` prints `124`.

- [ ] **Step 5: Commit**

```bash
jj describe -m "feat(core): overflow is one string with stash, blocks and take

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 2: Sidecar format version 2, with version 1 migration

**Files:**
- Modify: `crates/tulis-core/src/storage.rs`, `crates/tulis/src/workspace.rs` (test)

**Interfaces:**
- Consumes: `Document.overflow: String` (Task 1).
- Produces: `storage::FORMAT_VERSION == 2`; `read_sidecar` accepts versions 1 and 2; private `migrate_v1(&mut serde_json::Value)`.

A version 1 sidecar's `document.overflow` is a list of items. Before the document is parsed, the item texts are joined, in order, with `"\n\n"` and put back as a string. `Document::validate` then runs as for version 2. The next save writes version 2. The app test edits the pretty-printed sidecar text instead of using `serde_json`, which the app crate doesn't depend on.

- [ ] **Step 1: Write the failing tests**

```diff
diff --git a/crates/tulis-core/src/storage.rs b/crates/tulis-core/src/storage.rs
--- a/crates/tulis-core/src/storage.rs
+++ b/crates/tulis-core/src/storage.rs
@@ -338,6 +338,57 @@ mod tests {
         );
     }
 
+    /// Rewrite a saved sidecar as version 1, whose overflow was a list of items.
+    fn downgrade_to_v1(md: &Path, items: &[&str]) {
+        let side = sidecar_path(md);
+        let mut json: serde_json::Value =
+            serde_json::from_slice(&fs::read(&side).unwrap()).unwrap();
+        json["format_version"] = 1.into();
+        json["document"]["overflow"] = items
+            .iter()
+            .enumerate()
+            .map(|(i, text)| {
+                serde_json::json!({
+                    "id": 90 + i,
+                    "text": text,
+                    "created_at": "2026-10-28T09:00:00Z",
+                    "from_paragraph": null,
+                })
+            })
+            .collect();
+        fs::write(&side, serde_json::to_vec(&json).unwrap()).unwrap();
+    }
+
+    #[test]
+    fn version_1_sidecar_overflow_items_are_joined_and_saved_as_version_2() {
+        let dir = tempfile::tempdir().unwrap();
+        let path = dir.path().join("essay.md");
+        let doc = rich_doc();
+        save(&doc, &path).unwrap();
+        downgrade_to_v1(&path, &["first", "second\nline"]);
+        let opened = open(&path).unwrap();
+        assert!(opened.notices.is_empty(), "{:?}", opened.notices);
+        assert_eq!(opened.document.overflow, "first\n\nsecond\nline");
+        assert_eq!(opened.document.paragraphs, doc.paragraphs);
+        save(&opened.document, &path).unwrap();
+        let json: serde_json::Value =
+            serde_json::from_slice(&fs::read(sidecar_path(&path)).unwrap()).unwrap();
+        assert_eq!(json["format_version"], 2);
+        assert_eq!(json["document"]["overflow"], "first\n\nsecond\nline");
+        assert_eq!(open(&path).unwrap().document, opened.document);
+    }
+
+    #[test]
+    fn version_1_sidecar_without_overflow_items_opens_empty() {
+        let dir = tempfile::tempdir().unwrap();
+        let path = dir.path().join("essay.md");
+        save(&Document::from_texts(&["Text."]), &path).unwrap();
+        downgrade_to_v1(&path, &[]);
+        let opened = open(&path).unwrap();
+        assert!(opened.notices.is_empty(), "{:?}", opened.notices);
+        assert_eq!(opened.document.overflow, "");
+    }
+
     #[test]
     fn unknown_sidecar_version_is_ignored_with_notice() {
         let dir = tempfile::tempdir().unwrap();
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -668,6 +668,38 @@ mod tests {
         ws.update_in(cx, |ws, _, cx| assert!(!ws.save_now(cx)));
     }
 
+    #[gpui::test]
+    fn opening_a_version_1_sidecar_joins_its_overflow_and_saves_version_2(cx: &mut TestAppContext) {
+        init(cx);
+        let dir = tempfile::tempdir().unwrap();
+        let path = dir.path().join("essay.md");
+        tulis_core::storage::save(&tulis_core::Document::from_texts(&["Hello."]), &path).unwrap();
+        // Today's sidecar, rewritten in the version 1 format: overflow was a list of items.
+        let side = tulis_core::storage::sidecar_path(&path);
+        let items = r#"[
+            {"id": 7, "text": "Old idea.", "created_at": "2026-10-28T09:00:00Z", "from_paragraph": null},
+            {"id": 8, "text": "Another one.", "created_at": "2026-10-28T09:05:00Z", "from_paragraph": null}
+        ]"#;
+        let v1 = std::fs::read_to_string(&side)
+            .unwrap()
+            .replace(r#""format_version": 2"#, r#""format_version": 1"#)
+            .replace(r#""overflow": """#, &format!(r#""overflow": {items}"#));
+        std::fs::write(&side, v1).unwrap();
+        let open_path = path.clone();
+        let (ws, cx) = cx.add_window_view(move |window, cx| {
+            Workspace::new(Config::default(), None, Some(open_path), window, cx)
+        });
+        let overflow = ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.overflow.clone());
+        assert_eq!(overflow, "Old idea.\n\nAnother one.");
+        ws.update_in(cx, |ws, _, cx| assert!(ws.save_now(cx)));
+        let saved = std::fs::read_to_string(&side).unwrap();
+        assert!(saved.contains(r#""format_version": 2"#), "{saved}");
+        assert!(
+            saved.contains(r#""overflow": "Old idea.\n\nAnother one.""#),
+            "{saved}"
+        );
+    }
+
     /// gpui-component widgets need their globals; some tests also press keys.
     fn init(cx: &mut TestAppContext) {
         cx.update(|cx| {
```

- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace --no-fail-fast`
Expected: three failures: `version_1_sidecar_overflow_items_are_joined_and_saved_as_version_2`, `version_1_sidecar_without_overflow_items_opens_empty` (the sidecar is ignored with a notice) and `opening_a_version_1_sidecar_joins_its_overflow_and_saves_version_2`.

- [ ] **Step 3: Implement**

```diff
diff --git a/crates/tulis-core/src/storage.rs b/crates/tulis-core/src/storage.rs
--- a/crates/tulis-core/src/storage.rs
+++ b/crates/tulis-core/src/storage.rs
@@ -9,7 +9,8 @@ use sha2::{Digest, Sha256};
 
 use crate::model::Document;
 
-pub const FORMAT_VERSION: u32 = 1;
+/// Version 2 keeps the Overflow scratchpad as one string; version 1 kept a list of items.
+pub const FORMAT_VERSION: u32 = 2;
 
 #[derive(Debug, thiserror::Error)]
 pub enum StorageError {
@@ -149,8 +150,8 @@ fn read_sidecar(path: &Path, notices: &mut Vec<String>) -> Option<SidecarIn> {
             return None;
         }
     };
-    match serde_json::from_slice::<SidecarHeader>(&bytes) {
-        Ok(header) if header.format_version == FORMAT_VERSION => {}
+    let version = match serde_json::from_slice::<SidecarHeader>(&bytes) {
+        Ok(header) if matches!(header.format_version, 1 | FORMAT_VERSION) => header.format_version,
         Ok(header) => {
             notices.push(format!(
                 "Ignoring sidecar with unknown format version {}",
@@ -162,8 +163,16 @@ fn read_sidecar(path: &Path, notices: &mut Vec<String>) -> Option<SidecarIn> {
             notices.push(format!("Ignoring unreadable sidecar: {e}"));
             return None;
         }
-    }
-    match serde_json::from_slice::<SidecarIn>(&bytes) {
+    };
+    let parsed = if version == 1 {
+        serde_json::from_slice::<serde_json::Value>(&bytes).and_then(|mut json| {
+            migrate_v1(&mut json);
+            serde_json::from_value::<SidecarIn>(json)
+        })
+    } else {
+        serde_json::from_slice::<SidecarIn>(&bytes)
+    };
+    match parsed {
         Ok(sidecar) => match sidecar.document.validate() {
             Ok(()) => Some(sidecar),
             Err(reason) => {
@@ -178,6 +187,29 @@ fn read_sidecar(path: &Path, notices: &mut Vec<String>) -> Option<SidecarIn> {
     }
 }
 
+/// A version 1 sidecar's overflow items become the scratchpad's text: their texts, in order,
+/// joined by blank lines.
+fn migrate_v1(sidecar: &mut serde_json::Value) {
+    let Some(document) = sidecar
+        .get_mut("document")
+        .and_then(serde_json::Value::as_object_mut)
+    else {
+        return;
+    };
+    let texts: Vec<&str> = document
+        .get("overflow")
+        .and_then(serde_json::Value::as_array)
+        .map(|items| {
+            items
+                .iter()
+                .filter_map(|item| item.get("text").and_then(serde_json::Value::as_str))
+                .collect()
+        })
+        .unwrap_or_default();
+    let joined = texts.join("\n\n");
+    document.insert("overflow".into(), joined.into());
+}
+
 /// Rebuild a document from Markdown paragraphs, reusing sidecar paragraphs whose export text
 /// matches exactly (each at most once). Overflow is kept. Returns the document and the number of
 /// sidecar paragraphs with alternates/ghosts that could not be matched.
```

- [ ] **Step 4: Run the tests (expect success)**

Run: `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: 138 app tests and 144 core tests pass; clippy counts match the baseline. Launch check as in Task 1.

- [ ] **Step 5: Commit**

```bash
jj describe -m "feat(storage): sidecar version 2; version 1 overflow items are joined on open

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 3: Stash toasts, and `Vgs` removes the paragraphs

**Files:**
- Modify: `crates/tulis/src/editor/features.rs`, `crates/tulis/src/editor/vim_exec.rs`, `crates/tulis/src/editor/tests.rs`, `crates/tulis/src/workspace.rs` (test)

**Interfaces:**
- Consumes: `Document::{stash, stash_paragraphs}` (Task 1).
- Produces: `Editor::stash_paragraphs(&mut self, paras: Range<usize>, cx)`; every successful stash toasts "Stashed to Overflow" and leaves the panel as it is.

The bug: in VisualLine mode, `select_whole_paragraphs` selects each paragraph's text, so `Vgs` stashed the text and left an empty paragraph, while `Vd` removed it. `vim_key` now sends `Stash` from VisualLine mode to `stash_paragraphs`, which removes them. An empty selection (Ctrl+Shift+X on a caret) uses the same method for the caret's paragraph.

- [ ] **Step 1: Write the failing tests**

```diff
diff --git a/crates/tulis/src/editor/tests.rs b/crates/tulis/src/editor/tests.rs
--- a/crates/tulis/src/editor/tests.rs
+++ b/crates/tulis/src/editor/tests.rs
@@ -257,6 +257,10 @@ fn stashing_the_paragraph_appends_it_and_undo_takes_it_back(cx: &mut TestAppCont
         editor.read_with(cx, |e, _| e.doc.overflow.clone()),
         "older\n\na"
     );
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.last_notice().map(str::to_string)),
+        Some("Stashed to Overflow".into())
+    );
     editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
     assert_eq!(texts(&editor, cx), vec!["a", "b"]);
     assert_eq!(editor.read_with(cx, |e, _| e.doc.overflow.clone()), "older");
@@ -1399,6 +1403,8 @@ fn visual_line_g_commands_act_on_whole_paragraphs(cx: &mut TestAppContext) {
         editor.read_with(cx, |e, _| e.doc.overflow.clone()),
         "Stash this paragraph."
     );
+    // Like `Vd`, the paragraph goes too.
+    assert_eq!(texts(&editor, cx), vec!["Keep."]);
     assert_eq!(
         editor.read_with(cx, |e, _| e.vim_mode()),
         Some(Mode::Normal)
@@ -1418,3 +1424,31 @@ fn visual_line_g_commands_act_on_whole_paragraphs(cx: &mut TestAppContext) {
     let session = editor.read_with(cx, |e, _| e.alt_session.clone()).unwrap();
     assert_eq!((session.selection, session.range), (true, 0..10));
 }
+
+#[gpui::test]
+fn visual_line_stash_removes_the_paragraphs_like_visual_line_delete(cx: &mut TestAppContext) {
+    let (editor, cx) = vim_editor(cx, &["One.", "Two.", "Three."]);
+    type_str(&editor, cx, "Vjd");
+    let deleted = texts(&editor, cx);
+    assert_eq!(deleted, vec!["Three."]);
+
+    let (editor, cx) = vim_editor(cx, &["One.", "Two.", "Three."]);
+    type_str(&editor, cx, "Vjgs");
+    assert_eq!(texts(&editor, cx), deleted);
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.doc.overflow.clone()),
+        "One.\nTwo."
+    );
+    assert_eq!(
+        editor.read_with(cx, |e, _| (e.cursor(), e.vim_mode())),
+        (DocPos::new(0, 0), Some(Mode::Normal))
+    );
+    type_str(&editor, cx, "u");
+    assert_eq!(texts(&editor, cx), vec!["One.", "Two.", "Three."]);
+    assert_eq!(editor.read_with(cx, |e, _| e.doc.overflow.clone()), "");
+
+    // The last paragraph: the page keeps one empty paragraph.
+    let (editor, cx) = vim_editor(cx, &["Only."]);
+    type_str(&editor, cx, "Vgs");
+    assert_eq!(texts(&editor, cx), vec![""]);
+}
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -813,6 +813,8 @@ mod tests {
         cx.simulate_keystrokes("secondary-shift-x");
         let stashed = ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.overflow.clone());
         assert_eq!(stashed, "two");
+        // Stashing doesn't open the panel.
+        assert!(!ws.read_with(cx, |ws, _| ws.show_overflow));
     }
 
     /// A workspace in a `Root` window (tabs and inputs need it), with the page focused.
```

- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: 3 failures: `stashing_the_paragraph_appends_it_and_undo_takes_it_back` (no toast), `visual_line_g_commands_act_on_whole_paragraphs` and `visual_line_stash_removes_the_paragraphs_like_visual_line_delete` (an empty paragraph is left behind).

- [ ] **Step 3: Implement**

```diff
diff --git a/crates/tulis/src/editor/features.rs b/crates/tulis/src/editor/features.rs
--- a/crates/tulis/src/editor/features.rs
+++ b/crates/tulis/src/editor/features.rs
@@ -11,6 +11,9 @@ use tulis_core::{DocPos, Selection};
 use super::{AltEdit, CycledGroup, Editor, TrimSuggestion};
 use crate::actions::*;
 
+const STASHED: &str = "Stashed to Overflow";
+const NOTHING_TO_STASH: &str = "Nothing to move to overflow";
+
 impl Editor {
     /// The selection within one paragraph, or the cursor's whole paragraph when nothing is selected.
     pub(crate) fn target_range(&self) -> Result<(usize, Range<usize>), OpError> {
@@ -162,6 +165,8 @@ impl Editor {
         self.toggle_ghost_here(cx);
     }
 
+    /// Ctrl+Shift+X, `gs`, the menu: stash the selection, or the cursor's paragraph, in
+    /// Overflow. The panel stays as it is.
     pub(super) fn send_to_overflow(
         &mut self,
         _: &SendToOverflow,
@@ -170,24 +175,34 @@ impl Editor {
     ) {
         self.finish_alt_edit();
         let sel = self.selection.range();
-        let whole = sel.start == sel.end;
-        let text = if whole {
-            self.doc.paragraphs[sel.start.para].export_text()
-        } else {
-            self.doc.export_text_in(sel.clone())
-        };
-        if text.trim().is_empty() {
-            return self.set_notice("Nothing to move to overflow", cx);
+        if sel.start == sel.end {
+            return self.stash_paragraphs(sel.start.para..sel.start.para + 1, cx);
+        }
+        if self.doc.export_text_in(sel.clone()).trim().is_empty() {
+            return self.set_notice(NOTHING_TO_STASH, cx);
         }
         self.begin_edit(EditKind::Other, sel.start.para..sel.end.para + 1);
         self.doc.stash(sel.clone());
-        self.selection = if whole {
-            let para = sel.start.para.min(self.doc.paragraphs.len() - 1);
-            Selection::caret(DocPos::new(para, 0))
-        } else {
-            Selection::caret(sel.start)
-        };
+        self.selection = Selection::caret(sel.start);
+        self.end_edit(cx);
+        self.set_notice(STASHED, cx);
+    }
+
+    /// Stash whole paragraphs, removing them from the page (no selection, or vim `Vgs`).
+    pub(crate) fn stash_paragraphs(&mut self, paras: Range<usize>, cx: &mut Context<Self>) {
+        self.finish_alt_edit();
+        let blank = self.doc.paragraphs[paras.clone()]
+            .iter()
+            .all(|p| p.export_text().trim().is_empty());
+        if blank {
+            return self.set_notice(NOTHING_TO_STASH, cx);
+        }
+        self.begin_edit(EditKind::Other, paras.clone());
+        self.doc.stash_paragraphs(paras.clone());
+        let para = paras.start.min(self.doc.paragraphs.len() - 1);
+        self.selection = Selection::caret(DocPos::new(para, 0));
         self.end_edit(cx);
+        self.set_notice(STASHED, cx);
     }
 
     /// Accept the trim suggestion under the cursor (or the next one, or the first) by ghosting it.
diff --git a/crates/tulis/src/editor/vim_exec.rs b/crates/tulis/src/editor/vim_exec.rs
--- a/crates/tulis/src/editor/vim_exec.rs
+++ b/crates/tulis/src/editor/vim_exec.rs
@@ -31,6 +31,12 @@ impl Editor {
         let was_line = vim.mode() == Mode::VisualLine;
         let commands = vim.feed(key);
         for command in commands {
+            if was_line && command == VimCommand::Stash {
+                // Whole paragraphs leave the page, as with `Vd`.
+                let range = self.selection.range();
+                self.stash_paragraphs(range.start.para..range.end.para + 1, cx);
+                continue;
+            }
             if was_line && is_selection_command(command) {
                 self.select_whole_paragraphs();
             } else if was_visual && is_selection_command(command) {
```

- [ ] **Step 4: Run the tests (expect success)**

Run: `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: 139 app tests and 144 core tests pass; clippy counts match the baseline. Launch check as in Task 1.

- [ ] **Step 5: Commit**

```bash
jj describe -m "fix: stash toasts \"Stashed to Overflow\"; Vgs removes the paragraphs like Vd

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 4: The scratchpad panel

**Files:**
- Create: `crates/tulis/src/scratchpad.rs`
- Modify: `crates/tulis-core/src/history.rs`, `crates/tulis/src/editor/features.rs`, `crates/tulis/src/main.rs`, `crates/tulis/src/workspace.rs`, `crates/tulis/src/editor/tests.rs`

**Interfaces:**
- Consumes: `Document.overflow` (Task 1).
- Produces:
  - `History::<S>::for_each_state(&mut self, f: impl FnMut(&mut S))`
  - `Editor::set_overflow(&mut self, text: &str, cx)`
  - `scratchpad::{Scratchpad, SCRATCHPAD_WIDTH = 260.}`, `Scratchpad::new(Entity<Editor>, &mut Window, &mut Context<Self>)`, `Scratchpad.input: Entity<TextareaState>`, `Scratchpad::has_focus(&self, &Window, &App) -> bool`
  - `Workspace.scratchpad: Option<Entity<Scratchpad>>`, `Workspace::ensure_scratchpad(&mut self, &mut Window, &mut Context<Self>) -> Entity<Scratchpad>`, `Workspace::scratchpad_focused(&self, &Window, &App) -> bool`
  - Test helpers in `workspace.rs`: `scratchpad_value`, `focus_scratchpad`, `page_focused`

Page undo snapshots hold the whole `Document`, overflow included. That is what lets one Ctrl+Z undo a stash or a Ctrl+Enter on both sides. Typing in the scratchpad must not be lost to a later page undo, so `set_overflow` writes the new text into every stored step. The view has two sync paths. A field `Change` calls `set_overflow`. Anything else that changes `doc.overflow` notifies the editor, and the observer calls `set_value` if the field shows something else. `set_value` emits no `Change`, so the two paths don't loop. Hiding the panel while the field has focus gives the page focus back.

- [ ] **Step 1: Write the failing tests**

```diff
diff --git a/crates/tulis-core/src/history.rs b/crates/tulis-core/src/history.rs
--- a/crates/tulis-core/src/history.rs
+++ b/crates/tulis-core/src/history.rs
@@ -180,6 +180,19 @@ mod tests {
         assert_ne!(h.newest_undo_id(), typing);
     }
 
+    #[test]
+    fn for_each_state_rewrites_undo_and_redo_steps() {
+        let mut h = History::new(10);
+        let t0 = Instant::now();
+        h.record(EditKind::Other, t0, || s("a"));
+        h.record(EditKind::Other, t0, || s("b"));
+        assert_eq!(h.undo(s("c")), Some(s("b")));
+        h.for_each_state(|state| state.push('!'));
+        assert_eq!(h.redo(s("b")), Some(s("c!")));
+        assert_eq!(h.undo(s("c!")), Some(s("b")));
+        assert_eq!(h.undo(s("b")), Some(s("a!")));
+    }
+
     #[test]
     fn limit_drops_oldest_steps() {
         let mut h = History::new(2);
diff --git a/crates/tulis/src/editor/tests.rs b/crates/tulis/src/editor/tests.rs
--- a/crates/tulis/src/editor/tests.rs
+++ b/crates/tulis/src/editor/tests.rs
@@ -266,6 +266,24 @@ fn stashing_the_paragraph_appends_it_and_undo_takes_it_back(cx: &mut TestAppCont
     assert_eq!(editor.read_with(cx, |e, _| e.doc.overflow.clone()), "older");
 }
 
+#[gpui::test]
+fn scratchpad_edits_mark_the_document_dirty_but_page_undo_keeps_them(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["Page."]);
+    type_str(&editor, cx, "x");
+    editor.update(cx, |e, cx| {
+        e.dirty = false;
+        e.set_overflow("An idea.", cx);
+        assert!(e.dirty);
+    });
+    let overflow = |cx: &mut VisualTestContext| editor.read_with(cx, |e, _| e.doc.overflow.clone());
+    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
+    assert_eq!(texts(&editor, cx), vec!["Page."]);
+    assert_eq!(overflow(cx), "An idea.");
+    editor.update_in(cx, |e, window, cx| e.redo(&Redo, window, cx));
+    assert_eq!(texts(&editor, cx), vec!["xPage."]);
+    assert_eq!(overflow(cx), "An idea.");
+}
+
 #[gpui::test]
 fn accepting_a_trim_ghosts_it_and_escape_dismisses(cx: &mut TestAppContext) {
     let (editor, cx) = editor_with(cx, &["I really love tea, really."]);
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -838,6 +838,58 @@ mod tests {
         (ws, cx)
     }
 
+    fn scratchpad_value(ws: &gpui::Entity<Workspace>, cx: &mut gpui::VisualTestContext) -> String {
+        ws.read_with(cx, |ws, cx| {
+            let pad = ws.scratchpad.as_ref().expect("the panel has opened");
+            pad.read(cx).input.read(cx).value().to_string()
+        })
+    }
+
+    fn focus_scratchpad(ws: &gpui::Entity<Workspace>, cx: &mut gpui::VisualTestContext) {
+        ws.update_in(cx, |ws, window, cx| {
+            let input = ws.scratchpad.as_ref().unwrap().read(cx).input.clone();
+            window.focus(&gpui::Focusable::focus_handle(&input, cx), cx);
+        });
+    }
+
+    fn page_focused(ws: &gpui::Entity<Workspace>, cx: &mut gpui::VisualTestContext) -> bool {
+        ws.update_in(cx, |ws, window, cx| {
+            gpui::Focusable::focus_handle(ws.editor.read(cx), cx).is_focused(window)
+        })
+    }
+
+    #[gpui::test]
+    fn the_scratchpad_edits_the_documents_overflow(cx: &mut TestAppContext) {
+        let (ws, cx) = root_workspace(cx, &["Page text."]);
+        ws.update(cx, |ws, cx| {
+            ws.editor
+                .update(cx, |e, _| e.doc.overflow = "First idea.".into())
+        });
+        cx.simulate_keystrokes("secondary-\\");
+        cx.run_until_parked();
+        assert!(ws.read_with(cx, |ws, _| ws.show_overflow));
+        assert_eq!(scratchpad_value(&ws, cx), "First idea.");
+        // Typing in it edits the document's overflow and marks the document dirty.
+        focus_scratchpad(&ws, cx);
+        cx.simulate_keystrokes("ctrl-end");
+        cx.simulate_input(" More.");
+        let (overflow, dirty) = ws.read_with(cx, |ws, cx| {
+            let e = ws.editor.read(cx);
+            (e.doc.overflow.clone(), e.dirty)
+        });
+        assert_eq!((overflow.as_str(), dirty), ("First idea. More.", true));
+        // A stash from the page shows up in it.
+        ws.update_in(cx, |ws, window, cx| ws.focus_editor(window, cx));
+        cx.simulate_keystrokes("secondary-shift-x");
+        cx.run_until_parked();
+        assert_eq!(scratchpad_value(&ws, cx), "First idea. More.\n\nPage text.");
+        // Hiding the panel while it has focus gives the page focus back.
+        focus_scratchpad(&ws, cx);
+        cx.simulate_keystrokes("secondary-\\");
+        cx.run_until_parked();
+        assert!(page_focused(&ws, cx));
+    }
+
     fn sidebar_focused(ws: &gpui::Entity<Workspace>, cx: &mut gpui::VisualTestContext) -> bool {
         let sidebar = ws.read_with(cx, |ws, _| ws.alt_sidebar.clone());
         sidebar.is_some_and(|s| cx.update(|window, cx| s.read(cx).list_focus.is_focused(window)))
```

- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile error in `tulis-core`: ``no method named `for_each_state` ``. (The app tests also need `set_overflow` and `Workspace.scratchpad`.)

- [ ] **Step 3: Implement**

New file `crates/tulis/src/scratchpad.rs`:

```rust
//! The Overflow scratchpad: a right panel with one multi-line text field bound to
//! `doc.overflow`.

use gpui::{
    App, Context, Entity, Focusable, IntoElement, ParentElement, Render, Styled, Subscription,
    Window, div, prelude::*, px, relative,
};
use gpui_component::input::{InputEvent, Textarea, TextareaState};

use crate::editor::Editor;
use crate::workspace::Workspace;

pub const SCRATCHPAD_WIDTH: f32 = 260.;

pub struct Scratchpad {
    editor: Entity<Editor>,
    /// The text field. Its value mirrors `doc.overflow`; it keeps its own undo history.
    pub(crate) input: Entity<TextareaState>,
    _subscriptions: Vec<Subscription>,
}

impl Scratchpad {
    pub fn new(editor: Entity<Editor>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Ctrl+Shift+X stashes the selection or paragraph here.")
        });
        let text = editor.read(cx).doc.overflow.clone();
        input.update(cx, |input, cx| input.set_value(text, window, cx));
        let subscriptions = vec![
            // Page commands (stash, undo, open) change `doc.overflow`: show the new text.
            cx.observe_in(&editor, window, |this, _, window, cx| this.pull(window, cx)),
            cx.subscribe_in(&input, window, |this, _, event: &InputEvent, _, cx| {
                if let InputEvent::Change = event {
                    this.push(cx);
                }
            }),
        ];
        Self {
            editor,
            input,
            _subscriptions: subscriptions,
        }
    }

    /// The field changed: store its text in the document.
    fn push(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).value();
        self.editor.update(cx, |e, cx| e.set_overflow(&text, cx));
    }

    /// The document's overflow changed elsewhere: show it. `set_value` emits no `Change`, and
    /// starts the field's undo history afresh.
    fn pull(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.editor.read(cx).doc.overflow.clone();
        if self.input.read(cx).value().as_ref() != text {
            self.input
                .update(cx, |input, cx| input.set_value(text, window, cx));
        }
        cx.notify();
    }

    pub(crate) fn has_focus(&self, window: &Window, cx: &App) -> bool {
        self.input.focus_handle(cx).is_focused(window)
    }
}

impl Render for Scratchpad {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.editor.read(cx).theme;
        div()
            .id("scratchpad")
            .key_context("Scratchpad")
            .w(px(SCRATCHPAD_WIDTH))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(t.rule)
            .bg(t.panel)
            .text_sm()
            .child(
                div()
                    .px_3()
                    .pt_2()
                    .pb_1()
                    .italic()
                    .text_color(t.accent)
                    .child("Overflow"),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .px_2()
                    .child(Textarea::new(&self.input).appearance(false).h(relative(1.))),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child("ctrl+↵ or drag into the page to use"),
            )
    }
}

impl Workspace {
    /// The scratchpad's view, created on first use.
    pub(crate) fn ensure_scratchpad(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<Scratchpad> {
        let editor = self.editor.clone();
        self.scratchpad
            .get_or_insert_with(|| cx.new(|cx| Scratchpad::new(editor, window, cx)))
            .clone()
    }

    pub(crate) fn scratchpad_focused(&self, window: &Window, cx: &App) -> bool {
        self.scratchpad
            .as_ref()
            .is_some_and(|pad| pad.read(cx).has_focus(window, cx))
    }
}
```

```diff
diff --git a/crates/tulis-core/src/history.rs b/crates/tulis-core/src/history.rs
--- a/crates/tulis-core/src/history.rs
+++ b/crates/tulis-core/src/history.rs
@@ -80,6 +80,13 @@ impl<S: Clone> History<S> {
         Some(next)
     }
 
+    /// Apply `f` to every stored state, undo and redo steps alike. For changes that undo must
+    /// not revert (the Overflow scratchpad's own edits).
+    pub fn for_each_state(&mut self, mut f: impl FnMut(&mut S)) {
+        self.undo.iter_mut().for_each(|(_, state)| f(state));
+        self.redo.iter_mut().for_each(f);
+    }
+
     pub fn can_undo(&self) -> bool {
         !self.undo.is_empty()
     }
diff --git a/crates/tulis/src/editor/features.rs b/crates/tulis/src/editor/features.rs
--- a/crates/tulis/src/editor/features.rs
+++ b/crates/tulis/src/editor/features.rs
@@ -205,6 +205,19 @@ impl Editor {
         self.set_notice(STASHED, cx);
     }
 
+    /// The scratchpad's own edits. They mark the document dirty but aren't page undo steps:
+    /// every stored step takes the new text, so page undo and redo leave it alone.
+    pub fn set_overflow(&mut self, text: &str, cx: &mut Context<Self>) {
+        if self.doc.overflow == text {
+            return;
+        }
+        self.doc.overflow = text.to_string();
+        self.history
+            .for_each_state(|(doc, _)| doc.overflow = text.to_string());
+        self.dirty = true;
+        cx.notify();
+    }
+
     /// Accept the trim suggestion under the cursor (or the next one, or the first) by ghosting it.
     pub(super) fn accept_trim(&mut self, _: &AcceptTrim, _: &mut Window, cx: &mut Context<Self>) {
         if self.trims.is_empty() {
diff --git a/crates/tulis/src/main.rs b/crates/tulis/src/main.rs
--- a/crates/tulis/src/main.rs
+++ b/crates/tulis/src/main.rs
@@ -7,6 +7,7 @@ mod editor;
 mod header_dim;
 mod keymap;
 mod runs;
+mod scratchpad;
 mod sidebar;
 mod theme;
 mod toast;
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -29,6 +29,8 @@ pub struct Workspace {
     pub(crate) cheat_sheet: Option<Entity<crate::cheat_sheet::CheatSheet>>,
     /// The alternates sidebar's view, created the first time it opens.
     pub(crate) alt_sidebar: Option<Entity<crate::sidebar::AltSidebar>>,
+    /// The Overflow scratchpad's view, created the first time the panel opens.
+    pub(crate) scratchpad: Option<Entity<crate::scratchpad::Scratchpad>>,
     pub(crate) preview: bool,
     preview_focus: FocusHandle,
     pub(crate) header_dim: HeaderDim,
@@ -73,6 +75,7 @@ impl Workspace {
             banner: None,
             cheat_sheet: None,
             alt_sidebar: None,
+            scratchpad: None,
             preview: false,
             preview_focus: cx.focus_handle(),
             header_dim: HeaderDim::default(),
@@ -146,39 +149,22 @@ impl Workspace {
         self.editor.update(cx, |e, cx| e.cycle_theme(cx));
     }
 
+    /// XYZ, Ctrl+\, vim `gx`: show or hide the Overflow panel. Hiding it while it has focus
+    /// gives the page focus back.
     pub(crate) fn toggle_overflow(
         &mut self,
         _: &ToggleOverflowPanel,
-        _: &mut Window,
+        window: &mut Window,
         cx: &mut Context<Self>,
     ) {
         self.show_overflow = !self.show_overflow;
+        if self.show_overflow {
+            self.ensure_scratchpad(window, cx);
+        } else if self.scratchpad_focused(window, cx) {
+            self.focus_editor(window, cx);
+        }
         cx.notify();
     }
-
-    /// The Overflow panel: the scratchpad's text (read-only until the scratchpad view lands).
-    fn render_overflow(&self, cx: &mut Context<Self>) -> impl IntoElement {
-        let t = self.editor.read(cx).theme;
-        let text = self.editor.read(cx).doc.overflow.clone();
-        div()
-            .w(px(280.))
-            .h_full()
-            .flex()
-            .flex_col()
-            .border_l_1()
-            .border_color(t.rule)
-            .bg(t.panel)
-            .child(div().px_3().py_2().text_sm().child("Overflow"))
-            .child(
-                div()
-                    .id("overflow-text")
-                    .flex_1()
-                    .overflow_y_scroll()
-                    .px_3()
-                    .text_sm()
-                    .child(text),
-            )
-    }
 }
 
 /// The writing column's text width (the element adds `GUTTER` on each side).
@@ -361,7 +347,7 @@ impl Render for Workspace {
                     })
                     .child(self.render_main(cx))
                     .when(self.show_overflow, |row| {
-                        row.child(self.render_overflow(cx))
+                        row.children(self.scratchpad.clone())
                     }),
             )
             .child(self.render_footer(cx))
```

- [ ] **Step 4: Run the tests (expect success)**

Run: `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: 141 app tests and 145 core tests pass; clippy counts match the baseline. Launch check as in Task 1; then open the panel with Ctrl+\ and check it is 260px wide with the title, field and hint.

- [ ] **Step 5: Commit**

```bash
jj describe -m "feat: Overflow scratchpad panel bound to doc.overflow

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 5: Ctrl+Enter moves a block to the page, Ctrl+Shift+Enter copies it

**Files:**
- Modify: `crates/tulis/src/editor/features.rs`, `crates/tulis/src/actions.rs`, `crates/tulis/src/commands.rs`, `crates/tulis/src/scratchpad.rs`, `crates/tulis/src/editor/tests.rs`, `crates/tulis/src/workspace.rs` (test)

**Interfaces:**
- Consumes: `Document::{overflow_block_at, take_overflow}` (Task 1), `Scratchpad` (Task 4).
- Produces:
  - `Editor::use_overflow(&mut self, range: Range<usize>, at: DocPos, copy: bool, cx) -> bool`
  - `Editor::use_overflow_selection(&mut self, selected: Range<usize>, copy: bool, cx) -> bool`
  - Actions `OverflowUse` (`secondary-enter`) and `OverflowCopy` (`secondary-shift-enter`), both in the `Scratchpad > Input` context, listed in the cheat sheet's Overflow group.

`use_overflow` trims the text, puts the caret at `at`, and inserts with `insert_text` (paragraph splits included). Coalescing is broken before and after, so even a one-letter block is its own undo step. Without `copy` it then removes the range with `take_overflow`. That happens after `insert_text` recorded its snapshot, so one page undo restores both sides. Whitespace (or a cursor on a blank line) toasts "Nothing to use here". The page's selection, if any, is not replaced: the text goes in at its head. A move focuses the page; a copy leaves focus in the scratchpad.

- [ ] **Step 1: Write the failing tests**

```diff
diff --git a/crates/tulis/src/editor/tests.rs b/crates/tulis/src/editor/tests.rs
--- a/crates/tulis/src/editor/tests.rs
+++ b/crates/tulis/src/editor/tests.rs
@@ -284,6 +284,63 @@ fn scratchpad_edits_mark_the_document_dirty_but_page_undo_keeps_them(cx: &mut Te
     assert_eq!(overflow(cx), "An idea.");
 }
 
+#[gpui::test]
+fn using_a_block_moves_it_to_the_cursor_as_one_undo_step(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["First.", ""]);
+    editor.update(cx, |e, _| {
+        e.doc.overflow = "Alpha.\n\nBeta gamma.".into();
+        e.selection = Selection::caret(DocPos::new(1, 0));
+    });
+    let overflow = |cx: &mut VisualTestContext| editor.read_with(cx, |e, _| e.doc.overflow.clone());
+    // Ctrl+Enter with the field's cursor inside "Beta gamma.": that block moves.
+    assert!(editor.update(cx, |e, cx| e.use_overflow_selection(10..10, false, cx)));
+    assert_eq!(texts(&editor, cx), vec!["First.", "Beta gamma."]);
+    assert_eq!(overflow(cx), "Alpha.");
+    assert_eq!(editor.read_with(cx, |e, _| e.cursor()), DocPos::new(1, 11));
+    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
+    assert_eq!(texts(&editor, cx), vec!["First.", ""]);
+    assert_eq!(overflow(cx), "Alpha.\n\nBeta gamma.");
+    editor.update_in(cx, |e, window, cx| e.redo(&Redo, window, cx));
+    assert_eq!(texts(&editor, cx), vec!["First.", "Beta gamma."]);
+    assert_eq!(overflow(cx), "Alpha.");
+}
+
+#[gpui::test]
+fn copying_a_selection_keeps_it_in_overflow(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["Say "]);
+    editor.update(cx, |e, _| {
+        e.doc.overflow = "Alpha.\n\nBeta gamma.".into();
+        e.selection = Selection::caret(DocPos::new(0, 4));
+    });
+    // Ctrl+Shift+Enter with "gamma" selected in the field.
+    assert!(editor.update(cx, |e, cx| e.use_overflow_selection(13..18, true, cx)));
+    assert_eq!(texts(&editor, cx), vec!["Say gamma"]);
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.doc.overflow.clone()),
+        "Alpha.\n\nBeta gamma."
+    );
+    // A one-letter block is its own undo step, not part of the typing before it.
+    type_str(&editor, cx, "!");
+    editor.update(cx, |e, _| e.doc.overflow = "x".into());
+    assert!(editor.update(cx, |e, cx| e.use_overflow_selection(0..0, true, cx)));
+    assert_eq!(texts(&editor, cx), vec!["Say gamma!x"]);
+    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
+    assert_eq!(texts(&editor, cx), vec!["Say gamma!"]);
+}
+
+#[gpui::test]
+fn using_overflow_on_a_blank_line_does_nothing(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["Page."]);
+    editor.update(cx, |e, _| e.doc.overflow = "A\n\n\nB".into());
+    assert!(!editor.update(cx, |e, cx| e.use_overflow_selection(2..2, false, cx)));
+    assert!(!editor.update(cx, |e, cx| e.use_overflow_selection(1..3, false, cx)));
+    assert_eq!(texts(&editor, cx), vec!["Page."]);
+    assert_eq!(
+        editor.read_with(cx, |e, _| e.last_notice().map(str::to_string)),
+        Some("Nothing to use here".into())
+    );
+}
+
 #[gpui::test]
 fn accepting_a_trim_ghosts_it_and_escape_dismisses(cx: &mut TestAppContext) {
     let (editor, cx) = editor_with(cx, &["I really love tea, really."]);
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -876,6 +876,39 @@ mod tests {
         assert!(page_focused(&ws, cx));
     }
 
+    #[gpui::test]
+    fn ctrl_enter_in_the_scratchpad_moves_the_block_and_ctrl_shift_enter_copies(
+        cx: &mut TestAppContext,
+    ) {
+        let (ws, cx) = root_workspace(cx, &["Page."]);
+        ws.update(cx, |ws, cx| {
+            ws.editor.update(cx, |e, _| e.doc.overflow = "Idea.".into())
+        });
+        cx.simulate_keystrokes("secondary-\\");
+        cx.run_until_parked();
+        focus_scratchpad(&ws, cx);
+        let page_text = |cx: &mut gpui::VisualTestContext| {
+            ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.paragraphs[0].text())
+        };
+        cx.simulate_keystrokes("secondary-enter");
+        cx.run_until_parked();
+        assert_eq!(page_text(cx), "Idea.Page.");
+        assert_eq!(scratchpad_value(&ws, cx), "");
+        assert!(page_focused(&ws, cx));
+        // Page undo puts the block back in the scratchpad.
+        cx.simulate_keystrokes("secondary-z");
+        cx.run_until_parked();
+        assert_eq!(page_text(cx), "Page.");
+        assert_eq!(scratchpad_value(&ws, cx), "Idea.");
+        // Copy: the text stays, and so does the focus.
+        focus_scratchpad(&ws, cx);
+        cx.simulate_keystrokes("secondary-shift-enter");
+        cx.run_until_parked();
+        assert_eq!(page_text(cx), "Idea.Page.");
+        assert_eq!(scratchpad_value(&ws, cx), "Idea.");
+        assert!(!page_focused(&ws, cx));
+    }
+
     fn sidebar_focused(ws: &gpui::Entity<Workspace>, cx: &mut gpui::VisualTestContext) -> bool {
         let sidebar = ws.read_with(cx, |ws, _| ws.alt_sidebar.clone());
         sidebar.is_some_and(|s| cx.update(|window, cx| s.read(cx).list_focus.is_focused(window)))
```

- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile errors: ``no method named `use_overflow_selection` ``.

- [ ] **Step 3: Implement**

```diff
diff --git a/crates/tulis/src/actions.rs b/crates/tulis/src/actions.rs
--- a/crates/tulis/src/actions.rs
+++ b/crates/tulis/src/actions.rs
@@ -47,6 +47,8 @@ gpui::actions!(
         ToggleGhost,
         SendToOverflow,
         ToggleOverflowPanel,
+        OverflowUse,
+        OverflowCopy,
         AiAlternates,
         AiTrimParagraph,
         AiTrimDocument,
diff --git a/crates/tulis/src/commands.rs b/crates/tulis/src/commands.rs
--- a/crates/tulis/src/commands.rs
+++ b/crates/tulis/src/commands.rs
@@ -452,6 +452,25 @@ pub static COMMANDS: &[Command] = &[
         Some("gx"),
         ToggleOverflowPanel
     ),
+    // In the scratchpad's text field, ahead of the field's own Ctrl+Enter (a line break).
+    cmd!(
+        "overflow_use",
+        "Move the block (or selection) to the page",
+        Overflow,
+        ["secondary-enter"],
+        Some("Scratchpad > Input"),
+        None,
+        OverflowUse
+    ),
+    cmd!(
+        "overflow_copy",
+        "Copy the block (or selection) to the page",
+        Overflow,
+        ["secondary-shift-enter"],
+        Some("Scratchpad > Input"),
+        None,
+        OverflowCopy
+    ),
     // Lab
     cmd!(
         "open_lab",
diff --git a/crates/tulis/src/editor/features.rs b/crates/tulis/src/editor/features.rs
--- a/crates/tulis/src/editor/features.rs
+++ b/crates/tulis/src/editor/features.rs
@@ -13,6 +13,7 @@ use crate::actions::*;
 
 const STASHED: &str = "Stashed to Overflow";
 const NOTHING_TO_STASH: &str = "Nothing to move to overflow";
+const NOTHING_TO_USE: &str = "Nothing to use here";
 
 impl Editor {
     /// The selection within one paragraph, or the cursor's whole paragraph when nothing is selected.
@@ -218,6 +219,55 @@ impl Editor {
         cx.notify();
     }
 
+    /// Put the scratchpad text at byte `range` into the page at `at`, trimmed, as one undo
+    /// step (Ctrl+Enter, a drop). It leaves the scratchpad unless `copy`. Returns false when the
+    /// range holds only whitespace.
+    pub fn use_overflow(
+        &mut self,
+        range: Range<usize>,
+        at: DocPos,
+        copy: bool,
+        cx: &mut Context<Self>,
+    ) -> bool {
+        let text = self.doc.overflow.get(range.clone()).unwrap_or("").trim();
+        if text.is_empty() {
+            self.set_notice(NOTHING_TO_USE, cx);
+            return false;
+        }
+        let text = text.to_string();
+        let at = self.finish_alt_edit().apply(at);
+        self.selection = Selection::caret(self.doc.clamp(at));
+        // Its own undo step, even for one letter typed into the page right before or after.
+        self.history.break_coalescing();
+        self.insert_text(&text, cx);
+        self.history.break_coalescing();
+        if !copy {
+            self.doc.take_overflow(range);
+        }
+        cx.notify();
+        true
+    }
+
+    /// Ctrl+Enter in the scratchpad: its selection (a byte range of `doc.overflow`) or, with
+    /// none, the block at its cursor goes to the page cursor.
+    pub fn use_overflow_selection(
+        &mut self,
+        selected: Range<usize>,
+        copy: bool,
+        cx: &mut Context<Self>,
+    ) -> bool {
+        let range = if selected.is_empty() {
+            self.doc.overflow_block_at(selected.start)
+        } else {
+            Some(selected)
+        };
+        let Some(range) = range else {
+            self.set_notice(NOTHING_TO_USE, cx);
+            return false;
+        };
+        self.use_overflow(range, self.selection.head, copy, cx)
+    }
+
     /// Accept the trim suggestion under the cursor (or the next one, or the first) by ghosting it.
     pub(super) fn accept_trim(&mut self, _: &AcceptTrim, _: &mut Window, cx: &mut Context<Self>) {
         if self.trims.is_empty() {
diff --git a/crates/tulis/src/scratchpad.rs b/crates/tulis/src/scratchpad.rs
--- a/crates/tulis/src/scratchpad.rs
+++ b/crates/tulis/src/scratchpad.rs
@@ -7,6 +7,7 @@ use gpui::{
 };
 use gpui_component::input::{InputEvent, Textarea, TextareaState};
 
+use crate::actions::{OverflowCopy, OverflowUse};
 use crate::editor::Editor;
 use crate::workspace::Workspace;
 
@@ -60,6 +61,26 @@ impl Scratchpad {
         cx.notify();
     }
 
+    /// Ctrl+Enter: move the field's selection, or the block at its cursor, to the page cursor,
+    /// and focus the page.
+    fn use_block(&mut self, _: &OverflowUse, window: &mut Window, cx: &mut Context<Self>) {
+        let selected = self.input.read(cx).selected_range();
+        let used = self
+            .editor
+            .update(cx, |e, cx| e.use_overflow_selection(selected, false, cx));
+        if used {
+            let page = self.editor.focus_handle(cx);
+            window.focus(&page, cx);
+        }
+    }
+
+    /// Ctrl+Shift+Enter: the same, but the text stays here and so does the focus.
+    fn copy_block(&mut self, _: &OverflowCopy, _: &mut Window, cx: &mut Context<Self>) {
+        let selected = self.input.read(cx).selected_range();
+        self.editor
+            .update(cx, |e, cx| e.use_overflow_selection(selected, true, cx));
+    }
+
     pub(crate) fn has_focus(&self, window: &Window, cx: &App) -> bool {
         self.input.focus_handle(cx).is_focused(window)
     }
@@ -71,6 +92,8 @@ impl Render for Scratchpad {
         div()
             .id("scratchpad")
             .key_context("Scratchpad")
+            .on_action(cx.listener(Self::use_block))
+            .on_action(cx.listener(Self::copy_block))
             .w(px(SCRATCHPAD_WIDTH))
             .flex_none()
             .h_full()
```

- [ ] **Step 4: Run the tests (expect success)**

Run: `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: 145 app tests and 145 core tests pass; clippy counts match the baseline. The workspace test also checks that the field gets no line break from Ctrl+Enter. Launch check as in Task 1.

- [ ] **Step 5: Commit**

```bash
jj describe -m "feat: Ctrl+Enter / Ctrl+Shift+Enter move or copy a scratchpad block to the page

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 6: Focus page ↔ scratchpad (Ctrl+Shift+\\, `gX`), and Esc

**Files:**
- Modify: `crates/tulis-core/src/vim.rs`, `crates/tulis/src/actions.rs`, `crates/tulis/src/commands.rs`, `crates/tulis/src/editor/vim_exec.rs`, `crates/tulis/src/scratchpad.rs`, `crates/tulis/src/workspace.rs`

**Interfaces:**
- Consumes: `Workspace::{ensure_scratchpad, scratchpad_focused}` (Task 4).
- Produces:
  - `VimCommand::FocusOverflow` (`gX`, a `g` view command: it needs no operator and keeps Visual mode)
  - Actions `FocusOverflow` (`secondary-shift-\\` and `secondary-|`, window-wide, vim `gX`) and `FocusPage` (`escape` in the `Scratchpad` context, a Hidden row)
  - `Workspace::toggle_overflow_focus(&mut self, &FocusOverflow, &mut Window, &mut Context<Self>)`

Ctrl+Shift+\ focuses the scratchpad, opening the panel if needed, or the page when the scratchpad has focus. `secondary-|` is bound too, for platforms that report the shifted key; the cheat sheet does the same for `Ctrl+?`. Esc reaches `FocusPage` because the field's own Escape propagates when it has nothing to dismiss. The panel stays open.

- [ ] **Step 1: Write the failing tests**

```diff
diff --git a/crates/tulis-core/src/vim.rs b/crates/tulis-core/src/vim.rs
--- a/crates/tulis-core/src/vim.rs
+++ b/crates/tulis-core/src/vim.rs
@@ -661,6 +661,7 @@ mod tests {
         assert_eq!(feed(&mut v, "g?"), vec![CheatSheet]);
         assert_eq!(feed(&mut v, "gl"), vec![Lab]);
         assert_eq!(feed(&mut v, "gx"), vec![ToggleOverflow]);
+        assert_eq!(feed(&mut v, "gX"), vec![FocusOverflow]);
         assert_eq!(v.mode(), Mode::Normal);
     }
 
diff --git a/crates/tulis/src/commands.rs b/crates/tulis/src/commands.rs
--- a/crates/tulis/src/commands.rs
+++ b/crates/tulis/src/commands.rs
@@ -711,6 +711,7 @@ mod tests {
             VimCommand::CheatSheet => "cheat_sheet",
             VimCommand::Lab => "open_lab",
             VimCommand::ToggleOverflow => "toggle_overflow",
+            VimCommand::FocusOverflow => "focus_overflow",
             VimCommand::Alternatives => "alternatives",
             VimCommand::AiAlternatives => "ai_alternates",
             VimCommand::Stash => "stash",
@@ -760,6 +761,7 @@ mod tests {
             VimCommand::CheatSheet,
             VimCommand::Lab,
             VimCommand::ToggleOverflow,
+            VimCommand::FocusOverflow,
             VimCommand::Alternatives,
             VimCommand::AiAlternatives,
             VimCommand::Stash,
@@ -811,6 +813,12 @@ mod tests {
         assert_eq!(theme[0].0, Group::View);
         assert_eq!(theme[0].1[0].vim, "gt");
         assert_eq!(cheat_sheet("gx")[0].1[0].label, "Show / hide Overflow");
+        let focus = &cheat_sheet("gX")[0].1;
+        assert!(
+            focus
+                .iter()
+                .any(|row| row.vim == "gX" && row.label == "Focus page ↔ Overflow")
+        );
         assert_eq!(cheat_sheet("linkedin")[0].1[0].key, "—");
         assert_eq!(cheat_sheet("ai alt")[0].1[0].label, "AI alternatives");
         assert!(cheat_sheet("zzzz").is_empty());
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -909,6 +909,59 @@ mod tests {
         assert!(!page_focused(&ws, cx));
     }
 
+    fn scratchpad_focused(ws: &gpui::Entity<Workspace>, cx: &mut gpui::VisualTestContext) -> bool {
+        ws.update_in(cx, |ws, window, cx| ws.scratchpad_focused(window, cx))
+    }
+
+    #[gpui::test]
+    fn ctrl_shift_backslash_moves_focus_between_the_page_and_the_scratchpad(
+        cx: &mut TestAppContext,
+    ) {
+        let (ws, cx) = root_workspace(cx, &["Page."]);
+        cx.simulate_keystrokes("secondary-shift-\\");
+        cx.run_until_parked();
+        assert!(ws.read_with(cx, |ws, _| ws.show_overflow));
+        assert!(scratchpad_focused(&ws, cx));
+        cx.simulate_keystrokes("secondary-shift-\\");
+        cx.run_until_parked();
+        assert!(page_focused(&ws, cx));
+        cx.simulate_keystrokes("secondary-shift-\\");
+        cx.run_until_parked();
+        assert!(scratchpad_focused(&ws, cx));
+        // Typing goes into the scratchpad, not the page.
+        cx.simulate_input("note");
+        assert_eq!(scratchpad_value(&ws, cx), "note");
+        let page = ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.paragraphs[0].text());
+        assert_eq!(page, "Page.");
+    }
+
+    #[gpui::test]
+    fn escape_in_the_scratchpad_returns_to_the_page(cx: &mut TestAppContext) {
+        let (ws, cx) = root_workspace(cx, &["Page."]);
+        cx.simulate_keystrokes("secondary-shift-\\");
+        cx.run_until_parked();
+        assert!(scratchpad_focused(&ws, cx));
+        cx.simulate_keystrokes("escape");
+        cx.run_until_parked();
+        assert!(page_focused(&ws, cx));
+        // The panel stays open.
+        assert!(ws.read_with(cx, |ws, _| ws.show_overflow));
+    }
+
+    #[gpui::test]
+    fn vim_g_shift_x_focuses_the_scratchpad(cx: &mut TestAppContext) {
+        let (ws, cx) = root_workspace(cx, &["Page."]);
+        let editor = ws.read_with(cx, |ws, _| ws.editor.clone());
+        editor.update_in(cx, |e, window, cx| {
+            e.vim = Some(tulis_core::vim::Vim::new());
+            e.replace_text_in_range(None, "g", window, cx);
+            e.replace_text_in_range(None, "X", window, cx);
+        });
+        cx.run_until_parked();
+        assert!(ws.read_with(cx, |ws, _| ws.show_overflow));
+        assert!(scratchpad_focused(&ws, cx));
+    }
+
     fn sidebar_focused(ws: &gpui::Entity<Workspace>, cx: &mut gpui::VisualTestContext) -> bool {
         let sidebar = ws.read_with(cx, |ws, _| ws.alt_sidebar.clone());
         sidebar.is_some_and(|s| cx.update(|window, cx| s.read(cx).list_focus.is_focused(window)))
```

- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile error in `tulis-core`: ``cannot find value `FocusOverflow` ``.

- [ ] **Step 3: Implement**

```diff
diff --git a/crates/tulis-core/src/vim.rs b/crates/tulis-core/src/vim.rs
--- a/crates/tulis-core/src/vim.rs
+++ b/crates/tulis-core/src/vim.rs
@@ -107,6 +107,8 @@ pub enum VimCommand {
     Lab,
     /// `gx`: show or hide the Overflow panel.
     ToggleOverflow,
+    /// `gX`: move focus between the page and the Overflow scratchpad.
+    FocusOverflow,
     /// `ga`: the alternates sidebar for the selection or the word under the cursor.
     Alternatives,
     /// `gA`: AI alternatives for the selection or the cursor's paragraph.
@@ -124,6 +126,7 @@ fn g_view_command(c: char) -> Option<VimCommand> {
         '?' => VimCommand::CheatSheet,
         'l' => VimCommand::Lab,
         'x' => VimCommand::ToggleOverflow,
+        'X' => VimCommand::FocusOverflow,
         _ => return None,
     })
 }
diff --git a/crates/tulis/src/actions.rs b/crates/tulis/src/actions.rs
--- a/crates/tulis/src/actions.rs
+++ b/crates/tulis/src/actions.rs
@@ -49,6 +49,8 @@ gpui::actions!(
         ToggleOverflowPanel,
         OverflowUse,
         OverflowCopy,
+        FocusOverflow,
+        FocusPage,
         AiAlternates,
         AiTrimParagraph,
         AiTrimDocument,
diff --git a/crates/tulis/src/commands.rs b/crates/tulis/src/commands.rs
--- a/crates/tulis/src/commands.rs
+++ b/crates/tulis/src/commands.rs
@@ -452,6 +452,24 @@ pub static COMMANDS: &[Command] = &[
         Some("gx"),
         ToggleOverflowPanel
     ),
+    cmd!(
+        "focus_overflow",
+        "Focus page ↔ Overflow",
+        Overflow,
+        ["secondary-shift-\\", "secondary-|"],
+        None,
+        Some("gX"),
+        FocusOverflow
+    ),
+    cmd!(
+        "scratchpad_escape",
+        "Overflow: back to the page",
+        Hidden,
+        ["escape"],
+        Some("Scratchpad"),
+        None,
+        FocusPage
+    ),
     // In the scratchpad's text field, ahead of the field's own Ctrl+Enter (a line break).
     cmd!(
         "overflow_use",
diff --git a/crates/tulis/src/editor/vim_exec.rs b/crates/tulis/src/editor/vim_exec.rs
--- a/crates/tulis/src/editor/vim_exec.rs
+++ b/crates/tulis/src/editor/vim_exec.rs
@@ -10,7 +10,8 @@ use tulis_core::{DocPos, Selection};
 
 use super::Editor;
 use crate::actions::{
-    AiAlternates, SendToOverflow, ShowCheatSheet, ToggleOverflowPanel, TogglePreview, ToggleVim,
+    AiAlternates, FocusOverflow, SendToOverflow, ShowCheatSheet, ToggleOverflowPanel,
+    TogglePreview, ToggleVim,
 };
 
 /// Commands that act on the selection; from Visual mode they get its inclusive end.
@@ -274,6 +275,7 @@ impl Editor {
             VimCommand::TogglePreview => window.dispatch_action(Box::new(TogglePreview), cx),
             VimCommand::CheatSheet => window.dispatch_action(Box::new(ShowCheatSheet), cx),
             VimCommand::ToggleOverflow => window.dispatch_action(Box::new(ToggleOverflowPanel), cx),
+            VimCommand::FocusOverflow => window.dispatch_action(Box::new(FocusOverflow), cx),
         }
     }
 
diff --git a/crates/tulis/src/scratchpad.rs b/crates/tulis/src/scratchpad.rs
--- a/crates/tulis/src/scratchpad.rs
+++ b/crates/tulis/src/scratchpad.rs
@@ -7,7 +7,7 @@ use gpui::{
 };
 use gpui_component::input::{InputEvent, Textarea, TextareaState};
 
-use crate::actions::{OverflowCopy, OverflowUse};
+use crate::actions::{FocusOverflow, FocusPage, OverflowCopy, OverflowUse};
 use crate::editor::Editor;
 use crate::workspace::Workspace;
 
@@ -81,6 +81,12 @@ impl Scratchpad {
             .update(cx, |e, cx| e.use_overflow_selection(selected, true, cx));
     }
 
+    /// Esc: back to the page (the panel stays open).
+    fn focus_page(&mut self, _: &FocusPage, window: &mut Window, cx: &mut Context<Self>) {
+        let page = self.editor.focus_handle(cx);
+        window.focus(&page, cx);
+    }
+
     pub(crate) fn has_focus(&self, window: &Window, cx: &App) -> bool {
         self.input.focus_handle(cx).is_focused(window)
     }
@@ -94,6 +100,7 @@ impl Render for Scratchpad {
             .key_context("Scratchpad")
             .on_action(cx.listener(Self::use_block))
             .on_action(cx.listener(Self::copy_block))
+            .on_action(cx.listener(Self::focus_page))
             .w(px(SCRATCHPAD_WIDTH))
             .flex_none()
             .h_full()
@@ -143,6 +150,24 @@ impl Workspace {
             .clone()
     }
 
+    /// Ctrl+Shift+\, vim `gX`: focus the scratchpad (opening the panel), or the page when the
+    /// scratchpad has focus.
+    pub(crate) fn toggle_overflow_focus(
+        &mut self,
+        _: &FocusOverflow,
+        window: &mut Window,
+        cx: &mut Context<Self>,
+    ) {
+        if self.scratchpad_focused(window, cx) {
+            self.focus_editor(window, cx);
+        } else {
+            self.show_overflow = true;
+            let input = self.ensure_scratchpad(window, cx).read(cx).input.clone();
+            window.focus(&input.focus_handle(cx), cx);
+        }
+        cx.notify();
+    }
+
     pub(crate) fn scratchpad_focused(&self, window: &Window, cx: &App) -> bool {
         self.scratchpad
             .as_ref()
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -318,6 +318,7 @@ impl Render for Workspace {
                 }
             }))
             .on_action(cx.listener(Self::toggle_overflow))
+            .on_action(cx.listener(Self::toggle_overflow_focus))
             .on_action(cx.listener(Self::cycle_theme))
             .on_action(cx.listener(Self::toggle_markers))
             .on_action(cx.listener(Self::open_lab))
```

- [ ] **Step 4: Run the tests (expect success)**

Run: `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: 148 app tests and 145 core tests pass; clippy counts match the baseline. Launch check as in Task 1.

- [ ] **Step 5: Commit**

```bash
jj describe -m "feat: Ctrl+Shift+\\ / gX move focus between page and scratchpad; Esc returns

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 7: Drag a block's `⠿` grip onto the page

**Files:**
- Modify: `crates/tulis/src/scratchpad.rs`, `crates/tulis/src/editor/features.rs`, `crates/tulis/src/workspace.rs`, `crates/tulis/src/editor/tests.rs`

**Interfaces:**
- Consumes: `Editor::use_overflow` (Task 5), `LayoutCache::pos_for_window_point`, `Editor::current_layout`.
- Produces:
  - `scratchpad::OverflowDrag { pub range: Range<usize>, pub text: String }` (the drag payload)
  - `scratchpad::Grip { range, left, top, bottom }` (window coordinates), `Scratchpad::grips(&self, &App) -> Vec<Grip>`, `Scratchpad.hovered: Option<Range<usize>>`
  - `Editor::drop_overflow(&mut self, range: Range<usize>, text: &str, point: Point<Pixels>, copy: bool, cx) -> bool`

Grip geometry comes from the field's last layout. A block spanning buffer lines `first..=last` covers display rows `buffer_line_to_display_row_range(first).start .. buffer_line_to_display_row_range(last).end`. Its top is `text_bounds().origin.y + line_height * row`; the origin already includes the scroll offset. In the scratch run, this matched `bounds_for_range` at each block's start, wrapped lines included. Blocks whose first line is out of view get no grip. Moving the pointer over the panel sets `hovered` to the block whose rows contain it, and only that block's grip is drawn, `anchored()` in an 18px gutter left of the field. Leaving the panel clears it. A drag shows a small card with the block's text. The page's `on_drop` reads the pointer position and `secondary` (Ctrl) from the window. `drop_overflow` does nothing if the scratchpad's text at `range` is no longer what was dragged.

- [ ] **Step 1: Write the failing tests**

```diff
diff --git a/crates/tulis/src/editor/tests.rs b/crates/tulis/src/editor/tests.rs
--- a/crates/tulis/src/editor/tests.rs
+++ b/crates/tulis/src/editor/tests.rs
@@ -341,6 +341,49 @@ fn using_overflow_on_a_blank_line_does_nothing(cx: &mut TestAppContext) {
     );
 }
 
+#[gpui::test]
+fn dropping_a_block_puts_it_where_it_lands(cx: &mut TestAppContext) {
+    let (editor, cx) = editor_with(cx, &["One two.", "Three."]);
+    editor.update(cx, |e, _| e.doc.overflow = "Block.\n\nOther.".into());
+    cx.run_until_parked();
+    let point = editor.read_with(cx, |e, _| {
+        let layout = e.layout.as_ref().expect("painted");
+        let p = layout.window_point(DocPos::new(0, 4)).unwrap();
+        gpui::point(p.x + gpui::px(1.), p.y + layout.line_height / 2.)
+    });
+    let overflow = |cx: &mut VisualTestContext| editor.read_with(cx, |e, _| e.doc.overflow.clone());
+    // The scratchpad changed under the drag: nothing happens.
+    assert!(!editor.update(cx, |e, cx| e.drop_overflow(
+        0..6,
+        "Blocks",
+        point,
+        false,
+        cx
+    )));
+    assert!(editor.update(cx, |e, cx| e.drop_overflow(
+        0..6,
+        "Block.",
+        point,
+        false,
+        cx
+    )));
+    assert_eq!(texts(&editor, cx), vec!["One Block.two.", "Three."]);
+    assert_eq!(overflow(cx), "Other.");
+    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
+    assert_eq!(texts(&editor, cx), vec!["One two.", "Three."]);
+    assert_eq!(overflow(cx), "Block.\n\nOther.");
+    // Ctrl held while dropping: a copy.
+    assert!(editor.update(cx, |e, cx| e.drop_overflow(
+        8..14,
+        "Other.",
+        point,
+        true,
+        cx
+    )));
+    assert_eq!(texts(&editor, cx), vec!["One Other.two.", "Three."]);
+    assert_eq!(overflow(cx), "Block.\n\nOther.");
+}
+
 #[gpui::test]
 fn accepting_a_trim_ghosts_it_and_escape_dismisses(cx: &mut TestAppContext) {
     let (editor, cx) = editor_with(cx, &["I really love tea, really."]);
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -963,6 +963,48 @@ mod tests {
         assert!(scratchpad_focused(&ws, cx));
     }
 
+    #[gpui::test]
+    fn dragging_a_grip_onto_the_page_moves_the_block_there(cx: &mut TestAppContext) {
+        use gpui::{Modifiers, MouseButton, point, px};
+
+        let (ws, cx) = root_workspace(cx, &["Page text."]);
+        cx.simulate_resize(gpui::size(px(1000.), px(700.)));
+        ws.update(cx, |ws, cx| {
+            ws.editor
+                .update(cx, |e, _| e.doc.overflow = "Dragged.\n\nStays.".into())
+        });
+        cx.simulate_keystrokes("secondary-\\");
+        cx.run_until_parked();
+        let pad = ws.read_with(cx, |ws, _| ws.scratchpad.clone().unwrap());
+        let grips = pad.read_with(cx, |pad, cx| pad.grips(cx));
+        assert_eq!(
+            grips.iter().map(|g| g.range.clone()).collect::<Vec<_>>(),
+            vec![0..8, 10..16]
+        );
+        // Hovering a block shows its grip.
+        let grip = grips[0].clone();
+        let over_text = point(grip.left + px(60.), grip.top + px(4.));
+        cx.simulate_mouse_move(over_text, None, Modifiers::none());
+        assert_eq!(pad.read_with(cx, |pad, _| pad.hovered.clone()), Some(0..8));
+        let target = ws.read_with(cx, |ws, cx| {
+            let layout = ws.editor.read(cx).layout.as_ref().expect("painted");
+            let p = layout.window_point(DocPos::new(0, 5)).unwrap();
+            point(p.x + px(1.), p.y + layout.line_height / 2.)
+        });
+        let on_grip = point(grip.left + px(4.), grip.top + px(6.));
+        cx.simulate_mouse_down(on_grip, MouseButton::Left, Modifiers::none());
+        cx.simulate_mouse_move(target, MouseButton::Left, Modifiers::none());
+        cx.simulate_mouse_up(target, MouseButton::Left, Modifiers::none());
+        cx.run_until_parked();
+        let (page, overflow) = ws.read_with(cx, |ws, cx| {
+            let e = ws.editor.read(cx);
+            (e.doc.paragraphs[0].text(), e.doc.overflow.clone())
+        });
+        assert_eq!(page, "Page Dragged.text.");
+        assert_eq!(overflow, "Stays.");
+        assert!(page_focused(&ws, cx));
+    }
+
     fn sidebar_focused(ws: &gpui::Entity<Workspace>, cx: &mut gpui::VisualTestContext) -> bool {
         let sidebar = ws.read_with(cx, |ws, _| ws.alt_sidebar.clone());
         sidebar.is_some_and(|s| cx.update(|window, cx| s.read(cx).list_focus.is_focused(window)))
```

- [ ] **Step 2: Run the tests (expect failure)**

Run: `cargo test --workspace`
Expected: compile errors: ``no method named `drop_overflow` ``, ``no method named `grips` `` and ``no field `hovered` ``.

- [ ] **Step 3: Implement**

```diff
diff --git a/crates/tulis/src/editor/features.rs b/crates/tulis/src/editor/features.rs
--- a/crates/tulis/src/editor/features.rs
+++ b/crates/tulis/src/editor/features.rs
@@ -3,7 +3,7 @@
 use std::ops::Range;
 use std::time::Instant;
 
-use gpui::{Context, Window};
+use gpui::{Context, Pixels, Point, Window};
 use tulis_core::alternates::{AltLevel, OpError};
 use tulis_core::history::EditKind;
 use tulis_core::{DocPos, Selection};
@@ -248,6 +248,27 @@ impl Editor {
         true
     }
 
+    /// A scratchpad block dropped on the page at window `point`: it goes in at the position under
+    /// the pointer, moved, or copied with `copy`. `range` and `text` are what the grip dragged;
+    /// if the scratchpad changed there since, nothing happens.
+    pub fn drop_overflow(
+        &mut self,
+        range: Range<usize>,
+        text: &str,
+        point: Point<Pixels>,
+        copy: bool,
+        cx: &mut Context<Self>,
+    ) -> bool {
+        if self.doc.overflow.get(range.clone()) != Some(text) {
+            return false;
+        }
+        let Some(layout) = self.current_layout() else {
+            return false;
+        };
+        let at = layout.pos_for_window_point(point);
+        self.use_overflow(range, at, copy, cx)
+    }
+
     /// Ctrl+Enter in the scratchpad: its selection (a byte range of `doc.overflow`) or, with
     /// none, the block at its cursor goes to the page cursor.
     pub fn use_overflow_selection(
diff --git a/crates/tulis/src/scratchpad.rs b/crates/tulis/src/scratchpad.rs
--- a/crates/tulis/src/scratchpad.rs
+++ b/crates/tulis/src/scratchpad.rs
@@ -1,25 +1,72 @@
 //! The Overflow scratchpad: a right panel with one multi-line text field bound to
 //! `doc.overflow`.
 
+use std::ops::Range;
+
 use gpui::{
-    App, Context, Entity, Focusable, IntoElement, ParentElement, Render, Styled, Subscription,
-    Window, div, prelude::*, px, relative,
+    App, Context, Entity, Focusable, IntoElement, MouseMoveEvent, ParentElement, Pixels, Render,
+    SharedString, Styled, Subscription, Window, anchored, div, point, prelude::*, px, relative,
 };
 use gpui_component::input::{InputEvent, Textarea, TextareaState};
 
 use crate::actions::{FocusOverflow, FocusPage, OverflowCopy, OverflowUse};
 use crate::editor::Editor;
+use crate::theme::Theme;
 use crate::workspace::Workspace;
 
 pub const SCRATCHPAD_WIDTH: f32 = 260.;
+/// Space left of the text field for the blocks' `⠿` grips.
+const GRIP_GUTTER: f32 = 18.;
 
 pub struct Scratchpad {
     editor: Entity<Editor>,
     /// The text field. Its value mirrors `doc.overflow`; it keeps its own undo history.
     pub(crate) input: Entity<TextareaState>,
+    /// The block under the pointer, whose grip is showing.
+    pub(crate) hovered: Option<Range<usize>>,
     _subscriptions: Vec<Subscription>,
 }
 
+/// What a grip drag carries: the block's byte range in `doc.overflow`, and its text then.
+pub struct OverflowDrag {
+    pub range: Range<usize>,
+    pub text: String,
+}
+
+/// A block's grip, in window coordinates: its left edge and the block's lines from top to
+/// bottom.
+#[derive(Clone, Debug)]
+pub(crate) struct Grip {
+    pub(crate) range: Range<usize>,
+    pub(crate) left: Pixels,
+    pub(crate) top: Pixels,
+    pub(crate) bottom: Pixels,
+}
+
+/// What follows the pointer during a drag: the block's text in a small card.
+struct DragPreview {
+    text: SharedString,
+    theme: Theme,
+}
+
+impl Render for DragPreview {
+    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
+        let t = self.theme;
+        div()
+            .max_w(px(SCRATCHPAD_WIDTH - 20.))
+            .px_2()
+            .py_1()
+            .rounded_md()
+            .border_1()
+            .border_color(t.rule)
+            .bg(t.panel)
+            .text_color(t.text)
+            .text_sm()
+            .line_clamp(3)
+            .child(self.text.clone())
+    }
+}
+
 impl Scratchpad {
     pub fn new(editor: Entity<Editor>, window: &mut Window, cx: &mut Context<Self>) -> Self {
         let input = cx.new(|cx| {
@@ -40,10 +87,82 @@ impl Scratchpad {
         Self {
             editor,
             input,
+            hovered: None,
             _subscriptions: subscriptions,
         }
     }
 
+    /// Each block's grip, from the field's last layout. A block whose first line is out of view
+    /// has none.
+    pub(crate) fn grips(&self, cx: &App) -> Vec<Grip> {
+        let input = self.input.read(cx);
+        let (Some(text), Some(line_height)) = (input.text_bounds(), input.line_height()) else {
+            return Vec::new();
+        };
+        let view = input.input_bounds();
+        let overflow = &self.editor.read(cx).doc.overflow;
+        let rows = input.display_map();
+        let left = view.left() - px(GRIP_GUTTER);
+        self.editor
+            .read(cx)
+            .doc
+            .overflow_blocks()
+            .into_iter()
+            .filter_map(|range| {
+                let first = overflow[..range.start].matches('\n').count();
+                let last = first + overflow[range.clone()].matches('\n').count();
+                let start = rows.buffer_line_to_display_row_range(first)?.start;
+                let end = rows.buffer_line_to_display_row_range(last)?.end;
+                // The text's origin moves with the scroll position.
+                let top = text.origin.y + line_height * start as f32;
+                let bottom = text.origin.y + line_height * end as f32;
+                (top >= view.top() - px(1.) && top < view.bottom()).then_some(Grip {
+                    range,
+                    left,
+                    top,
+                    bottom,
+                })
+            })
+            .collect()
+    }
+
+    /// Show the grip of the block under the pointer.
+    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
+        let y = event.position.y;
+        let hovered = self
+            .grips(cx)
+            .into_iter()
+            .find(|g| g.top <= y && y < g.bottom)
+            .map(|g| g.range);
+        if hovered != self.hovered {
+            self.hovered = hovered;
+            cx.notify();
+        }
+    }
+
+    fn render_grip(&self, grip: Grip, t: Theme, cx: &App) -> impl IntoElement {
+        let text = self.editor.read(cx).doc.overflow[grip.range.clone()].to_string();
+        let drag = OverflowDrag {
+            range: grip.range.clone(),
+            text,
+        };
+        anchored().position(point(grip.left, grip.top)).child(
+            div()
+                .id(("overflow-grip", grip.range.start))
+                .px_1()
+                .cursor_grab()
+                .text_color(t.text_muted)
+                .hover(|d| d.text_color(t.accent))
+                .child("⠿")
+                .on_drag(drag, move |drag: &OverflowDrag, _, _, cx| {
+                    cx.new(|_| DragPreview {
+                        text: drag.text.clone().into(),
+                        theme: t,
+                    })
+                }),
+        )
+    }
+
     /// The field changed: store its text in the document.
     fn push(&mut self, cx: &mut Context<Self>) {
         let text = self.input.read(cx).value();
@@ -95,12 +214,22 @@ impl Scratchpad {
 impl Render for Scratchpad {
     fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
         let t = self.editor.read(cx).theme;
+        let grip = self
+            .grips(cx)
+            .into_iter()
+            .find(|g| Some(&g.range) == self.hovered.as_ref());
         div()
             .id("scratchpad")
             .key_context("Scratchpad")
             .on_action(cx.listener(Self::use_block))
             .on_action(cx.listener(Self::copy_block))
             .on_action(cx.listener(Self::focus_page))
+            .on_mouse_move(cx.listener(Self::on_mouse_move))
+            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
+                if !*hovered && this.hovered.take().is_some() {
+                    cx.notify();
+                }
+            }))
             .w(px(SCRATCHPAD_WIDTH))
             .flex_none()
             .h_full()
@@ -123,9 +252,11 @@ impl Render for Scratchpad {
                 div()
                     .flex_1()
                     .min_h(px(0.))
-                    .px_2()
+                    .pl(px(GRIP_GUTTER))
+                    .pr_2()
                     .child(Textarea::new(&self.input).appearance(false).h(relative(1.))),
             )
+            .children(grip.map(|grip| self.render_grip(grip, t, cx)))
             .child(
                 div()
                     .px_3()
diff --git a/crates/tulis/src/workspace.rs b/crates/tulis/src/workspace.rs
--- a/crates/tulis/src/workspace.rs
+++ b/crates/tulis/src/workspace.rs
@@ -19,6 +19,7 @@ use crate::actions::{
 };
 use crate::editor::{Editor, EditorEvent, GUTTER};
 use crate::header_dim::HeaderDim;
+use crate::scratchpad::OverflowDrag;
 use crate::theme;
 
 pub struct Workspace {
@@ -260,6 +261,7 @@ impl Workspace {
         let (scroll, background) = (editor.scroll.clone(), editor.theme.background);
         let page_focus = editor.focus_handle(cx);
         let menu_editor = self.editor.clone();
+        let (drop_editor, drop_focus) = (self.editor.clone(), page_focus.clone());
         div()
             .id("page")
             .relative()
@@ -289,6 +291,16 @@ impl Workspace {
             )
             .child(edge_fade(true, background))
             .child(edge_fade(false, background))
+            // A scratchpad block dragged by its grip; Ctrl held copies it.
+            .on_drop(move |drag: &OverflowDrag, window, cx| {
+                let (at, copy) = (window.mouse_position(), window.modifiers().secondary(cx));
+                let used = drop_editor.update(cx, |e, cx| {
+                    e.drop_overflow(drag.range.clone(), &drag.text, at, copy, cx)
+                });
+                if used {
+                    window.focus(&drop_focus, cx);
+                }
+            })
             // The editor's own right-click handler runs first and picks the target.
             .context_menu(move |menu, _, cx| {
                 let ghost = menu_editor.read(cx).ghost_menu_label();
```

- [ ] **Step 4: Run the tests (expect success)**

Run: `cargo fmt --all && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: 150 app tests and 145 core tests pass; clippy counts match the baseline. Launch check as in Task 1.

- [ ] **Step 5: Commit**

```bash
jj describe -m "feat: drag a scratchpad block's grip onto the page (Ctrl copies)

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
--- a/README.md
+++ b/README.md
@@ -1,7 +1,7 @@
 # Tulis
 
 A writing app for playing with your words: keep alternates inline, ghost passages to judge a
-paragraph without them, stash text in an overflow panel, and ask an AI for alternatives or cuts
+paragraph without them, stash text in an Overflow scratchpad, and ask an AI for alternatives or cuts
 (never rewrites). Built with Rust and [GPUI](https://www.gpui.rs/).
 
 ## Run
@@ -25,8 +25,10 @@ paragraph that still matches and tells you how many paragraphs lost theirs.
 | `Ctrl+Enter` | Accept: keep only the shown alternate/version |
 | `Ctrl+Shift+Backspace` | Delete the shown alternate/version |
 | `Ctrl+/` (or `Ctrl+G`) | Ghost / revive the selection or paragraph |
-| `Ctrl+Shift+X` (or `Ctrl+Shift+O`) | Stash the selection or paragraph in overflow |
-| `Ctrl+\` | Show/hide the overflow panel |
+| `Ctrl+Shift+X` (or `Ctrl+Shift+O`) | Stash the selection or paragraph in Overflow (appended after a blank line; the panel stays closed) |
+| `Ctrl+\` | Show/hide the Overflow scratchpad: free text, blocks separated by blank lines, with its own undo |
+| `Ctrl+Shift+\` | Move focus between the page and the scratchpad (opens it); `Esc` in the scratchpad returns to the page |
+| `Ctrl+Enter` / `Ctrl+Shift+Enter` (in the scratchpad) | Move / copy its selection, or the block at its cursor, to the page cursor. Or drag a block's `⠿` grip onto the page (hold `Ctrl` to copy) |
 | `Ctrl+Shift+G` (or `Ctrl+J`) | AI alternatives for the selection, else the paragraph (as versions); an open sidebar follows to that target. In the sidebar (or its "✦ Ask AI" row): for the sidebar's target |
 | `Ctrl+Shift+T` / `Ctrl+Shift+Alt+T` | AI trim suggestions for the paragraph / document; `Tab` accepts (ghosts), `Esc` dismisses |
 | `Ctrl+Alt+V` | Toggle vim mode |
@@ -39,8 +41,8 @@ paragraph that still matches and tells you how many paragraphs lost theirs.
 | `Ctrl+S` / `Ctrl+Shift+S` / `Ctrl+O` / `Ctrl+N` / `Ctrl+Q` | Save / Save as / Open / New / Quit |
 
 Vim mode: `h j k l w b e 0 ^ $ gg G { }` with counts; `i a I A o O`; `d c y` + motion, `dd cc yy`,
-`x p P u Ctrl+R`; `v`/`V`; `]a`/`[a` cycle alternates; `gh` toggles ghost; `ga` sidebar, `gA` AI alternatives, `gs` stash (in Visual mode these act on the selection); `gt gm gp g? gl gx`
-(theme, markers, preview, cheat sheet, Lab, Overflow). In `V` (VisualLine) mode `ga gA gs gh` act on whole paragraphs.
+`x p P u Ctrl+R`; `v`/`V`; `]a`/`[a` cycle alternates; `gh` toggles ghost; `ga` sidebar, `gA` AI alternatives, `gs` stash (in Visual mode these act on the selection); `gt gm gp g? gl gx gX`
+(theme, markers, preview, cheat sheet, Lab, Overflow, focus Overflow). In `V` (VisualLine) mode `ga gA gs gh` act on whole paragraphs; `Vgs` removes them, like `Vd`.
 
 Sentence tab: sentences end at `.`, `!` or `?` followed by a space or the end of the paragraph.
 Known gaps: a sentence ending in an abbreviation such as `etc.` or `e.g.` runs on into the next one;
@@ -87,7 +89,7 @@ cargo test --workspace
 - [ ] Caret stays visible when typing past the bottom; scrolling with the wheel works
 - [ ] `Alt+↑/↓` swaps a group's options; dotted underline on alternates; `alt i/n` badge in the header
 - [ ] `Ctrl+/` (and `Ctrl+G`) ghosts to ~10% and back; ghosted text absent from saved `.md`
-- [ ] Overflow: send, panel toggle, click to insert, × to remove, "keep after insert"
+- [ ] Overflow: stash, panel toggle, type in the scratchpad, Ctrl+Enter / drag back to the page
 - [ ] Save/Open round-trips alternates, ghosts, overflow; external edit shows drift notice
 - [ ] Unsaved-changes prompt on New/Open/Quit/window close
 - [ ] AI alternates (violet underline) and trim (red strike, Tab/Esc) with a real key; missing key notice without
@@ -117,6 +119,13 @@ cargo test --workspace
 - [ ] Right-click a word: the menu targets it (or the selection); Ghost it turns into Revive it on ghosted text; Stash moves it to Overflow
 - [ ] Ctrl+Z / Ctrl+Shift+Z work while the sidebar has focus; Esc on the page (with no AI request, trims or alternate edit pending) closes the sidebar
 - [ ] Shift+F10 or the Menu key opens the right-click menu at the caret
+- [ ] XYZ / Ctrl+\ / `gx` opens a 260px Overflow panel on the right: italic "Overflow" title, a text field, and "ctrl+↵ or drag into the page to use" below it
+- [ ] Ctrl+Shift+X / `gs` / the menu stash appends after a blank line with a "Stashed to Overflow" toast and doesn't open the panel; `Vgs` leaves no empty paragraph
+- [ ] Typing in the scratchpad marks the file unsaved; its Ctrl+Z undoes its own typing; page Ctrl+Z leaves it alone
+- [ ] Ctrl+Enter in the scratchpad moves the block at its cursor (or its selection) to the page cursor and focuses the page; one page Ctrl+Z puts it back; Ctrl+Shift+Enter copies and keeps focus
+- [ ] Hovering a block shows a `⠿` grip next to its first line; dragging it onto the page inserts it where it's dropped (Ctrl held copies)
+- [ ] Ctrl+Shift+\ / `gX` moves focus page ↔ scratchpad; Esc in the scratchpad returns to the page
+- [ ] A file saved by an older Tulis opens with its overflow items joined into the scratchpad; saving writes sidecar version 2
 
 ## License
 
```

- [ ] **Step 2: Full verification**

Run: `cargo fmt --all --check && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: formatting is clean, 150 app tests and 145 core tests pass, and the clippy counts match the baseline.

Run: `cargo build && timeout 6 target/debug/tulis /tmp/smoke.md; echo $?`
Expected: `124`, with nothing on stderr.

Then walk through the new smoke-checklist items by hand. Report any that fail rather than ticking them.

- [ ] **Step 3: Commit**

```bash
jj describe -m "docs: README keys and smoke checklist for redesign part 3

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

## Decisions this plan makes where the spec is silent or ambiguous

- **Page undo and the scratchpad.** Undo snapshots keep the whole document, so one Ctrl+Z undoes a stash or a Ctrl+Enter on both sides. Scratchpad typing is written into every stored step (`History::for_each_state`), so page undo and redo never revert it. As a result, undoing an older stash after editing the scratchpad brings the text back to the page and leaves it in the scratchpad too.
- **The field's own undo** covers typing in it. When the text changes from outside (stash, Ctrl+Enter, page undo, open), `set_value` replaces it and starts its undo history afresh.
- **A stash is one block:** the stashed lines without blank lines, trimmed, appended after one blank line. A multi-paragraph stash keeps single line breaks, so Ctrl+Enter splits it back into paragraphs. An empty selection stashes the caret's paragraph and removes it, as v1 did. The page always keeps one paragraph.
- **`Vgs`** removes the selected paragraphs (`stash_paragraphs`), the same as `Vd`.
- **Blocks** are runs of non-blank lines; whitespace-only lines count as blank. `take_overflow` joins the gap with one blank line if either side had one, else with one line break, and leaves none at either end of the scratchpad.
- **Ctrl+Enter** inserts the trimmed text at the page caret (the head of a page selection, which is not replaced) as one undo step, and focuses the page. Ctrl+Shift+Enter copies and keeps focus in the scratchpad. A cursor on a blank line, or a whitespace-only selection, toasts "Nothing to use here".
- **Ctrl+Enter's binding** lives in `Scratchpad > Input`, so it wins over the field's own Ctrl+Enter (which would insert a line break) and doesn't touch the page's Ctrl+Enter (keep the shown alternate).
- **Grips** show for the block under the pointer only, in a gutter left of the field, and only when the block's first line is in view. A drop that lands on the page outside the text column snaps to the nearest position. A drag whose block changed in the meantime does nothing.
- **Focus toggle** also opens the panel. Hiding the panel (Ctrl+\, XYZ, `gx`) while the field has focus gives the page focus. Esc in the scratchpad returns to the page and leaves the panel open.
- **Version 1 migration** joins item texts exactly, in order, with `"\n\n"`; item ids, dates and source paragraphs are dropped. A version 1 sidecar with an unreadable overflow list becomes an empty scratchpad rather than an ignored sidecar.
- **The panel is created lazily**, the first time it opens, like the sidebar. The hint reads "ctrl+↵ or drag into the page to use" on every platform, as the spec gives it.
- **`chrono`** had no users left after `OverflowItem` went, so it is removed from both crates.
