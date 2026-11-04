//! The Lab popover, anchored above the footer's LAB button: what the Lab can do to its
//! paragraphs (the selection's, the cursor's or the whole document's).

use gpui::{
    App, ClickEvent, Context, Entity, EventEmitter, FocusHandle, FontWeight, IntoElement,
    ParentElement, Render, RenderOnce, Styled, Subscription, Window, div, prelude::*, px,
};
use gpui_component::Selectable;
use tulis_core::ParaId;

use crate::actions::{LabChoose, LabNext, LabPrev};
use crate::chrome::icon;
use crate::commands;
use crate::editor::{Check, Editor, TrimLevel};
use crate::theme::Theme;

pub const LAB_WIDTH: f32 = 340.;

/// Something the Lab can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabItem {
    Check(Check),
    Trim(TrimLevel),
}

impl LabItem {
    /// In the order the popover lists them (and the arrow keys walk them): the checks, then
    /// the trim levels.
    pub fn all() -> Vec<LabItem> {
        Check::ALL
            .into_iter()
            .map(LabItem::Check)
            .chain(TrimLevel::ALL.into_iter().map(LabItem::Trim))
            .collect()
    }
}

/// The popover asks the workspace to close it.
pub enum LabEvent {
    Close,
}

pub struct LabPanel {
    editor: Entity<Editor>,
    pub(crate) focus: FocusHandle,
    /// The paragraphs the Lab works on.
    pub(crate) scope: Vec<ParaId>,
    /// Index into `LabItem::all()`.
    pub(crate) highlight: usize,
    /// "what each idea does" is expanded.
    pub(crate) explain: bool,
    _subscription: Subscription,
}

impl EventEmitter<LabEvent> for LabPanel {}

impl LabPanel {
    pub fn new(editor: Entity<Editor>, cx: &mut Context<Self>) -> Self {
        Self {
            _subscription: cx.observe(&editor, |_, _, cx| cx.notify()),
            editor,
            focus: cx.focus_handle(),
            scope: Vec::new(),
            highlight: 0,
            explain: false,
        }
    }

    /// Work on `scope` from now on, starting from the first row.
    pub(crate) fn reset(&mut self, scope: Vec<ParaId>, cx: &mut Context<Self>) {
        self.scope = scope;
        self.highlight = 0;
        cx.notify();
    }

    fn move_highlight(&mut self, forward: bool, cx: &mut Context<Self>) {
        let count = LabItem::all().len();
        self.highlight = if forward {
            (self.highlight + 1) % count
        } else {
            (self.highlight + count - 1) % count
        };
        cx.notify();
    }

    fn next(&mut self, _: &LabNext, _: &mut Window, cx: &mut Context<Self>) {
        self.move_highlight(true, cx);
    }

    fn prev(&mut self, _: &LabPrev, _: &mut Window, cx: &mut Context<Self>) {
        self.move_highlight(false, cx);
    }

    fn choose(&mut self, _: &LabChoose, _: &mut Window, cx: &mut Context<Self>) {
        if let Some(item) = LabItem::all().get(self.highlight).copied() {
            self.run(item, cx);
        }
    }

    /// Run `item` on the scope. The popover closes now, or when the AI is done (closing it
    /// before then stops the run).
    pub(crate) fn run(&mut self, item: LabItem, cx: &mut Context<Self>) {
        let scope = self.scope.clone();
        self.editor.update(cx, |e, cx| match item {
            LabItem::Check(check) => e.run_check(check, &scope, cx),
            LabItem::Trim(level) => e.run_trim(level, &scope, cx),
        });
        if !self.editor.read(cx).lab_running() {
            cx.emit(LabEvent::Close);
        }
    }
}

impl Render for LabPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let t = editor.theme;
        let running = editor.lab_running();
        let highlight = self.highlight;
        let explain = self.explain;
        let items = LabItem::all();
        let checks = items
            .iter()
            .enumerate()
            .filter_map(|(ix, item)| match item {
                LabItem::Check(check) => Some((ix, *check)),
                LabItem::Trim(_) => None,
            });
        let trims = items
            .iter()
            .enumerate()
            .filter_map(|(ix, item)| match item {
                LabItem::Trim(level) => Some((ix, *level)),
                LabItem::Check(_) => None,
            });
        div()
            .id("lab-panel")
            .debug_selector(|| "lab-panel".into())
            .key_context("Lab")
            .track_focus(&self.focus)
            .on_action(cx.listener(Self::next))
            .on_action(cx.listener(Self::prev))
            .on_action(cx.listener(Self::choose))
            .w(px(LAB_WIDTH))
            .flex()
            .flex_col()
            .gap_2()
            .text_sm()
            .text_color(t.text)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_baseline()
                    .child(
                        div()
                            .flex_1()
                            .font_family(crate::theme::SCRIPT_FONT)
                            .text_size(px(crate::theme::SCRIPT_SIZE))
                            .text_color(t.accent)
                            .child("The Lab guide…"),
                    )
                    .child(
                        div()
                            .id("lab-explain")
                            .debug_selector(|| "lab-explain".into())
                            .text_xs()
                            .text_color(t.text_muted)
                            .underline()
                            .cursor_pointer()
                            .child("what each idea does")
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.explain = !this.explain;
                                cx.notify();
                            })),
                    ),
            )
            .child(div().flex().flex_col().children(checks.map(|(ix, check)| {
                div()
                    .id(("lab-check", ix))
                    .debug_selector(move || format!("lab-check-{ix}"))
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .when(ix == highlight, |d| d.bg(t.selection))
                    .hover(|d| d.bg(t.selection))
                    .child(check.label())
                    .when(explain, |d| {
                        d.child(
                            div()
                                .debug_selector(move || format!("lab-desc-{ix}"))
                                .text_xs()
                                .text_color(t.text_muted)
                                .child(check.description()),
                        )
                    })
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.highlight = ix;
                        this.run(LabItem::Check(check), cx);
                    }))
            })))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .gap_1()
                    .children(trims.map(|(ix, level)| {
                        div()
                            .id(("lab-trim", ix))
                            .px_2()
                            .py_0p5()
                            .rounded_md()
                            .border_1()
                            .border_color(t.rule)
                            .text_xs()
                            .cursor_pointer()
                            .when(ix == highlight, |d| d.bg(t.selection))
                            .hover(|d| d.border_color(t.accent))
                            .child(level.label())
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.highlight = ix;
                                this.run(LabItem::Trim(level), cx);
                            }))
                    })),
            )
            .when(running, |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(t.text_muted)
                        .child("Working… Esc stops"),
                )
            })
    }
}

/// The footer's dashed Lab button, with its key: the Lab popover's trigger.
#[derive(IntoElement)]
pub struct LabButton {
    pub open: bool,
    pub theme: Theme,
}

impl Selectable for LabButton {
    fn selected(mut self, selected: bool) -> Self {
        self.open = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.open
    }
}

impl RenderOnce for LabButton {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let t = self.theme;
        div()
            .id("lab")
            .debug_selector(|| "lab-button".into())
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.))
            .pl(px(12.))
            .pr(px(10.))
            .py(px(6.))
            .rounded(px(8.))
            .border_1()
            .border_dashed()
            .border_color(t.accent)
            .text_color(t.accent)
            .cursor_pointer()
            .when(self.open, |d| d.bg(t.selection))
            .when(!self.open, |d| d.hover(|d| d.bg(t.panel)))
            .child(icon(gpui_kit_assets::IconName::FlaskConical, 14., t.accent))
            .child(div().font_weight(FontWeight::MEDIUM).child("Lab"))
            .children(commands::command_keys("open_lab").map(|keys| key_pill(keys, &t)))
    }
}

/// A key drawn as a small pill: `Ctrl L`.
pub(crate) fn key_pill(keys: &str, t: &Theme) -> impl IntoElement {
    div()
        .px(px(5.))
        .py(px(1.))
        .rounded(px(4.))
        .bg(t.selection)
        .text_size(px(11.))
        .line_height(px(14.))
        .text_color(t.accent)
        .child(commands::key_pill(keys))
}
