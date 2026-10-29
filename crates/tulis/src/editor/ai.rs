//! AI alternates and AI trim: build requests, run them off the UI thread, apply results.

use std::ops::Range;
use std::sync::Arc;

use gpui::{Context, Window, prelude::*};
use tulis_core::ai::{AiError, CompletionRequest, LlmProvider, prompts, provider_from_config};
use tulis_core::history::EditKind;
use tulis_core::{AiKind, DocPos, ParaId, Selection, Source};

use super::{Editor, TrimSuggestion};
use crate::actions::{AiAlternates, AiTrimDocument, AiTrimParagraph};

pub(crate) const STALE_NOTICE: &str = "Text changed while AI was working; result dropped";

/// What an alternates request was about, so the result can be applied (or recognised as stale).
pub(crate) struct AlternatesJob {
    para_id: ParaId,
    range: Range<usize>,
    original: String,
    whole_paragraph: bool,
    source: Source,
}

impl Editor {
    fn provider(&mut self, cx: &mut Context<Self>) -> Option<Arc<dyn LlmProvider>> {
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
        self.finish_alt_edit();
        let (para, range) = match self.target_range() {
            Ok(target) => target,
            Err(e) => return self.set_notice(e.to_string(), cx),
        };
        // Rewrite whole groups: part of an alternate stands for all of it.
        let range = self.doc.paragraphs[para].expand_to_alt_boundaries(range);
        let whole_paragraph = self.selection.is_empty();
        let (original, export, para_id) = {
            let p = &self.doc.paragraphs[para];
            (p.text()[range.clone()].to_string(), p.export_text(), p.id)
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
            source,
        };
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
        if self.doc.paragraphs[para].text().get(job.range.clone()) != Some(job.original.as_str()) {
            return self.set_notice(STALE_NOTICE, cx);
        }
        let count = texts.len();
        self.begin_edit(EditKind::Other, para..para + 1);
        if job.whole_paragraph {
            if let Err(e) = self.doc.add_ai_versions(para, texts, job.source) {
                return self.set_notice(e.to_string(), cx);
            }
            self.selection = Selection::caret(DocPos::new(para, 0));
        } else {
            match self
                .doc
                .add_ai_alternates(para, job.range, texts, job.source)
            {
                Ok(span) => self.selection = Selection::caret(DocPos::new(para, span.end)),
                Err(e) => return self.set_notice(e.to_string(), cx),
            }
        }
        self.end_edit(cx);
        self.set_notice(format!("{count} AI alternatives — Alt+↑/↓ to cycle"), cx);
    }

    pub(super) fn ai_trim_paragraph(
        &mut self,
        _: &AiTrimParagraph,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let para = self.selection.head.para;
        self.start_trim(vec![para], cx);
    }

    pub(super) fn ai_trim_document(
        &mut self,
        _: &AiTrimDocument,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let all = (0..self.doc.paragraphs.len()).collect();
        self.start_trim(all, cx);
    }

    fn start_trim(&mut self, paragraphs: Vec<usize>, cx: &mut Context<Self>) {
        self.finish_alt_edit();
        let jobs: Vec<(ParaId, String, CompletionRequest)> = paragraphs
            .into_iter()
            .filter_map(|i| {
                let p = &self.doc.paragraphs[i];
                let export = p.export_text();
                (!export.trim().is_empty())
                    .then(|| (p.id, p.text(), prompts::trim_request(&export)))
            })
            .collect();
        if jobs.is_empty() {
            return self.set_notice("Nothing to trim", cx);
        }
        let Some(provider) = self.provider(cx) else {
            return;
        };
        // One request per paragraph, in sequence, on the background executor.
        let work = cx.background_spawn(async move {
            jobs.into_iter()
                .map(|(id, text, request)| (id, text, provider.complete(&request)))
                .collect::<Vec<_>>()
        });
        self.ai_task = Some(cx.spawn(async move |this, cx| {
            let results = work.await;
            this.update(cx, |editor, cx| {
                editor.ai_task = None;
                editor.apply_trims(results, cx);
            })
            .ok();
        }));
        self.set_notice("AI is looking for cuts…", cx);
    }

    pub(crate) fn apply_trims(
        &mut self,
        results: Vec<(ParaId, String, Result<String, AiError>)>,
        cx: &mut Context<Self>,
    ) {
        let mut found = 0;
        let mut stale = 0;
        let mut error = None;
        for (id, text, result) in results {
            let response = match result {
                Ok(response) => response,
                Err(e) => {
                    error.get_or_insert(e.to_string());
                    continue;
                }
            };
            let Some(para) = self.doc.para_index(id) else {
                stale += 1;
                continue;
            };
            let p = &self.doc.paragraphs[para];
            if p.text() != text {
                stale += 1;
                continue;
            }
            let ranges = prompts::locate_trims(&prompts::parse_trim(&response), p);
            found += ranges.len();
            self.trims.retain(|t| t.para_id != id);
            self.trims.extend(
                ranges
                    .into_iter()
                    .map(|range| TrimSuggestion { para_id: id, range }),
            );
        }
        let mut notice = match (found, error) {
            (0, Some(e)) => e,
            (0, None) => "No cuts suggested".to_string(),
            (n, _) => format!("{n} suggested cuts — Tab to accept, Esc to dismiss"),
        };
        if stale > 0 {
            notice.push_str(&format!(" ({stale} changed paragraphs skipped)"));
        }
        self.set_notice(notice, cx);
    }
}
