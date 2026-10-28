//! The editor: owns the document, selection and history, and renders through `EditorElement`.

mod actions;
mod element;
mod input;
pub mod layout;
#[cfg(test)]
mod tests;

use std::ops::Range;
use std::sync::Arc;
use std::time::Instant;

use gpui::{
    App, Context, CursorStyle, FocusHandle, Focusable, MouseButton, Pixels, ScrollHandle, Task,
    Window, div, prelude::*,
};
use tulis_core::ai::LlmProvider;
use tulis_core::config::Config;
use tulis_core::history::{EditKind, History};
use tulis_core::vim::{Mode, Vim};
use tulis_core::{DocPos, Document, ParaId, Selection};

use element::EditorElement;
use layout::LayoutCache;

pub type Snapshot = (Document, Selection);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrimSuggestion {
    pub para_id: ParaId,
    pub range: Range<usize>,
}

/// The alternate group being edited (see "alternate-editing state" in the spec).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AltEdit {
    pub para_id: ParaId,
    pub start: usize,
}

pub struct Editor {
    pub doc: Document,
    pub selection: Selection,
    pub config: Config,
    pub dirty: bool,
    pub notice: Option<String>,
    pub trims: Vec<TrimSuggestion>,
    pub vim: Option<Vim>,
    pub alt_edit: Option<AltEdit>,
    pub ai_task: Option<Task<()>>,
    pub scroll: ScrollHandle,
    /// Used instead of the configured provider when set (tests).
    pub provider_override: Option<Arc<dyn LlmProvider>>,
    pub(crate) layout: Option<LayoutCache>,
    /// IME marked text, as a byte range of `doc.flat_text()`.
    pub(crate) marked: Option<Range<usize>>,
    pub(crate) goal_x: Option<Pixels>,
    pub(crate) autoscroll: bool,
    pub(crate) is_selecting: bool,
    pub(crate) register: String,
    pub(crate) register_linewise: bool,
    pub(crate) history: History<Snapshot>,
    focus_handle: FocusHandle,
}

const HISTORY_LIMIT: usize = 500;

impl Editor {
    pub fn new(config: Config, doc: Document, cx: &mut Context<Self>) -> Self {
        let vim = config.editor.vim_mode.then(Vim::new);
        Self {
            doc,
            selection: Selection::default(),
            config,
            dirty: false,
            notice: None,
            trims: Vec::new(),
            vim,
            alt_edit: None,
            ai_task: None,
            scroll: ScrollHandle::new(),
            provider_override: None,
            layout: None,
            marked: None,
            goal_x: None,
            autoscroll: true,
            is_selecting: false,
            register: String::new(),
            register_linewise: false,
            history: History::new(HISTORY_LIMIT),
            focus_handle: cx.focus_handle(),
        }
    }

    /// Replace the document (open/new). Clears history and transient state.
    pub fn load(&mut self, doc: Document, cx: &mut Context<Self>) {
        self.doc = doc;
        self.selection = Selection::default();
        self.history = History::new(HISTORY_LIMIT);
        self.trims.clear();
        self.alt_edit = None;
        self.ai_task = None;
        self.marked = None;
        self.goal_x = None;
        self.dirty = false;
        self.autoscroll = true;
        cx.notify();
    }

    pub fn set_notice(&mut self, notice: impl Into<String>, cx: &mut Context<Self>) {
        self.notice = Some(notice.into());
        cx.notify();
    }

    pub fn cursor(&self) -> DocPos {
        self.selection.head
    }

    pub fn vim_mode(&self) -> Option<Mode> {
        self.vim.as_ref().map(Vim::mode)
    }

    /// True when typed characters drive vim instead of being inserted.
    pub fn vim_captures_text(&self) -> bool {
        matches!(
            self.vim_mode(),
            Some(Mode::Normal | Mode::Visual | Mode::VisualLine)
        )
    }
}

impl Focusable for Editor {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// How positions move when alternate editing ends and an emptied option is replaced by the
/// previous one (only positions at or after the group's start, in its paragraph, move).
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Shift {
    para: usize,
    start: usize,
    delta: usize,
}

impl Shift {
    pub(crate) fn apply(self, pos: DocPos) -> DocPos {
        if self.delta > 0 && pos.para == self.para && pos.offset >= self.start {
            DocPos::new(pos.para, pos.offset + self.delta)
        } else {
            pos
        }
    }
}

impl Editor {
    /// Record an undo step (call before mutating) and drop trim suggestions in the touched
    /// paragraphs (`touched` is a range of paragraph indices).
    pub(crate) fn begin_edit(&mut self, kind: EditKind, touched: Range<usize>) {
        let (doc, selection) = (&self.doc, self.selection);
        self.history
            .record(kind, Instant::now(), || (doc.clone(), selection));
        let end = touched.end.min(self.doc.paragraphs.len());
        let ids: Vec<ParaId> = self
            .doc
            .paragraphs
            .get(touched.start.min(end)..end)
            .map(|ps| ps.iter().map(|p| p.id).collect())
            .unwrap_or_default();
        self.trims.retain(|t| !ids.contains(&t.para_id));
    }

    pub(crate) fn end_edit(&mut self, cx: &mut Context<Self>) {
        self.selection = Selection {
            anchor: self.doc.clamp(self.selection.anchor),
            head: self.doc.clamp(self.selection.head),
        };
        self.dirty = true;
        self.autoscroll = true;
        self.goal_x = None;
        self.notice = None;
        cx.notify();
    }

    /// Move the selection. Ends alternate editing when the selection leaves the edited group.
    pub(crate) fn set_selection(&mut self, selection: Selection, cx: &mut Context<Self>) {
        self.selection = Selection {
            anchor: self.doc.clamp(selection.anchor),
            head: self.doc.clamp(selection.head),
        };
        if self.alt_edit.is_some() && self.alt_edit_target(self.selection.range()).is_none() {
            self.finish_alt_edit();
        }
        self.marked = None;
        self.history.break_coalescing();
        self.autoscroll = true;
        cx.notify();
    }

    /// End alternate editing. Also shifts the selection; returns the shift for other positions.
    pub(crate) fn finish_alt_edit(&mut self) -> Shift {
        let Some(edit) = self.alt_edit.take() else {
            return Shift::default();
        };
        let Some(para) = self.doc.para_index(edit.para_id) else {
            return Shift::default();
        };
        let Some(seg) = self.doc.paragraphs[para].alt_starting_at(edit.start) else {
            return Shift::default();
        };
        let before = self.doc.paragraphs[para].len();
        self.doc.finish_alt_edit(para, seg);
        let after = self.doc.paragraphs[para].len();
        if after != before {
            self.dirty = true;
        }
        let shift = Shift {
            para,
            start: edit.start,
            delta: after.saturating_sub(before),
        };
        self.selection = Selection {
            anchor: shift.apply(self.selection.anchor),
            head: shift.apply(self.selection.head),
        };
        shift
    }

    /// If `range` lies within the edited group (edges included), its paragraph, segment and
    /// range local to the group's active text.
    pub(crate) fn alt_edit_target(
        &self,
        range: Range<DocPos>,
    ) -> Option<(usize, usize, Range<usize>)> {
        let edit = self.alt_edit?;
        let para = self.doc.para_index(edit.para_id)?;
        if range.start.para != para || range.end.para != para {
            return None;
        }
        let p = &self.doc.paragraphs[para];
        let seg = p.alt_starting_at(edit.start)?;
        let span = p.segment_ranges()[seg].clone();
        (span.start <= range.start.offset && range.end.offset <= span.end).then(|| {
            (
                para,
                seg,
                range.start.offset - span.start..range.end.offset - span.start,
            )
        })
    }

    /// After typing strictly inside an alternate, keep editing that group.
    fn enter_alt_edit_if_inside(&mut self, pos: DocPos) {
        let p = &self.doc.paragraphs[pos.para];
        if let Some(seg) = p.alt_at(pos.offset) {
            let span = p.segment_ranges()[seg].clone();
            if span.start < pos.offset {
                self.alt_edit = Some(AltEdit {
                    para_id: p.id,
                    start: span.start,
                });
            }
        }
    }

    /// Replace the selection with `text` (newlines split paragraphs).
    pub fn insert_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let range = self.selection.range();
        let kind = if text.chars().count() == 1 && text != "\n" && range.start == range.end {
            EditKind::Typing {
                para: range.start.para,
            }
        } else {
            EditKind::Other
        };
        if !text.contains('\n')
            && let Some((para, seg, local)) = self.alt_edit_target(range.clone())
        {
            self.begin_edit(kind, para..para + 1);
            let start = self.doc.paragraphs[para].segment_ranges()[seg].start;
            self.doc.edit_alt(para, seg, local.clone(), text);
            self.selection = Selection::caret(DocPos::new(para, start + local.start + text.len()));
            self.end_edit(cx);
            return;
        }
        let shift = self.finish_alt_edit();
        let range = shift.apply(range.start)..shift.apply(range.end);
        self.begin_edit(kind, range.start.para..range.end.para + 1);
        let start = self.doc.delete(range);
        let end = self.doc.insert(start, text);
        self.selection = Selection::caret(end);
        if !text.is_empty() {
            self.enter_alt_edit_if_inside(end);
        }
        self.end_edit(cx);
    }

    pub(crate) fn delete_range(
        &mut self,
        range: Range<DocPos>,
        kind: EditKind,
        cx: &mut Context<Self>,
    ) {
        if range.start >= range.end {
            return;
        }
        if let Some((para, seg, local)) = self.alt_edit_target(range.clone()) {
            self.begin_edit(kind, para..para + 1);
            self.doc.edit_alt(para, seg, local, "");
            self.selection = Selection::caret(range.start);
            self.end_edit(cx);
            return;
        }
        let shift = self.finish_alt_edit();
        let range = shift.apply(range.start)..shift.apply(range.end);
        self.begin_edit(kind, range.start.para..range.end.para + 1);
        let start = self.doc.delete(range);
        self.selection = Selection::caret(start);
        self.end_edit(cx);
    }

    pub(crate) fn undo_steps(&mut self, count: usize, cx: &mut Context<Self>) {
        self.finish_alt_edit();
        for _ in 0..count.max(1) {
            let current = (self.doc.clone(), self.selection);
            let Some((doc, selection)) = self.history.undo(current) else {
                break;
            };
            self.doc = doc;
            self.selection = selection;
        }
        self.after_history(cx);
    }

    pub(crate) fn redo_steps(&mut self, count: usize, cx: &mut Context<Self>) {
        self.finish_alt_edit();
        for _ in 0..count.max(1) {
            let current = (self.doc.clone(), self.selection);
            let Some((doc, selection)) = self.history.redo(current) else {
                break;
            };
            self.doc = doc;
            self.selection = selection;
        }
        self.after_history(cx);
    }

    fn after_history(&mut self, cx: &mut Context<Self>) {
        self.alt_edit = None;
        // A restored snapshot may hold an alternate whose active option is empty.
        for para in 0..self.doc.paragraphs.len() {
            for seg in (0..self.doc.paragraphs[para].segments().len()).rev() {
                let Some(segment) = self.doc.paragraphs[para].segments().get(seg) else {
                    continue;
                };
                if segment.is_alt() && segment.text().is_empty() {
                    self.doc.finish_alt_edit(para, seg);
                }
            }
        }
        self.selection = Selection {
            anchor: self.doc.clamp(self.selection.anchor),
            head: self.doc.clamp(self.selection.head),
        };
        self.trims.clear();
        self.marked = None;
        self.goal_x = None;
        self.dirty = true;
        self.autoscroll = true;
        cx.notify();
    }
}

impl Render for Editor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("Editor")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .w_full()
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .on_action(cx.listener(Self::page_up))
            .on_action(cx.listener(Self::page_down))
            .on_action(cx.listener(Self::word_left))
            .on_action(cx.listener(Self::word_right))
            .on_action(cx.listener(Self::select_word_left))
            .on_action(cx.listener(Self::select_word_right))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::select_home))
            .on_action(cx.listener(Self::select_end))
            .on_action(cx.listener(Self::doc_start))
            .on_action(cx.listener(Self::doc_end))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(EditorElement::new(cx.entity()))
    }
}
