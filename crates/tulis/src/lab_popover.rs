//! The Lab popover, anchored above the footer's LAB button: what the Lab can do to its
//! paragraphs (the selection's, the cursor's or the whole document's).

use gpui::{
    App, ClickEvent, Context, Entity, EventEmitter, FocusHandle, Hsla, IntoElement, ParentElement,
    Render, RenderOnce, Styled, Subscription, Window, div, prelude::*, px,
};
use gpui_component::Selectable;
use tulis_core::ParaId;

use crate::actions::{LabChoose, LabNext, LabPrev};
use crate::editor::{Editor, TrimLevel};

pub const LAB_WIDTH: f32 = 340.;

/// Something the Lab can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LabItem {
    Trim(TrimLevel),
}

impl LabItem {
    /// In the order the popover lists them (and the arrow keys walk them).
    pub fn all() -> Vec<LabItem> {
        TrimLevel::ALL.into_iter().map(LabItem::Trim).collect()
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
        let items = LabItem::all();
        let trims = items.iter().enumerate().map(|(ix, item)| {
            let LabItem::Trim(level) = item;
            (ix, *level)
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
            .child(div().italic().text_color(t.accent).child("The Lab guide…"))
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

/// The footer's dashed LAB button: the Lab popover's trigger.
#[derive(IntoElement)]
pub struct LabButton {
    pub open: bool,
    pub accent: Hsla,
    pub hover: Hsla,
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
        div()
            .id("lab")
            .debug_selector(|| "lab-button".into())
            .px_3()
            .py_0p5()
            .rounded_md()
            .border_1()
            .border_dashed()
            .border_color(self.accent)
            .text_color(self.accent)
            .cursor_pointer()
            .when(self.open, |d| d.bg(self.hover))
            .hover(|d| d.bg(self.hover))
            .child("LAB")
    }
}
