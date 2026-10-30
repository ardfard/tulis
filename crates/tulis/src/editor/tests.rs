use gpui::{Entity, EntityInputHandler, TestAppContext, VisualTestContext};
use tulis_core::config::Config;
use tulis_core::{DocPos, Document, Selection};

use super::{AltEdit, Editor, TrimSuggestion};
use crate::actions::*;
use tulis_core::OverflowId;

pub(crate) fn editor_with<'a>(
    cx: &'a mut TestAppContext,
    texts: &[&str],
) -> (Entity<Editor>, &'a mut VisualTestContext) {
    let doc = Document::from_texts(texts);
    cx.add_window_view(|_, cx| Editor::new(Config::default(), doc, cx))
}

pub(crate) fn texts(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> Vec<String> {
    editor.read_with(cx, |e, _| {
        e.doc.paragraphs.iter().map(|p| p.text()).collect()
    })
}

pub(crate) fn type_str(editor: &Entity<Editor>, cx: &mut VisualTestContext, text: &str) {
    editor.update_in(cx, |e, window, cx| {
        for ch in text.chars() {
            e.replace_text_in_range(None, &ch.to_string(), window, cx);
        }
    });
}

fn select(editor: &Entity<Editor>, cx: &mut VisualTestContext, a: DocPos, b: DocPos) {
    editor.update(cx, |e, _| e.selection = Selection { anchor: a, head: b });
}

#[gpui::test]
fn typing_inserts_and_undo_restores(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &[""]);
    type_str(&editor, cx, "hi");
    assert_eq!(texts(&editor, cx), vec!["hi"]);
    assert!(editor.read_with(cx, |e, _| e.dirty));
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(texts(&editor, cx), vec![""]);
    editor.update_in(cx, |e, window, cx| e.redo(&Redo, window, cx));
    assert_eq!(texts(&editor, cx), vec!["hi"]);
}

#[gpui::test]
fn enter_splits_and_backspace_joins(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["hello world"]);
    select(&editor, cx, DocPos::new(0, 5), DocPos::new(0, 5));
    editor.update_in(cx, |e, window, cx| e.newline(&Newline, window, cx));
    assert_eq!(texts(&editor, cx), vec!["hello", " world"]);
    assert_eq!(editor.read_with(cx, |e, _| e.cursor()), DocPos::new(1, 0));
    editor.update_in(cx, |e, window, cx| e.backspace(&Backspace, window, cx));
    assert_eq!(texts(&editor, cx), vec!["hello world"]);
    assert_eq!(editor.read_with(cx, |e, _| e.cursor()), DocPos::new(0, 5));
}

#[gpui::test]
fn replace_selection_cut_and_paste(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["hello world"]);
    editor.update_in(cx, |e, window, cx| e.select_all(&SelectAll, window, cx));
    type_str(&editor, cx, "x");
    assert_eq!(texts(&editor, cx), vec!["x"]);
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(texts(&editor, cx), vec!["hello world"]);
    select(&editor, cx, DocPos::new(0, 0), DocPos::new(0, 5));
    editor.update_in(cx, |e, window, cx| e.cut(&Cut, window, cx));
    assert_eq!(texts(&editor, cx), vec![" world"]);
    editor.update_in(cx, |e, window, cx| {
        e.doc_end(&DocEnd, window, cx);
        e.paste(&Paste, window, cx);
    });
    assert_eq!(texts(&editor, cx), vec![" worldhello"]);
}

#[gpui::test]
fn word_and_document_motions(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["one two", "three"]);
    editor.update_in(cx, |e, window, cx| e.doc_end(&DocEnd, window, cx));
    assert_eq!(editor.read_with(cx, |e, _| e.cursor()), DocPos::new(1, 5));
    editor.update_in(cx, |e, window, cx| e.word_left(&WordLeft, window, cx));
    assert_eq!(editor.read_with(cx, |e, _| e.cursor()), DocPos::new(1, 0));
    editor.update_in(cx, |e, window, cx| e.word_left(&WordLeft, window, cx));
    assert_eq!(editor.read_with(cx, |e, _| e.cursor()), DocPos::new(0, 4));
    editor.update_in(cx, |e, window, cx| {
        e.select_word_right(&SelectWordRight, window, cx)
    });
    let sel = editor.read_with(cx, |e, _| e.selection);
    assert_eq!(
        (sel.anchor, sel.head),
        (DocPos::new(0, 4), DocPos::new(1, 0))
    );
}

#[gpui::test]
fn alt_editing_keeps_typing_inside_the_group(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    editor.update(cx, |e, _| {
        e.doc.make_alt(0, 2..6).unwrap();
        e.selection = Selection {
            anchor: DocPos::new(0, 2),
            head: DocPos::new(0, 6),
        };
        e.alt_edit = Some(AltEdit {
            para_id: e.doc.paragraphs[0].id,
            start: 2,
        });
    });
    type_str(&editor, cx, "adore");
    assert_eq!(texts(&editor, cx), vec!["I adore tea"]);
    let info = editor.read_with(cx, |e, _| e.doc.alt_info(e.cursor()).unwrap());
    assert_eq!((info.index, info.count), (1, 2));
    editor.update_in(cx, |e, window, cx| {
        e.right(&Right, window, cx);
        e.right(&Right, window, cx);
    });
    assert!(editor.read_with(cx, |e, _| e.alt_edit.is_none()));
    assert_eq!(texts(&editor, cx), vec!["I adore tea"]);
}

#[gpui::test]
fn emptied_alternate_restores_previous_option_when_leaving(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    editor.update(cx, |e, _| {
        e.doc.make_alt(0, 2..6).unwrap();
        e.selection = Selection {
            anchor: DocPos::new(0, 2),
            head: DocPos::new(0, 6),
        };
        e.alt_edit = Some(AltEdit {
            para_id: e.doc.paragraphs[0].id,
            start: 2,
        });
    });
    editor.update_in(cx, |e, window, cx| e.backspace(&Backspace, window, cx));
    assert_eq!(texts(&editor, cx), vec!["I  tea"]);
    editor.update_in(cx, |e, window, cx| e.right(&Right, window, cx));
    assert_eq!(texts(&editor, cx), vec!["I love tea"]);
    assert_eq!(editor.read_with(cx, |e, _| e.cursor()), DocPos::new(0, 7));
    assert!(editor.read_with(cx, |e, _| !e.doc.paragraphs[0].has_metadata()));
}

#[gpui::test]
fn ime_marked_text_is_replaced_on_commit(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &[""]);
    editor.update_in(cx, |e, window, cx| {
        e.replace_and_mark_text_in_range(None, "ka", None, window, cx);
        assert_eq!(e.marked, Some(0..2));
        e.replace_text_in_range(None, "か", window, cx);
        assert!(e.marked.is_none());
    });
    assert_eq!(texts(&editor, cx), vec!["か"]);
}

#[gpui::test]
fn undo_never_restores_an_empty_alternate(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    editor.update(cx, |e, _| {
        e.doc.make_alt(0, 2..6).unwrap();
        e.selection = Selection {
            anchor: DocPos::new(0, 2),
            head: DocPos::new(0, 6),
        };
        e.alt_edit = Some(AltEdit {
            para_id: e.doc.paragraphs[0].id,
            start: 2,
        });
    });
    editor.update_in(cx, |e, window, cx| e.backspace(&Backspace, window, cx));
    type_str(&editor, cx, "x");
    for _ in 0..3 {
        editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
        editor.read_with(cx, |e, _| {
            assert!(e.alt_edit.is_none());
            for p in &e.doc.paragraphs {
                assert!(
                    p.segments()
                        .iter()
                        .all(|s| !(s.is_alt() && s.text().is_empty())),
                    "empty alternate restored"
                );
            }
        });
    }
}

#[gpui::test]
fn make_alternate_then_type_new_wording_and_cycle_back(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    select(&editor, cx, DocPos::new(0, 2), DocPos::new(0, 6));
    editor.update_in(cx, |e, window, cx| {
        e.make_alternate(&MakeAlternate, window, cx)
    });
    type_str(&editor, cx, "adore");
    assert_eq!(texts(&editor, cx), vec!["I adore tea"]);
    editor.update_in(cx, |e, window, cx| e.cycle_prev(&CyclePrev, window, cx));
    assert_eq!(texts(&editor, cx), vec!["I love tea"]);
    editor.update_in(cx, |e, window, cx| e.cycle_next(&CycleNext, window, cx));
    assert_eq!(texts(&editor, cx), vec!["I adore tea"]);
    editor.update_in(cx, |e, window, cx| {
        e.accept_alternate(&AcceptAlternate, window, cx)
    });
    assert!(editor.read_with(cx, |e, _| !e.doc.paragraphs[0].has_metadata()));
}

#[gpui::test]
fn make_alternate_without_selection_creates_paragraph_version(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Draft one."]);
    editor.update_in(cx, |e, window, cx| {
        e.make_alternate(&MakeAlternate, window, cx)
    });
    editor.update_in(cx, |e, window, cx| {
        e.select_all(&SelectAll, window, cx);
        e.replace_text_in_range(None, "Draft two.", window, cx);
    });
    assert_eq!(texts(&editor, cx), vec!["Draft two."]);
    editor.update_in(cx, |e, window, cx| e.cycle_next(&CycleNext, window, cx));
    assert_eq!(texts(&editor, cx), vec!["Draft one."]);
    editor.update_in(cx, |e, window, cx| {
        e.delete_alternate(&DeleteAlternate, window, cx)
    });
    assert_eq!(texts(&editor, cx), vec!["Draft two."]);
    assert_eq!(
        editor.read_with(cx, |e, _| e.doc.paragraphs[0].versions.len()),
        1
    );
}

#[gpui::test]
fn ghost_toggle_on_paragraph_and_selection(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Keep. Maybe cut."]);
    select(&editor, cx, DocPos::new(0, 5), DocPos::new(0, 16));
    editor.update_in(cx, |e, window, cx| e.toggle_ghost(&ToggleGhost, window, cx));
    assert_eq!(
        editor.read_with(cx, |e, _| e.doc.paragraphs[0].export_text()),
        "Keep."
    );
    editor.update_in(cx, |e, window, cx| e.toggle_ghost(&ToggleGhost, window, cx));
    assert_eq!(
        editor.read_with(cx, |e, _| e.doc.paragraphs[0].export_text()),
        "Keep. Maybe cut."
    );
    select(&editor, cx, DocPos::new(0, 0), DocPos::new(0, 0));
    editor.update_in(cx, |e, window, cx| e.toggle_ghost(&ToggleGhost, window, cx));
    assert_eq!(editor.read_with(cx, |e, _| e.doc.word_count()), 0);
}

#[gpui::test]
fn overflow_round_trip(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["a", "b"]);
    editor.update_in(cx, |e, window, cx| {
        e.send_to_overflow(&SendToOverflow, window, cx)
    });
    assert_eq!(texts(&editor, cx), vec!["b"]);
    let id: OverflowId = editor.read_with(cx, |e, _| e.doc.overflow[0].id);
    editor.update(cx, |e, cx| e.insert_overflow(id, true, cx));
    assert_eq!(texts(&editor, cx), vec!["ab"]);
    assert!(editor.read_with(cx, |e, _| e.doc.overflow.is_empty()));
}

#[gpui::test]
fn accepting_a_trim_ghosts_it_and_escape_dismisses(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I really love tea, really."]);
    editor.update(cx, |e, _| {
        let id = e.doc.paragraphs[0].id;
        e.trims = vec![
            TrimSuggestion {
                para_id: id,
                range: 2..9,
            }, // "really "
            TrimSuggestion {
                para_id: id,
                range: 18..25,
            }, // " really"
        ];
    });
    // The cursor is at (0,0): the next suggestion at/after it is 2..9.
    editor.update_in(cx, |e, window, cx| e.accept_trim(&AcceptTrim, window, cx));
    assert_eq!(
        editor.read_with(cx, |e, _| e.doc.paragraphs[0].export_text()),
        "I love tea, really."
    );
    assert_eq!(editor.read_with(cx, |e, _| e.trims.len()), 1);
    editor.update_in(cx, |e, window, cx| e.cancel(&Cancel, window, cx));
    assert!(editor.read_with(cx, |e, _| e.trims.is_empty()));
}

use std::sync::Arc;
use tulis_core::ai::MockProvider;

fn with_mock(editor: &Entity<Editor>, cx: &mut VisualTestContext, responses: Vec<&str>) {
    let responses = responses.into_iter().map(|r| Ok(r.to_string())).collect();
    editor.update(cx, |e, _| {
        e.provider_override = Some(Arc::new(MockProvider::new(responses)))
    });
}

#[gpui::test]
fn ai_alternates_for_selection_are_ai_marked(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    with_mock(&editor, cx, vec![r#"["adore", "like"]"#]);
    select(&editor, cx, DocPos::new(0, 2), DocPos::new(0, 6));
    editor.update_in(cx, |e, window, cx| {
        e.ai_alternates(&AiAlternates, window, cx)
    });
    cx.run_until_parked();
    assert_eq!(texts(&editor, cx), vec!["I adore tea"]);
    editor.read_with(cx, |e, _| {
        assert!(e.doc.paragraphs[0].segments()[1].active_is_ai());
        assert!(e.ai_task.is_none());
    });
}

#[gpui::test]
fn ai_alternates_without_selection_add_paragraph_versions(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Tea is nice."]);
    with_mock(&editor, cx, vec![r#"["Tea is lovely.", "I like tea."]"#]);
    editor.update_in(cx, |e, window, cx| {
        e.ai_alternates(&AiAlternates, window, cx)
    });
    cx.run_until_parked();
    assert_eq!(texts(&editor, cx), vec!["Tea is lovely."]);
    assert_eq!(
        editor.read_with(cx, |e, _| e.doc.paragraphs[0].versions.len()),
        3
    );
}

#[gpui::test]
fn stale_ai_result_is_dropped(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    with_mock(&editor, cx, vec![r#"["adore"]"#]);
    select(&editor, cx, DocPos::new(0, 2), DocPos::new(0, 6));
    editor.update_in(cx, |e, window, cx| {
        e.ai_alternates(&AiAlternates, window, cx);
        e.selection = Selection::caret(DocPos::new(0, 0));
        e.replace_text_in_range(None, "X", window, cx);
    });
    cx.run_until_parked();
    assert_eq!(texts(&editor, cx), vec!["XI love tea"]);
    assert_eq!(
        editor.read_with(cx, |e, _| e.last_notice().map(str::to_string)),
        Some(super::ai::STALE_NOTICE.to_string())
    );
}

#[gpui::test]
fn ai_trim_suggests_cuts_that_tab_accepts(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I really love tea."]);
    with_mock(&editor, cx, vec![r#"["really "]"#]);
    editor.update_in(cx, |e, window, cx| {
        e.ai_trim_paragraph(&AiTrimParagraph, window, cx)
    });
    cx.run_until_parked();
    assert_eq!(editor.read_with(cx, |e, _| e.trims.clone()).len(), 1);
    editor.update_in(cx, |e, window, cx| e.accept_trim(&AcceptTrim, window, cx));
    assert_eq!(
        editor.read_with(cx, |e, _| e.doc.paragraphs[0].export_text()),
        "I love tea."
    );
}

#[gpui::test]
fn missing_api_key_is_reported(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    editor.update(cx, |e, _| {
        e.config.ai.api_key_env = "TULIS_TEST_NO_SUCH_KEY".into()
    });
    editor.update_in(cx, |e, window, cx| {
        e.ai_alternates(&AiAlternates, window, cx)
    });
    assert_eq!(
        editor.read_with(cx, |e, _| e.last_notice().map(str::to_string)),
        Some("Set $TULIS_TEST_NO_SUCH_KEY to use AI".to_string())
    );
}

use tulis_core::vim::{Mode, Vim};

fn vim_editor<'a>(
    cx: &'a mut TestAppContext,
    texts: &[&str],
) -> (Entity<Editor>, &'a mut VisualTestContext) {
    let (editor, cx) = editor_with(cx, texts);
    editor.update(cx, |e, _| e.vim = Some(Vim::new()));
    (editor, cx)
}

#[gpui::test]
fn vim_delete_word_undo_and_delete_char(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["hello world"]);
    type_str(&editor, cx, "dw");
    assert_eq!(texts(&editor, cx), vec!["world"]);
    assert_eq!(editor.read_with(cx, |e, _| e.register.clone()), "hello ");
    type_str(&editor, cx, "u");
    assert_eq!(texts(&editor, cx), vec!["hello world"]);
    type_str(&editor, cx, "x");
    assert_eq!(texts(&editor, cx), vec!["ello world"]);
    type_str(&editor, cx, "p");
    assert_eq!(texts(&editor, cx), vec!["ehllo world"]);
}

#[gpui::test]
fn vim_insert_mode_types_and_escape_returns_to_normal(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &[""]);
    type_str(&editor, cx, "iab");
    assert_eq!(texts(&editor, cx), vec!["ab"]);
    editor.update_in(cx, |e, window, cx| e.cancel(&Cancel, window, cx));
    editor.read_with(cx, |e, _| {
        assert_eq!(e.vim_mode(), Some(Mode::Normal));
        assert_eq!(e.cursor(), DocPos::new(0, 1));
    });
}

#[gpui::test]
fn vim_paragraph_delete_and_put(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["a", "b", "c"]);
    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(1, 0)));
    type_str(&editor, cx, "dd");
    assert_eq!(texts(&editor, cx), vec!["a", "c"]);
    type_str(&editor, cx, "p");
    assert_eq!(texts(&editor, cx), vec!["a", "c", "b"]);
    assert_eq!(editor.read_with(cx, |e, _| e.cursor()), DocPos::new(2, 0));
}

#[gpui::test]
fn vim_change_word_and_open_paragraph(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["old text"]);
    type_str(&editor, cx, "cwnew");
    assert_eq!(texts(&editor, cx), vec!["new text"]);
    editor.update_in(cx, |e, window, cx| e.cancel(&Cancel, window, cx));
    type_str(&editor, cx, "onext");
    assert_eq!(texts(&editor, cx), vec!["new text", "next"]);
}

#[gpui::test]
fn vim_visual_delete_is_inclusive(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["abcdef"]);
    type_str(&editor, cx, "lvlld");
    assert_eq!(texts(&editor, cx), vec!["aef"]);
    assert_eq!(
        editor.read_with(cx, |e, _| e.vim_mode()),
        Some(Mode::Normal)
    );
}

#[gpui::test]
fn vim_alternate_and_ghost_shortcuts(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["I love tea"]);
    editor.update(cx, |e, _| {
        e.doc.make_alt(0, 2..6).unwrap();
        e.doc.edit_alt(0, 1, 0..4, "adore");
        e.selection = Selection::caret(DocPos::new(0, 3));
    });
    type_str(&editor, cx, "[a");
    assert_eq!(texts(&editor, cx), vec!["I love tea"]);
    type_str(&editor, cx, "gh");
    assert_eq!(editor.read_with(cx, |e, _| e.doc.word_count()), 0);
}

#[gpui::test]
fn toggle_vim_switches_mode(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["x"]);
    editor.update_in(cx, |e, window, cx| e.toggle_vim(&ToggleVim, window, cx));
    assert_eq!(
        editor.read_with(cx, |e, _| e.vim_mode()),
        Some(Mode::Normal)
    );
    editor.update_in(cx, |e, window, cx| e.toggle_vim(&ToggleVim, window, cx));
    assert_eq!(editor.read_with(cx, |e, _| e.vim_mode()), None);
}

#[gpui::test]
fn vim_dw_on_last_word_stays_within_the_paragraph(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["one two", "three"]);
    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(0, 4)));
    type_str(&editor, cx, "dw");
    assert_eq!(texts(&editor, cx), vec!["one ", "three"]);
    assert_eq!(editor.read_with(cx, |e, _| e.register.clone()), "two");
}

#[gpui::test]
fn vim_cw_on_last_word_stays_within_the_paragraph(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["one two", "three"]);
    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(0, 4)));
    type_str(&editor, cx, "cwx");
    assert_eq!(texts(&editor, cx), vec!["one x", "three"]);
}

/// Make "world" (6..11) an alternate and empty it with Backspace.
fn make_alt_and_empty_it(editor: &Entity<Editor>, cx: &mut VisualTestContext, emptied: &str) {
    select(editor, cx, DocPos::new(0, 6), DocPos::new(0, 11));
    editor.update_in(cx, |e, window, cx| {
        e.make_alternate(&MakeAlternate, window, cx);
        e.backspace(&Backspace, window, cx);
    });
    assert_eq!(texts(editor, cx), vec![emptied]);
}

#[gpui::test]
fn backspace_before_emptied_alternate_keeps_restored_text(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Hello world"]);
    make_alt_and_empty_it(&editor, cx, "Hello ");
    editor.update_in(cx, |e, window, cx| e.backspace(&Backspace, window, cx));
    assert_eq!(texts(&editor, cx), vec!["Helloworld"]);
    assert_eq!(editor.read_with(cx, |e, _| e.cursor()), DocPos::new(0, 5));
}

#[gpui::test]
fn delete_at_emptied_alternate_deletes_after_restored_text(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Hello world!"]);
    make_alt_and_empty_it(&editor, cx, "Hello !");
    editor.update_in(cx, |e, window, cx| e.delete(&Delete, window, cx));
    assert_eq!(texts(&editor, cx), vec!["Hello world"]);
}

#[gpui::test]
fn ai_alternates_on_part_of_a_group_rewrite_the_whole_group(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["a quick brown fox"]);
    let mock = Arc::new(MockProvider::new(vec![Ok(r#"["tan"]"#.to_string())]));
    editor.update(cx, |e, _| {
        e.doc.make_alt(0, 2..13).unwrap();
        e.provider_override = Some(mock.clone());
    });
    select(&editor, cx, DocPos::new(0, 8), DocPos::new(0, 13));
    editor.update_in(cx, |e, window, cx| {
        e.ai_alternates(&AiAlternates, window, cx)
    });
    cx.run_until_parked();
    let request = mock.requests.lock().unwrap()[0].clone();
    assert!(request.user.ends_with("Passage to rewrite:\nquick brown"));
    assert_eq!(texts(&editor, cx), vec!["a tan fox"]);
    let info = editor.read_with(cx, |e, _| e.doc.alt_info(DocPos::new(0, 3)).unwrap());
    assert_eq!(info.count, 3, "existing options are kept");
}

#[gpui::test]
fn carriage_return_while_editing_an_alternate_splits_the_paragraph(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    select(&editor, cx, DocPos::new(0, 2), DocPos::new(0, 6));
    editor.update_in(cx, |e, window, cx| {
        e.make_alternate(&MakeAlternate, window, cx);
        e.replace_text_in_range(None, "a\rb", window, cx);
    });
    assert_eq!(texts(&editor, cx), vec!["I a", "b tea"]);
    editor.update_in(cx, |e, window, cx| {
        e.select_all(&SelectAll, window, cx);
        e.replace_text_in_range(None, "x\r\ny", window, cx);
    });
    assert_eq!(texts(&editor, cx), vec!["x", "y"]);
}

const DISCARD_NOTICE: &str = "Other versions/alternates discarded — Ctrl+Z to restore";

#[gpui::test]
fn splitting_a_multi_version_paragraph_warns_and_undo_restores(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Draft one."]);
    editor.update_in(cx, |e, window, cx| {
        e.make_alternate(&MakeAlternate, window, cx)
    });
    select(&editor, cx, DocPos::new(0, 10), DocPos::new(0, 10));
    editor.update_in(cx, |e, window, cx| e.newline(&Newline, window, cx));
    assert_eq!(
        editor.read_with(cx, |e, _| e.last_notice().map(str::to_string)),
        None
    );
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    select(&editor, cx, DocPos::new(0, 5), DocPos::new(0, 5));
    editor.update_in(cx, |e, window, cx| e.newline(&Newline, window, cx));
    assert_eq!(texts(&editor, cx), vec!["Draft", " one."]);
    assert_eq!(
        editor
            .read_with(cx, |e, _| e.last_notice().map(str::to_string))
            .as_deref(),
        Some(DISCARD_NOTICE)
    );
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(
        editor.read_with(cx, |e, _| e.doc.paragraphs[0].versions.len()),
        2
    );
}

#[gpui::test]
fn splitting_inside_an_alternate_warns(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    editor.update(cx, |e, _| {
        e.doc.make_alt(0, 2..6).unwrap();
    });
    select(&editor, cx, DocPos::new(0, 4), DocPos::new(0, 4));
    editor.update_in(cx, |e, window, cx| e.newline(&Newline, window, cx));
    assert_eq!(texts(&editor, cx), vec!["I lo", "ve tea"]);
    assert_eq!(
        editor
            .read_with(cx, |e, _| e.last_notice().map(str::to_string))
            .as_deref(),
        Some(DISCARD_NOTICE)
    );
}

#[gpui::test]
#[allow(clippy::reversed_empty_ranges)] // reversed ranges are the point of this test
fn input_handler_accepts_reversed_and_out_of_range_ranges(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["hello"]);
    editor.update_in(cx, |e, window, cx| {
        let mut actual = None;
        assert_eq!(
            e.text_for_range(3..1, &mut actual, window, cx).as_deref(),
            Some("el")
        );
        assert_eq!(actual, Some(1..3));
        assert_eq!(
            e.text_for_range(9..2, &mut actual, window, cx).as_deref(),
            Some("llo")
        );
        e.replace_text_in_range(Some(4..1), "X", window, cx);
    });
    assert_eq!(texts(&editor, cx), vec!["hXo"]);
}

#[gpui::test]
fn cycle_theme_advances_and_announces(cx: &mut TestAppContext) {
    use crate::theme::ThemeId;
    let (editor, cx) = editor_with(cx, &["x"]);
    assert_eq!(editor.read_with(cx, |e, _| e.theme.id), ThemeId::Midnight);
    editor.update(cx, |e, cx| e.cycle_theme(cx));
    assert_eq!(editor.read_with(cx, |e, _| e.theme.id), ThemeId::Paper);
    assert_eq!(
        editor
            .read_with(cx, |e, _| e.last_notice().map(str::to_string))
            .as_deref(),
        Some("Theme: Paper")
    );
}

#[gpui::test]
fn vim_gt_cycles_theme(cx: &mut TestAppContext) {
    use crate::theme::ThemeId;
    let (editor, cx) = vim_editor(cx, &["x"]);
    type_str(&editor, cx, "gtgt");
    assert_eq!(editor.read_with(cx, |e, _| e.theme.id), ThemeId::Sepia);
}

#[gpui::test]
fn unknown_theme_in_config_falls_back_with_a_notice(cx: &mut TestAppContext) {
    use crate::theme::ThemeId;
    let mut config = Config::default();
    config.editor.theme = "neon".into();
    let (editor, cx) =
        cx.add_window_view(|_, cx| Editor::new(config, Document::from_texts(&["x"]), cx));
    assert_eq!(editor.read_with(cx, |e, _| e.theme.id), ThemeId::Midnight);
    assert_eq!(
        editor
            .read_with(cx, |e, _| e.last_notice().map(str::to_string))
            .as_deref(),
        Some("Unknown theme \"neon\"; using Midnight")
    );
}

#[gpui::test]
fn vim_gm_toggles_markers(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["x"]);
    assert!(editor.read_with(cx, |e, _| e.show_markers));
    type_str(&editor, cx, "gm");
    assert!(!editor.read_with(cx, |e, _| e.show_markers));
    assert_eq!(
        editor
            .read_with(cx, |e, _| e.last_notice().map(str::to_string))
            .as_deref(),
        Some("Markers hidden")
    );
}

#[gpui::test]
fn vim_gl_runs_the_document_trim_until_the_lab_exists(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["I really love tea."]);
    with_mock(&editor, cx, vec![r#"["really "]"#]);
    type_str(&editor, cx, "gl");
    cx.run_until_parked();
    assert_eq!(editor.read_with(cx, |e, _| e.trims.len()), 1);
}

#[gpui::test]
fn cycling_boxes_the_group_until_the_cursor_leaves(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    editor.update(cx, |e, _| {
        e.doc.make_alt(0, 2..6).unwrap();
        e.doc.edit_alt(0, 1, 0..4, "adore");
        e.selection = Selection::caret(DocPos::new(0, 3));
    });
    editor.update_in(cx, |e, window, cx| e.cycle_next(&CycleNext, window, cx));
    let cycled = editor.read_with(cx, |e, _| e.cycled.clone()).unwrap();
    assert_eq!(cycled.range, 2..6);
    editor.update_in(cx, |e, window, cx| e.left(&Left, window, cx));
    assert!(editor.read_with(cx, |e, _| e.cycled.is_some()));
    editor.update_in(cx, |e, window, cx| e.home(&Home, window, cx));
    assert!(editor.read_with(cx, |e, _| e.cycled.is_none()));
}

#[gpui::test]
fn clicking_the_version_handle_shows_the_next_version(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Draft one."]);
    editor.update_in(cx, |e, window, cx| {
        e.make_alternate(&MakeAlternate, window, cx);
        e.replace_text_in_range(None, " Two.", window, cx);
    });
    cx.simulate_resize(gpui::size(gpui::px(800.), gpui::px(600.)));
    cx.run_until_parked();
    let handle = editor.read_with(cx, |e, _| {
        let layout = e.layout.as_ref().expect("painted");
        assert_eq!(layout.version_handles.len(), 1);
        layout.version_handles[0].1
    });
    let before = editor.read_with(cx, |e, _| e.doc.paragraphs[0].active);
    cx.simulate_click(handle.center(), gpui::Modifiers::none());
    let after = editor.read_with(cx, |e, _| e.doc.paragraphs[0].active);
    assert_ne!(before, after);
    assert!(editor.read_with(cx, |e, _| e.cycled.is_some()));
}

#[gpui::test]
fn hidden_markers_record_no_handles(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Draft one."]);
    editor.update_in(cx, |e, window, cx| {
        e.make_alternate(&MakeAlternate, window, cx);
        e.toggle_markers(cx);
    });
    cx.simulate_resize(gpui::size(gpui::px(800.), gpui::px(600.)));
    cx.run_until_parked();
    let handles = editor.read_with(cx, |e, _| e.layout.as_ref().unwrap().version_handles.len());
    assert_eq!(handles, 0);
}

#[gpui::test]
fn vim_visual_gs_stashes_the_selection_and_returns_to_normal(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["Keep this. Stash that."]);
    editor.update(cx, |e, _| {
        e.selection = Selection::caret(DocPos::new(0, 11))
    });
    type_str(&editor, cx, "v$gs");
    assert_eq!(texts(&editor, cx), vec!["Keep this. "]);
    assert_eq!(
        editor.read_with(cx, |e, _| e.doc.overflow[0].text.clone()),
        "Stash that."
    );
    assert_eq!(
        editor.read_with(cx, |e, _| e.vim_mode()),
        Some(Mode::Normal)
    );
}

#[gpui::test]
fn vim_g_shift_a_asks_ai_for_the_paragraph(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["Tea is nice."]);
    with_mock(&editor, cx, vec![r#"["Tea is lovely."]"#]);
    type_str(&editor, cx, "gA");
    cx.run_until_parked();
    assert_eq!(texts(&editor, cx), vec!["Tea is lovely."]);
}

use super::Scope;
use super::alt_session::AltRow;

fn rows(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> Vec<(String, bool, bool)> {
    editor.read_with(cx, |e, _| {
        e.alt_rows()
            .into_iter()
            .map(|AltRow { text, shown, ai }| (text, shown, ai))
            .collect()
    })
}

fn row(text: &str, shown: bool) -> (String, bool, bool) {
    (text.to_string(), shown, false)
}

#[gpui::test]
fn opening_the_sidebar_lists_the_word_and_creates_nothing(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    select(&editor, cx, DocPos::new(0, 6), DocPos::new(0, 6)); // just after "love"
    editor.update(cx, |e, cx| e.open_alternates(None, cx));
    let session = editor.read_with(cx, |e, _| e.alt_session.clone()).unwrap();
    assert_eq!(
        (session.scope, session.selection, session.range),
        (Scope::Word, false, 2..6)
    );
    assert_eq!(rows(&editor, cx), vec![row("love", true)]);
    editor.update(cx, |e, cx| e.close_alternates(cx));
    editor.read_with(cx, |e, _| {
        assert!(e.alt_session.is_none());
        assert!(!e.doc.paragraphs[0].has_metadata());
        assert!(!e.dirty);
    });
}

#[gpui::test]
fn word_tab_lists_a_groups_options_in_stored_order(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    editor.update(cx, |e, _| {
        e.doc.add_option(0, 2..6, "adore".into()).unwrap();
        e.selection = Selection::caret(DocPos::new(0, 3));
    });
    editor.update(cx, |e, cx| e.open_alternates(None, cx));
    assert_eq!(
        rows(&editor, cx),
        vec![row("love", false), row("adore", true)]
    );
    assert_eq!(
        editor.read_with(cx, |e, _| e.alt_session.as_ref().unwrap().highlight),
        1
    );
}

#[gpui::test]
fn selection_sentence_and_paragraph_tabs_retarget(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["One two. Three four."]);
    select(&editor, cx, DocPos::new(0, 9), DocPos::new(0, 14));
    editor.update(cx, |e, cx| e.open_alternates(None, cx));
    let session = editor.read_with(cx, |e, _| e.alt_session.clone()).unwrap();
    assert!(session.selection);
    assert_eq!(rows(&editor, cx), vec![row("Three", true)]);
    editor.update(cx, |e, cx| e.set_alt_scope(Scope::Sentence, cx));
    assert_eq!(rows(&editor, cx), vec![row("Three four.", true)]);
    editor.update(cx, |e, cx| {
        e.doc.add_version(0, "Two versions.".into());
        e.doc.set_active_version(0, 0);
        e.set_alt_scope(Scope::Paragraph, cx);
    });
    assert_eq!(
        rows(&editor, cx),
        vec![
            row("One two. Three four.", true),
            row("Two versions.", false)
        ]
    );
}

#[gpui::test]
fn a_multi_paragraph_selection_does_not_open_the_sidebar(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["a", "b"]);
    select(&editor, cx, DocPos::new(0, 0), DocPos::new(1, 1));
    editor.update(cx, |e, cx| e.open_alternates(None, cx));
    assert!(editor.read_with(cx, |e, _| e.alt_session.is_none()));
    assert_eq!(
        editor
            .read_with(cx, |e, _| e.last_notice().map(str::to_string))
            .as_deref(),
        Some("selection must be within one paragraph")
    );
}

#[gpui::test]
fn the_sidebar_closes_when_its_target_disappears(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea", "Next"]);
    select(&editor, cx, DocPos::new(0, 3), DocPos::new(0, 3));
    editor.update(cx, |e, cx| e.open_alternates(None, cx));
    // Editing another paragraph leaves it open.
    select(&editor, cx, DocPos::new(1, 4), DocPos::new(1, 4));
    type_str(&editor, cx, "!");
    assert!(editor.read_with(cx, |e, _| e.alt_session.is_some()));
    // Typing inside the target closes it.
    select(&editor, cx, DocPos::new(0, 4), DocPos::new(0, 4));
    type_str(&editor, cx, "x");
    assert!(editor.read_with(cx, |e, _| e.alt_session.is_none()));
    // Deleting its paragraph closes the Paragraph tab.
    select(&editor, cx, DocPos::new(1, 0), DocPos::new(1, 0));
    editor.update(cx, |e, cx| e.open_alternates(Some(Scope::Paragraph), cx));
    let end = editor.read_with(cx, |e, _| e.doc.end_pos());
    select(&editor, cx, DocPos::new(0, 11), end);
    editor.update_in(cx, |e, window, cx| e.backspace(&Backspace, window, cx));
    assert_eq!(texts(&editor, cx), vec!["I loxve tea"]);
    assert!(editor.read_with(cx, |e, _| e.alt_session.is_none()));
}

#[gpui::test]
fn vim_visual_ga_opens_the_selection_tab(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["I love tea"]);
    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(0, 2)));
    type_str(&editor, cx, "vlllga");
    let session = editor.read_with(cx, |e, _| e.alt_session.clone()).unwrap();
    assert!(session.selection);
    assert_eq!(session.range, 2..6); // Visual mode includes the character under the cursor
    assert_eq!(rows(&editor, cx), vec![row("love", true)]);
    assert_eq!(
        editor.read_with(cx, |e, _| e.vim_mode()),
        Some(Mode::Normal)
    );
}

/// "I love tea" with options love / adore / like on 2..6, showing "love", cursor in the group.
fn three_options(cx: &mut TestAppContext) -> (Entity<Editor>, &mut VisualTestContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    editor.update(cx, |e, _| {
        e.doc.add_option(0, 2..6, "adore".into()).unwrap();
        e.doc.add_option(0, 2..7, "like".into()).unwrap();
        e.doc.set_active_option(0, 1, 0);
        e.selection = Selection::caret(DocPos::new(0, 3));
    });
    editor.update(cx, |e, cx| e.open_alternates(None, cx));
    (editor, cx)
}

#[gpui::test]
fn previewing_shows_options_live_as_one_undo_step(cx: &mut TestAppContext) {
    let (editor, cx) = three_options(cx);
    editor.update(cx, |e, cx| e.move_alt_highlight(1, cx));
    assert_eq!(texts(&editor, cx), vec!["I adore tea"]);
    editor.update(cx, |e, cx| e.move_alt_highlight(1, cx));
    assert_eq!(texts(&editor, cx), vec!["I like tea"]);
    editor.update(cx, |e, cx| e.move_alt_highlight(1, cx)); // clamps at the last row
    editor.update(cx, |e, cx| e.move_alt_highlight(-2, cx));
    assert_eq!(texts(&editor, cx), vec!["I love tea"]);
    editor.update(cx, |e, cx| e.move_alt_highlight(2, cx));
    assert_eq!(
        rows(&editor, cx),
        vec![row("love", false), row("adore", false), row("like", true)]
    );
    // One undo undoes the whole preview session.
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(texts(&editor, cx), vec!["I love tea"]);
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(texts(&editor, cx), vec!["I love tea"]);
    assert_eq!(
        editor.read_with(cx, |e, _| e.doc.alt_info(DocPos::new(0, 3)).unwrap().count),
        3
    );
}

#[gpui::test]
fn keeping_an_option_shows_it_and_moves_the_cursor_after_it(cx: &mut TestAppContext) {
    let (editor, cx) = three_options(cx);
    editor.update(cx, |e, cx| e.keep_alternate(1, cx));
    assert_eq!(texts(&editor, cx), vec!["I adore tea"]);
    editor.read_with(cx, |e, _| {
        assert_eq!(e.cursor(), DocPos::new(0, 7));
        assert!(e.alt_session.is_some(), "Enter keeps the sidebar open");
    });
}

#[gpui::test]
fn removing_options_never_removes_the_last_one(cx: &mut TestAppContext) {
    let (editor, cx) = three_options(cx);
    editor.update(cx, |e, cx| e.remove_alternate(1, cx));
    assert_eq!(
        rows(&editor, cx),
        vec![row("love", true), row("like", false)]
    );
    editor.update(cx, |e, cx| e.remove_alternate(0, cx));
    assert_eq!(texts(&editor, cx), vec!["I like tea"]);
    assert_eq!(rows(&editor, cx), vec![row("like", true)]);
    editor.update(cx, |e, cx| e.remove_alternate(0, cx));
    assert_eq!(texts(&editor, cx), vec!["I like tea"]);
    assert!(editor.read_with(cx, |e, _| !e.doc.paragraphs[0].has_metadata()));
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    // Undo changes the target's text, so the sidebar closes; the option is back.
    assert_eq!(texts(&editor, cx), vec!["I love tea"]);
}

#[gpui::test]
fn paragraph_tab_previews_and_removes_versions(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Draft one."]);
    editor.update(cx, |e, cx| {
        e.doc.add_version(0, "Draft two.".into());
        e.open_alternates(Some(Scope::Paragraph), cx);
    });
    assert_eq!(
        rows(&editor, cx),
        vec![row("Draft one.", false), row("Draft two.", true)]
    );
    editor.update(cx, |e, cx| e.move_alt_highlight(-1, cx));
    assert_eq!(texts(&editor, cx), vec!["Draft one."]);
    editor.update(cx, |e, cx| e.remove_alternate(1, cx));
    assert_eq!(rows(&editor, cx), vec![row("Draft one.", true)]);
}

#[gpui::test]
fn the_first_add_creates_the_group_and_shows_the_new_option(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    select(&editor, cx, DocPos::new(0, 3), DocPos::new(0, 3));
    editor.update(cx, |e, cx| e.open_alternates(None, cx));
    editor.update(cx, |e, cx| e.add_alternate_text("  adore ", cx));
    assert_eq!(texts(&editor, cx), vec!["I adore tea"]);
    assert_eq!(
        rows(&editor, cx),
        vec![row("love", false), row("adore", true)]
    );
    editor.update(cx, |e, cx| e.add_alternate_text("like", cx));
    assert_eq!(rows(&editor, cx).len(), 3);
    assert_eq!(
        editor.read_with(cx, |e, _| e.alt_session.as_ref().unwrap().highlight),
        2
    );
    editor.update(cx, |e, cx| e.add_alternate_text("   ", cx)); // blank: ignored
    assert_eq!(rows(&editor, cx).len(), 3);
    editor.update_in(cx, |e, window, cx| {
        e.undo(&Undo, window, cx);
        e.undo(&Undo, window, cx);
    });
    assert_eq!(texts(&editor, cx), vec!["I love tea"]);
    assert!(editor.read_with(cx, |e, _| !e.doc.paragraphs[0].has_metadata()));
}

#[gpui::test]
fn adding_on_the_paragraph_tab_adds_a_version(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Draft one."]);
    editor.update(cx, |e, cx| {
        e.open_alternates(Some(Scope::Paragraph), cx);
        e.add_alternate_text("Draft two.", cx);
    });
    assert_eq!(texts(&editor, cx), vec!["Draft two."]);
    assert_eq!(
        rows(&editor, cx),
        vec![row("Draft one.", false), row("Draft two.", true)]
    );
}

#[gpui::test]
fn a_sentence_option_merges_inner_groups_with_a_toast_and_undo_restores_them(
    cx: &mut TestAppContext,
) {
    let (editor, cx) = editor_with(cx, &["I love tea. Bye."]);
    editor.update(cx, |e, _| {
        e.doc.add_option(0, 2..6, "adore".into()).unwrap();
        e.selection = Selection::caret(DocPos::new(0, 0));
    });
    editor.update(cx, |e, cx| {
        e.open_alternates(Some(Scope::Sentence), cx);
        e.add_alternate_text("Tea is love.", cx);
    });
    assert_eq!(texts(&editor, cx), vec!["Tea is love. Bye."]);
    assert_eq!(
        rows(&editor, cx),
        vec![row("I adore tea.", false), row("Tea is love.", true)]
    );
    assert_eq!(
        editor
            .read_with(cx, |e, _| e.last_notice().map(str::to_string))
            .as_deref(),
        Some(super::alt_session::MERGED_NOTICE)
    );
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(texts(&editor, cx), vec!["I adore tea. Bye."]);
    let inner = editor.read_with(cx, |e, _| e.doc.alt_info(DocPos::new(0, 3)).unwrap());
    assert_eq!((inner.span, inner.count), (2..7, 2));
}

#[gpui::test]
fn asking_ai_from_the_sidebar_appends_ai_options_to_its_target(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea. Bye."]);
    with_mock(&editor, cx, vec![r#"["Tea is love.", "I adore tea."]"#]);
    editor.update(cx, |e, cx| {
        e.open_alternates(Some(Scope::Sentence), cx);
        e.ask_ai_alternates(cx);
    });
    cx.run_until_parked();
    assert_eq!(texts(&editor, cx), vec!["Tea is love. Bye."]);
    let listed = rows(&editor, cx);
    assert_eq!(listed.len(), 3);
    assert_eq!(listed[0], row("I love tea.", false));
    assert_eq!(listed[1], ("Tea is love.".to_string(), true, true));
    assert_eq!(listed[2], ("I adore tea.".to_string(), false, true));
}
