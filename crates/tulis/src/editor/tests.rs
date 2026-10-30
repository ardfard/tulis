use gpui::{Entity, EntityInputHandler, TestAppContext, VisualTestContext};
use tulis_core::config::Config;
use tulis_core::{DocPos, Document, Selection};

use super::{AltEdit, Editor, TrimSuggestion};
use crate::actions::*;

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
fn stashing_the_paragraph_appends_it_and_undo_takes_it_back(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["a", "b"]);
    editor.update(cx, |e, _| e.doc.overflow = "older".into());
    editor.update_in(cx, |e, window, cx| {
        e.send_to_overflow(&SendToOverflow, window, cx)
    });
    assert_eq!(texts(&editor, cx), vec!["b"]);
    assert_eq!(
        editor.read_with(cx, |e, _| e.doc.overflow.clone()),
        "older\n\na"
    );
    assert_eq!(
        editor.read_with(cx, |e, _| e.last_notice().map(str::to_string)),
        Some("Stashed to Overflow".into())
    );
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(texts(&editor, cx), vec!["a", "b"]);
    assert_eq!(editor.read_with(cx, |e, _| e.doc.overflow.clone()), "older");
}

#[gpui::test]
fn scratchpad_edits_mark_the_document_dirty_but_page_undo_keeps_them(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Page."]);
    type_str(&editor, cx, "x");
    editor.update(cx, |e, cx| {
        e.dirty = false;
        e.set_overflow("An idea.", cx);
        assert!(e.dirty);
    });
    let overflow = |cx: &mut VisualTestContext| editor.read_with(cx, |e, _| e.doc.overflow.clone());
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(texts(&editor, cx), vec!["Page."]);
    assert_eq!(overflow(cx), "An idea.");
    editor.update_in(cx, |e, window, cx| e.redo(&Redo, window, cx));
    assert_eq!(texts(&editor, cx), vec!["xPage."]);
    assert_eq!(overflow(cx), "An idea.");
}

#[gpui::test]
fn using_a_block_moves_it_to_the_cursor_as_one_undo_step(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["First.", ""]);
    editor.update(cx, |e, _| {
        e.doc.overflow = "Alpha.\n\nBeta gamma.".into();
        e.selection = Selection::caret(DocPos::new(1, 0));
    });
    let overflow = |cx: &mut VisualTestContext| editor.read_with(cx, |e, _| e.doc.overflow.clone());
    // Ctrl+Enter with the field's cursor inside "Beta gamma.": that block moves.
    assert!(editor.update(cx, |e, cx| e.use_overflow_selection(10..10, false, cx)));
    assert_eq!(texts(&editor, cx), vec!["First.", "Beta gamma."]);
    assert_eq!(overflow(cx), "Alpha.");
    assert_eq!(editor.read_with(cx, |e, _| e.cursor()), DocPos::new(1, 11));
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(texts(&editor, cx), vec!["First.", ""]);
    assert_eq!(overflow(cx), "Alpha.\n\nBeta gamma.");
    editor.update_in(cx, |e, window, cx| e.redo(&Redo, window, cx));
    assert_eq!(texts(&editor, cx), vec!["First.", "Beta gamma."]);
    assert_eq!(overflow(cx), "Alpha.");
}

#[gpui::test]
fn copying_a_selection_keeps_it_in_overflow(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Say "]);
    editor.update(cx, |e, _| {
        e.doc.overflow = "Alpha.\n\nBeta gamma.".into();
        e.selection = Selection::caret(DocPos::new(0, 4));
    });
    // Ctrl+Shift+Enter with "gamma" selected in the field.
    assert!(editor.update(cx, |e, cx| e.use_overflow_selection(13..18, true, cx)));
    assert_eq!(texts(&editor, cx), vec!["Say gamma"]);
    assert_eq!(
        editor.read_with(cx, |e, _| e.doc.overflow.clone()),
        "Alpha.\n\nBeta gamma."
    );
    // A one-letter block is its own undo step, not part of the typing before it.
    type_str(&editor, cx, "!");
    editor.update(cx, |e, _| e.doc.overflow = "x".into());
    assert!(editor.update(cx, |e, cx| e.use_overflow_selection(0..0, true, cx)));
    assert_eq!(texts(&editor, cx), vec!["Say gamma!x"]);
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(texts(&editor, cx), vec!["Say gamma!"]);
}

#[gpui::test]
fn using_overflow_on_a_blank_line_does_nothing(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Page."]);
    editor.update(cx, |e, _| e.doc.overflow = "A\n\n\nB".into());
    assert!(!editor.update(cx, |e, cx| e.use_overflow_selection(2..2, false, cx)));
    assert!(!editor.update(cx, |e, cx| e.use_overflow_selection(1..3, false, cx)));
    assert_eq!(texts(&editor, cx), vec!["Page."]);
    assert_eq!(
        editor.read_with(cx, |e, _| e.last_notice().map(str::to_string)),
        Some("Nothing to use here".into())
    );
}

#[gpui::test]
fn dropping_a_block_puts_it_where_it_lands(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["One two.", "Three."]);
    editor.update(cx, |e, _| e.doc.overflow = "Block.\n\nOther.".into());
    cx.run_until_parked();
    let point = editor.read_with(cx, |e, _| {
        let layout = e.layout.as_ref().expect("painted");
        let p = layout.window_point(DocPos::new(0, 4)).unwrap();
        gpui::point(p.x + gpui::px(1.), p.y + layout.line_height / 2.)
    });
    let overflow = |cx: &mut VisualTestContext| editor.read_with(cx, |e, _| e.doc.overflow.clone());
    // The scratchpad changed under the drag: nothing happens but a toast.
    assert!(!editor.update(cx, |e, cx| e.drop_overflow(
        0..6,
        "Blocks",
        point,
        false,
        cx
    )));
    assert_eq!(texts(&editor, cx), vec!["One two.", "Three."]);
    assert_eq!(
        editor.read_with(cx, |e, _| e.last_notice().map(str::to_string)),
        Some("Overflow changed — drag again".into())
    );
    assert!(editor.update(cx, |e, cx| e.drop_overflow(
        0..6,
        "Block.",
        point,
        false,
        cx
    )));
    assert_eq!(texts(&editor, cx), vec!["One Block.two.", "Three."]);
    assert_eq!(overflow(cx), "Other.");
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(texts(&editor, cx), vec!["One two.", "Three."]);
    assert_eq!(overflow(cx), "Block.\n\nOther.");
    // Ctrl held while dropping: a copy.
    assert!(editor.update(cx, |e, cx| e.drop_overflow(
        8..14,
        "Other.",
        point,
        true,
        cx
    )));
    assert_eq!(texts(&editor, cx), vec!["One Other.two.", "Three."]);
    assert_eq!(overflow(cx), "Block.\n\nOther.");
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
fn clicking_the_version_handle_opens_the_paragraph_tab(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Intro.", "Draft one."]);
    editor.update(cx, |e, _| {
        e.doc.add_version(1, "Draft two.".into());
    });
    cx.simulate_resize(gpui::size(gpui::px(800.), gpui::px(600.)));
    cx.run_until_parked();
    let handle = editor.read_with(cx, |e, _| {
        let layout = e.layout.as_ref().expect("painted");
        assert_eq!(layout.version_handles.len(), 1);
        layout.version_handles[0].1
    });
    cx.simulate_click(handle.center(), gpui::Modifiers::none());
    let session = editor.read_with(cx, |e, _| e.alt_session.clone()).unwrap();
    assert_eq!(session.scope, Scope::Paragraph);
    assert_eq!(
        session.para_id,
        editor.read_with(cx, |e, _| e.doc.paragraphs[1].id)
    );
    assert_eq!(
        rows(&editor, cx),
        vec![row("Draft one.", false), row("Draft two.", true)]
    );
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
        editor.read_with(cx, |e, _| e.doc.overflow.clone()),
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
        e.ask_ai_for_session(cx);
    });
    cx.run_until_parked();
    assert_eq!(texts(&editor, cx), vec!["Tea is love. Bye."]);
    let listed = rows(&editor, cx);
    assert_eq!(listed.len(), 3);
    assert_eq!(listed[0], row("I love tea.", false));
    assert_eq!(listed[1], ("Tea is love.".to_string(), true, true));
    assert_eq!(listed[2], ("I adore tea.".to_string(), false, true));
}

#[gpui::test]
fn the_page_dims_except_the_target_paragraph_while_the_sidebar_is_open(cx: &mut TestAppContext) {
    use super::element::{DIM_ALPHA, paragraph_alpha};
    let (editor, cx) = editor_with(cx, &["One.", "Two."]);
    let alphas = |cx: &mut VisualTestContext| {
        editor.read_with(cx, |e, _| {
            e.doc
                .paragraphs
                .iter()
                .map(|p| paragraph_alpha(e, p.id, true))
                .collect::<Vec<_>>()
        })
    };
    assert_eq!(alphas(cx), vec![1., 1.]);
    select(&editor, cx, DocPos::new(1, 1), DocPos::new(1, 1));
    editor.update(cx, |e, cx| e.open_alternates(None, cx));
    assert_eq!(alphas(cx), vec![DIM_ALPHA, 1.]);
    editor.update(cx, |e, cx| e.close_alternates(cx));
    assert_eq!(alphas(cx), vec![1., 1.]);
}

#[gpui::test]
fn right_click_targets_the_selection_or_the_word_under_the_pointer(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love green tea", ""]);
    editor.update(cx, |e, cx| e.target_for_context_menu(DocPos::new(0, 8), cx));
    let sel = editor.read_with(cx, |e, _| e.selection.range());
    assert_eq!(sel, DocPos::new(0, 7)..DocPos::new(0, 12));
    // An existing selection is kept.
    select(&editor, cx, DocPos::new(0, 2), DocPos::new(0, 12));
    editor.update(cx, |e, cx| {
        e.target_for_context_menu(DocPos::new(0, 14), cx)
    });
    let sel = editor.read_with(cx, |e, _| e.selection.range());
    assert_eq!(sel, DocPos::new(0, 2)..DocPos::new(0, 12));
    // Just after a word, that word; with no word there, the cursor just moves.
    select(&editor, cx, DocPos::new(0, 0), DocPos::new(0, 0));
    editor.update(cx, |e, cx| e.target_for_context_menu(DocPos::new(0, 1), cx));
    let sel = editor.read_with(cx, |e, _| e.selection.range());
    assert_eq!(sel, DocPos::new(0, 0)..DocPos::new(0, 1));
    select(&editor, cx, DocPos::new(0, 0), DocPos::new(0, 0));
    editor.update(cx, |e, cx| e.target_for_context_menu(DocPos::new(1, 0), cx));
    assert_eq!(
        editor.read_with(cx, |e, _| e.selection),
        Selection::caret(DocPos::new(1, 0))
    );
}

#[gpui::test]
fn context_menu_commands_act_on_the_word_and_the_ghost_label_follows(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love green tea"]);
    editor.update(cx, |e, cx| e.target_for_context_menu(DocPos::new(0, 8), cx));
    assert_eq!(
        editor.read_with(cx, |e, _| e.ghost_menu_label()),
        "Ghost it"
    );
    // Alternatives for selection.
    editor.update(cx, |e, cx| e.open_alternates(None, cx));
    let session = editor.read_with(cx, |e, _| e.alt_session.clone()).unwrap();
    assert!(session.selection);
    assert_eq!(rows(&editor, cx), vec![row("green", true)]);
    editor.update(cx, |e, cx| e.close_alternates(cx));
    // Ghost it, then the label reads Revive it.
    editor.update(cx, |e, cx| e.toggle_ghost_here(cx));
    assert_eq!(
        editor.read_with(cx, |e, _| e.doc.paragraphs[0].export_text()),
        "I love  tea"
    );
    assert_eq!(
        editor.read_with(cx, |e, _| e.ghost_menu_label()),
        "Revive it"
    );
    editor.update(cx, |e, cx| e.toggle_ghost_here(cx));
    // AI alternatives for selection.
    with_mock(&editor, cx, vec![r#"["verdant"]"#]);
    editor.update(cx, |e, cx| e.ask_ai_alternates(cx));
    cx.run_until_parked();
    assert_eq!(texts(&editor, cx), vec!["I love verdant tea"]);
    // Stash this in Overflow.
    editor.update(cx, |e, cx| e.target_for_context_menu(DocPos::new(0, 3), cx));
    editor.update_in(cx, |e, window, cx| {
        e.send_to_overflow(&SendToOverflow, window, cx)
    });
    assert_eq!(texts(&editor, cx), vec!["I  verdant tea"]);
    assert_eq!(editor.read_with(cx, |e, _| e.doc.overflow.clone()), "love");
}

#[gpui::test]
fn a_preview_after_undo_records_its_own_undo_step(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Draft one."]);
    select(&editor, cx, DocPos::new(0, 10), DocPos::new(0, 10));
    type_str(&editor, cx, " More");
    editor.update(cx, |e, cx| {
        e.doc.add_version(0, "Draft two.".into());
        e.doc.set_active_version(0, 0);
        e.open_alternates(Some(Scope::Paragraph), cx);
        e.preview_alternate(1, cx);
    });
    assert_eq!(texts(&editor, cx), vec!["Draft two."]);
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(texts(&editor, cx), vec!["Draft one. More"]);
    editor.update(cx, |e, cx| e.preview_alternate(1, cx));
    assert_eq!(texts(&editor, cx), vec!["Draft two."]);
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(texts(&editor, cx), vec!["Draft one. More"]);
}

#[gpui::test]
fn every_preview_clears_trims_and_the_cycled_box(cx: &mut TestAppContext) {
    let (editor, cx) = three_options(cx);
    editor.update(cx, |e, cx| e.preview_alternate(1, cx));
    editor.update(cx, |e, cx| {
        let para_id = e.doc.paragraphs[0].id;
        e.trims = vec![TrimSuggestion {
            para_id,
            range: 0..1,
        }];
        e.cycled = Some(super::CycledGroup {
            para_id,
            range: 2..7,
        });
        e.preview_alternate(2, cx);
    });
    editor.read_with(cx, |e, _| {
        assert!(e.trims.is_empty());
        assert!(e.cycled.is_none());
    });
}

#[gpui::test]
fn removing_a_row_that_is_not_there_records_no_undo_step(cx: &mut TestAppContext) {
    let (editor, cx) = three_options(cx);
    editor.update(cx, |e, cx| e.remove_alternate(7, cx));
    assert!(editor.read_with(cx, |e, _| !e.history.can_undo()));
    let (editor, cx) = editor_with(cx, &["Draft one."]);
    editor.update(cx, |e, cx| {
        e.doc.add_version(0, "Draft two.".into());
        e.open_alternates(Some(Scope::Paragraph), cx);
        e.remove_alternate(5, cx);
    });
    assert!(editor.read_with(cx, |e, _| !e.history.can_undo()));
}

#[gpui::test]
fn a_sentence_tab_over_a_selection_ignores_the_whitespace_after_it(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["One. Two. Three."]);
    select(&editor, cx, DocPos::new(0, 0), DocPos::new(0, 5)); // "One. "
    editor.update(cx, |e, cx| e.open_alternates(Some(Scope::Sentence), cx));
    assert_eq!(rows(&editor, cx), vec![row("One.", true)]);
}

#[gpui::test]
fn edits_before_the_target_in_its_paragraph_shift_it(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    select(&editor, cx, DocPos::new(0, 3), DocPos::new(0, 3));
    editor.update(cx, |e, cx| e.open_alternates(None, cx));
    select(&editor, cx, DocPos::new(0, 0), DocPos::new(0, 0));
    type_str(&editor, cx, "X");
    editor.update(cx, |e, cx| e.insert_text("Oh, ", cx));
    assert_eq!(texts(&editor, cx), vec!["XOh, I love tea"]);
    let session = editor.read_with(cx, |e, _| e.alt_session.clone());
    assert_eq!(session.map(|s| s.range), Some(7..11));
    // The origin moved too: retargeting finds the same word.
    editor.update(cx, |e, cx| e.set_alt_scope(Scope::Word, cx));
    assert_eq!(rows(&editor, cx), vec![row("love", true)]);
    // Deleting before it shifts it back.
    select(&editor, cx, DocPos::new(0, 0), DocPos::new(0, 5));
    editor.update_in(cx, |e, window, cx| e.backspace(&Backspace, window, cx));
    let session = editor.read_with(cx, |e, _| e.alt_session.clone());
    assert_eq!(session.map(|s| s.range), Some(2..6));
}

#[gpui::test]
fn undo_rehighlights_the_shown_row_while_the_sidebar_stays(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Draft one."]);
    editor.update(cx, |e, cx| {
        e.doc.add_version(0, "Draft two.".into());
        e.doc.set_active_version(0, 0);
        e.open_alternates(Some(Scope::Paragraph), cx);
        e.preview_alternate(1, cx);
    });
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
    assert_eq!(rows(&editor, cx)[0], row("Draft one.", true));
    assert_eq!(
        editor.read_with(cx, |e, _| e.alt_session.as_ref().unwrap().highlight),
        0
    );
}

#[gpui::test]
fn the_origin_follows_a_sentence_target(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Hello there. Bye."]);
    select(&editor, cx, DocPos::new(0, 12), DocPos::new(0, 12));
    editor.update(cx, |e, cx| {
        e.open_alternates(Some(Scope::Sentence), cx);
        e.add_alternate_text("Hi.", cx);
    });
    assert_eq!(texts(&editor, cx), vec!["Hi. Bye."]);
    editor.update(cx, |e, cx| e.set_alt_scope(Scope::Sentence, cx));
    assert_eq!(
        rows(&editor, cx),
        vec![row("Hello there.", false), row("Hi.", true)]
    );
}

#[gpui::test]
fn page_ai_alternatives_use_the_selection_and_retarget_the_sidebar(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea. Bye."]);
    with_mock(&editor, cx, vec![r#"["adore"]"#]);
    editor.update(cx, |e, cx| e.open_alternates(Some(Scope::Sentence), cx));
    select(&editor, cx, DocPos::new(0, 2), DocPos::new(0, 6));
    editor.update_in(cx, |e, window, cx| {
        e.ai_alternates(&AiAlternates, window, cx)
    });
    let session = editor.read_with(cx, |e, _| e.alt_session.clone()).unwrap();
    assert_eq!(
        (session.scope, session.selection, session.range),
        (Scope::Word, true, 2..6)
    );
    cx.run_until_parked();
    assert_eq!(texts(&editor, cx), vec!["I adore tea. Bye."]);
    assert_eq!(
        rows(&editor, cx),
        vec![row("love", false), ("adore".to_string(), true, true)]
    );
}

#[gpui::test]
fn previewing_while_ai_works_still_adds_its_options_to_the_group(cx: &mut TestAppContext) {
    let (editor, cx) = three_options(cx);
    with_mock(&editor, cx, vec![r#"["cherish"]"#]);
    editor.update(cx, |e, cx| {
        e.ask_ai_for_session(cx);
        e.preview_alternate(1, cx);
    });
    assert_eq!(texts(&editor, cx), vec!["I adore tea"]);
    cx.run_until_parked();
    assert_eq!(texts(&editor, cx), vec!["I cherish tea"]);
    assert_eq!(rows(&editor, cx).len(), 4);
    assert_eq!(rows(&editor, cx)[3], ("cherish".to_string(), true, true));
}

#[gpui::test]
fn previewing_versions_while_ai_works_still_adds_its_versions(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Draft one."]);
    with_mock(&editor, cx, vec![r#"["Draft three."]"#]);
    editor.update(cx, |e, cx| {
        e.doc.add_version(0, "Draft two.".into());
        e.doc.set_active_version(0, 0);
        e.open_alternates(Some(Scope::Paragraph), cx);
        e.ask_ai_for_session(cx);
        e.preview_alternate(1, cx);
    });
    cx.run_until_parked();
    assert_eq!(texts(&editor, cx), vec!["Draft three."]);
    assert_eq!(rows(&editor, cx).len(), 3);
}

#[gpui::test]
fn escape_on_the_page_closes_the_sidebar_when_nothing_else_is_pending(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    let open = |cx: &mut VisualTestContext| editor.read_with(cx, |e, _| e.alt_session.is_some());
    let escape = |cx: &mut VisualTestContext| {
        editor.update_in(cx, |e, window, cx| e.cancel(&Cancel, window, cx))
    };
    select(&editor, cx, DocPos::new(0, 3), DocPos::new(0, 3));
    editor.update(cx, |e, cx| e.open_alternates(None, cx));
    escape(cx);
    assert!(!open(cx));
    // Trim suggestions are dismissed first.
    editor.update(cx, |e, cx| {
        let para_id = e.doc.paragraphs[0].id;
        e.trims = vec![TrimSuggestion {
            para_id,
            range: 0..1,
        }];
        e.open_alternates(None, cx);
    });
    escape(cx);
    assert!(open(cx));
    escape(cx);
    assert!(!open(cx));
    // So is editing an alternate.
    editor.update(cx, |e, cx| {
        e.open_alternates(Some(Scope::Paragraph), cx);
        e.alt_edit = Some(AltEdit {
            para_id: e.doc.paragraphs[0].id,
            start: 2,
        });
    });
    escape(cx);
    assert!(open(cx));
    escape(cx);
    assert!(!open(cx));
}

#[gpui::test]
fn vim_escape_closes_the_sidebar_only_from_idle_normal_mode(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["I love tea"]);
    let open = |cx: &mut VisualTestContext| editor.read_with(cx, |e, _| e.alt_session.is_some());
    let escape = |cx: &mut VisualTestContext| {
        editor.update_in(cx, |e, window, cx| e.cancel(&Cancel, window, cx))
    };
    editor.update(cx, |e, cx| e.open_alternates(None, cx));
    type_str(&editor, cx, "i");
    escape(cx); // Insert → Normal
    assert!(open(cx));
    type_str(&editor, cx, "v");
    escape(cx); // Visual → Normal
    assert!(open(cx));
    type_str(&editor, cx, "g");
    escape(cx); // drops the pending `g`
    assert!(open(cx));
    escape(cx);
    assert!(!open(cx));
    assert_eq!(
        editor.read_with(cx, |e, _| e.vim_mode()),
        Some(Mode::Normal)
    );
}

#[gpui::test]
fn visual_line_g_commands_act_on_whole_paragraphs(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["Keep.", "Stash this paragraph."]);
    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(1, 3)));
    type_str(&editor, cx, "Vgs");
    assert_eq!(
        editor.read_with(cx, |e, _| e.doc.overflow.clone()),
        "Stash this paragraph."
    );
    // Like `Vd`, the paragraph goes too.
    assert_eq!(texts(&editor, cx), vec!["Keep."]);
    assert_eq!(
        editor.read_with(cx, |e, _| e.vim_mode()),
        Some(Mode::Normal)
    );

    let (editor, cx) = vim_editor(cx, &["Hello world", "Next"]);
    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(0, 3)));
    type_str(&editor, cx, "Vgh");
    assert_eq!(
        editor.read_with(cx, |e, _| e.doc.paragraphs[0].export_text()),
        ""
    );

    let (editor, cx) = vim_editor(cx, &["I love tea"]);
    editor.update(cx, |e, _| e.selection = Selection::caret(DocPos::new(0, 3)));
    type_str(&editor, cx, "Vga");
    let session = editor.read_with(cx, |e, _| e.alt_session.clone()).unwrap();
    assert_eq!((session.selection, session.range), (true, 0..10));
}

#[gpui::test]
fn visual_line_stash_removes_the_paragraphs_like_visual_line_delete(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["One.", "Two.", "Three."]);
    type_str(&editor, cx, "Vjd");
    let deleted = texts(&editor, cx);
    assert_eq!(deleted, vec!["Three."]);

    let (editor, cx) = vim_editor(cx, &["One.", "Two.", "Three."]);
    type_str(&editor, cx, "Vjgs");
    assert_eq!(texts(&editor, cx), deleted);
    assert_eq!(
        editor.read_with(cx, |e, _| e.doc.overflow.clone()),
        "One.\nTwo."
    );
    assert_eq!(
        editor.read_with(cx, |e, _| (e.cursor(), e.vim_mode())),
        (DocPos::new(0, 0), Some(Mode::Normal))
    );
    type_str(&editor, cx, "u");
    assert_eq!(texts(&editor, cx), vec!["One.", "Two.", "Three."]);
    assert_eq!(editor.read_with(cx, |e, _| e.doc.overflow.clone()), "");

    // The last paragraph: the page keeps one empty paragraph.
    let (editor, cx) = vim_editor(cx, &["Only."]);
    type_str(&editor, cx, "Vgs");
    assert_eq!(texts(&editor, cx), vec![""]);
}

fn overflow_of(editor: &Entity<Editor>, cx: &mut VisualTestContext) -> String {
    editor.read_with(cx, |e, _| e.doc.overflow.clone())
}

fn undo_page(editor: &Entity<Editor>, cx: &mut VisualTestContext) {
    editor.update_in(cx, |e, window, cx| e.undo(&Undo, window, cx));
}

fn redo_page(editor: &Entity<Editor>, cx: &mut VisualTestContext) {
    editor.update_in(cx, |e, window, cx| e.redo(&Redo, window, cx));
}

#[gpui::test]
fn undoing_a_move_after_a_scratchpad_edit_puts_the_block_back(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Page."]);
    editor.update(cx, |e, _| {
        e.doc.overflow = "Keep.\n\nPrecious block.".into()
    });
    // Ctrl+Enter in "Precious block.".
    assert!(editor.update(cx, |e, cx| e.use_overflow_selection(9..9, false, cx)));
    assert_eq!(texts(&editor, cx), vec!["Precious block.Page."]);
    assert_eq!(overflow_of(&editor, cx), "Keep.");
    editor.update(cx, |e, cx| e.set_overflow("Keep. more", cx));
    undo_page(&editor, cx);
    assert_eq!(texts(&editor, cx), vec!["Page."]);
    // The text around the gap changed: the block comes back as a new one at the end.
    assert_eq!(overflow_of(&editor, cx), "Keep. more\n\nPrecious block.");
    redo_page(&editor, cx);
    assert_eq!(texts(&editor, cx), vec!["Precious block.Page."]);
    assert_eq!(overflow_of(&editor, cx), "Keep. more");
}

#[gpui::test]
fn redoing_a_stash_after_a_scratchpad_edit_keeps_both(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Stash me.", "Keep."]);
    editor.update_in(cx, |e, window, cx| {
        e.send_to_overflow(&SendToOverflow, window, cx)
    });
    assert_eq!(overflow_of(&editor, cx), "Stash me.");
    undo_page(&editor, cx);
    assert_eq!(texts(&editor, cx), vec!["Stash me.", "Keep."]);
    assert_eq!(overflow_of(&editor, cx), "");
    editor.update(cx, |e, cx| e.set_overflow("note", cx));
    redo_page(&editor, cx);
    assert_eq!(texts(&editor, cx), vec!["Keep."]);
    assert_eq!(overflow_of(&editor, cx), "note\n\nStash me.");
}

#[gpui::test]
fn stash_undo_redo_round_trips_the_scratchpad(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["a", "b"]);
    editor.update(cx, |e, _| e.doc.overflow = "older".into());
    editor.update_in(cx, |e, window, cx| {
        e.send_to_overflow(&SendToOverflow, window, cx)
    });
    assert_eq!(overflow_of(&editor, cx), "older\n\na");
    undo_page(&editor, cx);
    assert_eq!(overflow_of(&editor, cx), "older");
    redo_page(&editor, cx);
    assert_eq!(texts(&editor, cx), vec!["b"]);
    assert_eq!(overflow_of(&editor, cx), "older\n\na");
    undo_page(&editor, cx);
    assert_eq!(texts(&editor, cx), vec!["a", "b"]);
    assert_eq!(overflow_of(&editor, cx), "older");
}

#[gpui::test]
fn undoing_a_stash_whose_text_was_edited_keeps_the_edit(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Stash me.", "Keep."]);
    editor.update_in(cx, |e, window, cx| {
        e.send_to_overflow(&SendToOverflow, window, cx)
    });
    editor.update(cx, |e, cx| e.set_overflow("Stash me, edited.", cx));
    undo_page(&editor, cx);
    assert_eq!(texts(&editor, cx), vec!["Stash me.", "Keep."]);
    assert_eq!(overflow_of(&editor, cx), "Stash me, edited.");
    assert_eq!(
        editor.read_with(cx, |e, _| e.last_notice().map(str::to_string)),
        Some("Stashed text was edited in Overflow; kept it there".into())
    );
    // Redo removes the paragraph again but leaves the scratchpad alone.
    redo_page(&editor, cx);
    assert_eq!(texts(&editor, cx), vec!["Keep."]);
    assert_eq!(overflow_of(&editor, cx), "Stash me, edited.");
}

#[gpui::test]
fn take_undo_redo_keeps_the_scratchpad_edits_around_it(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &[""]);
    let far = "x".repeat(40);
    editor.update(cx, |e, _| {
        e.doc.overflow = format!("Alpha.\n\nBeta.\n\nGamma.{far}")
    });
    assert!(editor.update(cx, |e, cx| e.use_overflow_selection(9..9, false, cx)));
    assert_eq!(texts(&editor, cx), vec!["Beta."]);
    assert_eq!(overflow_of(&editor, cx), format!("Alpha.\n\nGamma.{far}"));
    // An edit away from the gap: undo puts the block back where it was.
    editor.update(cx, |e, cx| {
        e.set_overflow(&format!("Alpha.\n\nGamma.{far} later"), cx)
    });
    undo_page(&editor, cx);
    assert_eq!(texts(&editor, cx), vec![""]);
    assert_eq!(
        overflow_of(&editor, cx),
        format!("Alpha.\n\nBeta.\n\nGamma.{far} later")
    );
    redo_page(&editor, cx);
    assert_eq!(texts(&editor, cx), vec!["Beta."]);
    assert_eq!(
        overflow_of(&editor, cx),
        format!("Alpha.\n\nGamma.{far} later")
    );
    undo_page(&editor, cx);
    assert_eq!(
        overflow_of(&editor, cx),
        format!("Alpha.\n\nBeta.\n\nGamma.{far} later")
    );
}

#[gpui::test]
fn undoing_a_drop_after_a_scratchpad_edit_puts_the_block_back(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["One two."]);
    editor.update(cx, |e, _| e.doc.overflow = "Block.\n\nOther.".into());
    cx.run_until_parked();
    let point = editor.read_with(cx, |e, _| {
        let layout = e.layout.as_ref().expect("painted");
        let p = layout.window_point(DocPos::new(0, 4)).unwrap();
        gpui::point(p.x + gpui::px(1.), p.y + layout.line_height / 2.)
    });
    assert!(editor.update(cx, |e, cx| e.drop_overflow(
        0..6,
        "Block.",
        point,
        false,
        cx
    )));
    assert_eq!(overflow_of(&editor, cx), "Other.");
    editor.update(cx, |e, cx| e.set_overflow("Other. Edited.", cx));
    undo_page(&editor, cx);
    assert_eq!(texts(&editor, cx), vec!["One two."]);
    assert_eq!(overflow_of(&editor, cx), "Other. Edited.\n\nBlock.");
}
