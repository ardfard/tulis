//! The alternates sidebar: Word/Selection, Sentence and Paragraph tabs over the list of options
//! the editor's `AltSession` describes.

use gpui::{
    App, ClickEvent, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement, Render,
    Styled, Subscription, WeakEntity, Window, div, prelude::*, px,
};
use gpui_component::Sizable;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::tab::{Tab, TabBar};

use crate::actions::{
    AiAlternates, AltFocusInput, AltKeep, AltNext, AltPrev, AltRemove, CloseAlternates, Redo, Undo,
};
use crate::editor::{Editor, Scope};
use crate::workspace::Workspace;

pub const SIDEBAR_WIDTH: f32 = 240.;

pub struct AltSidebar {
    editor: Entity<Editor>,
    workspace: WeakEntity<Workspace>,
    /// Focus of the option list (its keys: arrows, j/k, Enter, Delete, dd, i/o).
    pub(crate) list_focus: FocusHandle,
    /// The type-to-add row.
    pub(crate) input: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl AltSidebar {
    pub fn new(
        editor: Entity<Editor>,
        workspace: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder("type another way to say it…"));
        let subscriptions = vec![
            cx.observe(&editor, |_, _, cx| cx.notify()),
            cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.add_typed(window, cx);
                }
            }),
        ];
        Self {
            editor,
            workspace,
            list_focus: cx.focus_handle(),
            input,
            _subscriptions: subscriptions,
        }
    }

    /// Enter in the text field: add its text as an option (or version), show it, clear the
    /// field and go back to the list.
    fn add_typed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.input.read(cx).value().to_string();
        if text.trim().is_empty() {
            return;
        }
        self.editor
            .update(cx, |e, cx| e.add_alternate_text(&text, cx));
        self.input
            .update(cx, |input, cx| input.set_value("", window, cx));
        window.focus(&self.list_focus, cx);
    }

    /// `i` / `o`: focus the text field.
    fn focus_input(&mut self, _: &AltFocusInput, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.input.focus_handle(cx);
        window.focus(&input, cx);
    }

    /// Ctrl+Shift+G or the "✦ Ask AI" row: AI alternatives for this target.
    fn ask_ai(&mut self, _: &AiAlternates, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| e.ask_ai_for_session(cx));
    }

    /// Esc: close the sidebar, keeping the shown option (the workspace refocuses the page).
    fn close(&mut self, _: &CloseAlternates, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| e.close_alternates(cx));
    }

    fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| e.undo_steps(1, cx));
    }

    fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| e.redo_steps(1, cx));
    }

    fn next(&mut self, _: &AltNext, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| e.move_alt_highlight(1, cx));
    }

    fn prev(&mut self, _: &AltPrev, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| e.move_alt_highlight(-1, cx));
    }

    /// Enter (or a click): keep the highlighted option and return to the page. The sidebar
    /// stays open.
    fn keep(&mut self, _: &AltKeep, window: &mut Window, cx: &mut Context<Self>) {
        let index = self.highlight(cx);
        self.keep_row(index, window, cx);
    }

    fn keep_row(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| e.keep_alternate(index, cx));
        let main = match self.workspace.upgrade() {
            Some(ws) => ws.read(cx).main_focus(cx),
            None => self.editor.focus_handle(cx),
        };
        window.focus(&main, cx);
    }

    fn remove(&mut self, _: &AltRemove, _: &mut Window, cx: &mut Context<Self>) {
        let index = self.highlight(cx);
        self.editor
            .update(cx, |e, cx| e.remove_alternate(index, cx));
    }

    /// Focus is in the sidebar: its list or its text field.
    pub(crate) fn has_focus(&self, window: &Window, cx: &App) -> bool {
        self.list_focus.is_focused(window) || self.input.focus_handle(cx).is_focused(window)
    }

    fn highlight(&self, cx: &App) -> usize {
        self.editor
            .read(cx)
            .alt_session
            .as_ref()
            .map_or(0, |s| s.highlight)
    }

    fn tab_label(scope: Scope, selection: bool) -> &'static str {
        match scope {
            Scope::Word if selection => "Selection",
            Scope::Word => "Word",
            Scope::Sentence => "Sentence",
            Scope::Paragraph => "Paragraph",
        }
    }
}

impl Render for AltSidebar {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let t = editor.theme;
        let Some(session) = editor.alt_session.clone() else {
            return div().into_any_element();
        };
        let rows = editor.alt_rows();
        let editor_handle = self.editor.clone();
        let tabs = TabBar::new("alt-tabs")
            .underline()
            .small()
            .selected_index(session.scope.index())
            .children(Scope::ALL.map(|scope| {
                Tab::new().child(
                    div()
                        .italic()
                        .text_color(t.accent)
                        .child(Self::tab_label(scope, session.selection)),
                )
            }))
            .on_click(move |ix: &usize, _, cx: &mut App| {
                editor_handle.update(cx, |e, cx| e.set_alt_scope(Scope::ALL[*ix], cx));
            });
        div()
            .id("alt-sidebar")
            .key_context("AltSidebar")
            .on_action(cx.listener(Self::close))
            .on_action(cx.listener(Self::ask_ai))
            .on_action(cx.listener(Self::undo))
            .on_action(cx.listener(Self::redo))
            .w(px(SIDEBAR_WIDTH))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(t.rule)
            .bg(t.panel)
            .text_sm()
            .child(div().px_2().pt_2().child(tabs))
            .child(
                div()
                    .id("alt-list")
                    .key_context("AltList")
                    .track_focus(&self.list_focus)
                    .on_action(cx.listener(Self::next))
                    .on_action(cx.listener(Self::prev))
                    .on_action(cx.listener(Self::keep))
                    .on_action(cx.listener(Self::remove))
                    .on_action(cx.listener(Self::focus_input))
                    .flex_1()
                    .overflow_y_scroll()
                    .p_2()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(rows.into_iter().enumerate().map(|(ix, row)| {
                        let highlighted = ix == session.highlight;
                        div()
                            .id(("alt-row", ix))
                            .flex()
                            .flex_row()
                            .gap_2()
                            .px_2()
                            .py_1()
                            .rounded_md()
                            .cursor_pointer()
                            .when(highlighted, |d| d.bg(t.selection))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.keep_row(ix, window, cx)
                            }))
                            .child(
                                div()
                                    .w(px(10.))
                                    .flex_none()
                                    .text_color(if row.shown { t.accent } else { t.text_muted })
                                    .child(if row.shown { "●" } else { "·" }),
                            )
                            .child(div().flex_1().min_w(px(0.)).line_clamp(3).child(row.text))
                            .when(row.ai, |d| {
                                d.child(div().flex_none().text_color(t.ai_accent).child("✦"))
                            })
                    })),
            )
            .child(div().px_2().child(Input::new(&self.input).small()))
            .child(
                div()
                    .id("alt-ask-ai")
                    .m_2()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(t.ai_accent)
                    .hover(|d| d.bg(t.selection))
                    .child("✦ Ask AI")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.ask_ai(&AiAlternates, window, cx)
                    })),
            )
            .into_any_element()
    }
}

impl Workspace {
    /// The editor opened the sidebar: create its view on first use and focus its list.
    pub(crate) fn alternates_opened(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let editor = self.editor.clone();
        let workspace = cx.entity().downgrade();
        let sidebar = self
            .alt_sidebar
            .get_or_insert_with(|| cx.new(|cx| AltSidebar::new(editor, workspace, window, cx)))
            .clone();
        let focus = sidebar.read(cx).list_focus.clone();
        let handles = vec![focus.clone(), sidebar.read(cx).input.focus_handle(cx)];
        self.editor.update(cx, |e, _| e.sidebar_focus = handles);
        window.focus(&focus, cx);
        cx.notify();
    }

    /// The sidebar closed (Esc, or its target disappeared): if focus was in it, the page gets
    /// it back.
    pub(crate) fn alternates_closed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let had_focus = self
            .alt_sidebar
            .as_ref()
            .is_some_and(|s| s.read(cx).has_focus(window, cx));
        if had_focus {
            window.focus(&self.main_focus(cx), cx);
        }
        cx.notify();
    }
}
