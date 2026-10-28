//! The root view: the writing column, the overflow panel, the status bar and file commands.

use std::path::{Path, PathBuf};

use gpui::{App, Context, Entity, Focusable, Window, div, prelude::*, px};
use tulis_core::Document;
use tulis_core::config::Config;
use tulis_core::storage;

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
        let parts = vec![
            format!("{name}{dirty}"),
            format!("{} words", editor.doc.word_count()),
        ];
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

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let scroll = self.editor.read(cx).scroll.clone();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(theme::paper())
            .text_color(theme::ink())
            .children(
                self.banner
                    .clone()
                    .map(|b| div().px_4().py_2().bg(theme::banner()).text_sm().child(b)),
            )
            .child(
                div().flex_1().min_h(px(0.)).flex().flex_row().child(
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
                                .child(div().w_full().max_w(px(680.)).child(self.editor.clone())),
                        ),
                ),
            )
            .child(self.render_status(cx))
    }
}
