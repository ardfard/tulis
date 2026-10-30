//! The Lab on the page: trim previews, the runner every Lab request goes through, and its
//! cancel flag.

use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{Context, Window, prelude::*};
use tulis_core::ai::{AiError, CompletionRequest, prompts};
use tulis_core::history::EditKind;
use tulis_core::stats;
use tulis_core::{DocPos, ParaId, Selection};

use super::{Editor, EditorEvent};
use crate::actions::{NextMark, PrevMark};

/// How much a trim asks the AI to cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrimLevel {
    Original,
    Slight,
    Tighten,
    Sharper,
    Half,
}

impl TrimLevel {
    pub const ALL: [TrimLevel; 5] = [
        TrimLevel::Original,
        TrimLevel::Slight,
        TrimLevel::Tighten,
        TrimLevel::Sharper,
        TrimLevel::Half,
    ];

    /// The share of the words to cut.
    pub fn percent(self) -> u8 {
        match self {
            TrimLevel::Original => 0,
            TrimLevel::Slight => 10,
            TrimLevel::Tighten => 20,
            TrimLevel::Sharper => 30,
            TrimLevel::Half => 50,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            TrimLevel::Original => "Original",
            TrimLevel::Slight => "Slight trim",
            TrimLevel::Tighten => "Tighten more",
            TrimLevel::Sharper => "Even sharper",
            TrimLevel::Half => "Cut in half",
        }
    }
}

/// A passage a trim would cut: a byte range of its paragraph's rendered text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrimSuggestion {
    pub para_id: ParaId,
    pub range: Range<usize>,
}

/// The faded preview of a trim level. It is view state: editing a paragraph drops its cuts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrimPreview {
    pub level: TrimLevel,
    /// In document order.
    pub cuts: Vec<TrimSuggestion>,
    /// Indices into `cuts` the writer chose to keep; the others are active.
    pub kept: HashSet<usize>,
}

impl TrimPreview {
    pub fn new(level: TrimLevel, cuts: Vec<TrimSuggestion>) -> Self {
        Self {
            level,
            cuts,
            kept: HashSet::new(),
        }
    }

    pub fn is_active(&self, ix: usize) -> bool {
        ix < self.cuts.len() && !self.kept.contains(&ix)
    }

    /// The active cuts in paragraph `id` (drawn faded).
    pub fn active_ranges(&self, id: ParaId) -> Vec<Range<usize>> {
        self.cuts
            .iter()
            .enumerate()
            .filter(|(ix, cut)| cut.para_id == id && self.is_active(*ix))
            .map(|(_, cut)| cut.range.clone())
            .collect()
    }

    /// Keep only the cuts `keep` accepts; kept cuts stay kept.
    pub(crate) fn retain(&mut self, keep: impl Fn(&TrimSuggestion) -> bool) {
        let cuts = std::mem::take(&mut self.cuts);
        let kept = std::mem::take(&mut self.kept);
        for (ix, cut) in cuts.into_iter().enumerate() {
            if keep(&cut) {
                if kept.contains(&ix) {
                    self.kept.insert(self.cuts.len());
                }
                self.cuts.push(cut);
            }
        }
    }
}

/// A button on the Lab bar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BarAction {
    MakeCuts,
    Done,
}

impl BarAction {
    pub fn label(self) -> &'static str {
        match self {
            BarAction::MakeCuts => "Make the cuts",
            BarAction::Done => "Done",
        }
    }

    /// For element ids and test selectors.
    pub fn id(self) -> &'static str {
        match self {
            BarAction::MakeCuts => "make-cuts",
            BarAction::Done => "done",
        }
    }
}

/// The bar above the footer while the Lab has something on the page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LabBar {
    pub title: String,
    pub hint: Option<String>,
    pub actions: Vec<BarAction>,
}

/// One paragraph's Lab request, with the paragraph as it was when asked (results for a
/// paragraph that changed meanwhile are dropped).
pub(crate) struct LabJob {
    pub para_id: ParaId,
    /// The rendered text.
    pub text: String,
    /// The export text (what was sent).
    pub export: String,
    pub request: CompletionRequest,
}

/// Each job that ran, with what came back.
pub(crate) type LabResults = Vec<(LabJob, Result<String, AiError>)>;

/// Key of the trim cache: level, paragraph and the export text that was sent.
pub(crate) type TrimKey = (TrimLevel, ParaId, String);

/// What a finished run found, for one paragraph: its rendered text then, and the ranges.
type Found = Vec<(ParaId, String, Vec<Range<usize>>)>;

/// The toast for a run in which `failed` of `total` requests failed (`None` when none did).
/// When all of them failed, it is the first error.
fn failure_notice(failed: usize, total: usize, first_error: Option<String>) -> Option<String> {
    match failed {
        0 => None,
        n if n == total => first_error,
        n => Some(format!("{n} of {total} paragraphs failed")),
    }
}

impl Editor {
    /// The paragraphs the Lab works on: the selection's, or the whole document's when nothing is
    /// selected.
    pub fn lab_scope(&self) -> Vec<ParaId> {
        let range = self.selection.range();
        let paras = if range.start == range.end {
            0..self.doc.paragraphs.len()
        } else {
            range.start.para..range.end.para + 1
        };
        self.doc.paragraphs[paras].iter().map(|p| p.id).collect()
    }

    /// A Lab run is waiting for the AI.
    pub fn lab_running(&self) -> bool {
        self.ai_cancel.is_some()
    }

    /// Run `jobs` one at a time off the UI thread, then hand what came back to `apply` with the
    /// number of jobs, and emit `LabDone`. The run's cancel flag is checked before each
    /// request; `stop_ai` sets it and drops the task, so a cancelled run sends no more requests
    /// and applies nothing.
    pub(crate) fn run_lab(
        &mut self,
        jobs: Vec<LabJob>,
        working: &str,
        apply: impl FnOnce(&mut Self, LabResults, usize, &mut Context<Self>) + 'static,
        cx: &mut Context<Self>,
    ) {
        let Some(provider) = self.provider(cx) else {
            return;
        };
        self.stop_ai();
        let cancel = Arc::new(AtomicBool::new(false));
        self.ai_cancel = Some(cancel.clone());
        let total = jobs.len();
        let flag = cancel.clone();
        let work = cx.background_spawn(async move {
            let mut results = Vec::new();
            for job in jobs {
                if flag.load(Ordering::SeqCst) {
                    break;
                }
                let result = provider.complete(&job.request);
                results.push((job, result));
            }
            results
        });
        self.ai_task = Some(cx.spawn(async move |this, cx| {
            let results = work.await;
            this.update(cx, |editor, cx| {
                editor.ai_task = None;
                editor.ai_cancel = None;
                if !cancel.load(Ordering::SeqCst) {
                    apply(editor, results, total, cx);
                }
                cx.emit(EditorEvent::LabDone);
                cx.notify();
            })
            .ok();
        }));
        self.set_notice(working, cx);
    }

    /// Stop the running AI request or Lab run: raise its cancel flag, so no further paragraph
    /// is sent, and drop its task. Returns whether one was running.
    pub(crate) fn stop_ai(&mut self) -> bool {
        if let Some(flag) = self.ai_cancel.take() {
            flag.store(true, Ordering::SeqCst);
        }
        self.ai_task.take().is_some()
    }

    /// Preview trim `level` over the paragraphs in `scope` (ids of paragraphs that are gone are
    /// skipped): one request per non-empty paragraph, unless the cache has its answer.
    /// `Original` clears the preview.
    pub fn run_trim(&mut self, level: TrimLevel, scope: &[ParaId], cx: &mut Context<Self>) {
        self.finish_alt_edit();
        if level == TrimLevel::Original {
            self.trim_preview = None;
            cx.notify();
            return;
        }
        let mut cached: Found = Vec::new();
        let mut jobs = Vec::new();
        for &id in scope {
            let Some(para) = self.doc.para_index(id) else {
                continue;
            };
            let p = &self.doc.paragraphs[para];
            let (text, export) = (p.text(), p.export_text());
            if export.trim().is_empty() {
                continue;
            }
            match self.trim_cache.get(&(level, id, export.clone())) {
                Some((cached_text, ranges)) if *cached_text == text => {
                    cached.push((id, text, ranges.clone()))
                }
                _ => jobs.push(LabJob {
                    para_id: id,
                    request: prompts::trim_request(&export, level.percent()),
                    text,
                    export,
                }),
            }
        }
        if jobs.is_empty() {
            if cached.is_empty() {
                return self.set_notice("Nothing to trim", cx);
            }
            return self.show_trim(level, cached, cx);
        }
        self.run_lab(
            jobs,
            "AI is looking for cuts…",
            move |editor, results, total, cx| {
                editor.apply_trim_results(level, cached, results, total, cx)
            },
            cx,
        );
    }

    fn apply_trim_results(
        &mut self,
        level: TrimLevel,
        mut found: Found,
        results: LabResults,
        total: usize,
        cx: &mut Context<Self>,
    ) {
        let mut failed = 0;
        let mut first_error = None;
        for (job, result) in results {
            let response = match result {
                Ok(response) => response,
                Err(e) => {
                    failed += 1;
                    first_error.get_or_insert(e.to_string());
                    continue;
                }
            };
            let Some(para) = self.doc.para_index(job.para_id) else {
                continue;
            };
            let p = &self.doc.paragraphs[para];
            if p.text() != job.text {
                continue;
            }
            let ranges = prompts::locate_trims(&prompts::parse_trim(&response), p);
            self.trim_cache.insert(
                (level, job.para_id, job.export),
                (job.text.clone(), ranges.clone()),
            );
            found.push((job.para_id, job.text, ranges));
        }
        let notice = failure_notice(failed, total, first_error);
        let all_failed = failed == total && found.is_empty();
        if !all_failed {
            self.show_trim(level, found, cx);
        }
        if let Some(notice) = notice {
            self.set_notice(notice, cx);
        }
    }

    /// Show the preview of `found` (paragraphs whose text changed since are left out).
    fn show_trim(&mut self, level: TrimLevel, found: Found, cx: &mut Context<Self>) {
        let mut cuts: Vec<(usize, TrimSuggestion)> = Vec::new();
        for (id, text, ranges) in found {
            let Some(para) = self.doc.para_index(id) else {
                continue;
            };
            if self.doc.paragraphs[para].text() != text {
                continue;
            }
            cuts.extend(
                ranges
                    .into_iter()
                    .map(|range| (para, TrimSuggestion { para_id: id, range })),
            );
        }
        cuts.sort_by_key(|(para, cut)| (*para, cut.range.start));
        if cuts.is_empty() {
            self.trim_preview = None;
            self.set_notice("No cuts suggested", cx);
        } else {
            let cuts = cuts.into_iter().map(|(_, cut)| cut).collect();
            self.trim_preview = Some(TrimPreview::new(level, cuts));
        }
        cx.notify();
    }

    /// The bar's text and buttons, if the Lab has something showing.
    pub fn lab_bar(&self) -> Option<LabBar> {
        let preview = self.trim_preview.as_ref()?;
        let before = stats::document_counts(&self.doc).words;
        let after = stats::words_without(&self.doc, &self.lab_items());
        let pct = if before == 0 {
            0
        } else {
            ((before - after.min(before)) as f32 * 100. / before as f32).round() as usize
        };
        Some(LabBar {
            title: format!(
                "{} {before} → {after} words · −{pct}%",
                preview.level.label()
            ),
            hint: Some("Faded words would go. Click one to keep it.".into()),
            actions: vec![BarAction::MakeCuts, BarAction::Done],
        })
    }

    /// A click on the bar.
    pub fn bar_action(&mut self, action: BarAction, cx: &mut Context<Self>) {
        match action {
            BarAction::MakeCuts => self.make_cuts(cx),
            BarAction::Done => {
                self.stop_ai();
                self.trim_preview = None;
                cx.notify();
            }
        }
    }

    /// Ghost every active cut, as one undo step, and close the preview.
    fn make_cuts(&mut self, cx: &mut Context<Self>) {
        self.finish_alt_edit();
        let cuts = self.lab_items();
        if cuts.is_empty() {
            return self.set_notice("No cuts to make", cx);
        }
        // Not `begin_edit`: ghosting leaves the rendered text (and so the other cuts) as is.
        self.record_step(EditKind::Other);
        let mut made = 0;
        for (para, range) in cuts {
            if self.doc.set_ghost(para, range, true).is_ok() {
                made += 1;
            }
        }
        self.trim_preview = None;
        self.dirty = true;
        self.end_edit(cx);
        let passages = if made == 1 { "passage" } else { "passages" };
        self.set_notice(
            format!("Cut {made} {passages} (ghosted — Ctrl+Z to undo)"),
            cx,
        );
    }

    /// A click (or Enter) at `pos`: keep the active cut there, or make a kept one active
    /// again. Returns false when there is no cut at `pos`.
    pub(crate) fn toggle_cut_at(&mut self, pos: DocPos, cx: &mut Context<Self>) -> bool {
        let Some(id) = self.doc.paragraphs.get(pos.para).map(|p| p.id) else {
            return false;
        };
        let Some(preview) = &mut self.trim_preview else {
            return false;
        };
        let hit = preview.cuts.iter().position(|cut| {
            cut.para_id == id && cut.range.start <= pos.offset && pos.offset <= cut.range.end
        });
        let Some(ix) = hit else {
            return false;
        };
        if !preview.kept.remove(&ix) {
            preview.kept.insert(ix);
        }
        cx.notify();
        true
    }

    /// The active cuts of the trim preview, as `(paragraph, range)` in document order.
    fn lab_items(&self) -> Vec<(usize, Range<usize>)> {
        let Some(preview) = &self.trim_preview else {
            return Vec::new();
        };
        preview
            .cuts
            .iter()
            .enumerate()
            .filter(|(ix, _)| preview.is_active(*ix))
            .filter_map(|(_, cut)| Some((self.doc.para_index(cut.para_id)?, cut.range.clone())))
            .collect()
    }

    /// Tab, `]m`, `[m`: put the cursor at the start of the next (or previous) active cut,
    /// `count` times, wrapping around.
    pub(crate) fn jump_lab_item(&mut self, forward: bool, count: usize, cx: &mut Context<Self>) {
        let items = self.lab_items();
        if items.is_empty() {
            return;
        }
        let start = self.selection.range().start;
        let mut at = (start.para, start.offset);
        for _ in 0..count.max(1) {
            let next = if forward {
                items
                    .iter()
                    .find(|(p, r)| (*p, r.start) > at)
                    .or(items.first())
            } else {
                items
                    .iter()
                    .rev()
                    .find(|(p, r)| (*p, r.start) < at)
                    .or(items.last())
            };
            if let Some((para, range)) = next {
                at = (*para, range.start);
            }
        }
        self.set_selection(Selection::caret(DocPos::new(at.0, at.1)), cx);
    }

    pub(super) fn next_mark(&mut self, _: &NextMark, _: &mut Window, cx: &mut Context<Self>) {
        self.jump_lab_item(true, 1, cx);
    }

    pub(super) fn prev_mark(&mut self, _: &PrevMark, _: &mut Window, cx: &mut Context<Self>) {
        self.jump_lab_item(false, 1, cx);
    }
}
