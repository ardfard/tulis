//! The Lab, from the editor's side: trim previews, the runner and its cancel flag.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use gpui::{Entity, TestAppContext, VisualTestContext};
use tulis_core::ai::{AiError, CompletionRequest, LlmProvider, MockProvider};
use tulis_core::vim::Vim;
use tulis_core::{DocPos, ParaId, Selection};

use super::lab::{BarAction, Check, TrimSuggestion};
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

fn bar_title(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> Option<String> {
    editor.read_with(cx, |e, _| e.lab_bar().map(|bar| bar.title))
}

#[gpui::test]
fn the_bar_counts_words_before_and_after_the_active_cuts(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I really love tea.", "It is very, very good."]);
    assert_eq!(bar_title(&editor, cx), None);
    preview(&editor, cx, &[(0, 2..9), (1, 6..12)]);
    assert_eq!(
        bar_title(&editor, cx).as_deref(),
        Some("Slight trim 9 → 7 words · −22%")
    );
    let bar = editor.read_with(cx, |e, _| e.lab_bar()).unwrap();
    assert_eq!(
        bar.hint.as_deref(),
        Some("Faded words would go. Click one to keep it.")
    );
    assert_eq!(
        bar.actions,
        vec![BarAction::MakeCuts, BarAction::Walk, BarAction::Done]
    );
    // A kept cut counts again.
    editor.update(cx, |e, cx| assert!(e.toggle_cut_at(DocPos::new(0, 4), cx)));
    assert_eq!(
        bar_title(&editor, cx).as_deref(),
        Some("Slight trim 9 → 8 words · −11%")
    );
}

#[gpui::test]
fn clicking_or_enter_keeps_a_cut_and_clicking_again_reactivates_it(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I really love tea."]);
    preview(&editor, cx, &[(0, 2..9)]);
    let active = |cx: &mut VisualTestContext| {
        editor.read_with(cx, |e, _| e.trim_preview.as_ref().unwrap().is_active(0))
    };
    // Enter with the cursor in a cut keeps it, and doesn't split the paragraph.
    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(0, 5)));
    editor.update_in(cx, |e, window, cx| e.newline(&Newline, window, cx));
    assert!(!active(cx));
    assert_eq!(texts(&editor, cx), vec!["I really love tea."]);
    // A click on a kept cut makes it active again; a click elsewhere does nothing.
    editor.update(cx, |e, cx| assert!(e.toggle_cut_at(DocPos::new(0, 3), cx)));
    assert!(active(cx));
    editor.update(cx, |e, cx| {
        assert!(!e.toggle_cut_at(DocPos::new(0, 12), cx))
    });
    assert!(active(cx));
    // Enter outside every cut starts a new paragraph as usual.
    editor.update(cx, |e, _| {
        e.selection = Selection::caret(DocPos::new(0, 13))
    });
    editor.update_in(cx, |e, window, cx| e.newline(&Newline, window, cx));
    assert_eq!(texts(&editor, cx), vec!["I really love", " tea."]);
}

#[gpui::test]
fn make_the_cuts_ghosts_every_active_cut_in_one_undo_step(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I really love tea, truly.", "It is very good."]);
    preview(&editor, cx, &[(0, 2..9), (0, 17..24), (1, 6..11)]);
    editor.update(cx, |e, cx| assert!(e.toggle_cut_at(DocPos::new(0, 20), cx)));
    editor.update(cx, |e, cx| e.bar_action(BarAction::MakeCuts, cx));
    let exports = |cx: &mut VisualTestContext| {
        editor.read_with(cx, |e, _| {
            e.doc
                .paragraphs
                .iter()
                .map(|p| p.export_text())
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(exports(cx), vec!["I love tea, truly.", "It is good."]);
    assert!(editor.read_with(cx, |e, _| e.trim_preview.is_none()));
    assert_eq!(
        notice(&editor, cx).as_deref(),
        Some("Cut 2 passages (ghosted — Ctrl+Z to undo)")
    );
    // The rendered text is unchanged; one undo revives every cut.
    assert_eq!(
        texts(&editor, cx),
        vec!["I really love tea, truly.", "It is very good."]
    );
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(
        exports(cx),
        vec!["I really love tea, truly.", "It is very good."]
    );
}

#[gpui::test]
fn done_clears_the_preview_without_changes(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I really love tea."]);
    preview(&editor, cx, &[(0, 2..9)]);
    editor.update(cx, |e, cx| e.bar_action(BarAction::Done, cx));
    assert!(editor.read_with(cx, |e, _| e.trim_preview.is_none()));
    assert!(!editor.read_with(cx, |e, _| e.dirty));
    assert_eq!(texts(&editor, cx), vec!["I really love tea."]);
}

fn exports(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> Vec<String> {
    editor.read_with(cx, |e, _| {
        e.doc.paragraphs.iter().map(|p| p.export_text()).collect()
    })
}

fn selected(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> (DocPos, DocPos) {
    editor.read_with(cx, |e, _| {
        let r = e.selection.range();
        (r.start, r.end)
    })
}

#[gpui::test]
fn walking_through_cuts_or_keeps_each_cut_in_turn(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I really love tea, truly.", "It is very good."]);
    preview(&editor, cx, &[(0, 2..9), (0, 17..24), (1, 6..11)]);
    editor.update(cx, |e, cx| e.bar_action(BarAction::Walk, cx));
    assert_eq!(bar_title(&editor, cx).as_deref(), Some("Cut 1 of 3"));
    let bar = editor.read_with(cx, |e, _| e.lab_bar()).unwrap();
    assert_eq!(
        bar.hint.as_deref(),
        Some("y or Enter cuts it · n or Backspace keeps it · Esc stops")
    );
    // The current cut is selected (and scrolled to).
    assert_eq!(
        selected(&editor, cx),
        (DocPos::new(0, 2), DocPos::new(0, 9))
    );
    // `y` cuts it, without vim and without typing.
    type_str(&editor, cx, "y");
    assert_eq!(exports(&editor, cx)[0], "I love tea, truly.");
    assert_eq!(bar_title(&editor, cx).as_deref(), Some("Cut 2 of 3"));
    assert_eq!(
        selected(&editor, cx),
        (DocPos::new(0, 17), DocPos::new(0, 24))
    );
    // Backspace keeps it.
    editor.update_in(cx, |e, window, cx| e.backspace(&Backspace, window, cx));
    assert_eq!(bar_title(&editor, cx).as_deref(), Some("Cut 3 of 3"));
    // Enter cuts the last one, which ends the walk and the preview.
    editor.update_in(cx, |e, window, cx| e.newline(&Newline, window, cx));
    assert_eq!(
        exports(&editor, cx),
        vec!["I love tea, truly.", "It is good."]
    );
    assert!(editor.read_with(cx, |e, _| e.trim_preview.is_none()));
    assert_eq!(
        notice(&editor, cx).as_deref(),
        Some("Walked through 3 cuts: 2 cut, 1 kept")
    );
    // Each cut was its own undo step.
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(
        exports(&editor, cx),
        vec!["I love tea, truly.", "It is very good."]
    );
    // Now `y` is typed again.
    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(1, 0)));
    type_str(&editor, cx, "y");
    assert_eq!(texts(&editor, cx)[1], "yIt is very good.");
}

#[gpui::test]
fn escape_stops_the_walk_and_leaves_the_rest_of_the_preview(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I really love tea, truly."]);
    preview(&editor, cx, &[(0, 2..9), (0, 17..24)]);
    editor.update(cx, |e, cx| e.bar_action(BarAction::Walk, cx));
    type_str(&editor, cx, "n");
    editor.update_in(cx, |e, window, cx| e.cancel(&Cancel, window, cx));
    let preview = editor.read_with(cx, |e, _| e.trim_preview.clone()).unwrap();
    assert!(preview.walk.is_none());
    assert!(!preview.is_active(0) && preview.is_active(1));
    assert_eq!(
        bar_title(&editor, cx).as_deref(),
        Some("Slight trim 5 → 4 words · −20%")
    );
    // `y` and `n` are text again.
    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(0, 0)));
    type_str(&editor, cx, "n");
    assert_eq!(texts(&editor, cx), vec!["nI really love tea, truly."]);
}

fn marks(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> Vec<(usize, String, String)> {
    editor.read_with(cx, |e, _| {
        e.marks
            .iter()
            .map(|m| {
                let para = e.doc.para_index(m.para_id).unwrap();
                let text = e.doc.paragraphs[para].text()[m.range.clone()].to_string();
                (para, text, m.note.clone())
            })
            .collect()
    })
}

#[gpui::test]
fn the_long_sentence_check_marks_locally(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(
        cx,
        &[
            "Short one. This one has more than five words in it.",
            "Tiny.",
        ],
    );
    let mock = mock(&editor, cx, &[]);
    editor.update(cx, |e, _| e.config.editor.long_sentence_words = 5);
    let all = ids(&editor, cx);
    editor.update(cx, |e, cx| e.run_check(Check::Long, &all, cx));
    assert!(mock.requests.lock().unwrap().is_empty());
    assert!(!editor.read_with(cx, |e, _| e.lab_running()));
    assert_eq!(
        marks(&editor, cx),
        vec![(
            0,
            "This one has more than five words in it.".to_string(),
            "9 words".to_string()
        )]
    );
    let bar = editor.read_with(cx, |e, _| e.lab_bar()).unwrap();
    assert_eq!(bar.title, "1 marked · long sentences");
    assert_eq!(bar.actions, vec![BarAction::Clear]);
    // Clear removes the marks.
    editor.update(cx, |e, cx| e.bar_action(BarAction::Clear, cx));
    assert!(marks(&editor, cx).is_empty());
    assert!(editor.read_with(cx, |e, _| e.lab_bar()).is_none());
}

#[gpui::test]
fn ai_mark_checks_locate_exact_text_and_drop_invented_text(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I think it is fine.", "", "Maybe so, really."]);
    let mock = mock(
        &editor,
        cx,
        &[
            r#"[{"text": "I think", "note": "hedge"}]"#,
            r#"[{"text": "really", "note": "filler"}, {"text": "perhaps", "note": "invented"}]"#,
        ],
    );
    let all = ids(&editor, cx);
    editor.update(cx, |e, cx| e.run_check(Check::Hedges, &all, cx));
    cx.run_until_parked();
    let requests = mock.requests.lock().unwrap().clone();
    assert_eq!(requests.len(), 2);
    assert!(requests[0].system.contains("hedges"));
    assert_eq!(
        marks(&editor, cx),
        vec![
            (0, "I think".to_string(), "hedge".to_string()),
            (2, "really".to_string(), "filler".to_string()),
        ]
    );
    assert_eq!(
        bar_title(&editor, cx).as_deref(),
        Some("2 marked · hedges and filler")
    );
    // The tone check sends an excerpt of the whole document along.
    let mock = self::mock(&editor, cx, &["[]"]);
    editor.update(cx, |e, cx| e.run_check(Check::Tone, &all[2..], cx));
    cx.run_until_parked();
    let request = mock.requests.lock().unwrap()[0].clone();
    assert!(request.user.contains("I think it is fine."));
    assert!(request.user.ends_with("Paragraph:\nMaybe so, really."));
    // Nothing found: the marks go, with a toast.
    assert!(marks(&editor, cx).is_empty());
    assert_eq!(notice(&editor, cx).as_deref(), Some("Nothing marked"));
}

#[gpui::test]
fn tab_moves_between_marks_and_the_bar_shows_the_note_under_the_cursor(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I think it is fine.", "Maybe so, really."]);
    mock(
        &editor,
        cx,
        &[
            r#"[{"text": "I think", "note": "hedge"}]"#,
            r#"[{"text": "really", "note": "filler"}]"#,
        ],
    );
    let all = ids(&editor, cx);
    editor.update(cx, |e, cx| e.run_check(Check::Hedges, &all, cx));
    cx.run_until_parked();
    let hint = |cx: &mut VisualTestContext| editor.read_with(cx, |e, _| e.lab_bar().unwrap().hint);
    editor.update(cx, |e, _| {
        e.selection = Selection::caret(DocPos::new(0, 10))
    });
    assert_eq!(hint(cx), None);
    editor.update_in(cx, |e, window, cx| e.next_mark(&NextMark, window, cx));
    assert_eq!(
        editor.read_with(cx, |e, _| e.selection),
        Selection::caret(DocPos::new(1, 10))
    );
    assert_eq!(hint(cx).as_deref(), Some("filler"));
    editor.update_in(cx, |e, window, cx| e.prev_mark(&PrevMark, window, cx));
    assert_eq!(hint(cx).as_deref(), Some("hedge"));
    // Editing a paragraph drops its marks; Esc clears the rest.
    type_str(&editor, cx, "Oh, ");
    assert_eq!(marks(&editor, cx).len(), 1);
    editor.update_in(cx, |e, window, cx| e.cancel(&Cancel, window, cx));
    assert!(marks(&editor, cx).is_empty());
    assert_eq!(notice(&editor, cx).as_deref(), Some("Marks cleared"));
}

#[gpui::test]
fn ctrl_shift_g_inside_a_mark_targets_the_mark(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I think it is fine."]);
    let mock = mock(
        &editor,
        cx,
        &[r#"[{"text": "I think", "note": "hedge"}]"#, r#"["I hold"]"#],
    );
    let all = ids(&editor, cx);
    editor.update(cx, |e, cx| e.run_check(Check::Hedges, &all, cx));
    cx.run_until_parked();
    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(0, 3)));
    editor.update_in(cx, |e, window, cx| {
        e.ai_alternates(&AiAlternates, window, cx)
    });
    cx.run_until_parked();
    let request = mock.requests.lock().unwrap()[1].clone();
    assert!(request.user.ends_with("Passage to rewrite:\nI think"));
    assert_eq!(texts(&editor, cx), vec!["I hold it is fine."]);
}

#[gpui::test]
fn a_check_replaces_the_trim_preview_and_a_trim_replaces_the_marks(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["One two three four five six seven."]);
    preview(&editor, cx, &[(0, 4..8)]);
    editor.update(cx, |e, _| e.config.editor.long_sentence_words = 3);
    let all = ids(&editor, cx);
    editor.update(cx, |e, cx| e.run_check(Check::Long, &all, cx));
    assert!(editor.read_with(cx, |e, _| e.trim_preview.is_none()));
    assert_eq!(marks(&editor, cx).len(), 1);
    mock(&editor, cx, &[r#"["two "]"#]);
    editor.update(cx, |e, cx| e.run_trim(TrimLevel::Slight, &all, cx));
    cx.run_until_parked();
    assert!(marks(&editor, cx).is_empty());
    assert!(editor.read_with(cx, |e, _| e.trim_preview.is_some()));
}

#[gpui::test]
fn fixes_become_ai_alternates_in_one_undo_step(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Its a nice day, its true.", "Teh end."]);
    let mock = mock(
        &editor,
        cx,
        &[
            r#"[{"original": "Its", "fixed": "It's"}, {"original": "a nice dya", "fixed": "x"}]"#,
            r#"[{"original": "Teh", "fixed": "The"}]"#,
        ],
    );
    let all = ids(&editor, cx);
    editor.update(cx, |e, cx| e.run_check(Check::Fix, &all, cx));
    cx.run_until_parked();
    assert!(
        mock.requests.lock().unwrap()[0]
            .system
            .contains("proofreader")
    );
    assert_eq!(
        texts(&editor, cx),
        vec!["It's a nice day, its true.", "The end."]
    );
    editor.read_with(cx, |e, _| {
        let info = e
            .doc
            .alt_info(DocPos::new(0, 1))
            .expect("an alternate group");
        assert_eq!(info.count, 2);
        assert!(e.doc.paragraphs[0].segments()[0].active_is_ai());
    });
    assert_eq!(
        bar_title(&editor, cx).as_deref(),
        Some("2 fixes proposed — Alt+↑/↓ to compare")
    );
    // The original is one Alt+↑ away.
    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(1, 1)));
    editor.update_in(cx, |e, window, cx| e.cycle_prev(&CyclePrev, window, cx));
    assert_eq!(texts(&editor, cx)[1], "Teh end.");
    // Clear hides the bar; the alternates stay.
    editor.update(cx, |e, cx| e.bar_action(BarAction::Clear, cx));
    assert!(editor.read_with(cx, |e, _| e.lab_bar()).is_none());
    // One undo step for every fix (and one for the cycle).
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(
        texts(&editor, cx),
        vec!["Its a nice day, its true.", "Teh end."]
    );
    assert!(editor.read_with(cx, |e, _| e.doc.alt_info(DocPos::new(0, 1)).is_none()));
}

#[gpui::test]
fn a_fix_check_with_nothing_to_fix_changes_nothing(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["All good."]);
    mock(&editor, cx, &["[]"]);
    let all = ids(&editor, cx);
    editor.update(cx, |e, cx| e.run_check(Check::Fix, &all, cx));
    cx.run_until_parked();
    assert_eq!(notice(&editor, cx).as_deref(), Some("No fixes needed"));
    assert!(!editor.read_with(cx, |e, _| e.dirty));
    assert!(editor.read_with(cx, |e, _| e.lab_bar()).is_none());
}
