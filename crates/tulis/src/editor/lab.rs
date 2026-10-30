//! The Lab on the page: trim previews, guide-check marks, the runner every Lab request goes
//! through, and its cancel flag.

use std::collections::HashSet;
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::{Context, Window, prelude::*};
use tulis_core::ai::prompts::MarkKind;
use tulis_core::ai::{AiError, CompletionRequest, prompts};
use tulis_core::history::EditKind;
use tulis_core::sentence;
use tulis_core::stats;
use tulis_core::{AiKind, DocPos, ParaId, Selection, Source};

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

/// A guide check: it marks passages (or, for typos, proposes fixes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Check {
    Fix,
    Weakest,
    Long,
    Convoluted,
    Tone,
    Hedges,
}

impl Check {
    /// In the order the Lab lists them.
    pub const ALL: [Check; 6] = [
        Check::Fix,
        Check::Weakest,
        Check::Long,
        Check::Convoluted,
        Check::Tone,
        Check::Hedges,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Check::Fix => "Fix punctuation and typos",
            Check::Weakest => "Mark the weakest sentences",
            Check::Long => "Mark sentences that run long",
            Check::Convoluted => "Mark convoluted sentences",
            Check::Tone => "Mark words that don't fit the tone",
            Check::Hedges => "Mark hedges and filler",
        }
    }

    /// What the bar calls its marks.
    pub fn short_label(self) -> &'static str {
        match self {
            Check::Fix => "fixes",
            Check::Weakest => "weakest sentences",
            Check::Long => "long sentences",
            Check::Convoluted => "convoluted sentences",
            Check::Tone => "off-tone words",
            Check::Hedges => "hedges and filler",
        }
    }

    /// The one-line description "what each idea does" shows.
    pub fn description(self) -> &'static str {
        match self {
            Check::Fix => "AI proposes corrections as alternates; Alt+↑/↓ compares them.",
            Check::Weakest => "AI marks the sentences that do the least work.",
            Check::Long => "Marks sentences over your word limit (long_sentence_words). No AI.",
            Check::Convoluted => "AI marks sentences that are hard to follow.",
            Check::Tone => "AI marks words that don't match the rest of the document.",
            Check::Hedges => "AI marks padding: I think, maybe, really, in order to…",
        }
    }

    fn mark_kind(self) -> Option<MarkKind> {
        match self {
            Check::Weakest => Some(MarkKind::Weakest),
            Check::Convoluted => Some(MarkKind::Convoluted),
            Check::Tone => Some(MarkKind::Tone),
            Check::Hedges => Some(MarkKind::Hedges),
            Check::Fix | Check::Long => None,
        }
    }
}

/// A passage a check marked. View state, like the trim preview: editing its paragraph drops it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    pub para_id: ParaId,
    /// A byte range of the paragraph's rendered text.
    pub range: Range<usize>,
    pub check: Check,
    pub note: String,
}

/// How much of the document the tone check sends along.
const TONE_EXCERPT_CHARS: usize = 1500;

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
    /// Walking through the cuts one at a time.
    pub walk: Option<Walk>,
}

/// A walk through a trim preview's cuts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Walk {
    /// The cut being decided (an index into `cuts`).
    pub at: usize,
    /// Cuts ghosted and kept so far.
    pub cut: usize,
    pub kept: usize,
}

impl TrimPreview {
    pub fn new(level: TrimLevel, cuts: Vec<TrimSuggestion>) -> Self {
        Self {
            level,
            cuts,
            kept: HashSet::new(),
            walk: None,
        }
    }

    /// The first active cut at index `from` or later.
    fn next_active(&self, from: usize) -> Option<usize> {
        (from..self.cuts.len()).find(|&ix| self.is_active(ix))
    }

    /// Remove cut `ix` (once it is ghosted); later indices shift down.
    fn remove(&mut self, ix: usize) {
        self.cuts.remove(ix);
        self.kept = self
            .kept
            .iter()
            .filter(|&&k| k != ix)
            .map(|&k| if k > ix { k - 1 } else { k })
            .collect();
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

    /// Keep only the cuts `keep` accepts; kept cuts stay kept. Dropping any ends a walk.
    pub(crate) fn retain(&mut self, keep: impl Fn(&TrimSuggestion) -> bool) {
        if !self.cuts.iter().all(&keep) {
            self.walk = None;
        }
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
    Walk,
    Done,
    Clear,
}

impl BarAction {
    pub fn label(self) -> &'static str {
        match self {
            BarAction::MakeCuts => "Make the cuts",
            BarAction::Walk => "Walk through",
            BarAction::Done => "Done",
            BarAction::Clear => "Clear",
        }
    }

    /// For element ids and test selectors.
    pub fn id(self) -> &'static str {
        match self {
            BarAction::MakeCuts => "make-cuts",
            BarAction::Walk => "walk",
            BarAction::Done => "done",
            BarAction::Clear => "clear",
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

/// Located fixes (or marks): ranges of a paragraph's rendered text, each with its text.
type Located = Vec<(Range<usize>, String)>;

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
            self.marks.clear();
            self.lab_fixes = None;
        }
        cx.notify();
    }

    /// The bar's text and buttons, if the Lab has something showing.
    pub fn lab_bar(&self) -> Option<LabBar> {
        let Some(preview) = self.trim_preview.as_ref() else {
            let Some(first) = self.marks.first() else {
                let fixes = self.lab_fixes?;
                let s = if fixes == 1 { "" } else { "es" };
                return Some(LabBar {
                    title: format!("{fixes} fix{s} proposed — Alt+↑/↓ to compare"),
                    hint: None,
                    actions: vec![BarAction::Clear],
                });
            };
            let head = self.selection.head;
            let note = self.mark_at(head).map(|ix| self.marks[ix].note.clone());
            return Some(LabBar {
                title: format!(
                    "{} marked · {}",
                    self.marks.len(),
                    first.check.short_label()
                ),
                hint: note.filter(|note| !note.is_empty()),
                actions: vec![BarAction::Clear],
            });
        };
        if let Some(walk) = preview.walk {
            let decided = walk.cut + walk.kept;
            let left = (walk.at..preview.cuts.len())
                .filter(|&ix| preview.is_active(ix))
                .count();
            return Some(LabBar {
                title: format!("Cut {} of {}", decided + 1, decided + left),
                hint: Some("y or Enter cuts it · n or Backspace keeps it · Esc stops".into()),
                actions: vec![BarAction::Done],
            });
        }
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
            actions: vec![BarAction::MakeCuts, BarAction::Walk, BarAction::Done],
        })
    }

    /// A click on the bar.
    pub fn bar_action(&mut self, action: BarAction, cx: &mut Context<Self>) {
        match action {
            BarAction::MakeCuts => self.make_cuts(cx),
            BarAction::Walk => self.start_walk(cx),
            BarAction::Done => {
                self.stop_ai();
                self.trim_preview = None;
                cx.notify();
            }
            BarAction::Clear => {
                self.stop_ai();
                self.marks.clear();
                self.lab_fixes = None;
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

    /// Walking through the trim preview's cuts (`y`/`n`, Enter/Backspace and Esc are captured).
    pub fn walking(&self) -> bool {
        self.trim_preview.as_ref().is_some_and(|t| t.walk.is_some())
    }

    /// Walk through: select the first active cut and capture the walk keys.
    fn start_walk(&mut self, cx: &mut Context<Self>) {
        let Some(preview) = &mut self.trim_preview else {
            return;
        };
        let Some(at) = preview.next_active(0) else {
            return self.set_notice("No cuts to walk through", cx);
        };
        preview.walk = Some(Walk {
            at,
            cut: 0,
            kept: 0,
        });
        self.select_walk_cut(cx);
    }

    /// Select the walk's current cut (scrolling to it).
    fn select_walk_cut(&mut self, cx: &mut Context<Self>) {
        let Some(preview) = &self.trim_preview else {
            return;
        };
        let Some(walk) = preview.walk else { return };
        let cut = &preview.cuts[walk.at];
        let Some(para) = self.doc.para_index(cut.para_id) else {
            return;
        };
        let range = cut.range.clone();
        self.set_selection(
            Selection {
                anchor: DocPos::new(para, range.start),
                head: DocPos::new(para, range.end),
            },
            cx,
        );
    }

    /// `y`/Enter (`cut`) ghosts the walk's current cut, as its own undo step; `n`/Backspace
    /// keeps it. Either moves on; after the last cut the walk and the preview end.
    pub(crate) fn walk_step(&mut self, cut: bool, cx: &mut Context<Self>) {
        let Some(walk) = self.trim_preview.as_ref().and_then(|t| t.walk) else {
            return;
        };
        let target = self.trim_preview.as_ref().and_then(|t| {
            let c = &t.cuts[walk.at];
            Some((self.doc.para_index(c.para_id)?, c.range.clone()))
        });
        let mut next = walk;
        if cut {
            if let Some((para, range)) = target {
                self.record_step(EditKind::Other);
                if self.doc.set_ghost(para, range, true).is_ok() {
                    self.dirty = true;
                }
            }
            next.cut += 1;
        } else {
            next.kept += 1;
        }
        let Some(preview) = &mut self.trim_preview else {
            return;
        };
        let from = if cut {
            preview.remove(walk.at);
            walk.at
        } else {
            preview.kept.insert(walk.at);
            walk.at + 1
        };
        match preview.next_active(from) {
            Some(at) => {
                preview.walk = Some(Walk { at, ..next });
                self.select_walk_cut(cx);
            }
            None => {
                self.trim_preview = None;
                let head = self.selection.head;
                self.set_selection(Selection::caret(head), cx);
                let total = next.cut + next.kept;
                self.set_notice(
                    format!(
                        "Walked through {total} cuts: {} cut, {} kept",
                        next.cut, next.kept
                    ),
                    cx,
                );
            }
        }
    }

    /// Esc during a walk: stop it, leaving the preview. Returns false when not walking.
    pub(crate) fn end_walk(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(preview) = &mut self.trim_preview else {
            return false;
        };
        if preview.walk.take().is_none() {
            return false;
        }
        let head = self.selection.head;
        self.set_selection(Selection::caret(head), cx);
        true
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

    /// The mark under `pos` (edges included), as an index into `marks`.
    pub(crate) fn mark_at(&self, pos: DocPos) -> Option<usize> {
        let id = self.doc.paragraphs.get(pos.para)?.id;
        self.marks.iter().position(|m| {
            m.para_id == id && m.range.start <= pos.offset && pos.offset <= m.range.end
        })
    }

    /// The marks in paragraph `id` (drawn tinted).
    pub(crate) fn mark_ranges(&self, id: ParaId) -> Vec<Range<usize>> {
        self.marks
            .iter()
            .filter(|m| m.para_id == id)
            .map(|m| m.range.clone())
            .collect()
    }

    /// Run `check` over the paragraphs in `scope`. The long-sentence check is local; the others
    /// send one request per non-empty paragraph. The marks replace any marks or trim preview.
    pub fn run_check(&mut self, check: Check, scope: &[ParaId], cx: &mut Context<Self>) {
        self.finish_alt_edit();
        let paras: Vec<usize> = scope
            .iter()
            .filter_map(|id| self.doc.para_index(*id))
            .filter(|&para| !self.doc.paragraphs[para].export_text().trim().is_empty())
            .collect();
        if paras.is_empty() {
            return self.set_notice("Nothing to check", cx);
        }
        if check == Check::Fix {
            return self.run_fixes(paras, cx);
        }
        let Some(kind) = check.mark_kind() else {
            let max = self.config.editor.long_sentence_words;
            let mut marks = Vec::new();
            for para in paras {
                let p = &self.doc.paragraphs[para];
                for (range, words) in sentence::long_sentences(&p.text(), max) {
                    marks.push(Mark {
                        para_id: p.id,
                        range,
                        check,
                        note: format!("{words} words"),
                    });
                }
            }
            return self.show_marks(marks, cx);
        };
        let context = (kind == MarkKind::Tone).then(|| self.document_excerpt());
        let jobs = paras
            .into_iter()
            .map(|para| {
                let p = &self.doc.paragraphs[para];
                let export = p.export_text();
                LabJob {
                    para_id: p.id,
                    text: p.text(),
                    request: prompts::mark_request(kind, &export, context.as_deref()),
                    export,
                }
            })
            .collect();
        self.run_lab(
            jobs,
            "AI is reading…",
            move |editor, results, total, cx| editor.apply_mark_results(check, results, total, cx),
            cx,
        );
    }

    /// "Fix punctuation and typos": one request per paragraph; the fixes become AI alternates.
    fn run_fixes(&mut self, paras: Vec<usize>, cx: &mut Context<Self>) {
        let Some(provider) = self.provider(cx) else {
            return;
        };
        let source = Source::Ai {
            provider: provider.name().into(),
            model: provider.model().into(),
            kind: AiKind::Rewrite,
        };
        let jobs = paras
            .into_iter()
            .map(|para| {
                let p = &self.doc.paragraphs[para];
                let export = p.export_text();
                LabJob {
                    para_id: p.id,
                    text: p.text(),
                    request: prompts::fix_request(&export),
                    export,
                }
            })
            .collect();
        self.run_lab(
            jobs,
            "AI is proofreading…",
            move |editor, results, total, cx| editor.apply_fix_results(source, results, total, cx),
            cx,
        );
    }

    /// Every located fix becomes an AI alternate (shown), all in one undo step.
    fn apply_fix_results(
        &mut self,
        source: Source,
        results: LabResults,
        total: usize,
        cx: &mut Context<Self>,
    ) {
        let mut failed = 0;
        let mut first_error = None;
        let mut found: Vec<(usize, Located)> = Vec::new();
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
            let fixes = prompts::locate_fixes(&prompts::parse_fixes(&response), p);
            if !fixes.is_empty() {
                found.push((para, fixes));
            }
        }
        let notice = failure_notice(failed, total, first_error);
        if found.is_empty() {
            if failed < total {
                self.set_notice("No fixes needed", cx);
            }
        } else {
            self.finish_alt_edit();
            self.begin_edit(EditKind::Other, 0..self.doc.paragraphs.len());
            let mut made = 0;
            for (para, fixes) in found {
                // Last first, so the earlier ranges still hold.
                for (range, fixed) in fixes.into_iter().rev() {
                    let added =
                        self.doc
                            .add_ai_alternates(para, range, vec![fixed], source.clone());
                    if added.is_ok() {
                        made += 1;
                    }
                }
            }
            self.trim_preview = None;
            self.marks.clear();
            self.lab_fixes = Some(made);
            self.end_edit(cx);
        }
        if let Some(notice) = notice {
            self.set_notice(notice, cx);
        }
    }

    /// The start of the document's export text, for the tone check.
    fn document_excerpt(&self) -> String {
        let text = self
            .doc
            .paragraphs
            .iter()
            .map(|p| p.export_text())
            .filter(|t| !t.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");
        text.chars().take(TONE_EXCERPT_CHARS).collect()
    }

    fn apply_mark_results(
        &mut self,
        check: Check,
        results: LabResults,
        total: usize,
        cx: &mut Context<Self>,
    ) {
        let mut failed = 0;
        let mut first_error = None;
        let mut marks = Vec::new();
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
            let hits = prompts::parse_marks(&response);
            marks.extend(
                prompts::locate_marks(&hits, p)
                    .into_iter()
                    .map(|(range, note)| Mark {
                        para_id: job.para_id,
                        range,
                        check,
                        note,
                    }),
            );
        }
        if failed < total {
            self.show_marks(marks, cx);
        }
        if let Some(notice) = failure_notice(failed, total, first_error) {
            self.set_notice(notice, cx);
        }
    }

    /// Show `marks` (in document order) in place of any marks or trim preview.
    fn show_marks(&mut self, mut marks: Vec<Mark>, cx: &mut Context<Self>) {
        let order = |m: &Mark| (self.doc.para_index(m.para_id), m.range.start);
        marks.sort_by_key(order);
        if marks.is_empty() {
            self.set_notice("Nothing marked", cx);
        }
        self.marks = marks;
        self.trim_preview = None;
        self.lab_fixes = None;
        cx.notify();
    }

    /// The active cuts of the trim preview, or else the marks, as `(paragraph, range)` in
    /// document order.
    fn lab_items(&self) -> Vec<(usize, Range<usize>)> {
        let Some(preview) = &self.trim_preview else {
            return self
                .marks
                .iter()
                .filter_map(|m| Some((self.doc.para_index(m.para_id)?, m.range.clone())))
                .collect();
        };
        preview
            .cuts
            .iter()
            .enumerate()
            .filter(|(ix, _)| preview.is_active(*ix))
            .filter_map(|(_, cut)| Some((self.doc.para_index(cut.para_id)?, cut.range.clone())))
            .collect()
    }

    /// Tab, `]m`, `[m`: put the cursor at the start of the next (or previous) active cut or mark,
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
