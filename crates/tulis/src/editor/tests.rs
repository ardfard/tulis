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
