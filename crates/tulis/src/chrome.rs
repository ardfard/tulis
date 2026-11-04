//! The workspace's chrome: header, footer, toasts, and the share commands.

use std::time::Instant;

use gpui::{
    Anchor, Animation, AnimationExt, AnyElement, AnyView, App, BoxShadow, ClickEvent,
    ClipboardItem, Context, Div, FontWeight, Hsla, IntoElement, ParentElement, RenderOnce,
    Stateful, Styled, Window, div, hsla, point, prelude::*, px,
};
use gpui_component::popover::Popover;
use gpui_component::tooltip::Tooltip;
use gpui_component::{Icon, IconName, Selectable};
use tulis_core::alternates::AltLevel;
use tulis_core::share::{self, ShareTarget};
use tulis_core::stats;
use tulis_core::vim::Mode;

use crate::actions::{
    Open, OpenLab, Save, ShareLinkedIn, ShareX, ShowCheatSheet, ToggleMarkers, ToggleOverflowPanel,
    TogglePreview,
};
use crate::assets;
use crate::commands;
use crate::lab_popover::LabButton;
use crate::pills;
use crate::theme::Theme;
use crate::toast::TOAST_FADE;
use crate::workspace::{Workspace, file_name};

pub const HEADER_HEIGHT: f32 = 44.;
pub const FOOTER_HEIGHT: f32 = 48.;

pub(crate) fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
        Mode::Visual => "VISUAL",
        Mode::VisualLine => "V-LINE",
    }
}

/// What the header shows besides the file name.
#[derive(Debug, PartialEq)]
pub(crate) struct HeaderStatus {
    pub counts: String,
    pub vim: Option<&'static str>,
    /// Alternate position and AI work, drawn as panel badges.
    pub badges: Vec<String>,
}

pub(crate) fn icon(name: impl Into<Icon>, size: f32, color: Hsla) -> Icon {
    Icon::new(name).size(px(size)).text_color(color)
}

pub(crate) fn tooltip_text(text: String) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |window, cx| Tooltip::new(text.clone()).build(window, cx)
}

/// One of the header's paired toggles: a 30 × 26 icon button, filled while on.
fn header_toggle(id: &'static str, name: IconName, on: bool, t: &Theme) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(30.))
        .h(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .cursor_pointer()
        .when(on, |d| d.bg(t.selection))
        .when(!on, |d| d.hover(|d| d.bg(t.rule)))
        .child(icon(name, 15., if on { t.accent } else { t.text_muted }))
}

/// Space between the footer and the toasts, and between toasts.
const TOAST_GAP: f32 = 12.;
/// The Lab bar card: its width, and its distance above the footer (toasts sit above it).
const LAB_BAR_WIDTH: f32 = 660.;
const LAB_BAR_GAP: f32 = 15.;

impl Workspace {
    /// What floats above the footer, centered: the toasts (newest last), then the Lab bar.
    pub(crate) fn render_bottom_overlays(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let lab_bar = self.render_lab_bar(cx);
        let editor = self.editor.read(cx);
        let t = editor.theme;
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(FOOTER_HEIGHT + TOAST_GAP))
            .px(px(20.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .children(editor.toasts.iter().map(|toast| {
                div()
                    .id(("toast", toast.id))
                    .px(px(16.))
                    .py(px(10.))
                    .rounded(px(10.))
                    .bg(t.panel)
                    .border_1()
                    .border_color(t.rule)
                    .text_size(px(14.))
                    .line_height(px(17.))
                    .text_color(t.text)
                    .child(toast.text.clone())
                    // Solid (no per-frame redraw) until the fade begins.
                    .map(|el| {
                        if toast.fading {
                            el.with_animation(
                                ("toast-fade", toast.id),
                                Animation::new(TOAST_FADE),
                                |el, progress| el.opacity(1. - progress),
                            )
                            .into_any_element()
                        } else {
                            el.into_any_element()
                        }
                    })
            }))
            .children(lab_bar)
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

    /// What the header shows: counts (of the selection, if any), the vim mode and the
    /// alternate and AI badges.
    pub(crate) fn header_status(&self, cx: &App) -> HeaderStatus {
        let editor = self.editor.read(cx);
        let range = editor.selection.range();
        let counts = if range.start == range.end {
            stats::document_counts(&editor.doc).label(false)
        } else {
            stats::selection_counts(&editor.doc, range).label(true)
        };
        let mut badges = Vec::new();
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
        HeaderStatus {
            counts,
            vim: editor.vim_mode().map(mode_label),
            badges,
        }
    }

    /// File name and unsaved dot on the left, the markers and preview toggles in one panel in
    /// the middle, counts and the cheat-sheet button on the right.
    pub(crate) fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let t = editor.theme;
        let show_markers = editor.show_markers;
        let dirty = editor.dirty;
        let previewing = self.preview;
        let name = self
            .path
            .as_deref()
            .map(file_name)
            .unwrap_or_else(|| "Untitled".into());
        let status = self.header_status(cx);
        let dimmed = self.header_dim.is_dimmed(Instant::now());
        let badge = |label: String| {
            div()
                .px(px(7.))
                .py(px(2.))
                .rounded(px(6.))
                .text_size(px(12.))
                .line_height(px(16.))
                .child(label)
        };
        let header = div()
            .id("header")
            .h(px(HEADER_HEIGHT))
            .flex_none()
            .px(px(20.))
            .flex()
            .flex_row()
            .items_center()
            .text_size(px(13.))
            .text_color(t.text_muted)
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .pl(px(7.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .debug_selector(|| "doc-name".into())
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(t.text)
                            .truncate()
                            .child(name),
                    )
                    .when(dirty, |row| {
                        row.child(
                            div()
                                .debug_selector(|| "unsaved-dot".into())
                                .flex_none()
                                .size(px(6.))
                                .rounded_full()
                                .bg(t.accent),
                        )
                    })
                    .children(status.vim.map(|mode| {
                        badge(mode.into())
                            .border_1()
                            .border_color(t.accent)
                            .text_color(t.accent)
                    }))
                    .children(
                        status
                            .badges
                            .into_iter()
                            .map(|label| badge(label).bg(t.panel)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.))
                    .p(px(3.))
                    .rounded(px(9.))
                    .bg(t.panel)
                    .child(
                        header_toggle("toggle-markers", IconName::Eye, show_markers, &t)
                            .tooltip(tooltip_text(commands::tooltip("Markers", "toggle_markers")))
                            .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                                ws.toggle_markers(&ToggleMarkers, window, cx)
                            })),
                    )
                    .child(
                        header_toggle("toggle-preview", IconName::FileText, previewing, &t)
                            .tooltip(tooltip_text(commands::tooltip(
                                "Markdown preview",
                                "toggle_preview",
                            )))
                            .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                                ws.toggle_preview(&TogglePreview, window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_end()
                    .gap(px(14.))
                    .child(
                        div()
                            .debug_selector(|| "counts".into())
                            .child(status.counts),
                    )
                    .child(
                        div()
                            .id("cheat-sheet")
                            .size(px(28.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(|d| d.bg(t.panel))
                            .child(icon(gpui_kit_assets::IconName::Keyboard, 17., t.text_muted))
                            .tooltip(tooltip_text(commands::tooltip(
                                "Cheat sheet",
                                "cheat_sheet",
                            )))
                            .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                                ws.show_cheat_sheet(&ShowCheatSheet, window, cx)
                            })),
                    ),
            );
        if dimmed {
            header
                .with_animation(
                    "header-dim",
                    Animation::new(crate::motion::SLOW),
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

    /// Copy the selection (or the whole document) as Markdown and open the target's share page.
    pub(crate) fn share(&mut self, target: ShareTarget, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, _| {
            e.finish_alt_edit();
        });
        let editor = self.editor.read(cx);
        let text = share::share_text(&editor.doc, Some(editor.selection.range()));
        if text.trim().is_empty() {
            self.editor
                .update(cx, |e, cx| e.set_notice("Nothing to share yet", cx));
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        let url = share::share_url(target, &text);
        cx.open_url(&url.url);
        let notice = if url.trimmed {
            "Copied — trimmed to 280 characters for X"
        } else {
            "Copied — opening the browser"
        };
        self.editor.update(cx, |e, cx| e.set_notice(notice, cx));
    }

    pub(crate) fn share_x(&mut self, _: &ShareX, _: &mut Window, cx: &mut Context<Self>) {
        self.share(ShareTarget::X, cx);
    }

    pub(crate) fn share_linkedin(
        &mut self,
        _: &ShareLinkedIn,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.share(ShareTarget::LinkedIn, cx);
    }

    /// The Lab bar, a card floating above the footer: what the Lab is showing, its keys and
    /// its buttons.
    fn render_lab_bar(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let editor = self.editor.read(cx);
        let t = editor.theme;
        let bar = editor.lab_bar()?;
        let keys = bar.keys;
        Some(
            div()
                .w_full()
                .flex()
                .justify_center()
                .pt(px(LAB_BAR_GAP - TOAST_GAP))
                .child(
                    div()
                        .id("lab-bar")
                        .debug_selector(|| "lab-bar".into())
                        .occlude()
                        .w(px(LAB_BAR_WIDTH))
                        .max_w_full()
                        .px(px(18.))
                        .py(px(14.))
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .rounded(px(12.))
                        .border_1()
                        .border_color(t.rule)
                        .bg(t.panel)
                        .shadow(vec![BoxShadow {
                            color: hsla(0., 0., 0., 0.33),
                            offset: point(px(0.), px(8.)),
                            blur_radius: px(22.),
                            spread_radius: px(0.),
                            inset: false,
                        }])
                        .text_size(px(14.))
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .items_center()
                                .gap_x(px(10.))
                                .child(
                                    div()
                                        .font_weight(FontWeight::BOLD)
                                        .text_color(t.text)
                                        .child(bar.title),
                                )
                                .children(
                                    bar.detail
                                        .map(|detail| div().text_color(t.accent).child(detail)),
                                ),
                        )
                        .when(!keys.is_empty(), |d| {
                            d.child(pills::hint_row(
                                keys.iter().map(|(keys, label)| (keys.as_slice(), *label)),
                                12.,
                                &t,
                            ))
                        })
                        .children(bar.hint.map(|hint| {
                            div()
                                .text_size(px(13.))
                                .text_color(t.text_muted)
                                .child(hint)
                        }))
                        .child(
                            div()
                                .pt(px(4.))
                                .flex()
                                .flex_row()
                                .gap(px(8.))
                                .text_size(px(13.))
                                .children(bar.actions.into_iter().map(|action| {
                                    div()
                                        .id(action.id())
                                        .debug_selector(move || format!("lab-bar-{}", action.id()))
                                        .px(px(14.))
                                        .py(px(7.))
                                        .rounded(px(8.))
                                        .cursor_pointer()
                                        .map(|d| {
                                            if action.primary() {
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
                                        .child(action.label())
                                        .on_click(cx.listener(
                                            move |ws, _: &ClickEvent, window, cx| {
                                                ws.editor
                                                    .update(cx, |e, cx| e.bar_action(action, cx));
                                                window.focus(&ws.main_focus(cx), cx);
                                            },
                                        ))
                                })),
                        ),
                )
                .into_any_element(),
        )
    }

    /// Save, Open and Share ▾ on the left, the Lab button in the middle, Overflow (with the
    /// stash count) on the right.
    pub(crate) fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let t = editor.theme;
        let dirty = editor.dirty;
        let stashed = editor.doc.overflow_blocks().len();
        let overflow_open = self.show_overflow;
        let icon_button = |id: &'static str, tooltip: String| {
            div()
                .id(id)
                .size(px(30.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|d| d.bg(t.panel))
                .tooltip(tooltip_text(tooltip))
        };
        let workspace = cx.entity();
        div()
            .id("footer")
            .h(px(FOOTER_HEIGHT))
            .flex_none()
            .px(px(20.))
            .flex()
            .flex_row()
            .items_center()
            .text_size(px(13.))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(2.))
                    .child(
                        icon_button("save", commands::tooltip("Save", "save"))
                            .child(icon(
                                gpui_kit_assets::IconName::Save,
                                16.,
                                if dirty { t.accent } else { t.text_muted },
                            ))
                            .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                                ws.save(&Save, window, cx)
                            })),
                    )
                    .child(
                        icon_button("open", commands::tooltip("Open", "open"))
                            .child(icon(IconName::FolderOpen, 16., t.text_muted))
                            .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                                ws.open(&Open, window, cx)
                            })),
                    )
                    .child(div().w(px(1.)).h(px(16.)).bg(t.rule))
                    .child(
                        Popover::new("share-menu")
                            .anchor(Anchor::BottomLeft)
                            .offset(px(6.))
                            .trigger(ShareButton {
                                open: self.share_open,
                                theme: t,
                            })
                            .open(self.share_open)
                            .on_open_change(cx.listener(|ws, open: &bool, _, cx| {
                                ws.share_open = *open;
                                cx.notify();
                            }))
                            .p(px(5.))
                            .rounded(px(10.))
                            .bg(t.panel)
                            .border_color(t.rule)
                            .content(move |_, _, _| {
                                let share_button = |target: ShareTarget| {
                                    let (id, label, logo, size) = match target {
                                        ShareTarget::X => {
                                            ("share-x", "Share to X", assets::X_LOGO, 17.)
                                        }
                                        ShareTarget::LinkedIn => (
                                            "share-linkedin",
                                            "Share to LinkedIn",
                                            assets::LINKEDIN_LOGO,
                                            18.,
                                        ),
                                    };
                                    let workspace = workspace.clone();
                                    div()
                                        .id(id)
                                        .debug_selector(move || id.into())
                                        .size(px(38.))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .rounded(px(7.))
                                        .cursor_pointer()
                                        .text_color(t.text)
                                        .hover(|d| d.bg(t.selection).text_color(t.accent))
                                        .child(Icon::empty().path(logo).size(px(size)))
                                        .tooltip(tooltip_text(label.into()))
                                        .on_click(move |_, _, cx| {
                                            workspace.update(cx, |ws, cx| {
                                                ws.share_open = false;
                                                ws.share(target, cx);
                                            });
                                        })
                                };
                                div()
                                    .flex()
                                    .flex_row()
                                    .gap(px(4.))
                                    .child(share_button(ShareTarget::X))
                                    .child(share_button(ShareTarget::LinkedIn))
                            }),
                    ),
            )
            .child({
                let lab = self.lab.clone();
                Popover::new("lab-popover")
                    .anchor(Anchor::BottomCenter)
                    .trigger(LabButton {
                        open: self.lab_open,
                        theme: t,
                    })
                    .open(self.lab_open)
                    .track_focus(&self.lab.read(cx).focus)
                    .on_open_change(cx.listener(|ws, open: &bool, window, cx| {
                        if *open {
                            ws.open_lab(&OpenLab, window, cx)
                        } else {
                            ws.close_lab(cx)
                        }
                    }))
                    .content(move |_, _, _| lab.clone())
            })
            .child(
                div().flex_1().flex().flex_row().justify_end().child(
                    div()
                        .id("overflow-button")
                        .debug_selector(|| "overflow-button".into())
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(8.))
                        .px(px(12.))
                        .py(px(6.))
                        .rounded(px(8.))
                        .border_1()
                        .border_color(t.rule)
                        .text_color(t.text_muted)
                        .cursor_pointer()
                        .when(overflow_open, |d| d.bg(t.selection))
                        .when(!overflow_open, |d| d.hover(|d| d.bg(t.panel)))
                        .child(icon(IconName::PanelRight, 14., t.text_muted))
                        .child("Overflow")
                        .when(stashed > 0, |d| {
                            d.child(
                                div()
                                    .debug_selector(|| "stash-count".into())
                                    .px(px(6.))
                                    .py(px(1.))
                                    .rounded(px(4.))
                                    .bg(t.panel)
                                    .text_size(px(11.))
                                    .line_height(px(14.))
                                    .text_color(t.text)
                                    .child(stashed.to_string()),
                            )
                        })
                        .tooltip(tooltip_text(commands::tooltip(
                            "Show / hide Overflow",
                            "toggle_overflow",
                        )))
                        .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                            ws.toggle_overflow(&ToggleOverflowPanel, window, cx)
                        })),
                ),
            )
    }
}

/// The footer's Share ▾ button: the trigger of the menu with the X and LinkedIn buttons.
#[derive(IntoElement)]
struct ShareButton {
    open: bool,
    theme: Theme,
}

impl Selectable for ShareButton {
    fn selected(mut self, selected: bool) -> Self {
        self.open = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.open
    }
}

impl RenderOnce for ShareButton {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let t = self.theme;
        let color = if self.open { t.accent } else { t.text_muted };
        div()
            .id("share")
            .debug_selector(|| "share-button".into())
            .h(px(30.))
            .px(px(8.))
            .flex()
            .flex_row()
            .items_center()
            .gap(px(6.))
            .rounded(px(6.))
            .cursor_pointer()
            .text_color(color)
            .when(self.open, |d| d.bg(t.selection))
            .when(!self.open, |d| d.hover(|d| d.bg(t.panel)))
            .child(icon(gpui_kit_assets::IconName::Share, 15., color))
            .child("Share")
            .child(icon(IconName::ChevronDown, 13., color))
    }
}
