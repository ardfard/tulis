//! The editor: owns the document, selection and history, and renders through `EditorElement`.

mod actions;
pub(crate) mod ai;
mod alt_session;
mod element;
mod features;
mod input;
mod lab;
#[cfg(test)]
mod lab_tests;
pub mod layout;
mod markers;
#[cfg(test)]
pub(crate) mod tests;
mod vim_exec;

use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use gpui::{
    App, Context, CursorStyle, FocusHandle, Focusable, MouseButton, Pixels, ScrollHandle,
    SharedString, Task, Window, div, prelude::*,
};
use tulis_core::ai::LlmProvider;
use tulis_core::config::Config;
use tulis_core::history::{EditKind, History};
use tulis_core::overflow::{OverflowOp, Replayed};
use tulis_core::vim::{Mode, Vim};
use tulis_core::{DocPos, Document, ParaId, Selection};

pub use alt_session::{AltSession, Scope};
use element::EditorElement;
pub(crate) use element::GUTTER;
pub use lab::{Check, Mark, TrimLevel, TrimPreview};
use layout::LayoutCache;

use crate::theme::{self, Theme};
use crate::toast::{TOAST_FADE_AT, TOAST_LIFETIME, ToastQueue};

/// An undo snapshot: the page, without the scratchpad (`doc.overflow` is left empty), and the
/// selection. Page commands that change the scratchpad record an `OverflowOp` with their step.
pub type Snapshot = (Document, Selection);

/// The undo snapshot of `doc`, leaving its scratchpad text out (and uncopied).
fn page_snapshot(doc: &mut Document, selection: Selection) -> Snapshot {
    let overflow = std::mem::take(&mut doc.overflow);
    let page = doc.clone();
    doc.overflow = overflow;
    (page, selection)
}

const EDITED_STASH: &str = "Stashed text was edited in Overflow; kept it there";
const EDITED_TAKE: &str = "Used text was edited in Overflow; kept it there";

/// Events the workspace listens for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorEvent {
    /// A character was typed into the page (drives the header's auto-dim).
    Typed,
    /// The alternates sidebar opened (or retargeted); the workspace focuses it.
    AlternatesOpened,
    /// The alternates sidebar closed; the workspace returns focus to the page.
    AlternatesClosed,
    /// A Lab run finished (the popover, kept open while it worked, closes).
    LabDone,
}

impl gpui::EventEmitter<EditorEvent> for Editor {}

/// The group (or paragraph, for versions) just cycled; boxed until the cursor leaves it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CycledGroup {
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
    pub toasts: ToastQueue,
    /// The Lab's trim preview, if one is showing.
    pub trim_preview: Option<TrimPreview>,
    /// The Lab's guide-check marks, in document order.
    pub marks: Vec<Mark>,
    /// How many fixes the last "Fix punctuation and typos" proposed, while its bar shows.
    pub lab_fixes: Option<usize>,
    /// Located cuts by level, paragraph and the text sent, with the rendered text they index.
    pub(crate) trim_cache: HashMap<lab::TrimKey, (String, Vec<Range<usize>>)>,
    pub vim: Option<Vim>,
    pub alt_edit: Option<AltEdit>,
    pub cycled: Option<CycledGroup>,
    /// The open alternates sidebar, if any.
    pub alt_session: Option<AltSession>,
    pub ai_task: Option<Task<()>>,
    /// The running Lab request's cancel flag (see `Editor::stop_ai`).
    pub ai_cancel: Option<Arc<AtomicBool>>,
    pub scroll: ScrollHandle,
    pub theme: Theme,
    /// Alternate and version markers are drawn (Ctrl+. / vim `gm`).
    pub show_markers: bool,
    /// The installed font family the page is set in (see `theme::resolve_font`).
    pub(crate) font_family: SharedString,
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
    pub(crate) history: History<Snapshot, OverflowOp>,
    /// Bumped whenever `doc.overflow` changes, so the scratchpad knows when to re-read it.
    pub(crate) overflow_gen: u64,
    /// The newest change to `doc.overflow` appended a block at its end (a stash).
    pub(crate) overflow_appended: bool,
    /// The sidebar's focus handles (its list and text field), set by the workspace.
    pub(crate) sidebar_focus: Vec<FocusHandle>,
    focus_handle: FocusHandle,
}

const HISTORY_LIMIT: usize = 500;

pub(crate) const DISCARD_NOTICE: &str = "Other versions/alternates discarded — Ctrl+Z to restore";

impl Editor {
    pub fn new(config: Config, doc: Document, cx: &mut Context<Self>) -> Self {
        let vim = config.editor.vim_mode.then(Vim::new);
        let (theme_id, theme_notice) = theme::theme_from_config(&config.editor.theme);
        let font_family = theme::resolve_font(
            &config.editor.font_family,
            &cx.text_system().all_font_names(),
            &theme::platform_mono(cx),
        );
        let mut editor = Self {
            doc,
            selection: Selection::default(),
            config,
            dirty: false,
            toasts: ToastQueue::default(),
            trim_preview: None,
            marks: Vec::new(),
            lab_fixes: None,
            trim_cache: HashMap::new(),
            vim,
            alt_edit: None,
            cycled: None,
            alt_session: None,
            ai_task: None,
            ai_cancel: None,
            scroll: ScrollHandle::new(),
            theme: theme_id.theme(),
            show_markers: true,
            font_family: font_family.into(),
            provider_override: None,
            layout: None,
            marked: None,
            goal_x: None,
            autoscroll: true,
            is_selecting: false,
            register: String::new(),
            register_linewise: false,
            history: History::new(HISTORY_LIMIT),
            overflow_gen: 0,
            overflow_appended: false,
            sidebar_focus: Vec::new(),
            focus_handle: cx.focus_handle(),
        };
        if let Some(notice) = theme_notice {
            editor.set_notice(notice, cx);
        }
        editor
    }

    /// Replace the document (open/new). Clears history and transient state.
    pub fn load(&mut self, doc: Document, cx: &mut Context<Self>) {
        self.doc = doc;
        self.selection = Selection::default();
        self.history = History::new(HISTORY_LIMIT);
        self.overflow_changed(false);
        self.trim_preview = None;
        self.marks.clear();
        self.lab_fixes = None;
        self.trim_cache.clear();
        self.alt_edit = None;
        self.cycled = None;
        self.stop_ai_for_other_work(cx);
        self.marked = None;
        self.close_alternates(cx);
        self.goal_x = None;
        self.dirty = false;
        self.autoscroll = true;
        cx.notify();
    }

    /// Show a toast; it fades from `TOAST_FADE_AT` and is removed after `TOAST_LIFETIME`.
    pub fn set_notice(&mut self, notice: impl Into<String>, cx: &mut Context<Self>) {
        let id = self.toasts.push(notice);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST_FADE_AT).await;
            this.update(cx, |e, cx| {
                e.toasts.start_fade(id);
                cx.notify();
            })
            .ok();
            cx.background_executor()
                .timer(TOAST_LIFETIME - TOAST_FADE_AT)
                .await;
            this.update(cx, |e, cx| {
                e.toasts.remove(id);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// The newest toast still showing (tests).
    #[cfg(test)]
    pub fn last_notice(&self) -> Option<&str> {
        self.toasts.iter().last().map(|t| t.text.as_str())
    }

    /// Switch to the next preset for this session and announce it.
    pub fn cycle_theme(&mut self, cx: &mut Context<Self>) {
        self.theme = self.theme.id.next().theme();
        theme::apply_to_components(&self.theme, cx);
        self.set_notice(format!("Theme: {}", self.theme.id.name()), cx);
    }

    pub fn toggle_markers(&mut self, cx: &mut Context<Self>) {
        self.show_markers = !self.show_markers;
        let state = if self.show_markers {
            "Markers shown"
        } else {
            "Markers hidden"
        };
        self.set_notice(state, cx);
    }

    pub fn cursor(&self) -> DocPos {
        self.selection.head
    }

    pub fn vim_mode(&self) -> Option<Mode> {
        self.vim.as_ref().map(Vim::mode)
    }

    /// The page dims around the sidebar's target while focus is in the sidebar.
    pub(crate) fn page_dimmed(&self, window: &Window) -> bool {
        self.alt_session.is_some() && self.sidebar_focus.iter().any(|f| f.is_focused(window))
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

    /// Shift a range: an end at the group's start stays (it lies before the restored text), a
    /// start there moves (a non-empty range then extends past it). An empty range is a caret.
    pub(crate) fn apply_range(self, range: Range<DocPos>) -> Range<DocPos> {
        if range.start == range.end {
            let pos = self.apply(range.start);
            return pos..pos;
        }
        let end_at_start = range.end.para == self.para && range.end.offset == self.start;
        let end = if end_at_start {
            range.end
        } else {
            self.apply(range.end)
        };
        self.apply(range.start)..end
    }
}

impl Editor {
    /// Record an undo step (call before mutating) and drop the trim preview's cuts in the
    /// touched paragraphs (`touched` is a range of paragraph indices).
    pub(crate) fn begin_edit(&mut self, kind: EditKind, touched: Range<usize>) {
        self.record_step(kind);
        self.drop_transient(touched);
    }

    /// Record an undo step (call before mutating).
    pub(crate) fn record_step(&mut self, kind: EditKind) {
        let (doc, selection) = (&mut self.doc, self.selection);
        self.history
            .record(kind, Instant::now(), || page_snapshot(doc, selection));
    }

    /// Note a change to `doc.overflow`; `appended` when it added a block at the end.
    pub(crate) fn overflow_changed(&mut self, appended: bool) {
        self.overflow_gen += 1;
        self.overflow_appended = appended;
    }

    /// Restore an undo snapshot's page, keeping the scratchpad as it is.
    fn restore_page(&mut self, (doc, selection): Snapshot) {
        let overflow = std::mem::take(&mut self.doc.overflow);
        self.doc = doc;
        self.doc.overflow = overflow;
        self.selection = selection;
    }

    /// Drop the cycled box and the trim preview's cuts in the touched paragraphs (`touched` is
    /// a range of paragraph indices), as every edit does.
    pub(crate) fn drop_transient(&mut self, touched: Range<usize>) {
        self.cycled = None;
        let end = touched.end.min(self.doc.paragraphs.len());
        let ids: Vec<ParaId> = self
            .doc
            .paragraphs
            .get(touched.start.min(end)..end)
            .map(|ps| ps.iter().map(|p| p.id).collect())
            .unwrap_or_default();
        if let Some(preview) = &mut self.trim_preview {
            preview.retain(|cut| !ids.contains(&cut.para_id));
        }
        self.marks.retain(|m| !ids.contains(&m.para_id));
    }

    pub(crate) fn end_edit(&mut self, cx: &mut Context<Self>) {
        self.selection = Selection {
            anchor: self.doc.clamp(self.selection.anchor),
            head: self.doc.clamp(self.selection.head),
        };
        self.dirty = true;
        self.autoscroll = true;
        self.goal_x = None;
        self.sync_alt_session(cx);
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
        if !self.in_cycled(self.selection.head) {
            self.cycled = None;
        }
        self.marked = None;
        self.history.break_coalescing();
        self.autoscroll = true;
        cx.notify();
    }

    /// Whether `pos` lies in the cycled group (edges included). False when there is none.
    fn in_cycled(&self, pos: DocPos) -> bool {
        self.cycled.as_ref().is_some_and(|c| {
            self.doc.para_index(c.para_id) == Some(pos.para)
                && c.range.start <= pos.offset
                && pos.offset <= c.range.end
        })
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
        let normalized;
        let text = if text.contains('\r') {
            normalized = text.replace("\r\n", "\n").replace('\r', "\n");
            normalized.as_str()
        } else {
            text
        };
        let range = self.selection.range();
        let kind = if text.chars().count() == 1 && text != "\n" && range.start == range.end {
            EditKind::Typing {
                para: range.start.para,
            }
        } else {
            EditKind::Other
        };
        if matches!(kind, EditKind::Typing { .. }) {
            cx.emit(EditorEvent::Typed);
        }
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
        let range = shift.apply_range(range);
        self.begin_edit(kind, range.start.para..range.end.para + 1);
        let start = self.doc.delete(range);
        let discards = text.contains('\n') && self.split_discards(start);
        let end = self.doc.insert(start, text);
        self.selection = Selection::caret(end);
        if !text.is_empty() {
            self.enter_alt_edit_if_inside(end);
        }
        self.end_edit(cx);
        if discards {
            self.set_notice(DISCARD_NOTICE, cx);
        }
    }

    /// Whether splitting the paragraph at `pos` drops other versions or flattens an alternate.
    fn split_discards(&self, pos: DocPos) -> bool {
        let p = &self.doc.paragraphs[pos.para];
        if pos.offset == 0 || pos.offset >= p.len() {
            return false;
        }
        p.versions.len() > 1
            || p.segments()
                .iter()
                .zip(p.segment_ranges())
                .any(|(seg, r)| seg.is_alt() && r.start < pos.offset && pos.offset < r.end)
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
        let range = shift.apply_range(range);
        self.begin_edit(kind, range.start.para..range.end.para + 1);
        let start = self.doc.delete(range);
        self.selection = Selection::caret(start);
        self.end_edit(cx);
    }

    pub(crate) fn undo_steps(&mut self, count: usize, cx: &mut Context<Self>) {
        self.step_history(count, false, cx);
    }

    pub(crate) fn redo_steps(&mut self, count: usize, cx: &mut Context<Self>) {
        self.step_history(count, true, cx);
    }

    /// Undo (or redo) up to `count` steps. Pages come back from snapshots; the scratchpad
    /// isn't in them, so the ops recorded with the steps are replayed on it as it is now.
    fn step_history(&mut self, count: usize, redo: bool, cx: &mut Context<Self>) {
        self.finish_alt_edit();
        let mut changed = false;
        let mut appended = false;
        let mut kept = None;
        for _ in 0..count.max(1) {
            let current = page_snapshot(&mut self.doc, self.selection);
            let doc = &mut self.doc;
            let replay = |op: Option<OverflowOp>| {
                let op = op?;
                let (how, next) = if redo {
                    doc.redo_overflow(&op)
                } else {
                    doc.undo_overflow(&op)
                };
                match how {
                    Replayed::Kept => {
                        kept = Some(match op {
                            OverflowOp::Stash { .. } => EDITED_STASH,
                            OverflowOp::Take { .. } => EDITED_TAKE,
                        })
                    }
                    Replayed::Appended => (changed, appended) = (true, true),
                    Replayed::Exact | Replayed::Moved => (changed, appended) = (true, false),
                }
                next
            };
            let step = if redo {
                self.history.redo_with(current, replay)
            } else {
                self.history.undo_with(current, replay)
            };
            let Some(snapshot) = step else { break };
            self.restore_page(snapshot);
        }
        if changed {
            self.overflow_changed(appended);
        }
        self.after_history(cx);
        if let Some(notice) = kept {
            self.set_notice(notice, cx);
        }
    }

    fn after_history(&mut self, cx: &mut Context<Self>) {
        self.alt_edit = None;
        self.cycled = None;
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
        self.trim_preview = None;
        self.marks.clear();
        self.marked = None;
        self.goal_x = None;
        self.dirty = true;
        self.autoscroll = true;
        self.sync_alt_session(cx);
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
            .on_action(cx.listener(Self::make_alternate))
            .on_action(cx.listener(Self::open_alternates_action))
            .on_action(cx.listener(Self::cycle_next))
            .on_action(cx.listener(Self::cycle_prev))
            .on_action(cx.listener(Self::accept_alternate))
            .on_action(cx.listener(Self::delete_alternate))
            .on_action(cx.listener(Self::toggle_ghost))
            .on_action(cx.listener(Self::send_to_overflow))
            .on_action(cx.listener(Self::next_mark))
            .on_action(cx.listener(Self::prev_mark))
            .on_action(cx.listener(Self::ai_alternates))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::open_context_menu))
            .on_action(cx.listener(Self::toggle_vim))
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::on_right_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .child(EditorElement::new(cx.entity()))
    }
}
