//! Alternates, ghosting, overflow and trim-acceptance commands.

use std::ops::Range;
use std::time::Instant;

use gpui::{Context, Window};
use tulis_core::alternates::{AltLevel, OpError};
use tulis_core::history::EditKind;
use tulis_core::{DocPos, Selection};

use super::{AltEdit, CycledGroup, Editor, TrimSuggestion};
use crate::actions::*;

const STASHED: &str = "Stashed to Overflow";
const NOTHING_TO_STASH: &str = "Nothing to move to overflow";

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
        self.doc.stash(sel.clone());
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
        self.doc.stash_paragraphs(paras.clone());
        let para = paras.start.min(self.doc.paragraphs.len() - 1);
        self.selection = Selection::caret(DocPos::new(para, 0));
        self.end_edit(cx);
        self.set_notice(STASHED, cx);
    }

    /// Accept the trim suggestion under the cursor (or the next one, or the first) by ghosting it.
    pub(super) fn accept_trim(&mut self, _: &AcceptTrim, _: &mut Window, cx: &mut Context<Self>) {
        if self.trims.is_empty() {
            return;
        }
        let head = self.selection.head;
        let head_id = self.doc.paragraphs[head.para].id;
        let order = |t: &TrimSuggestion| {
            (
                self.doc.para_index(t.para_id).unwrap_or(usize::MAX),
                t.range.start,
            )
        };
        let under_cursor = self.trims.iter().position(|t| {
            t.para_id == head_id && t.range.start <= head.offset && head.offset <= t.range.end
        });
        let ix = under_cursor
            .or_else(|| {
                (0..self.trims.len())
                    .filter(|&i| order(&self.trims[i]) >= (head.para, head.offset))
                    .min_by_key(|&i| order(&self.trims[i]))
            })
            .or_else(|| (0..self.trims.len()).min_by_key(|&i| order(&self.trims[i])));
        let Some(ix) = ix else { return };
        let trim = self.trims.remove(ix);
        let Some(para) = self.doc.para_index(trim.para_id) else {
            return;
        };
        // Not `begin_edit`: ghosting doesn't change rendered text, so the other suggestions stay valid.
        let (doc, selection) = (&self.doc, self.selection);
        self.history
            .record(EditKind::Other, Instant::now(), || (doc.clone(), selection));
        match self.doc.set_ghost(para, trim.range, true) {
            Ok(range) => {
                self.selection = Selection::caret(DocPos::new(para, range.end));
                self.dirty = true;
                self.autoscroll = true;
                cx.notify();
            }
            Err(e) => self.set_notice(e.to_string(), cx),
        }
    }

    /// Esc: cancel AI, else dismiss trims, else stop editing an alternate (and leave vim's
    /// Insert or Visual mode, or drop its pending keys). With none of that to do, close an open
    /// sidebar.
    pub(super) fn cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut Context<Self>) {
        if self.ai_task.take().is_some() {
            return self.set_notice("AI request cancelled", cx);
        }
        if !self.trims.is_empty() {
            self.trims.clear();
            return self.set_notice("Trim suggestions dismissed", cx);
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
