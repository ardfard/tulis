//! The root view: the writing column, the overflow panel, the status bar and file commands.

use std::path::{Path, PathBuf};

use gpui::{
    App, ClickEvent, Context, Entity, Focusable, PathPromptOptions, PromptLevel, Window, div,
    prelude::*, px,
};
use tulis_core::alternates::AltLevel;
use tulis_core::config::Config;
use tulis_core::storage;
use tulis_core::vim::Mode;
use tulis_core::{Document, OverflowId};

use crate::actions::{New, Open, Quit, Save, SaveAs, ToggleOverflowPanel};
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
        window: &mut Window,
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
        let handle = cx.entity();
        window.on_window_should_close(cx, move |window, cx| {
            handle.update(cx, |ws, cx| ws.request_close(window, cx))
        });
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
        if let Some(mode) = editor.vim_mode() {
            parts.push(
                match mode {
                    Mode::Normal => "NORMAL",
                    Mode::Insert => "INSERT",
                    Mode::Visual => "VISUAL",
                    Mode::VisualLine => "V-LINE",
                }
                .into(),
            );
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
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::save_as))
            .on_action(cx.listener(Self::open))
            .on_action(cx.listener(Self::new_file))
            .on_action(cx.listener(Self::quit))
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

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

impl Workspace {
    /// Save to the current path. Returns false when there is no path yet or saving failed.
    pub(crate) fn save_now(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(path) = self.path.clone() else {
            return false;
        };
        let result = storage::save(&self.editor.read(cx).doc, &path);
        match result {
            Ok(()) => {
                self.banner = None;
                self.editor.update(cx, |e, cx| {
                    e.dirty = false;
                    e.set_notice(format!("Saved {}", file_name(&path)), cx);
                });
                true
            }
            Err(e) => {
                self.banner = Some(format!("Could not save {}: {e}", path.display()));
                cx.notify();
                false
            }
        }
    }

    fn save(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
        if self.path.is_none() {
            self.save_as(&SaveAs, window, cx);
        } else {
            self.save_now(cx);
        }
    }

    fn save_as(&mut self, _: &SaveAs, window: &mut Window, cx: &mut Context<Self>) {
        let dir = self
            .path
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .or_else(home_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        let suggested = self
            .path
            .as_deref()
            .map(file_name)
            .unwrap_or_else(|| "untitled.md".into());
        let answer = cx.prompt_for_new_path(&dir, Some(&suggested));
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(mut path))) = answer.await else {
                return;
            };
            if path.extension().is_none() {
                path.set_extension("md");
            }
            this.update(cx, |ws, cx| {
                ws.path = Some(path);
                ws.save_now(cx);
            })
            .ok();
        })
        .detach();
    }

    fn open(&mut self, _: &Open, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_discard(window, cx, |_, window, cx| {
            let answer = cx.prompt_for_paths(PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: None,
            });
            cx.spawn_in(window, async move |this, cx| {
                let Ok(Ok(Some(paths))) = answer.await else {
                    return;
                };
                let Some(path) = paths.into_iter().next() else {
                    return;
                };
                this.update(cx, |ws, cx| ws.open_path(path, cx)).ok();
            })
            .detach();
        });
    }

    fn new_file(&mut self, _: &New, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_discard(window, cx, |ws, _, cx| {
            ws.path = None;
            ws.banner = None;
            ws.editor.update(cx, |e, cx| e.load(Document::new(), cx));
        });
    }

    fn quit(&mut self, _: &Quit, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_discard(window, cx, |_, _, cx| cx.quit());
    }

    /// Called when the window is about to close. Returns whether it may close now.
    fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.editor.read(cx).dirty {
            return true;
        }
        self.confirm_discard(window, cx, |_, window, _| window.remove_window());
        false
    }

    /// Run `then` now if there are no unsaved changes; otherwise ask Save / Don't Save / Cancel.
    /// Choosing Save on an untitled document opens Save As and does not continue.
    fn confirm_discard(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        then: impl FnOnce(&mut Self, &mut Window, &mut Context<Self>) + 'static,
    ) {
        if !self.editor.read(cx).dirty {
            then(self, window, cx);
            return;
        }
        let answer = window.prompt(
            PromptLevel::Warning,
            "Save changes to this document?",
            None,
            &["Save", "Don't Save", "Cancel"],
            cx,
        );
        cx.spawn_in(window, async move |this, cx| {
            let Ok(choice) = answer.await else { return };
            this.update_in(cx, |ws, window, cx| match choice {
                0 if ws.path.is_none() => ws.save_as(&SaveAs, window, cx),
                0 => {
                    if ws.save_now(cx) {
                        then(ws, window, cx);
                    }
                }
                1 => then(ws, window, cx),
                _ => {}
            })
            .ok();
        })
        .detach();
    }
}

#[cfg(test)]
mod tests {
    use gpui::{EntityInputHandler, TestAppContext};
    use tulis_core::config::Config;
    use tulis_core::{DocPos, Selection};

    use super::Workspace;

    #[gpui::test]
    fn opens_command_line_path_and_saves(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("essay.md");
        std::fs::write(&path, "Hello.\n").unwrap();
        let open_path = path.clone();
        let (ws, cx) = cx.add_window_view(move |window, cx| {
            Workspace::new(Config::default(), None, Some(open_path), window, cx)
        });
        let text = ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.paragraphs[0].text());
        assert_eq!(text, "Hello.");
        ws.update_in(cx, |ws, window, cx| {
            ws.editor.update(cx, |e, cx| {
                e.selection = Selection::caret(DocPos::new(0, 6));
                e.replace_text_in_range(None, " Bye.", window, cx);
            });
            assert!(ws.save_now(cx));
            assert!(!ws.editor.read(cx).dirty);
        });
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "Hello. Bye.\n");
        assert!(tulis_core::storage::sidecar_path(&path).exists());
    }

    #[gpui::test]
    fn missing_path_starts_a_new_file_there(cx: &mut TestAppContext) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("new.md");
        let open_path = path.clone();
        let (ws, cx) = cx.add_window_view(move |window, cx| {
            Workspace::new(Config::default(), None, Some(open_path), window, cx)
        });
        assert_eq!(
            ws.read_with(cx, |ws, _| ws.path.clone()),
            Some(path.clone())
        );
        ws.update_in(cx, |ws, _, cx| assert!(ws.save_now(cx)));
        assert!(path.exists());
    }

    #[gpui::test]
    fn save_without_path_reports_false(cx: &mut TestAppContext) {
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        ws.update_in(cx, |ws, _, cx| assert!(!ws.save_now(cx)));
    }
}
