//! Movement, editing, clipboard, undo and mouse handlers.

use gpui::{ClipboardItem, Context, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Window};
use tulis_core::history::EditKind;
use tulis_core::motion as mv;
use tulis_core::{DocPos, Selection};

use super::Editor;
use super::layout::LayoutCache;
use crate::actions::*;

const PAGE_LINES: isize = 20;

impl Editor {
    /// The last frame's layout, if it still matches the document's paragraphs.
    pub(crate) fn current_layout(&self) -> Option<&LayoutCache> {
        self.layout
            .as_ref()
            .filter(|l| l.paras.len() == self.doc.paragraphs.len())
    }

    fn extending(&self, shift_held: bool) -> bool {
        shift_held || self.vim.as_ref().is_some_and(|v| v.is_visual())
    }

    pub(crate) fn move_to(&mut self, pos: DocPos, extend: bool, cx: &mut Context<Self>) {
        let selection = if self.extending(extend) {
            Selection {
                anchor: self.selection.anchor,
                head: pos,
            }
        } else {
            Selection::caret(pos)
        };
        self.goal_x = None;
        self.set_selection(selection, cx);
    }

    fn horizontal(&mut self, forward: bool, extend: bool, cx: &mut Context<Self>) {
        let sel = self.selection;
        let target = if !self.extending(extend) && !sel.is_empty() {
            if forward {
                sel.range().end
            } else {
                sel.range().start
            }
        } else if forward {
            mv::next_grapheme(&self.doc, sel.head)
        } else {
            mv::prev_grapheme(&self.doc, sel.head)
        };
        self.move_to(target, extend, cx);
    }

    pub(crate) fn vertical_move(&mut self, lines: isize, extend: bool, cx: &mut Context<Self>) {
        let head = self.selection.head;
        let Some(layout) = self.current_layout() else {
            return;
        };
        let goal = self.goal_x.unwrap_or_else(|| layout.x_for(head));
        let target = layout.vertical(head, lines, goal);
        self.move_to(target, extend, cx);
        self.goal_x = Some(goal);
    }

    /// Start and end of the visual line at `pos` (the whole paragraph before the first layout).
    pub(crate) fn line_bounds(&self, pos: DocPos) -> (usize, usize) {
        let text = self.doc.paragraphs[pos.para].text();
        self.current_layout()
            .map_or((0, text.len()), |layout| layout.line_bounds(pos, &text))
    }

    fn line_edge(&mut self, end: bool, extend: bool, cx: &mut Context<Self>) {
        let head = self.selection.head;
        let (start, stop) = self.line_bounds(head);
        self.move_to(
            DocPos::new(head.para, if end { stop } else { start }),
            extend,
            cx,
        );
    }

    fn word_move(&mut self, forward: bool, extend: bool, cx: &mut Context<Self>) {
        let head = self.selection.head;
        let target = if forward {
            mv::next_word_start(&self.doc, head)
        } else {
            mv::prev_word_start(&self.doc, head)
        };
        self.move_to(target, extend, cx);
    }

    pub(super) fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(false, false, cx);
    }
    pub(super) fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(true, false, cx);
    }
    pub(super) fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(false, true, cx);
    }
    pub(super) fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.horizontal(true, true, cx);
    }
    pub(super) fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical_move(-1, false, cx);
    }
    pub(super) fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical_move(1, false, cx);
    }
    pub(super) fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical_move(-1, true, cx);
    }
    pub(super) fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical_move(1, true, cx);
    }
    pub(super) fn page_up(&mut self, _: &PageUp, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical_move(-PAGE_LINES, false, cx);
    }
    pub(super) fn page_down(&mut self, _: &PageDown, _: &mut Window, cx: &mut Context<Self>) {
        self.vertical_move(PAGE_LINES, false, cx);
    }
    pub(super) fn word_left(&mut self, _: &WordLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.word_move(false, false, cx);
    }
    pub(super) fn word_right(&mut self, _: &WordRight, _: &mut Window, cx: &mut Context<Self>) {
        self.word_move(true, false, cx);
    }
    pub(super) fn select_word_left(
        &mut self,
        _: &SelectWordLeft,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.word_move(false, true, cx);
    }
    pub(super) fn select_word_right(
        &mut self,
        _: &SelectWordRight,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.word_move(true, true, cx);
    }
    pub(super) fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        self.line_edge(false, false, cx);
    }
    pub(super) fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        self.line_edge(true, false, cx);
    }
    pub(super) fn select_home(&mut self, _: &SelectHome, _: &mut Window, cx: &mut Context<Self>) {
        self.line_edge(false, true, cx);
    }
    pub(super) fn select_end(&mut self, _: &SelectEnd, _: &mut Window, cx: &mut Context<Self>) {
        self.line_edge(true, true, cx);
    }
    pub(super) fn doc_start(&mut self, _: &DocStart, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(DocPos::default(), false, cx);
    }
    pub(super) fn doc_end(&mut self, _: &DocEnd, _: &mut Window, cx: &mut Context<Self>) {
        let end = self.doc.end_pos();
        self.move_to(end, false, cx);
    }
    pub(super) fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        let end = self.doc.end_pos();
        self.set_selection(
            Selection {
                anchor: DocPos::default(),
                head: end,
            },
            cx,
        );
    }

    pub(super) fn newline(&mut self, _: &Newline, _: &mut Window, cx: &mut Context<Self>) {
        self.insert_text("\n", cx);
    }

    pub(super) fn backspace(&mut self, _: &Backspace, _: &mut Window, cx: &mut Context<Self>) {
        let sel = self.selection;
        if !sel.is_empty() {
            self.delete_range(sel.range(), EditKind::Other, cx);
            return;
        }
        let start = mv::prev_grapheme(&self.doc, sel.head);
        let kind = if start.para == sel.head.para {
            EditKind::Typing { para: start.para }
        } else {
            EditKind::Other
        };
        self.delete_range(start..sel.head, kind, cx);
    }

    pub(super) fn delete(&mut self, _: &Delete, _: &mut Window, cx: &mut Context<Self>) {
        let sel = self.selection;
        if !sel.is_empty() {
            self.delete_range(sel.range(), EditKind::Other, cx);
            return;
        }
        let end = mv::next_grapheme(&self.doc, sel.head);
        let kind = if end.para == sel.head.para {
            EditKind::Typing { para: end.para }
        } else {
            EditKind::Other
        };
        self.delete_range(sel.head..end, kind, cx);
    }

    pub(super) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selection.is_empty() {
            let text = self.doc.text_in(self.selection.range());
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    pub(super) fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selection.is_empty() {
            self.copy(&Copy, window, cx);
            self.delete_range(self.selection.range(), EditKind::Other, cx);
        }
    }

    pub(super) fn paste(&mut self, _: &Paste, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.insert_text(&text, cx);
        }
    }

    pub(super) fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        self.undo_steps(1, cx);
    }

    pub(super) fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        self.redo_steps(1, cx);
    }

    pub(super) fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle);
        let Some(layout) = self.layout.as_ref() else {
            return;
        };
        let pos = self.doc.clamp(layout.pos_for_window_point(event.position));
        self.is_selecting = true;
        match event.click_count {
            2 => {
                let text = self.doc.paragraphs[pos.para].text();
                let word = mv::word_range_at(&text, pos.offset);
                let selection = Selection {
                    anchor: DocPos::new(pos.para, word.start),
                    head: DocPos::new(pos.para, word.end),
                };
                self.set_selection(selection, cx);
            }
            n if n >= 3 => {
                let len = self.doc.paragraphs[pos.para].len();
                let selection = Selection {
                    anchor: DocPos::new(pos.para, 0),
                    head: DocPos::new(pos.para, len),
                };
                self.set_selection(selection, cx);
            }
            _ => self.move_to(pos, event.modifiers.shift, cx),
        }
    }

    pub(super) fn on_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.is_selecting {
            return;
        }
        let Some(layout) = self.layout.as_ref() else {
            return;
        };
        let pos = self.doc.clamp(layout.pos_for_window_point(event.position));
        self.move_to(pos, true, cx);
    }

    pub(super) fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }
}
