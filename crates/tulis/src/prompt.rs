//! The in-window prompt (unsaved changes, replace a file): a themed card over a dimmed window,
//! used for every `window.prompt`. The first answer is the primary button, drawn last (on the
//! right); Enter picks it and Esc picks the last answer.

use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, FontWeight, Global, IntoElement,
    KeyDownEvent, ParentElement, PromptButton, PromptHandle, PromptLevel, PromptResponse, Render,
    RenderablePromptHandle, Styled, Window, div, prelude::*, px,
};

use crate::theme::{Theme, ThemeId};

/// The theme the window is painted with, for views (like this prompt) built outside it.
#[derive(Clone, Copy)]
pub(crate) struct ActiveTheme(pub Theme);

impl Global for ActiveTheme {}

pub(crate) fn build(
    _: PromptLevel,
    message: &str,
    detail: Option<&str>,
    answers: &[PromptButton],
    handle: PromptHandle,
    window: &mut Window,
    cx: &mut App,
) -> RenderablePromptHandle {
    let theme = cx
        .try_global::<ActiveTheme>()
        .map_or_else(|| ThemeId::Midnight.theme(), |t| t.0);
    let view = cx.new(|cx| Prompt {
        message: message.to_string(),
        detail: detail.map(str::to_string),
        answers: answers.iter().map(|a| a.label().to_string()).collect(),
        theme,
        focus: cx.focus_handle(),
    });
    handle.with_view(view, window, cx)
}

struct Prompt {
    message: String,
    detail: Option<String>,
    answers: Vec<String>,
    theme: Theme,
    focus: FocusHandle,
}

impl EventEmitter<PromptResponse> for Prompt {}

impl Focusable for Prompt {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Prompt {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let last = self.answers.len().saturating_sub(1);
        let buttons = self.answers.iter().enumerate().rev().map(|(ix, label)| {
            div()
                .id(ix)
                .px(px(14.))
                .py(px(7.))
                .rounded(px(8.))
                .cursor_pointer()
                .map(|d| {
                    if ix == 0 {
                        d.bg(t.accent)
                            .text_color(t.background)
                            .font_weight(FontWeight::SEMIBOLD)
                            .hover(|d| d.opacity(0.9))
                    } else {
                        d.border_1()
                            .border_color(t.rule)
                            .text_color(t.text)
                            .hover(|d| d.bg(t.rule))
                    }
                })
                .child(label.clone())
                .on_click(cx.listener(move |_, _, _, cx| {
                    cx.emit(PromptResponse(ix));
                    cx.stop_propagation();
                }))
        });
        div()
            .size_full()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(move |_, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "enter" => cx.emit(PromptResponse(0)),
                    "escape" => cx.emit(PromptResponse(last)),
                    _ => return,
                }
                cx.stop_propagation();
            }))
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::hsla(0., 0., 0., if t.dark { 0.55 } else { 0.25 }))
            .child(
                div()
                    .w(px(380.))
                    .p(px(22.))
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .rounded(px(14.))
                    .border_1()
                    .border_color(t.rule)
                    .bg(t.panel)
                    .text_size(px(13.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.text)
                                    .child(self.message.clone()),
                            )
                            .children(
                                self.detail
                                    .clone()
                                    .map(|d| div().text_color(t.text_muted).child(d)),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .justify_end()
                            .gap(px(8.))
                            .children(buttons),
                    ),
            )
    }
}
