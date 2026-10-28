//! The editor: owns the document, selection and history, and renders through `EditorElement`.

mod element;
mod input;
pub mod layout;

use std::ops::Range;
use std::sync::Arc;

use gpui::{
    App, Context, CursorStyle, FocusHandle, Focusable, Pixels, ScrollHandle, Task, Window, div,
    prelude::*,
};
use tulis_core::ai::LlmProvider;
use tulis_core::config::Config;
use tulis_core::history::History;
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

impl Render for Editor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .key_context("Editor")
            .track_focus(&self.focus_handle)
            .cursor(CursorStyle::IBeam)
            .w_full()
            .child(EditorElement::new(cx.entity()))
    }
}
