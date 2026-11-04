//! Key pills: keys drawn as small rounded labels, and hint rows built from them. Used by the
//! sidebar, context menu, Overflow panel, Lab bar and cheat sheet.

use gpui::{Div, IntoElement, ParentElement, SharedString, Styled, div, px};

use crate::theme::Theme;

/// Text size of pills in panels; the Lab bar and the cheat sheet use larger ones.
pub(crate) const PILL_SIZE: f32 = 11.;

/// One key (or chord), e.g. `Ctrl Shift A`, at text size `size`.
pub(crate) fn pill(label: impl Into<SharedString>, size: f32, t: &Theme) -> Div {
    div()
        .flex_none()
        .px(px(5.))
        .py(px(1.))
        .rounded(px(4.))
        .bg(t.rule)
        .text_size(px(size))
        .line_height(px(size + 4.))
        .text_color(t.text)
        .child(label.into())
}

/// Hints such as `[j] [k] preview   [↵] keep`: each group's pills, then what they do.
pub(crate) fn hint_row<'a>(
    hints: impl IntoIterator<Item = (&'a [String], &'a str)>,
    size: f32,
    t: &Theme,
) -> Div {
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_x(px(16.))
        .gap_y(px(6.))
        .text_size(px(size))
        .children(hints.into_iter().map(|(keys, label)| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.))
                .children(keys.iter().map(|key| pill(key.clone(), size, t)))
                .child(div().text_color(t.text_muted).child(label.to_string()))
        }))
}

/// Vim keys after a pill, in the page font: `ga`.
pub(crate) fn vim_keys(keys: &'static str, font: SharedString, t: &Theme) -> impl IntoElement {
    div()
        .font_family(font)
        .text_size(px(12.))
        .text_color(t.text_muted)
        .child(keys)
}
