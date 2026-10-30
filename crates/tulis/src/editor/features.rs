//! Alternates, ghosting and overflow commands, and Esc.

use std::ops::Range;

use gpui::{Context, Pixels, Point, Window};
use tulis_core::alternates::{AltLevel, OpError};
use tulis_core::history::EditKind;
use tulis_core::{DocPos, Selection};

use super::{AltEdit, CycledGroup, Editor};
use crate::actions::*;

const STASHED: &str = "Stashed to Overflow";
const NOTHING_TO_STASH: &str = "Nothing to move to overflow";
const NOTHING_TO_USE: &str = "Nothing to use here";
const OVERFLOW_CHANGED: &str = "Overflow changed — drag again";

impl Editor {
    /// The selection within one paragraph, or the cursor's whole paragraph when nothing is selected.
    pub(crate) fn target_range(&self) -> Result<(usize, Range<usize>), OpError> {
        let r = self.selection.range();
        if r.start.para != r.end.para {
            return Err(OpError::MultiParagraph);
        }
        if r.start == r.end {
            Ok((r.start.para, 0..self.doc.paragraphs[r.start.para].len()))
        } else {
            Ok((r.start.para, r.start.offset..r.end.offset))
        }
    }

    pub(super) fn make_alternate(
        &mut self,
        _: &MakeAlternate,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_alt_edit();
        let sel = self.selection.range();
        if sel.start.para != sel.end.para {
            self.set_notice(OpError::MultiParagraph.to_string(), cx);
            return;
        }
        let para = sel.start.para;
        let p = &self.doc.paragraphs[para];
        // A selection makes (or extends) a word-level group; a bare cursor inside a group adds an
        // option to it; otherwise the whole paragraph gets a new version.
        let range = if sel.start != sel.end {
            Some(sel.start.offset..sel.end.offset)
        } else {
            p.alt_at(sel.start.offset)
                .map(|i| p.segment_ranges()[i].clone())
        };
        let paragraph_empty = p.is_empty();
        match range {
            Some(range) => {
                self.begin_edit(EditKind::Other, para..para + 1);
                match self.doc.make_alt(para, range) {
                    Ok(span) => {
                        self.selection = Selection {
                            anchor: DocPos::new(para, span.start),
                            head: DocPos::new(para, span.end),
                        };
                        self.alt_edit = Some(AltEdit {
                            para_id: self.doc.paragraphs[para].id,
                            start: span.start,
                        });
                        self.end_edit(cx);
                    }
                    Err(e) => self.set_notice(e.to_string(), cx),
                }
            }
            None if paragraph_empty => self.set_notice("Write something first", cx),
            None => {
                self.begin_edit(EditKind::Other, para..para + 1);
                self.doc.make_para_version(para);
                self.end_edit(cx);
            }
        }
    }

    pub(crate) fn cycle(&mut self, forward: bool, cx: &mut Context<Self>) {
        self.finish_alt_edit();
        let head = self.selection.head;
        if self.doc.alt_info(head).is_none() {
            self.set_notice("No alternates here", cx);
            return;
        }
        self.begin_edit(EditKind::Other, head.para..head.para + 1);
        let info = self.doc.cycle(head, forward);
        if let Some(info) = &info {
            let offset = match info.level {
                AltLevel::Word => info.span.end,
                AltLevel::Paragraph => head.offset.min(info.span.end),
            };
            self.selection = Selection::caret(DocPos::new(head.para, offset));
        }
        self.end_edit(cx);
        self.cycled = info.map(|info| CycledGroup {
            para_id: self.doc.paragraphs[head.para].id,
            range: info.span,
        });
    }

    pub(super) fn cycle_next(&mut self, _: &CycleNext, _: &mut Window, cx: &mut Context<Self>) {
        self.cycle(true, cx);
    }

    pub(super) fn cycle_prev(&mut self, _: &CyclePrev, _: &mut Window, cx: &mut Context<Self>) {
        self.cycle(false, cx);
    }

    pub(super) fn accept_alternate(
        &mut self,
        _: &AcceptAlternate,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_alt_edit();
        let head = self.selection.head;
        if self.doc.alt_info(head).is_none() {
            self.set_notice("No alternates here", cx);
            return;
        }
        self.begin_edit(EditKind::Other, head.para..head.para + 1);
        self.doc.accept(head);
        self.end_edit(cx);
    }

    pub(super) fn delete_alternate(
        &mut self,
        _: &DeleteAlternate,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_alt_edit();
        let head = self.selection.head;
        if self.doc.alt_info(head).is_none() {
            self.set_notice("No alternates here", cx);
            return;
        }
        self.begin_edit(EditKind::Other, head.para..head.para + 1);
        if let Some(span) = self.doc.delete_option(head) {
            self.selection = Selection::caret(DocPos::new(head.para, span.end));
        }
        self.end_edit(cx);
    }

    pub(crate) fn toggle_ghost_here(&mut self, cx: &mut Context<Self>) {
        self.finish_alt_edit();
        let (para, range) = match self.target_range() {
            Ok(target) => target,
            Err(e) => return self.set_notice(e.to_string(), cx),
        };
        if range.is_empty() {
            return self.set_notice(OpError::EmptySelection.to_string(), cx);
        }
        self.begin_edit(EditKind::Other, para..para + 1);
        match self.doc.toggle_ghost(para, range) {
            Ok(_) => self.end_edit(cx),
            Err(e) => self.set_notice(e.to_string(), cx),
        }
    }

    pub(super) fn toggle_ghost(&mut self, _: &ToggleGhost, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_ghost_here(cx);
    }

    /// Ctrl+Shift+X, `gs`, the menu: stash the selection, or the cursor's paragraph, in
    /// Overflow. The panel stays as it is.
    pub(super) fn send_to_overflow(
        &mut self,
        _: &SendToOverflow,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.finish_alt_edit();
        let sel = self.selection.range();
        if sel.start == sel.end {
            return self.stash_paragraphs(sel.start.para..sel.start.para + 1, cx);
        }
        if self.doc.export_text_in(sel.clone()).trim().is_empty() {
            return self.set_notice(NOTHING_TO_STASH, cx);
        }
        self.begin_edit(EditKind::Other, sel.start.para..sel.end.para + 1);
        if let Some(op) = self.doc.stash(sel.clone()) {
            self.history.set_op(op);
            self.overflow_changed(true);
        }
        self.selection = Selection::caret(sel.start);
        self.end_edit(cx);
        self.set_notice(STASHED, cx);
    }

    /// Stash whole paragraphs, removing them from the page (no selection, or vim `Vgs`).
    pub(crate) fn stash_paragraphs(&mut self, paras: Range<usize>, cx: &mut Context<Self>) {
        self.finish_alt_edit();
        let blank = self.doc.paragraphs[paras.clone()]
            .iter()
            .all(|p| p.export_text().trim().is_empty());
        if blank {
            return self.set_notice(NOTHING_TO_STASH, cx);
        }
        self.begin_edit(EditKind::Other, paras.clone());
        if let Some(op) = self.doc.stash_paragraphs(paras.clone()) {
            self.history.set_op(op);
            self.overflow_changed(true);
        }
        let para = paras.start.min(self.doc.paragraphs.len() - 1);
        self.selection = Selection::caret(DocPos::new(para, 0));
        self.end_edit(cx);
        self.set_notice(STASHED, cx);
    }

    /// The scratchpad's own edits. They mark the document dirty but aren't page undo steps, and
    /// page undo and redo leave them alone (see `step_history`).
    pub fn set_overflow(&mut self, text: &str, cx: &mut Context<Self>) {
        if self.doc.overflow == text {
            return;
        }
        self.doc.overflow = text.to_string();
        self.overflow_changed(false);
        self.dirty = true;
        cx.notify();
    }

    /// Put the scratchpad text at byte `range` into the page at `at`, trimmed, as one undo
    /// step (Ctrl+Enter, a drop). It leaves the scratchpad unless `copy`. Returns false when the
    /// range holds only whitespace.
    pub fn use_overflow(
        &mut self,
        range: Range<usize>,
        at: DocPos,
        copy: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let text = self.doc.overflow.get(range.clone()).unwrap_or("").trim();
        if text.is_empty() {
            self.set_notice(NOTHING_TO_USE, cx);
            return false;
        }
        let text = text.to_string();
        let at = self.finish_alt_edit().apply(at);
        self.selection = Selection::caret(self.doc.clamp(at));
        // Its own undo step, even for one letter typed into the page right before or after.
        self.history.break_coalescing();
        self.insert_text(&text, cx);
        self.history.break_coalescing();
        if !copy {
            // The insert's undo step carries the take, so undo can put the text back.
            let op = self.doc.take_overflow(range);
            self.history.set_op(op);
            self.overflow_changed(false);
        }
        cx.notify();
        true
    }

    /// A scratchpad block dropped on the page at window `point`: it goes in at the position under
    /// the pointer, moved, or copied with `copy`. `range` and `text` are what the grip dragged;
    /// if the scratchpad changed there since, nothing happens but a toast.
    pub fn drop_overflow(
        &mut self,
        range: Range<usize>,
        text: &str,
        point: Point<Pixels>,
        copy: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.doc.overflow.get(range.clone()) != Some(text) {
            self.set_notice(OVERFLOW_CHANGED, cx);
            return false;
        }
        let Some(layout) = self.current_layout() else {
            return false;
        };
        let at = layout.pos_for_window_point(point);
        self.use_overflow(range, at, copy, cx)
    }

    /// Ctrl+Enter in the scratchpad: its selection (a byte range of `doc.overflow`) or, with
    /// none, the block at its cursor goes to the page cursor.
    pub fn use_overflow_selection(
        &mut self,
        selected: Range<usize>,
        copy: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        let range = if selected.is_empty() {
            self.doc.overflow_block_at(selected.start)
        } else {
            Some(selected)
        };
        let Some(range) = range else {
            self.set_notice(NOTHING_TO_USE, cx);
            return false;
        };
        self.use_overflow(range, self.selection.head, copy, cx)
    }

    /// Esc: cancel AI, else end a walk through cuts, else close the trim preview (or clear the
    /// marks), else stop editing an alternate (and leave
    /// vim's Insert or Visual mode, or drop its pending keys). With none of that to do, close an
    /// open sidebar.
    pub(super) fn cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        if self.stop_ai_for_other_work(cx).is_some() {
            return self.set_notice("AI request cancelled", cx);
        }
        if self.end_walk(cx) {
            return;
        }
        if self.trim_preview.take().is_some() {
            return self.set_notice("Trim preview closed", cx);
        }
        if !self.marks.is_empty() {
            self.marks.clear();
            return self.set_notice("Marks cleared", cx);
        }
        if self.lab_fixes.take().is_some() {
            return cx.notify();
        }
        let idle = self.alt_edit.is_none()
            && self
                .vim
                .as_ref()
                .is_none_or(|v| v.mode() == tulis_core::vim::Mode::Normal && !v.is_pending());
        self.finish_alt_edit();
        if self.vim.is_some() {
            self.vim_key(tulis_core::vim::VimKey::Escape, window, cx);
        }
        if idle {
            self.close_alternates(cx);
        }
        cx.notify();
    }
}
