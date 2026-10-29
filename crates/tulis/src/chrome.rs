//! The workspace's chrome: header, footer, toasts, and the share commands.

use std::time::Instant;

use gpui::{
    Animation, AnimationExt, App, ClickEvent, Context, IntoElement, ParentElement, Styled, Window,
    div, prelude::*, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{IconName, Selectable, Sizable};
use tulis_core::alternates::AltLevel;
use tulis_core::stats;
use tulis_core::vim::Mode;

use crate::actions::{ShowCheatSheet, ToggleMarkers, TogglePreview};
use crate::toast::{TOAST_LIFETIME, toast_opacity};
use crate::workspace::{Workspace, file_name};

pub const HEADER_HEIGHT: f32 = 40.;

pub(crate) fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
        Mode::Visual => "VISUAL",
        Mode::VisualLine => "V-LINE",
    }
}

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

    /// Called when the editor reports typing: track the burst and schedule the repaint that
    /// dims (or undims) the header.
    pub(crate) fn typed(&mut self, cx: &mut Context<Self>) {
        self.header_dim.on_key(Instant::now());
        self.schedule_dim_repaint(cx);
    }

    fn schedule_dim_repaint(&mut self, cx: &mut Context<Self>) {
        let Some(delay) = self.header_dim.next_change(Instant::now()) else {
            self.dim_timer = None;
            return;
        };
        self.dim_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            this.update(cx, |ws, cx| {
                cx.notify();
                ws.schedule_dim_repaint(cx);
            })
            .ok();
        }));
    }

    /// The left part of the header: counts, then the vim, alternate and AI badges.
    pub(crate) fn header_status(&self, cx: &App) -> (String, Vec<String>) {
        let editor = self.editor.read(cx);
        let range = editor.selection.range();
        let counts = if range.start == range.end {
            stats::document_counts(&editor.doc).label(false)
        } else {
            stats::selection_counts(&editor.doc, range).label(true)
        };
        let mut badges = Vec::new();
        if let Some(mode) = editor.vim_mode() {
            badges.push(mode_label(mode).to_string());
        }
        if let Some(info) = editor.doc.alt_info(editor.cursor()) {
            let label = match info.level {
                AltLevel::Word => "alt",
                AltLevel::Paragraph => "ver",
            };
            badges.push(format!("{label} {}/{}", info.index + 1, info.count));
        }
        if editor.ai_task.is_some() {
            badges.push("AI…".into());
        }
        (counts, badges)
    }

    pub(crate) fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let t = editor.theme;
        let show_markers = editor.show_markers;
        let previewing = self.preview.is_some();
        let (counts, badges) = self.header_status(cx);
        let dimmed = self.header_dim.is_dimmed(Instant::now());
        let header = div()
            .id("header")
            .h(px(HEADER_HEIGHT))
            .flex_none()
            .px_4()
            .flex()
            .flex_row()
            .items_center()
            .text_xs()
            .text_color(t.text_muted)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(counts)
                    .children(badges.into_iter().map(|badge| {
                        div()
                            .px_1()
                            .rounded_sm()
                            .border_1()
                            .border_color(t.rule)
                            .text_color(t.accent)
                            .child(badge)
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("toggle-markers")
                            .ghost()
                            .xsmall()
                            .icon(if show_markers {
                                IconName::Eye
                            } else {
                                IconName::EyeOff
                            })
                            .selected(show_markers)
                            .tooltip("Markers (Ctrl+.)")
                            .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                                ws.toggle_markers(&ToggleMarkers, window, cx)
                            })),
                    )
                    .child(
                        Button::new("toggle-preview")
                            .ghost()
                            .xsmall()
                            .icon(IconName::FileText)
                            .selected(previewing)
                            .tooltip("Markdown preview (Ctrl+Shift+M)")
                            .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                                ws.toggle_preview(&TogglePreview, window, cx)
                            })),
                    ),
            )
            .child(
                div().flex_1().flex().flex_row().justify_end().child(
                    Button::new("cheat-sheet")
                        .ghost()
                        .xsmall()
                        .icon(gpui_kit_assets::IconName::Keyboard)
                        .tooltip("Cheat sheet (Ctrl+?)")
                        .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                            ws.show_cheat_sheet(&ShowCheatSheet, window, cx)
                        })),
                ),
            );
        if dimmed {
            header
                .with_animation(
                    "header-dim",
                    Animation::new(std::time::Duration::from_millis(400)),
                    |el, progress| el.opacity(1. - progress),
                )
                .into_any_element()
        } else {
            header.into_any_element()
        }
    }

    /// Keep the window title in step with the file name and unsaved state.
    pub(crate) fn sync_window_title(&mut self, window: &mut Window, cx: &App) {
        let name = self
            .path
            .as_deref()
            .map(file_name)
            .unwrap_or_else(|| "Untitled".into());
        let dirty = if self.editor.read(cx).dirty {
            " •"
        } else {
            ""
        };
        let title = format!("{name}{dirty} — Tulis");
        if title != self.window_title {
            window.set_window_title(&title);
            self.window_title = title;
        }
    }
}
