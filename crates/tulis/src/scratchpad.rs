//! The Overflow scratchpad: a right panel with one multi-line text field bound to
//! `doc.overflow`.

use gpui::{
    App, Context, Entity, Focusable, IntoElement, ParentElement, Render, Styled, Subscription,
    Window, div, prelude::*, px, relative,
};
use gpui_component::input::{InputEvent, Textarea, TextareaState};

use crate::actions::{FocusOverflow, FocusPage, OverflowCopy, OverflowUse};
use crate::editor::Editor;
use crate::workspace::Workspace;

pub const SCRATCHPAD_WIDTH: f32 = 260.;

pub struct Scratchpad {
    editor: Entity<Editor>,
    /// The text field. Its value mirrors `doc.overflow`; it keeps its own undo history.
    pub(crate) input: Entity<TextareaState>,
    _subscriptions: Vec<Subscription>,
}

impl Scratchpad {
    pub fn new(editor: Entity<Editor>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Ctrl+Shift+X stashes the selection or paragraph here.")
        });
        let text = editor.read(cx).doc.overflow.clone();
        input.update(cx, |input, cx| input.set_value(text, window, cx));
        let subscriptions = vec![
            // Page commands (stash, undo, open) change `doc.overflow`: show the new text.
            cx.observe_in(&editor, window, |this, _, window, cx| this.pull(window, cx)),
            cx.subscribe_in(&input, window, |this, _, event: &InputEvent, _, cx| {
                if let InputEvent::Change = event {
                    this.push(cx);
                }
            }),
        ];
        Self {
            editor,
            input,
            _subscriptions: subscriptions,
        }
    }

    /// The field changed: store its text in the document.
    fn push(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).value();
        self.editor.update(cx, |e, cx| e.set_overflow(&text, cx));
    }

    /// The document's overflow changed elsewhere: show it. `set_value` emits no `Change`, and
    /// starts the field's undo history afresh.
    fn pull(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let text = self.editor.read(cx).doc.overflow.clone();
        if self.input.read(cx).value().as_ref() != text {
            self.input
                .update(cx, |input, cx| input.set_value(text, window, cx));
        }
        cx.notify();
    }

    /// Ctrl+Enter: move the field's selection, or the block at its cursor, to the page cursor,
    /// and focus the page.
    fn use_block(&mut self, _: &OverflowUse, window: &mut Window, cx: &mut Context<Self>) {
        let selected = self.input.read(cx).selected_range();
        let used = self
            .editor
            .update(cx, |e, cx| e.use_overflow_selection(selected, false, cx));
        if used {
            let page = self.editor.focus_handle(cx);
            window.focus(&page, cx);
        }
    }

    /// Ctrl+Shift+Enter: the same, but the text stays here and so does the focus.
    fn copy_block(&mut self, _: &OverflowCopy, _: &mut Window, cx: &mut Context<Self>) {
        let selected = self.input.read(cx).selected_range();
        self.editor
            .update(cx, |e, cx| e.use_overflow_selection(selected, true, cx));
    }

    /// Esc: back to the page (the panel stays open).
    fn focus_page(&mut self, _: &FocusPage, window: &mut Window, cx: &mut Context<Self>) {
        let page = self.editor.focus_handle(cx);
        window.focus(&page, cx);
    }

    pub(crate) fn has_focus(&self, window: &Window, cx: &App) -> bool {
        self.input.focus_handle(cx).is_focused(window)
    }
}

impl Render for Scratchpad {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.editor.read(cx).theme;
        div()
            .id("scratchpad")
            .key_context("Scratchpad")
            .on_action(cx.listener(Self::use_block))
            .on_action(cx.listener(Self::copy_block))
            .on_action(cx.listener(Self::focus_page))
            .w(px(SCRATCHPAD_WIDTH))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(t.rule)
            .bg(t.panel)
            .text_sm()
            .child(
                div()
                    .px_3()
                    .pt_2()
                    .pb_1()
                    .italic()
                    .text_color(t.accent)
                    .child("Overflow"),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .px_2()
                    .child(Textarea::new(&self.input).appearance(false).h(relative(1.))),
            )
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child("ctrl+↵ or drag into the page to use"),
            )
    }
}

impl Workspace {
    /// The scratchpad's view, created on first use.
    pub(crate) fn ensure_scratchpad(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<Scratchpad> {
        let editor = self.editor.clone();
        self.scratchpad
            .get_or_insert_with(|| cx.new(|cx| Scratchpad::new(editor, window, cx)))
            .clone()
    }

    /// Ctrl+Shift+\, vim `gX`: focus the scratchpad (opening the panel), or the page when the
    /// scratchpad has focus.
    pub(crate) fn toggle_overflow_focus(
        &mut self,
        _: &FocusOverflow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.scratchpad_focused(window, cx) {
            self.focus_editor(window, cx);
        } else {
            self.show_overflow = true;
            let input = self.ensure_scratchpad(window, cx).read(cx).input.clone();
            window.focus(&input.focus_handle(cx), cx);
        }
        cx.notify();
    }

    pub(crate) fn scratchpad_focused(&self, window: &Window, cx: &App) -> bool {
        self.scratchpad
            .as_ref()
            .is_some_and(|pad| pad.read(cx).has_focus(window, cx))
    }
}
