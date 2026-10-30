//! The alternates sidebar's model: its target (Word/Selection, Sentence or Paragraph), the rows
//! it lists, and the operations behind its keys. Kept on `Editor`, so it is tested without
//! gpui-component.

use std::ops::Range;

use gpui::{Context, Window};
use tulis_core::alternates::OpError;
use tulis_core::motion::word_range_at;
use tulis_core::sentence::sentence_range_at;
use tulis_core::{ParaId, SegKind};

use super::{Editor, EditorEvent};
use crate::actions::OpenAlternates;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// The group or word under the cursor, or exactly the selection ("Selection").
    Word,
    Sentence,
    Paragraph,
}

impl Scope {
    /// Tab order.
    pub const ALL: [Scope; 3] = [Scope::Word, Scope::Sentence, Scope::Paragraph];

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&s| s == self).unwrap_or(0)
    }
}

/// An open sidebar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AltSession {
    pub para_id: ParaId,
    pub scope: Scope,
    /// The selection (or caret) the sidebar was opened on, local to the paragraph.
    pub origin: Range<usize>,
    /// The sidebar was opened on a selection: the Word tab reads "Selection" and targets exactly
    /// `origin`.
    pub selection: bool,
    /// The target's byte range (the group's span once there is a group); `0..len` for Paragraph.
    pub range: Range<usize>,
    /// The target's text when last seen. The sidebar closes when it is no longer at `range`.
    pub text: String,
    /// The highlighted row.
    pub highlight: usize,
    /// A preview has recorded its undo step since the last recorded operation.
    pub(crate) previewed: bool,
}

/// One option (or version) listed in the sidebar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AltRow {
    pub text: String,
    /// Shown in the page (`●`).
    pub shown: bool,
    /// AI-written (`✦`).
    pub ai: bool,
}

/// The word at a caret: the run under it, or the word just before it when the caret sits on
/// whitespace right after a word.
fn word_at_caret(text: &str, offset: usize) -> Range<usize> {
    let range = word_range_at(text, offset);
    if !text[range.clone()].trim().is_empty() || offset == 0 {
        return range;
    }
    let before = text[..offset].chars().next_back().map_or(0, char::len_utf8);
    let previous = word_range_at(text, offset - before);
    if text[previous.clone()].trim().is_empty() {
        range
    } else {
        previous
    }
}

impl Editor {
    /// The target range of `scope` in paragraph `para`, for a sidebar opened on `origin`.
    pub(crate) fn scope_range(
        &self,
        para: usize,
        scope: Scope,
        origin: &Range<usize>,
        selection: bool,
    ) -> Range<usize> {
        let p = &self.doc.paragraphs[para];
        let text = p.text();
        let clamp = |offset: usize| text.floor_char_boundary(offset.min(text.len()));
        let origin = clamp(origin.start)..clamp(origin.end);
        match scope {
            Scope::Paragraph => 0..text.len(),
            Scope::Word if selection => origin,
            Scope::Word => match p.alt_at(origin.start) {
                Some(seg) => p.segment_ranges()[seg].clone(),
                None => word_at_caret(&text, origin.start),
            },
            Scope::Sentence => {
                let first = sentence_range_at(&text, origin.start);
                let last = sentence_range_at(&text, origin.end);
                p.expand_to_alt_boundaries(first.start.min(last.start)..first.end.max(last.end))
            }
        }
    }

    /// Open the sidebar on the selection (one paragraph) or the cursor. `None` opens the Word
    /// (or Selection) tab. Creates nothing in the document.
    pub fn open_alternates(&mut self, scope: Option<Scope>, cx: &mut Context<Self>) {
        self.finish_alt_edit();
        let sel = self.selection.range();
        if sel.start.para != sel.end.para {
            return self.set_notice(OpError::MultiParagraph.to_string(), cx);
        }
        let origin = sel.start.offset..sel.end.offset;
        self.alt_session = Some(AltSession {
            para_id: self.doc.paragraphs[sel.start.para].id,
            scope: scope.unwrap_or(Scope::Word),
            selection: !origin.is_empty(),
            origin,
            range: 0..0,
            text: String::new(),
            highlight: 0,
            previewed: false,
        });
        self.retarget_alt_session();
        cx.emit(EditorEvent::AlternatesOpened);
        cx.notify();
    }

    pub(super) fn open_alternates_action(
        &mut self,
        _: &OpenAlternates,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_alternates(None, cx);
    }

    /// Switch the sidebar's tab; the list follows the new target.
    pub fn set_alt_scope(&mut self, scope: Scope, cx: &mut Context<Self>) {
        if let Some(session) = self.alt_session.as_mut() {
            session.scope = scope;
            self.retarget_alt_session();
            cx.notify();
        }
    }

    /// Recompute the target from the scope and origin, and highlight the shown row.
    fn retarget_alt_session(&mut self) {
        let Some(session) = self.alt_session.as_ref() else {
            return;
        };
        let Some(para) = self.doc.para_index(session.para_id) else {
            self.alt_session = None;
            return;
        };
        let range = self.scope_range(para, session.scope, &session.origin, session.selection);
        let text = self.doc.paragraphs[para].text()[range.clone()].to_string();
        if let Some(session) = self.alt_session.as_mut() {
            session.range = range;
            session.text = text;
        }
        self.highlight_shown_row();
    }

    pub(crate) fn highlight_shown_row(&mut self) {
        let shown = self.alt_rows().iter().position(|r| r.shown).unwrap_or(0);
        if let Some(session) = self.alt_session.as_mut() {
            session.highlight = shown;
        }
    }

    /// The target's paragraph and, when the target is an existing group, its segment.
    pub(crate) fn alt_session_target(&self) -> Option<(usize, Option<usize>)> {
        let session = self.alt_session.as_ref()?;
        let para = self.doc.para_index(session.para_id)?;
        if session.scope == Scope::Paragraph {
            return Some((para, None));
        }
        let p = &self.doc.paragraphs[para];
        let ranges = p.segment_ranges();
        let seg =
            (0..ranges.len()).find(|&i| p.segments()[i].is_alt() && ranges[i] == session.range);
        Some((para, seg))
    }

    /// The rows the sidebar lists: one per option (or version) in stored order, or a single row
    /// with the target's text when there is no group (or only one version).
    pub fn alt_rows(&self) -> Vec<AltRow> {
        let Some(session) = self.alt_session.as_ref() else {
            return Vec::new();
        };
        let Some((para, seg)) = self.alt_session_target() else {
            return Vec::new();
        };
        let p = &self.doc.paragraphs[para];
        let listed = match (session.scope, seg) {
            (Scope::Paragraph, _) if p.versions.len() > 1 => Some((p.version_texts(), p.active)),
            (_, Some(seg)) => match &p.segments()[seg].kind {
                SegKind::Alt { active, .. } => Some((p.options_at(seg), *active)),
                SegKind::Plain(_) => None,
            },
            _ => None,
        };
        match listed {
            Some((items, active)) => items
                .into_iter()
                .enumerate()
                .map(|(i, (text, source))| AltRow {
                    text,
                    shown: i == active,
                    ai: source.is_ai(),
                })
                .collect(),
            None => {
                let text = match session.scope {
                    Scope::Paragraph => p.text(),
                    _ => session.text.clone(),
                };
                vec![AltRow {
                    text,
                    shown: true,
                    ai: false,
                }]
            }
        }
    }

    /// Close the sidebar, keeping whatever is shown.
    pub fn close_alternates(&mut self, cx: &mut Context<Self>) {
        if self.alt_session.take().is_some() {
            cx.emit(EditorEvent::AlternatesClosed);
            cx.notify();
        }
    }

    /// Close the sidebar if its target is gone: the paragraph was deleted, or (below Paragraph
    /// scope) the target's text is no longer at its range.
    pub(crate) fn sync_alt_session(&mut self, cx: &mut Context<Self>) {
        let Some(session) = self.alt_session.as_ref() else {
            return;
        };
        let alive = match self.doc.para_index(session.para_id) {
            None => false,
            Some(_) if session.scope == Scope::Paragraph => true,
            Some(para) => {
                self.doc.paragraphs[para].text().get(session.range.clone())
                    == Some(session.text.as_str())
            }
        };
        if !alive {
            self.close_alternates(cx);
        }
    }
}
