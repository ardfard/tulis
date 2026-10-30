//! The alternates sidebar's model: its target (Word/Selection, Sentence or Paragraph), the rows
//! it lists, and the operations behind its keys. Kept on `Editor`, so it is tested without
//! gpui-component.

use std::ops::Range;

use gpui::{Context, Window};
use tulis_core::alternates::OpError;
use tulis_core::history::EditKind;
use tulis_core::motion::word_range_at;
use tulis_core::sentence::sentence_range_at;
use tulis_core::{DocPos, ParaId, SegKind, Selection};

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
    /// The undo step this session's previews share: the id of the newest undo step right after
    /// the first preview recorded it. Previews record a new step once it is no longer the newest
    /// (after an undo, typing or any other recorded edit).
    pub(crate) preview_step: Option<u64>,
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
            preview_step: None,
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

impl Editor {
    /// Record the target's new range (and text) after an operation changed it.
    fn update_alt_target(&mut self, para: usize, range: Range<usize>) {
        let text = self.doc.paragraphs[para]
            .text()
            .get(range.clone())
            .unwrap_or_default()
            .to_string();
        if let Some(session) = self.alt_session.as_mut() {
            if session.scope == Scope::Word {
                session.origin = if session.selection {
                    range.clone()
                } else {
                    range.start..range.start
                };
            }
            session.range = range;
            session.text = text;
        }
    }

    /// Highlight row `index` and show it in the page. A session's previews share one undo step
    /// (recorded by the first; another recorded operation starts a new one).
    pub fn preview_alternate(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some((para, seg)) = self.alt_session_target() else {
            return;
        };
        let rows = self.alt_rows();
        let Some(row) = rows.get(index) else {
            return;
        };
        let Some(session) = self.alt_session.as_mut() else {
            return;
        };
        session.highlight = index;
        if row.shown {
            return cx.notify();
        }
        let scope = session.scope;
        let shares_step =
            session.preview_step.is_some() && session.preview_step == self.history.newest_undo_id();
        if shares_step {
            self.drop_transient(para..para + 1);
        } else {
            self.begin_edit(EditKind::Other, para..para + 1);
            let step = self.history.newest_undo_id();
            if let Some(session) = self.alt_session.as_mut() {
                session.preview_step = step;
            }
        }
        let range = match (scope, seg) {
            (Scope::Paragraph, _) => {
                self.doc.set_active_version(para, index);
                0..self.doc.paragraphs[para].len()
            }
            (_, Some(seg)) => match self.doc.set_active_option(para, seg, index) {
                Some(span) => span,
                None => return,
            },
            _ => return,
        };
        self.update_alt_target(para, range.clone());
        self.selection = Selection::caret(DocPos::new(para, range.end));
        self.end_edit(cx);
    }

    /// Move the highlight by `delta` rows (clamped) and preview that row.
    pub fn move_alt_highlight(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(session) = self.alt_session.as_ref() else {
            return;
        };
        let last = self.alt_rows().len().saturating_sub(1);
        let index = session.highlight.saturating_add_signed(delta).min(last);
        self.preview_alternate(index, cx);
    }

    /// Keep row `index`: show it and put the cursor at the end of the target.
    pub fn keep_alternate(&mut self, index: usize, cx: &mut Context<Self>) {
        self.preview_alternate(index, cx);
        if let Some((para, _)) = self.alt_session_target() {
            let end = self.alt_session.as_ref().map_or(0, |s| s.range.end);
            self.set_selection(Selection::caret(DocPos::new(para, end)), cx);
        }
    }

    /// Remove row `index` (never the last one). A group left with one option becomes plain text.
    pub fn remove_alternate(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some((para, seg)) = self.alt_session_target() else {
            return;
        };
        let rows = self.alt_rows().len();
        let Some(scope) = self.alt_session.as_ref().map(|s| s.scope) else {
            return;
        };
        // Two or more rows list a group's options (or the versions), so `index` names one.
        // Checked before recording, so a no-op records no undo step.
        let removable = rows >= 2
            && index < rows
            && match scope {
                Scope::Paragraph => self.doc.paragraphs[para].versions.len() == rows,
                Scope::Word | Scope::Sentence => seg.is_some(),
            };
        if !removable {
            return;
        }
        self.begin_edit(EditKind::Other, para..para + 1);
        let range = match (scope, seg) {
            (Scope::Paragraph, _) => {
                self.doc.remove_version(para, index);
                0..self.doc.paragraphs[para].len()
            }
            (_, Some(seg)) => self
                .doc
                .remove_option_at(para, seg, index)
                .expect("checked: the group has this option and another"),
            (_, None) => unreachable!("checked: the target is a group"),
        };
        self.update_alt_target(para, range.clone());
        if let Some(session) = self.alt_session.as_mut() {
            session.preview_step = None;
        }
        self.highlight_shown_row();
        self.selection = Selection::caret(DocPos::new(para, range.end));
        self.end_edit(cx);
    }
}

/// Shown when a Sentence or Selection group swallows the groups inside it.
pub(crate) const MERGED_NOTICE: &str = "Inner alternates merged — Ctrl+Z to restore";

impl Editor {
    /// After an add (typed or AI) changed the target to `range`: follow it and highlight the
    /// new shown row. The next preview records a fresh undo step.
    pub(crate) fn alt_session_added(&mut self, para: usize, range: Range<usize>) {
        self.update_alt_target(para, range);
        if let Some(session) = self.alt_session.as_mut() {
            session.preview_step = None;
        }
        self.highlight_shown_row();
    }

    /// Whether making a group over `range` (grown around alternates, as `make_alt` does) would
    /// merge alternates strictly inside it.
    pub(crate) fn merges_inner_groups(&self, para: usize, range: Range<usize>) -> bool {
        let p = &self.doc.paragraphs[para];
        let range = p.expand_to_alt_boundaries(range);
        p.segments().iter().zip(p.segment_ranges()).any(|(seg, r)| {
            seg.is_alt() && r != range && range.start <= r.start && r.end <= range.end
        })
    }

    /// Add `text` as a new option of the target (creating its group on the first add), or as a
    /// new version on the Paragraph tab, and show it.
    pub fn add_alternate_text(&mut self, text: &str, cx: &mut Context<Self>) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let Some((para, _)) = self.alt_session_target() else {
            return;
        };
        let Some((scope, range)) = self
            .alt_session
            .as_ref()
            .map(|s| (s.scope, s.range.clone()))
        else {
            return;
        };
        if scope != Scope::Paragraph && range.is_empty() {
            return self.set_notice("Nothing to rewrite here", cx);
        }
        let merged = scope != Scope::Paragraph && self.merges_inner_groups(para, range.clone());
        // Applied to a copy first, so a refused add records no undo step.
        let mut doc = self.doc.clone();
        let range = if scope == Scope::Paragraph {
            doc.add_version(para, text.to_string());
            0..doc.paragraphs[para].len()
        } else {
            match doc.add_option(para, range, text.to_string()) {
                Ok(span) => span,
                Err(e) => return self.set_notice(e.to_string(), cx),
            }
        };
        self.begin_edit(EditKind::Other, para..para + 1);
        self.doc = doc;
        self.alt_session_added(para, range.clone());
        self.selection = Selection::caret(DocPos::new(para, range.end));
        self.end_edit(cx);
        if merged {
            self.set_notice(MERGED_NOTICE, cx);
        }
    }
}
