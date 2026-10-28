//! The root view: the writing column, the overflow panel, the status bar and file commands.

use std::path::{Path, PathBuf};

use gpui::{App, ClickEvent, Context, Entity, Focusable, Window, div, prelude::*, px};
use tulis_core::alternates::AltLevel;
use tulis_core::config::Config;
use tulis_core::storage;
use tulis_core::{Document, OverflowId};

use crate::actions::ToggleOverflowPanel;
use crate::editor::Editor;
use crate::theme;

pub struct Workspace {
    pub editor: Entity<Editor>,
    pub path: Option<PathBuf>,
    pub(crate) show_overflow: bool,
    pub(crate) keep_in_overflow: bool,
    pub(crate) banner: Option<String>,
}

pub(crate) fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

impl Workspace {
    pub fn new(
        config: Config,
        warning: Option<String>,
        path: Option<PathBuf>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let editor = cx.new(|cx| Editor::new(config, Document::new(), cx));
        cx.observe(&editor, |_, _, cx| cx.notify()).detach();
        let mut workspace = Self {
            editor,
            path: None,
            show_overflow: false,
            keep_in_overflow: false,
            banner: None,
        };
        if let Some(path) = path {
            workspace.open_path(path, cx);
        }
        if let Some(warning) = warning {
            workspace
                .editor
                .update(cx, |e, cx| e.set_notice(warning, cx));
        }
        workspace
    }

    pub fn focus_editor(&self, window: &mut Window, cx: &mut App) {
        window.focus(&self.editor.focus_handle(cx));
    }

    pub(crate) fn open_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !path.exists() {
            self.path = Some(path);
            self.editor.update(cx, |e, cx| {
                e.load(Document::new(), cx);
                e.set_notice("New file", cx);
            });
            return;
        }
        match storage::open(&path) {
            Ok(opened) => {
                self.path = Some(path);
                self.banner = None;
                self.editor.update(cx, |e, cx| {
                    e.load(opened.document, cx);
                    if !opened.notices.is_empty() {
                        e.set_notice(opened.notices.join(" · "), cx);
                    }
                });
            }
            Err(e) => {
                self.banner = Some(format!("Could not open {}: {e}", path.display()));
                cx.notify();
            }
        }
    }

    fn render_status(&self, cx: &App) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let name = self
            .path
            .as_deref()
            .map(file_name)
            .unwrap_or_else(|| "Untitled".into());
        let dirty = if editor.dirty { " •" } else { "" };
        let mut parts = vec![
            format!("{name}{dirty}"),
            format!("{} words", editor.doc.word_count()),
        ];
        if let Some(info) = editor.doc.alt_info(editor.cursor()) {
            let label = match info.level {
                AltLevel::Word => "alt",
                AltLevel::Paragraph => "ver",
            };
            parts.push(format!("{label} {}/{}", info.index + 1, info.count));
        }
        if editor.ai_task.is_some() {
            parts.push("AI…".into());
        }
        div()
            .flex()
            .flex_row()
            .justify_between()
            .px_4()
            .py_1()
            .border_t_1()
            .border_color(theme::rule())
            .text_xs()
            .text_color(theme::muted())
            .child(parts.join("  ·  "))
            .child(editor.notice.clone().unwrap_or_default())
    }
}

fn preview(text: &str) -> String {
    const MAX: usize = 240;
    if text.chars().count() <= MAX {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(MAX).collect::<String>())
    }
}

impl Workspace {
    fn toggle_overflow(&mut self, _: &ToggleOverflowPanel, _: &mut Window, cx: &mut Context<Self>) {
        self.show_overflow = !self.show_overflow;
        cx.notify();
    }

    fn insert_overflow(&mut self, id: OverflowId, window: &mut Window, cx: &mut Context<Self>) {
        let remove = !self.keep_in_overflow;
        self.editor
            .update(cx, |e, cx| e.insert_overflow(id, remove, cx));
        self.focus_editor(window, cx);
    }

    fn render_overflow(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let items = self.editor.read(cx).doc.overflow.clone();
        let keep = self.keep_in_overflow;
        div()
            .w(px(280.))
            .h_full()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(theme::rule())
            .bg(theme::panel())
            .child(
                div()
                    .px_3()
                    .py_2()
                    .flex()
                    .justify_between()
                    .text_sm()
                    .child("Overflow")
                    .child(
                        div()
                            .id("keep-toggle")
                            .cursor_pointer()
                            .text_xs()
                            .text_color(theme::muted())
                            .child(if keep {
                                "☑ keep after insert"
                            } else {
                                "☐ keep after insert"
                            })
                            .on_click(cx.listener(|ws, _: &ClickEvent, _, cx| {
                                ws.keep_in_overflow = !ws.keep_in_overflow;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .id("overflow-list")
                    .flex_1()
                    .overflow_y_scroll()
                    .px_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .when(items.is_empty(), |list| {
                        list.child(
                            div()
                                .text_xs()
                                .text_color(theme::muted())
                                .child("Ctrl+Shift+O moves the selection or paragraph here."),
                        )
                    })
                    .children(items.into_iter().rev().map(|item| {
                        let id = item.id;
                        div()
                            .id(("overflow", id.0))
                            .p_2()
                            .rounded_md()
                            .bg(theme::paper())
                            .cursor_pointer()
                            .text_sm()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .on_click(cx.listener(move |ws, _: &ClickEvent, window, cx| {
                                ws.insert_overflow(id, window, cx)
                            }))
                            .child(div().flex_1().child(preview(&item.text)))
                            .child(
                                div()
                                    .id(("overflow-remove", id.0))
                                    .text_color(theme::muted())
                                    .child("×")
                                    .on_click(cx.listener(move |ws, _: &ClickEvent, _, cx| {
                                        cx.stop_propagation();
                                        ws.editor.update(cx, |e, cx| e.remove_overflow(id, cx));
                                    })),
                            )
                    })),
            )
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scroll = self.editor.read(cx).scroll.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::paper())
            .text_color(theme::ink())
            .on_action(cx.listener(Self::toggle_overflow))
            .children(
                self.banner
                    .clone()
                    .map(|b| div().px_4().py_2().bg(theme::banner()).text_sm().child(b)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .flex_row()
                    .child(
                        div()
                            .id("editor-scroll")
                            .flex_1()
                            .h_full()
                            .overflow_y_scroll()
                            .track_scroll(&scroll)
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .justify_center()
                                    .py(px(56.))
                                    .px(px(32.))
                                    .child(
                                        div().w_full().max_w(px(680.)).child(self.editor.clone()),
                                    ),
                            ),
                    )
                    .when(self.show_overflow, |row| {
                        row.child(self.render_overflow(cx))
                    }),
            )
            .child(self.render_status(cx))
    }
}
