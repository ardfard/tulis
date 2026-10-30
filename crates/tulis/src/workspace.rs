//! The root view: header, page, overflow panel, footer, toasts and file commands.

use std::path::{Path, PathBuf};

use gpui::{
    AnyElement, App, Context, Entity, FocusHandle, Focusable, Hsla, MouseMoveEvent,
    PathPromptOptions, PromptLevel, SharedString, Subscription, Task, Window, div,
    linear_color_stop, linear_gradient, prelude::*, px,
};
use gpui_component::menu::ContextMenuExt;
use gpui_component::text::TextView;
use tulis_core::Document;
use tulis_core::config::Config;
use tulis_core::storage;

use crate::actions::{
    AiAlternates, CycleTheme, New, Open, OpenAlternates, OpenLab, Quit, Save, SaveAs,
    SendToOverflow, ToggleGhost, ToggleMarkers, ToggleOverflowPanel, TogglePreview,
};
use crate::editor::{Editor, EditorEvent, GUTTER};
use crate::header_dim::HeaderDim;
use crate::theme;

pub struct Workspace {
    pub editor: Entity<Editor>,
    pub path: Option<PathBuf>,
    pub(crate) show_overflow: bool,
    pub(crate) banner: Option<String>,
    pub(crate) cheat_sheet: Option<Entity<crate::cheat_sheet::CheatSheet>>,
    /// The alternates sidebar's view, created the first time it opens.
    pub(crate) alt_sidebar: Option<Entity<crate::sidebar::AltSidebar>>,
    pub(crate) preview: bool,
    preview_focus: FocusHandle,
    pub(crate) header_dim: HeaderDim,
    /// Repaints the header when its auto-dim state is due to change.
    pub(crate) dim_timer: Option<Task<()>>,
    pub(crate) window_title: String,
    _subscriptions: Vec<Subscription>,
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
        let subscriptions =
            vec![
                cx.observe(&editor, |_, _, cx| cx.notify()),
                cx.subscribe_in(&editor, window, |ws, _, event: &EditorEvent, window, cx| {
                    match event {
                        EditorEvent::Typed => ws.typed(cx),
                        EditorEvent::AlternatesOpened => ws.alternates_opened(window, cx),
                        EditorEvent::AlternatesClosed => ws.alternates_closed(window, cx),
                    }
                }),
            ];
        let initial_theme = editor.read(cx).theme;
        theme::apply_to_components(&initial_theme, cx);
        let mut workspace = Self {
            editor,
            path: None,
            show_overflow: false,
            banner: None,
            cheat_sheet: None,
            alt_sidebar: None,
            preview: false,
            preview_focus: cx.focus_handle(),
            header_dim: HeaderDim::default(),
            dim_timer: None,
            window_title: String::new(),
            _subscriptions: subscriptions,
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
        let handle = self.editor.focus_handle(cx);
        window.focus(&handle, cx);
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
}

impl Workspace {
    pub(crate) fn toggle_markers(
        &mut self,
        _: &ToggleMarkers,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor.update(cx, |e, cx| e.toggle_markers(cx));
    }

    pub(crate) fn open_lab(&mut self, _: &OpenLab, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| e.open_lab(window, cx));
    }

    fn cycle_theme(&mut self, _: &CycleTheme, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| e.cycle_theme(cx));
    }

    pub(crate) fn toggle_overflow(
        &mut self,
        _: &ToggleOverflowPanel,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_overflow = !self.show_overflow;
        cx.notify();
    }

    /// The Overflow panel: the scratchpad's text (read-only until the scratchpad view lands).
    fn render_overflow(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.editor.read(cx).theme;
        let text = self.editor.read(cx).doc.overflow.clone();
        div()
            .w(px(280.))
            .h_full()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(t.rule)
            .bg(t.panel)
            .child(div().px_3().py_2().text_sm().child("Overflow"))
            .child(
                div()
                    .id("overflow-text")
                    .flex_1()
                    .overflow_y_scroll()
                    .px_3()
                    .text_sm()
                    .child(text),
            )
    }
}

/// The writing column's text width (the element adds `GUTTER` on each side).
pub(crate) const COLUMN_WIDTH: f32 = 640.;
/// Space above the first and below the last line, inside the scroll view.
const COLUMN_PADDING_Y: f32 = 64.;
/// Height of the fades over the scroll view's top and bottom edges.
pub(crate) const FADE_HEIGHT: f32 = 48.;

/// A fade from the background color (at the edge) to transparent, over `FADE_HEIGHT`.
fn edge_fade(top: bool, background: Hsla) -> impl IntoElement {
    let clear = Hsla {
        a: 0.,
        ..background
    };
    // Angle 180° runs top to bottom, 0° bottom to top; the first stop sits at the start.
    let angle = if top { 180. } else { 0. };
    div()
        .absolute()
        .left_0()
        .right_0()
        .h(px(FADE_HEIGHT))
        .map(|d| if top { d.top_0() } else { d.bottom_0() })
        .bg(linear_gradient(
            angle,
            linear_color_stop(background, 0.),
            linear_color_stop(clear, 1.),
        ))
}

impl Workspace {
    /// Swap the page for a read-only Markdown preview (export text: shown options, no ghosts),
    /// or back.
    pub(crate) fn toggle_preview(
        &mut self,
        _: &TogglePreview,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if std::mem::take(&mut self.preview) {
            self.focus_editor(window, cx);
        } else {
            self.editor.update(cx, |e, _| {
                e.finish_alt_edit();
            });
            self.preview = true;
            window.focus(&self.preview_focus, cx);
        }
        cx.notify();
    }

    /// The Markdown shown while the preview is open, computed from the live document.
    pub(crate) fn preview_text(&self, cx: &App) -> Option<String> {
        self.preview
            .then(|| storage::export_markdown(&self.editor.read(cx).doc))
    }

    fn render_preview(&self, md: SharedString) -> impl IntoElement {
        div()
            .id("md-preview")
            .key_context("Preview")
            .track_focus(&self.preview_focus)
            .flex_1()
            .h_full()
            .overflow_y_scroll()
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_center()
                    .py(px(COLUMN_PADDING_Y))
                    .px(px(32.))
                    .child(
                        div()
                            .w_full()
                            .max_w(px(COLUMN_WIDTH))
                            .child(TextView::markdown("md-preview-text", md).selectable(true)),
                    ),
            )
    }

    /// The page, or the Markdown preview while it is open.
    fn render_main(&self, cx: &App) -> AnyElement {
        match self.preview_text(cx) {
            Some(md) => self.render_preview(md.into()).into_any_element(),
            None => self.render_page(cx).into_any_element(),
        }
    }

    /// The scrollable page: a centered column with fades over its top and bottom edges.
    fn render_page(&self, cx: &App) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let (scroll, background) = (editor.scroll.clone(), editor.theme.background);
        let page_focus = editor.focus_handle(cx);
        let menu_editor = self.editor.clone();
        div()
            .id("page")
            .relative()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .child(
                div()
                    .id("editor-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .justify_center()
                            .py(px(COLUMN_PADDING_Y))
                            .px(px(32.))
                            .child(
                                div()
                                    .w_full()
                                    .max_w(px(COLUMN_WIDTH) + GUTTER * 2.)
                                    .child(self.editor.clone()),
                            ),
                    ),
            )
            .child(edge_fade(true, background))
            .child(edge_fade(false, background))
            // The editor's own right-click handler runs first and picks the target.
            .context_menu(move |menu, _, cx| {
                let ghost = menu_editor.read(cx).ghost_menu_label();
                menu.action_context(page_focus.clone())
                    .menu("Alternatives for selection", Box::new(OpenAlternates))
                    .menu("AI alternatives for selection", Box::new(AiAlternates))
                    .menu(ghost, Box::new(ToggleGhost))
                    .menu("Stash this in Overflow", Box::new(SendToOverflow))
            })
    }
}

impl Render for Workspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_window_title(window, cx);
        let t = self.editor.read(cx).theme;
        div()
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(t.background)
            .text_color(t.text)
            .on_mouse_move(cx.listener(|ws, _: &MouseMoveEvent, _, cx| {
                if ws.header_dim.on_mouse_move() {
                    cx.notify();
                }
            }))
            .on_action(cx.listener(Self::toggle_overflow))
            .on_action(cx.listener(Self::cycle_theme))
            .on_action(cx.listener(Self::toggle_markers))
            .on_action(cx.listener(Self::open_lab))
            .on_action(cx.listener(Self::toggle_preview))
            .on_action(cx.listener(Self::share_x))
            .on_action(cx.listener(Self::share_linkedin))
            .on_action(cx.listener(Self::show_cheat_sheet))
            .on_action(cx.listener(Self::save))
            .on_action(cx.listener(Self::save_as))
            .on_action(cx.listener(Self::open))
            .on_action(cx.listener(Self::new_file))
            .on_action(cx.listener(Self::quit))
            .child(self.render_header(cx))
            .children(
                self.banner
                    .clone()
                    .map(|b| div().px_4().py_2().bg(t.banner).text_sm().child(b)),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .flex_row()
                    .when(self.editor.read(cx).alt_session.is_some(), |row| {
                        row.children(self.alt_sidebar.clone())
                    })
                    .child(self.render_main(cx))
                    .when(self.show_overflow, |row| {
                        row.child(self.render_overflow(cx))
                    }),
            )
            .child(self.render_footer(cx))
            .child(self.render_toasts(cx))
    }
}

/// Save As appended ".md" to the chosen name and that names an existing file the dialog never
/// asked about.
pub(crate) fn needs_overwrite_confirm(chosen: &Path, final_path: &Path) -> bool {
    chosen != final_path && final_path.exists()
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
        // An emptied alternate being edited must not reach the sidecar.
        self.editor.update(cx, |e, cx| {
            e.finish_alt_edit();
            cx.notify();
        });
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

    pub(crate) fn save(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
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
            let chosen = path.clone();
            if path.extension().is_none() {
                path.set_extension("md");
            }
            if needs_overwrite_confirm(&chosen, &path) {
                let Ok(answer) = this.update_in(cx, |_, window, cx| {
                    window.prompt(
                        PromptLevel::Warning,
                        &format!("Replace {}?", file_name(&path)),
                        None,
                        &["Replace", "Cancel"],
                        cx,
                    )
                }) else {
                    return;
                };
                if answer.await != Ok(0) {
                    return;
                }
            }
            this.update(cx, |ws, cx| {
                ws.path = Some(path);
                ws.save_now(cx);
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn open(&mut self, _: &Open, window: &mut Window, cx: &mut Context<Self>) {
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

    use super::{Workspace, needs_overwrite_confirm};
    use crate::editor::AltEdit;

    #[test]
    fn overwrite_confirm_only_when_md_was_appended_onto_an_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let chosen = dir.path().join("essay");
        let appended = dir.path().join("essay.md");
        assert!(!needs_overwrite_confirm(&chosen, &appended));
        std::fs::write(&appended, "x").unwrap();
        assert!(needs_overwrite_confirm(&chosen, &appended));
        assert!(!needs_overwrite_confirm(&appended, &appended));
    }

    #[gpui::test]
    fn save_finishes_an_emptied_alternate_first(cx: &mut TestAppContext) {
        init(cx);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("essay.md");
        std::fs::write(&path, "Hello world\n").unwrap();
        let open_path = path.clone();
        let (ws, cx) = cx.add_window_view(move |window, cx| {
            Workspace::new(Config::default(), None, Some(open_path), window, cx)
        });
        ws.update_in(cx, |ws, _, cx| {
            ws.editor.update(cx, |e, _| {
                e.doc.make_alt(0, 6..11).unwrap();
                e.doc.edit_alt(0, 1, 0..5, "");
                e.alt_edit = Some(AltEdit {
                    para_id: e.doc.paragraphs[0].id,
                    start: 6,
                });
                e.selection = Selection::caret(DocPos::new(0, 6));
                e.dirty = true;
                assert_eq!(e.doc.paragraphs[0].text(), "Hello ");
            });
            assert!(ws.save_now(cx));
            assert!(ws.editor.read(cx).alt_edit.is_none());
        });
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "Hello world\n");
        let reopened = tulis_core::storage::open(&path).unwrap().document;
        assert_eq!(reopened.paragraphs[0].text(), "Hello world");
        assert!(
            reopened.paragraphs[0]
                .segments()
                .iter()
                .all(|s| !s.text().is_empty())
        );
    }

    #[gpui::test]
    fn non_utf8_file_shows_banner_and_keeps_document(cx: &mut TestAppContext) {
        init(cx);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("latin1.md");
        std::fs::write(&path, b"caf\xe9\n").unwrap();
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        ws.update_in(cx, |ws, window, cx| {
            ws.editor.update(cx, |e, cx| {
                e.replace_text_in_range(None, "draft", window, cx)
            });
            ws.open_path(path.clone(), cx);
            assert_eq!(ws.path, None);
            assert_eq!(ws.editor.read(cx).doc.paragraphs[0].text(), "draft");
            let banner = ws.banner.clone().unwrap();
            assert!(banner.ends_with("file is not valid UTF-8 text"), "{banner}");
        });
    }

    #[gpui::test]
    fn opens_command_line_path_and_saves(cx: &mut TestAppContext) {
        init(cx);
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
        init(cx);
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
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        ws.update_in(cx, |ws, _, cx| assert!(!ws.save_now(cx)));
    }

    #[gpui::test]
    fn opening_a_version_1_sidecar_joins_its_overflow_and_saves_version_2(cx: &mut TestAppContext) {
        init(cx);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("essay.md");
        tulis_core::storage::save(&tulis_core::Document::from_texts(&["Hello."]), &path).unwrap();
        // Today's sidecar, rewritten in the version 1 format: overflow was a list of items.
        let side = tulis_core::storage::sidecar_path(&path);
        let items = r#"[
            {"id": 7, "text": "Old idea.", "created_at": "2026-10-28T09:00:00Z", "from_paragraph": null},
            {"id": 8, "text": "Another one.", "created_at": "2026-10-28T09:05:00Z", "from_paragraph": null}
        ]"#;
        let v1 = std::fs::read_to_string(&side)
            .unwrap()
            .replace(r#""format_version": 2"#, r#""format_version": 1"#)
            .replace(r#""overflow": """#, &format!(r#""overflow": {items}"#));
        std::fs::write(&side, v1).unwrap();
        let open_path = path.clone();
        let (ws, cx) = cx.add_window_view(move |window, cx| {
            Workspace::new(Config::default(), None, Some(open_path), window, cx)
        });
        let overflow = ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.overflow.clone());
        assert_eq!(overflow, "Old idea.\n\nAnother one.");
        ws.update_in(cx, |ws, _, cx| assert!(ws.save_now(cx)));
        let saved = std::fs::read_to_string(&side).unwrap();
        assert!(saved.contains(r#""format_version": 2"#), "{saved}");
        assert!(
            saved.contains(r#""overflow": "Old idea.\n\nAnother one.""#),
            "{saved}"
        );
    }

    /// gpui-component widgets need their globals; some tests also press keys.
    fn init(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::keymap::bind(cx);
        });
    }

    #[gpui::test]
    fn cheat_sheet_opens_filters_and_closes_on_escape(cx: &mut TestAppContext) {
        init(cx);
        use gpui::AppContext as _;
        use gpui_component::WindowExt;
        init(cx);
        let (root, cx) = cx.add_window_view(|window, cx| {
            let ws = cx.new(|cx| Workspace::new(Config::default(), None, None, window, cx));
            gpui_component::Root::new(ws, window, cx)
        });
        let ws = root.read_with(cx, |root, _| {
            root.view().clone().downcast::<Workspace>().unwrap()
        });
        ws.update_in(cx, |ws, window, cx| {
            ws.show_cheat_sheet(&crate::actions::ShowCheatSheet, window, cx)
        });
        cx.run_until_parked();
        assert!(cx.update(|window, cx| window.has_active_dialog(cx)));
        let sheet = ws.read_with(cx, |ws, _| ws.cheat_sheet.clone().unwrap());
        sheet.update_in(cx, |sheet, window, cx| {
            sheet
                .filter
                .update(cx, |input, cx| input.set_value("theme", window, cx));
            let sections = sheet.sections(cx);
            assert_eq!(sections.len(), 1);
            assert_eq!(sections[0].1[0].label, "Cycle theme");
        });
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(!cx.update(|window, cx| window.has_active_dialog(cx)));
    }

    #[gpui::test]
    fn cheat_sheet_key_closes_it_while_the_filter_has_focus(cx: &mut TestAppContext) {
        use gpui::AppContext as _;
        use gpui_component::WindowExt;
        init(cx);
        let (root, cx) = cx.add_window_view(|window, cx| {
            let ws = cx.new(|cx| Workspace::new(Config::default(), None, None, window, cx));
            gpui_component::Root::new(ws, window, cx)
        });
        let ws = root.read_with(cx, |root, _| {
            root.view().clone().downcast::<Workspace>().unwrap()
        });
        ws.update_in(cx, |ws, window, cx| {
            ws.show_cheat_sheet(&crate::actions::ShowCheatSheet, window, cx)
        });
        cx.run_until_parked();
        assert!(cx.update(|window, cx| window.has_active_dialog(cx)));
        cx.simulate_keystrokes("secondary-shift-/");
        cx.run_until_parked();
        assert!(!cx.update(|window, cx| window.has_active_dialog(cx)));
    }

    #[gpui::test]
    fn vim_gx_toggles_the_overflow_panel(cx: &mut TestAppContext) {
        init(cx);
        let mut config = Config::default();
        config.editor.vim_mode = true;
        let (ws, cx) =
            cx.add_window_view(move |window, cx| Workspace::new(config, None, None, window, cx));
        ws.update_in(cx, |ws, window, cx| ws.focus_editor(window, cx));
        cx.run_until_parked();
        let editor = ws.read_with(cx, |ws, _| ws.editor.clone());
        editor.update_in(cx, |e, window, cx| {
            e.replace_text_in_range(None, "g", window, cx);
            e.replace_text_in_range(None, "x", window, cx);
        });
        cx.run_until_parked();
        assert!(ws.read_with(cx, |ws, _| ws.show_overflow));
    }

    #[gpui::test]
    fn new_ghost_and_stash_keys_work_on_the_page(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        ws.update_in(cx, |ws, window, cx| {
            ws.editor.update(cx, |e, cx| {
                e.replace_text_in_range(None, "One two", window, cx);
                e.selection = Selection {
                    anchor: DocPos::new(0, 0),
                    head: DocPos::new(0, 3),
                };
            });
            ws.focus_editor(window, cx);
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("secondary-/");
        let export = ws.read_with(cx, |ws, cx| {
            ws.editor.read(cx).doc.paragraphs[0].export_text()
        });
        assert_eq!(export, " two");
        ws.update(cx, |ws, cx| {
            ws.editor.update(cx, |e, _| {
                e.selection = Selection {
                    anchor: DocPos::new(0, 4),
                    head: DocPos::new(0, 7),
                }
            })
        });
        cx.simulate_keystrokes("secondary-shift-x");
        let stashed = ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.overflow.clone());
        assert_eq!(stashed, "two");
    }

    /// A workspace in a `Root` window (tabs and inputs need it), with the page focused.
    fn root_workspace<'a>(
        cx: &'a mut TestAppContext,
        texts: &[&str],
    ) -> (gpui::Entity<Workspace>, &'a mut gpui::VisualTestContext) {
        use gpui::AppContext as _;
        init(cx);
        let doc = tulis_core::Document::from_texts(texts);
        let (root, cx) = cx.add_window_view(|window, cx| {
            let ws = cx.new(|cx| Workspace::new(Config::default(), None, None, window, cx));
            ws.update(cx, |ws, cx| ws.editor.update(cx, |e, cx| e.load(doc, cx)));
            gpui_component::Root::new(ws, window, cx)
        });
        let ws = root.read_with(cx, |root, _| {
            root.view().clone().downcast::<Workspace>().unwrap()
        });
        ws.update_in(cx, |ws, window, cx| ws.focus_editor(window, cx));
        cx.run_until_parked();
        (ws, cx)
    }

    fn sidebar_focused(ws: &gpui::Entity<Workspace>, cx: &mut gpui::VisualTestContext) -> bool {
        let sidebar = ws.read_with(cx, |ws, _| ws.alt_sidebar.clone());
        sidebar.is_some_and(|s| cx.update(|window, cx| s.read(cx).list_focus.is_focused(window)))
    }

    #[gpui::test]
    fn ctrl_shift_a_opens_the_sidebar_and_escape_closes_it(cx: &mut TestAppContext) {
        let (ws, cx) = root_workspace(cx, &["I love tea"]);
        cx.simulate_keystrokes("secondary-shift-a");
        cx.run_until_parked();
        assert!(sidebar_focused(&ws, cx));
        assert!(ws.read_with(cx, |ws, cx| ws.editor.read(cx).alt_session.is_some()));
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(ws.read_with(cx, |ws, cx| ws.editor.read(cx).alt_session.is_none()));
        let page_focused = ws.update_in(cx, |ws, window, cx| {
            gpui::Focusable::focus_handle(ws.editor.read(cx), cx).is_focused(window)
        });
        assert!(page_focused);
    }

    #[gpui::test]
    fn sidebar_keys_preview_keep_and_remove(cx: &mut TestAppContext) {
        let (ws, cx) = root_workspace(cx, &["I love tea"]);
        ws.update(cx, |ws, cx| {
            ws.editor.update(cx, |e, cx| {
                e.doc.add_option(0, 2..6, "adore".into()).unwrap();
                e.doc.add_option(0, 2..7, "like".into()).unwrap();
                e.doc.set_active_option(0, 1, 0);
                e.selection = Selection::caret(DocPos::new(0, 3));
                e.open_alternates(None, cx);
            })
        });
        cx.run_until_parked();
        let text = |cx: &mut gpui::VisualTestContext| {
            ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.paragraphs[0].text())
        };
        cx.simulate_keystrokes("j");
        assert_eq!(text(cx), "I adore tea");
        cx.simulate_keystrokes("down k j");
        assert_eq!(text(cx), "I like tea");
        cx.simulate_keystrokes("d d");
        assert_eq!(text(cx), "I adore tea");
        cx.simulate_keystrokes("up enter");
        cx.run_until_parked();
        assert_eq!(text(cx), "I love tea");
        assert!(!sidebar_focused(&ws, cx));
        assert!(ws.read_with(cx, |ws, cx| ws.editor.read(cx).alt_session.is_some()));
    }

    #[gpui::test]
    fn typing_in_the_sidebar_field_adds_an_option(cx: &mut TestAppContext) {
        let (ws, cx) = root_workspace(cx, &["I love tea"]);
        ws.update(cx, |ws, cx| {
            ws.editor.update(cx, |e, cx| {
                e.selection = Selection::caret(DocPos::new(0, 3));
                e.open_alternates(None, cx);
            })
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("i");
        assert!(!sidebar_focused(&ws, cx));
        cx.simulate_input("adore");
        cx.simulate_keystrokes("enter");
        cx.run_until_parked();
        let text = ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.paragraphs[0].text());
        assert_eq!(text, "I adore tea");
        assert!(sidebar_focused(&ws, cx));
        let field = ws.read_with(cx, |ws, cx| {
            ws.alt_sidebar
                .as_ref()
                .unwrap()
                .read(cx)
                .input
                .read(cx)
                .value()
        });
        assert_eq!(field.as_ref(), "");
        // Esc in the field closes the sidebar too.
        cx.simulate_keystrokes("o");
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(ws.read_with(cx, |ws, cx| ws.editor.read(cx).alt_session.is_none()));
    }

    #[gpui::test]
    fn right_click_menu_opens_the_sidebar_on_the_word(cx: &mut TestAppContext) {
        let (ws, cx) = root_workspace(cx, &["I love green tea"]);
        cx.simulate_resize(gpui::size(gpui::px(1000.), gpui::px(700.)));
        cx.run_until_parked();
        let point = ws.read_with(cx, |ws, cx| {
            let layout = ws.editor.read(cx).layout.as_ref().expect("painted");
            let p = layout.window_point(DocPos::new(0, 8)).unwrap();
            gpui::point(p.x + gpui::px(2.), p.y + layout.line_height / 2.)
        });
        cx.simulate_mouse_down(point, gpui::MouseButton::Right, gpui::Modifiers::none());
        cx.simulate_mouse_up(point, gpui::MouseButton::Right, gpui::Modifiers::none());
        cx.run_until_parked();
        let sel = ws.read_with(cx, |ws, cx| ws.editor.read(cx).selection.range());
        assert_eq!(sel, DocPos::new(0, 7)..DocPos::new(0, 12));
        // First item: Alternatives for selection.
        cx.simulate_keystrokes("down enter");
        cx.run_until_parked();
        let session = ws.read_with(cx, |ws, cx| ws.editor.read(cx).alt_session.clone());
        assert_eq!(session.map(|s| s.range), Some(7..12));
        assert!(sidebar_focused(&ws, cx));
    }

    #[gpui::test]
    fn ctrl_shift_g_targets_the_sidebar_in_it_and_the_page_on_the_page(cx: &mut TestAppContext) {
        let (ws, cx) = root_workspace(cx, &["I love tea"]);
        ws.update(cx, |ws, cx| {
            ws.editor.update(cx, |e, cx| {
                let responses = vec![Ok(r#"["adore"]"#.into()), Ok(r#"["Tea, I love."]"#.into())];
                e.provider_override = Some(std::sync::Arc::new(tulis_core::ai::MockProvider::new(
                    responses,
                )));
                e.selection = Selection::caret(DocPos::new(0, 3));
                e.open_alternates(None, cx);
            })
        });
        cx.run_until_parked();
        cx.simulate_keystrokes("secondary-shift-g");
        cx.run_until_parked();
        let text = |cx: &mut gpui::VisualTestContext| {
            ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.paragraphs[0].text())
        };
        assert_eq!(text(cx), "I adore tea");
        // On the page, with a caret: the paragraph, and the sidebar follows to its Paragraph tab.
        ws.update_in(cx, |ws, window, cx| ws.focus_editor(window, cx));
        cx.simulate_keystrokes("secondary-shift-g");
        cx.run_until_parked();
        assert_eq!(text(cx), "Tea, I love.");
        let scope = ws.read_with(cx, |ws, cx| {
            ws.editor.read(cx).alt_session.as_ref().map(|s| s.scope)
        });
        assert_eq!(scope, Some(crate::editor::Scope::Paragraph));
    }

    #[gpui::test]
    fn undo_and_redo_work_while_the_sidebar_has_focus(cx: &mut TestAppContext) {
        let (ws, cx) = root_workspace(cx, &["Draft one."]);
        ws.update(cx, |ws, cx| {
            ws.editor.update(cx, |e, cx| {
                e.doc.add_version(0, "Draft two.".into());
                e.doc.set_active_version(0, 0);
                e.open_alternates(Some(crate::editor::Scope::Paragraph), cx);
            })
        });
        cx.run_until_parked();
        let text = |cx: &mut gpui::VisualTestContext| {
            ws.read_with(cx, |ws, cx| ws.editor.read(cx).doc.paragraphs[0].text())
        };
        cx.simulate_keystrokes("j");
        assert_eq!(text(cx), "Draft two.");
        cx.simulate_keystrokes("secondary-z");
        assert_eq!(text(cx), "Draft one.");
        cx.simulate_keystrokes("secondary-shift-z");
        assert_eq!(text(cx), "Draft two.");
        cx.simulate_keystrokes("secondary-z ctrl-y");
        assert_eq!(text(cx), "Draft two.");
        assert!(sidebar_focused(&ws, cx));
    }

    #[gpui::test]
    fn shift_f10_and_the_menu_key_open_the_context_menu_at_the_caret(cx: &mut TestAppContext) {
        for key in ["shift-f10", "menu"] {
            let (ws, cx) = root_workspace(cx, &["I love green tea"]);
            cx.simulate_resize(gpui::size(gpui::px(1000.), gpui::px(700.)));
            ws.update(cx, |ws, cx| {
                ws.editor
                    .update(cx, |e, _| e.selection = Selection::caret(DocPos::new(0, 9)))
            });
            cx.run_until_parked();
            cx.simulate_keystrokes(key);
            cx.run_until_parked();
            let sel = ws.read_with(cx, |ws, cx| ws.editor.read(cx).selection.range());
            assert_eq!(sel, DocPos::new(0, 7)..DocPos::new(0, 12), "{key}");
            cx.simulate_keystrokes("down enter");
            cx.run_until_parked();
            let session = ws.read_with(cx, |ws, cx| ws.editor.read(cx).alt_session.clone());
            assert_eq!(session.map(|s| s.range), Some(7..12), "{key}");
        }
    }

    #[gpui::test]
    fn the_page_dims_only_while_the_sidebar_has_focus(cx: &mut TestAppContext) {
        let (ws, cx) = root_workspace(cx, &["One.", "Two."]);
        let dimmed = |cx: &mut gpui::VisualTestContext| {
            ws.update_in(cx, |ws, window, cx| ws.editor.read(cx).page_dimmed(window))
        };
        assert!(!dimmed(cx));
        cx.simulate_keystrokes("secondary-shift-a");
        cx.run_until_parked();
        assert!(dimmed(cx));
        cx.simulate_keystrokes("i"); // the text field is in the sidebar too
        assert!(dimmed(cx));
        ws.update_in(cx, |ws, window, cx| ws.focus_editor(window, cx));
        cx.run_until_parked();
        assert!(ws.read_with(cx, |ws, cx| ws.editor.read(cx).alt_session.is_some()));
        assert!(!dimmed(cx));
    }

    #[gpui::test]
    fn closing_the_sidebar_leaves_focus_elsewhere_alone(cx: &mut TestAppContext) {
        let (ws, cx) = root_workspace(cx, &["I love tea"]);
        cx.simulate_keystrokes("secondary-shift-a");
        cx.run_until_parked();
        ws.update_in(cx, |ws, window, cx| {
            ws.show_cheat_sheet(&crate::actions::ShowCheatSheet, window, cx)
        });
        cx.run_until_parked();
        ws.update(cx, |ws, cx| {
            ws.editor.update(cx, |e, cx| e.close_alternates(cx))
        });
        cx.run_until_parked();
        let page_focused = ws.update_in(cx, |ws, window, cx| {
            gpui::Focusable::focus_handle(ws.editor.read(cx), cx).is_focused(window)
        });
        assert!(!page_focused);
    }

    #[gpui::test]
    fn toasts_stack_two_and_expire(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        let editor = ws.read_with(cx, |ws, _| ws.editor.clone());
        editor.update(cx, |e, cx| {
            e.set_notice("one", cx);
            e.set_notice("two", cx);
            e.set_notice("three", cx);
        });
        let shown = |cx: &mut gpui::VisualTestContext| {
            editor.read_with(cx, |e, _| {
                e.toasts.iter().map(|t| t.text.clone()).collect::<Vec<_>>()
            })
        };
        assert_eq!(shown(cx), vec!["two", "three"]);
        cx.run_until_parked();
        cx.executor().advance_clock(crate::toast::TOAST_LIFETIME);
        cx.run_until_parked();
        assert!(shown(cx).is_empty());
    }

    #[gpui::test]
    fn toast_is_solid_then_fades_in_its_last_fifth(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        let editor = ws.read_with(cx, |ws, _| ws.editor.clone());
        editor.update(cx, |e, cx| e.set_notice("hello", cx));
        let fading = |cx: &mut gpui::VisualTestContext| {
            editor.read_with(cx, |e, _| {
                e.toasts.iter().map(|t| t.fading).collect::<Vec<_>>()
            })
        };
        cx.run_until_parked();
        cx.executor().advance_clock(crate::toast::TOAST_FADE_AT / 2);
        cx.run_until_parked();
        assert_eq!(fading(cx), vec![false]);
        cx.executor().advance_clock(crate::toast::TOAST_FADE_AT / 2);
        cx.run_until_parked();
        assert_eq!(fading(cx), vec![true]);
        cx.executor().advance_clock(crate::toast::TOAST_FADE);
        cx.run_until_parked();
        assert!(fading(cx).is_empty());
    }

    #[gpui::test]
    fn cycling_the_theme_recolors_components(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        let background = |cx: &mut gpui::VisualTestContext| {
            cx.update(|_, cx| gpui_component::Theme::global(cx).background)
        };
        let midnight = crate::theme::ThemeId::Midnight.theme();
        assert_eq!(background(cx), midnight.background);
        ws.update_in(cx, |ws, window, cx| {
            ws.cycle_theme(&crate::actions::CycleTheme, window, cx)
        });
        let paper = crate::theme::ThemeId::Paper.theme();
        assert_eq!(background(cx), paper.background);
        assert!(!cx.update(|_, cx| gpui_component::Theme::global(cx).mode.is_dark()));
    }

    #[gpui::test]
    fn text_column_is_640px_wide_and_starts_after_the_gutter(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        cx.simulate_resize(gpui::size(gpui::px(1000.), gpui::px(700.)));
        cx.run_until_parked();
        let (width, x) = ws.read_with(cx, |ws, cx| {
            let layout = ws.editor.read(cx).layout.as_ref().expect("painted");
            (layout.width, layout.origin.x)
        });
        assert_eq!(width, gpui::px(super::COLUMN_WIDTH));
        // Centered: (1000 - 640) / 2 = 180.
        assert_eq!(x, gpui::px(180.));
    }

    #[gpui::test]
    fn column_shrinks_with_a_narrow_window(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        cx.simulate_resize(gpui::size(gpui::px(580.), gpui::px(800.)));
        cx.run_until_parked();
        let width = ws.read_with(cx, |ws, cx| {
            ws.editor.read(cx).layout.as_ref().expect("painted").width
        });
        // 580 - 2 * 32 padding - 2 * 24 gutter.
        assert_eq!(width, gpui::px(468.));
    }

    #[gpui::test]
    fn preview_shows_export_markdown_and_escape_closes_it(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        ws.update_in(cx, |ws, window, cx| {
            ws.editor.update(cx, |e, cx| {
                e.replace_text_in_range(None, "I really love tea\nBye.", window, cx);
                e.doc.set_ghost(0, 2..9, true).unwrap();
            });
            ws.toggle_preview(&crate::actions::TogglePreview, window, cx);
        });
        assert_eq!(
            ws.read_with(cx, |ws, cx| ws.preview_text(cx)).as_deref(),
            Some("I love tea\n\nBye.\n")
        );
        // The preview follows the document while it is open.
        ws.update_in(cx, |ws, window, cx| {
            ws.editor.update(cx, |e, cx| {
                e.replace_text_in_range(None, "Changed", window, cx);
            });
        });
        assert_eq!(
            ws.read_with(cx, |ws, cx| ws.preview_text(cx)).as_deref(),
            Some("I love tea\n\nBye.Changed\n")
        );
        cx.run_until_parked();
        cx.simulate_keystrokes("escape");
        assert!(ws.read_with(cx, |ws, cx| ws.preview_text(cx)).is_none());
    }

    #[gpui::test]
    fn share_copies_markdown_and_opens_the_intent_url(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        ws.update_in(cx, |ws, window, cx| {
            ws.editor.update(cx, |e, cx| {
                e.replace_text_in_range(None, "Tea & cake\nMore", window, cx);
                e.selection = Selection::caret(DocPos::new(0, 0));
            });
            ws.share(tulis_core::share::ShareTarget::X, cx);
        });
        assert_eq!(
            cx.read_from_clipboard().and_then(|c| c.text()).as_deref(),
            Some("Tea & cake\n\nMore")
        );
        assert_eq!(
            cx.opened_url().as_deref(),
            Some("https://x.com/intent/post?text=Tea%20%26%20cake%0A%0AMore")
        );
        ws.update_in(cx, |ws, window, cx| {
            ws.editor.update(cx, |e, _| {
                e.selection = Selection {
                    anchor: DocPos::new(0, 0),
                    head: DocPos::new(0, 3),
                };
            });
            ws.share_linkedin(&crate::actions::ShareLinkedIn, window, cx);
        });
        assert_eq!(
            cx.opened_url().as_deref(),
            Some("https://www.linkedin.com/feed/?shareActive=true&text=Tea")
        );
        let notice = ws.read_with(cx, |ws, cx| {
            ws.editor.read(cx).last_notice().map(str::to_string)
        });
        assert_eq!(notice.as_deref(), Some("Copied — opening the browser"));
    }

    #[gpui::test]
    fn sharing_an_empty_document_only_toasts(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        ws.update(cx, |ws, cx| ws.share(tulis_core::share::ShareTarget::X, cx));
        assert_eq!(cx.opened_url(), None);
        let notice = ws.read_with(cx, |ws, cx| {
            ws.editor.read(cx).last_notice().map(str::to_string)
        });
        assert_eq!(notice.as_deref(), Some("Nothing to share yet"));
    }

    #[gpui::test]
    fn header_shows_counts_selection_counts_and_vim_badge(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        ws.update_in(cx, |ws, window, cx| {
            ws.editor.update(cx, |e, cx| {
                e.replace_text_in_range(None, "# Title here", window, cx);
            });
            assert_eq!(ws.header_status(cx).0, "2 words · 10 chars");
            ws.editor.update(cx, |e, cx| {
                e.selection = Selection {
                    anchor: DocPos::new(0, 2),
                    head: DocPos::new(0, 7),
                };
                e.toggle_vim(&crate::actions::ToggleVim, window, cx);
            });
            let (counts, badges) = ws.header_status(cx);
            assert_eq!(counts, "1 word · 5 chars selected");
            assert_eq!(badges, vec!["NORMAL".to_string()]);
        });
    }

    #[gpui::test]
    fn typing_starts_a_dim_burst_and_mouse_move_ends_it(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        let editor = ws.read_with(cx, |ws, _| ws.editor.clone());
        editor.update_in(cx, |e, window, cx| {
            e.replace_text_in_range(None, "a", window, cx)
        });
        assert!(ws.read_with(cx, |ws, _| ws.dim_timer.is_some()));
        ws.update(cx, |ws, _| assert!(ws.header_dim.on_mouse_move()));
    }
}
