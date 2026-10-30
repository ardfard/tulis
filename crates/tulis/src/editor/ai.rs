//! AI alternates: build requests, run them off the UI thread, apply results.

use std::ops::Range;
use std::sync::Arc;

use gpui::{Context, Window, prelude::*};
use tulis_core::ai::{AiError, LlmProvider, prompts, provider_from_config};
use tulis_core::history::EditKind;
use tulis_core::{AiKind, DocPos, ParaId, Selection, Source};

use super::alt_session::MERGED_NOTICE;
use super::{Editor, Scope};
use crate::actions::AiAlternates;

pub(crate) const STALE_NOTICE: &str = "Text changed while AI was working; result dropped";

/// What an alternates request was about, so the result can be applied (or recognised as stale).
pub(crate) struct AlternatesJob {
    para_id: ParaId,
    range: Range<usize>,
    original: String,
    whole_paragraph: bool,
    /// The target was an existing group (or a paragraph with versions). Its result still
    /// applies after another option is shown, as long as the group still holds `original`.
    group: bool,
    source: Source,
}

impl Editor {
    pub(super) fn provider(&mut self, cx: &mut Context<Self>) -> Option<Arc<dyn LlmProvider>> {
        if let Some(provider) = self.provider_override.clone() {
            return Some(provider);
        }
        match provider_from_config(&self.config.ai, |name| std::env::var(name).ok()) {
            Ok(provider) => Some(provider),
            Err(e) => {
                self.set_notice(e.to_string(), cx);
                None
            }
        }
    }

    pub(super) fn ai_alternates(
        &mut self,
        _: &AiAlternates,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ask_ai_alternates(cx);
    }

    /// The sidebar's own AI entry points ("✦ Ask AI", Ctrl+Shift+G in the sidebar): AI
    /// alternatives for its target. Without a sidebar, the page's target.
    pub fn ask_ai_for_session(&mut self, cx: &mut Context<Self>) {
        self.finish_alt_edit();
        let session = self.alt_session_target().zip(
            self.alt_session
                .as_ref()
                .map(|s| (s.scope, s.range.clone())),
        );
        match session {
            Some(((para, _), (Scope::Paragraph, _))) => {
                let len = self.doc.paragraphs[para].len();
                self.request_ai_alternates(para, 0..len, true, cx)
            }
            Some(((para, _), (_, range))) => self.request_ai_alternates(para, range, false, cx),
            None => self.ask_ai_alternates(cx),
        }
    }

    /// The page's AI alternatives (Editor action, vim `gA`, context menu): for the selection, or
    /// the cursor's paragraph as versions. An open sidebar retargets to it first, so the results
    /// show in its list.
    pub fn ask_ai_alternates(&mut self, cx: &mut Context<Self>) {
        self.finish_alt_edit();
        let (para, range) = match self.target_range() {
            Ok(target) => target,
            Err(e) => return self.set_notice(e.to_string(), cx),
        };
        // Inside a Lab mark, the mark is the target.
        let head = self.selection.head;
        if self.selection.is_empty()
            && let Some(ix) = self.mark_at(head)
        {
            let range = self.marks[ix].range.clone();
            self.retarget_alt_session_to(head.para, range.clone(), false, cx);
            return self.request_ai_alternates(head.para, range, false, cx);
        }
        let whole_paragraph = self.selection.is_empty();
        self.retarget_alt_session_to(para, range.clone(), whole_paragraph, cx);
        self.request_ai_alternates(para, range, whole_paragraph, cx);
    }

    fn request_ai_alternates(
        &mut self,
        para: usize,
        range: Range<usize>,
        whole_paragraph: bool,
        cx: &mut Context<Self>,
    ) {
        // Rewrite whole groups: part of an alternate stands for all of it.
        let range = self.doc.paragraphs[para].expand_to_alt_boundaries(range);
        let (original, export, para_id, group) = {
            let p = &self.doc.paragraphs[para];
            let group = if whole_paragraph {
                p.versions.len() > 1
            } else {
                p.alt_starting_at(range.start)
                    .is_some_and(|seg| p.segment_ranges()[seg] == range)
            };
            (
                p.text()[range.clone()].to_string(),
                p.export_text(),
                p.id,
                group,
            )
        };
        if original.trim().is_empty() {
            return self.set_notice("Nothing to rewrite here", cx);
        }
        let count = self.config.ai.num_alternates.clamp(1, 8);
        let request = prompts::alternates_request(&export, &original, count);
        let Some(provider) = self.provider(cx) else {
            return;
        };
        let source = Source::Ai {
            provider: provider.name().into(),
            model: provider.model().into(),
            kind: AiKind::Rewrite,
        };
        let job = AlternatesJob {
            para_id,
            range,
            original,
            whole_paragraph,
            group,
            source,
        };
        self.stop_ai();
        let work = cx.background_spawn(async move { provider.complete(&request) });
        self.ai_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |editor, cx| {
                editor.ai_task = None;
                editor.apply_ai_alternates(job, result, cx);
            })
            .ok();
        }));
        self.set_notice("AI is writing alternatives…", cx);
    }

    pub(crate) fn apply_ai_alternates(
        &mut self,
        job: AlternatesJob,
        result: Result<String, AiError>,
        cx: &mut Context<Self>,
    ) {
        let response = match result {
            Ok(response) => response,
            Err(e) => return self.set_notice(e.to_string(), cx),
        };
        let texts = prompts::parse_alternates(&response, &job.original);
        if texts.is_empty() {
            return self.set_notice("AI returned no alternatives", cx);
        }
        self.finish_alt_edit();
        let Some(para) = self.doc.para_index(job.para_id) else {
            return self.set_notice(STALE_NOTICE, cx);
        };
        let Some(range) = self.ai_job_range(para, &job) else {
            return self.set_notice(STALE_NOTICE, cx);
        };
        let count = texts.len();
        let for_session = self.alt_session.as_ref().is_some_and(|s| {
            s.para_id == job.para_id
                && (s.scope == Scope::Paragraph) == job.whole_paragraph
                && (job.whole_paragraph
                    || self.doc.paragraphs[para].expand_to_alt_boundaries(s.range.clone()) == range)
        });
        let merged = !job.whole_paragraph && self.merges_inner_groups(para, range.clone());
        self.begin_edit(EditKind::Other, para..para + 1);
        let span = if job.whole_paragraph {
            if let Err(e) = self.doc.add_ai_versions(para, texts, job.source) {
                return self.set_notice(e.to_string(), cx);
            }
            self.selection = Selection::caret(DocPos::new(para, 0));
            0..self.doc.paragraphs[para].len()
        } else {
            match self.doc.add_ai_alternates(para, range, texts, job.source) {
                Ok(span) => {
                    self.selection = Selection::caret(DocPos::new(para, span.end));
                    span
                }
                Err(e) => return self.set_notice(e.to_string(), cx),
            }
        };
        if for_session {
            self.alt_session_added(para, span);
        }
        self.end_edit(cx);
        if merged {
            self.set_notice(MERGED_NOTICE, cx);
        }
        self.set_notice(format!("{count} AI alternatives — Alt+↑/↓ to cycle"), cx);
    }

    /// Where a finished alternates job applies now, or `None` when it is stale. A group job
    /// applies to the group's current span (or the paragraph) while the group still has the
    /// original text among its options (or versions); other jobs need the original text still
    /// at their range.
    fn ai_job_range(&self, para: usize, job: &AlternatesJob) -> Option<Range<usize>> {
        let p = &self.doc.paragraphs[para];
        let has_original =
            |items: Vec<(String, Source)>| items.iter().any(|(t, _)| *t == job.original);
        if job.group && job.whole_paragraph {
            return has_original(p.version_texts()).then(|| 0..p.len());
        }
        if job.group {
            let seg = p.alt_starting_at(job.range.start)?;
            return has_original(p.options_at(seg)).then(|| p.segment_ranges()[seg].clone());
        }
        (p.text().get(job.range.clone()) == Some(job.original.as_str())).then(|| job.range.clone())
    }
}
