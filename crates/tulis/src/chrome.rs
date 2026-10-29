//! The workspace's chrome: header, footer, toasts, and the share commands.

use gpui::{Animation, AnimationExt, App, IntoElement, ParentElement, Styled, div, prelude::*, px};

use crate::toast::{TOAST_LIFETIME, toast_opacity};
use crate::workspace::Workspace;

/// Distance of the toast stack from the window's bottom edge (above the footer).
const TOAST_BOTTOM: f32 = 56.;

impl Workspace {
    /// Toasts, centered above the bottom edge, newest last.
    pub(crate) fn render_toasts(&self, cx: &App) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let t = editor.theme;
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(TOAST_BOTTOM))
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .children(editor.toasts.iter().map(|toast| {
                div()
                    .id(("toast", toast.id))
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(t.panel)
                    .border_1()
                    .border_color(t.rule)
                    .text_xs()
                    .text_color(t.text)
                    .child(toast.text.clone())
                    .with_animation(
                        ("toast-fade", toast.id),
                        Animation::new(TOAST_LIFETIME),
                        |el, progress| el.opacity(toast_opacity(progress)),
                    )
            }))
    }
}
