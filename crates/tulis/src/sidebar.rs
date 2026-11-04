//! The alternates sidebar: Word/Selection, Sentence and Paragraph tabs over the list of options
//! the editor's `AltSession` describes.

use gpui::{
    App, ClickEvent, Context, Entity, FocusHandle, Focusable, FontWeight, IntoElement,
    ParentElement, Render, Styled, Subscription, WeakEntity, Window, div, prelude::*, px,
};
use gpui_component::Sizable;
use gpui_component::input::{Input, InputEvent, InputState};

use crate::actions::{
    AiAlternates, AltFocusInput, AltKeep, AltNext, AltPrev, AltRemove, CloseAlternates, Redo, Undo,
};
use crate::commands;
use crate::editor::{Editor, Scope};
use crate::pills;
use crate::workspace::Workspace;

pub const SIDEBAR_WIDTH: f32 = 240.;
/// Shown under the sidebar's text field so the list keys are discoverable: (command ids, the
/// index of the key to show, what it does), in two rows.
const KEY_HINT: [&[(&[(&str, usize)], &str)]; 2] = [
    &[
        (&[("alt_next", 1), ("alt_prev", 1)], "preview"),
        (&[("alt_keep", 0)], "keep"),
    ],
    &[
        (&[("alt_remove", 0)], "remove"),
        (&[("close_alternates", 0)], "close"),
    ],
];

/// The key hint rows' pills and labels, from the command table.
fn key_hint() -> Vec<Vec<(Vec<String>, &'static str)>> {
    KEY_HINT
        .iter()
        .map(|row| {
            row.iter()
                .map(|(keys, label)| {
                    let pills = keys
                        .iter()
                        .filter_map(|(id, ix)| commands::command_pills(id).get(*ix).cloned())
                        .collect();
                    (pills, *label)
                })
                .collect()
        })
        .collect()
}

pub struct AltSidebar {
    editor: Entity<Editor>,
    workspace: WeakEntity<Workspace>,
    /// Focus of the option list (its keys: arrows, j/k, Enter, Delete, dd, i/o).
    pub(crate) list_focus: FocusHandle,
    /// The type-to-add row.
    pub(crate) input: Entity<InputState>,
    /// The editor's `doc_gen` when the field was last cleared for a new document.
    seen_doc_gen: u64,
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
            cx.observe_in(&editor, window, |this, editor, window, cx| {
                // A different document: the half-typed alternative belongs to the old one.
                let doc_gen = editor.read(cx).doc_gen;
                if doc_gen != this.seen_doc_gen {
                    this.seen_doc_gen = doc_gen;
                    this.input
                        .update(cx, |input, cx| input.set_value("", window, cx));
                }
                cx.notify();
            }),
            cx.subscribe_in(&input, window, |this, _, event: &InputEvent, window, cx| {
                if let InputEvent::PressEnter { .. } = event {
                    this.add_typed(window, cx);
                }
            }),
        ];
        let seen_doc_gen = editor.read(cx).doc_gen;
        Self {
            editor,
            workspace,
            seen_doc_gen,
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
        let font = editor.font_family.clone();
        let tabs = div().flex().flex_row().gap(px(16.)).pb(px(6.)).children(
            Scope::ALL.into_iter().enumerate().map(|(ix, scope)| {
                let selected = scope == session.scope;
                div()
                    .id(("alt-tab", ix))
                    .cursor_pointer()
                    .font_family(crate::theme::SCRIPT_FONT)
                    .text_size(px(crate::theme::SCRIPT_SIZE))
                    .text_color(if selected { t.accent } else { t.text_muted })
                    .when(selected, |d| d.font_weight(FontWeight::BOLD))
                    .when(!selected, |d| d.hover(|d| d.text_color(t.text)))
                    .child(Self::tab_label(scope, session.selection))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.editor.update(cx, |e, cx| e.set_alt_scope(scope, cx));
                    }))
            }),
        );
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
            .px(px(14.))
            .py(px(18.))
            .gap(px(12.))
            .text_size(px(13.))
            .child(tabs)
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
                    .flex_initial()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .children(rows.into_iter().enumerate().map(|(ix, row)| {
                        let highlighted = ix == session.highlight;
                        div()
                            .id(("alt-row", ix))
                            .flex()
                            .flex_row()
                            .gap(px(10.))
                            .px(px(10.))
                            .py(px(8.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .when(highlighted, |d| d.bg(t.selection))
                            .when(!highlighted, |d| d.hover(|d| d.bg(t.rule)))
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                this.keep_row(ix, window, cx)
                            }))
                            // The same dot language as the paragraph version dots.
                            .child(
                                div()
                                    .w(px(12.))
                                    .flex_none()
                                    .text_size(px(13.))
                                    .line_height(px(19.5))
                                    .text_color(if row.shown { t.accent } else { t.text_muted })
                                    .child(if row.shown { "●" } else { "·" }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w(px(0.))
                                    .font_family(font.clone())
                                    .text_size(px(13.))
                                    .line_height(px(19.5))
                                    .text_color(if highlighted { t.text } else { t.text_muted })
                                    .line_clamp(3)
                                    .child(row.text),
                            )
                            .when(row.ai, |d| {
                                d.child(
                                    div()
                                        .flex_none()
                                        .text_size(px(12.))
                                        .line_height(px(19.5))
                                        .text_color(t.ai_accent)
                                        .child("✦"),
                                )
                            })
                    })),
            )
            .child(Input::new(&self.input).small())
            .child(
                div()
                    .id("alt-ask-ai")
                    .px(px(10.))
                    .py(px(8.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .text_color(t.ai_accent)
                    .hover(|d| d.bg(t.selection))
                    .child("✦ Ask AI")
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.ask_ai(&AiAlternates, window, cx)
                    })),
            )
            .child(
                div()
                    .debug_selector(|| "alt-key-hint".into())
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .children(key_hint().iter().map(|row| {
                        pills::hint_row(
                            row.iter().map(|(keys, label)| (keys.as_slice(), *label)),
                            pills::PILL_SIZE,
                            &t,
                        )
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
