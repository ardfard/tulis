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
