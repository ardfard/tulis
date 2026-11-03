//! The Overflow scratchpad: a right panel with one multi-line text field bound to
//! `doc.overflow`.

use std::ops::Range;

use gpui::{
    App, Context, Entity, Focusable, IntoElement, MouseMoveEvent, ParentElement, Pixels, Render,
    SharedString, Styled, Subscription, WeakEntity, Window, anchored, div, point, prelude::*, px,
    relative,
};
use gpui_component::input::{InputEvent, Textarea, TextareaState};

use crate::actions::{FocusOverflow, FocusPage, OverflowCopy, OverflowUse};
use crate::editor::Editor;
use crate::theme::Theme;
use crate::workspace::Workspace;

pub const SCRATCHPAD_WIDTH: f32 = 260.;
const CLOSE_PREVIEW: &str = "Close the preview first";
/// Space left of the text field for the blocks' `⠿` grips.
const GRIP_GUTTER: f32 = 18.;

pub struct Scratchpad {
    editor: Entity<Editor>,
    /// For the Markdown preview, which hides the page.
    workspace: WeakEntity<Workspace>,
    /// The text field. Its value mirrors `doc.overflow`; it keeps its own undo history.
    pub(crate) input: Entity<TextareaState>,
    /// The block under the pointer, whose grip is showing.
    pub(crate) hovered: Option<Range<usize>>,
    /// The editor's `overflow_gen` the field last matched.
    seen_gen: u64,
    /// The editor's `doc_gen` at the last pull: a new document starts the field afresh.
    seen_doc_gen: u64,
    _subscriptions: Vec<Subscription>,
}

/// What a grip drag carries: the block's byte range in `doc.overflow`, and its text then.
pub struct OverflowDrag {
    pub range: Range<usize>,
    pub text: String,
}

/// A block's grip, in window coordinates: its left edge and the block's lines from top to
/// bottom.
#[derive(Clone, Debug)]
pub(crate) struct Grip {
    pub(crate) range: Range<usize>,
    pub(crate) left: Pixels,
    pub(crate) top: Pixels,
    pub(crate) bottom: Pixels,
}

/// What follows the pointer during a drag: the block's text in a small card.
struct DragPreview {
    text: SharedString,
    theme: Theme,
}

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        div()
            .max_w(px(SCRATCHPAD_WIDTH - 20.))
            .px_2()
            .py_1()
            .rounded_md()
            .border_1()
            .border_color(t.rule)
            .bg(t.panel)
            .text_color(t.text)
            .text_sm()
            .line_clamp(3)
            .child(self.text.clone())
    }
}

/// Each block with its first and last line numbers in `text`, counted in one pass.
fn block_lines(text: &str, blocks: Vec<Range<usize>>) -> Vec<(Range<usize>, usize, usize)> {
    let newlines = |s: &str| s.bytes().filter(|&b| b == b'\n').count();
    let (mut line, mut counted) = (0, 0);
    blocks
        .into_iter()
        .map(|range| {
            line += newlines(&text[counted..range.start]);
            let first = line;
            line += newlines(&text[range.clone()]);
            counted = range.end;
            (range, first, line)
        })
        .collect()
}

impl Scratchpad {
    pub fn new(
        editor: Entity<Editor>,
        workspace: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Ctrl+Shift+X stashes the selection or paragraph here.")
        });
        let (text, seen_gen, seen_doc_gen) = {
            let e = editor.read(cx);
            (e.doc.overflow.clone(), e.overflow_gen, e.doc_gen)
        };
        input.update(cx, |input, cx| input.set_value(text, window, cx));
        let subscriptions = vec![
            // Page commands (stash, undo, open) change `doc.overflow`: show the new text.
            cx.observe_in(&editor, window, |this, _, window, cx| this.pull(window, cx)),
            cx.subscribe_in(&input, window, |this, _, event: &InputEvent, _, cx| {
                if let InputEvent::Change = event {
                    this.push(cx);
                }
            }),
        ];
        Self {
            editor,
            workspace,
            input,
            hovered: None,
            seen_gen,
            seen_doc_gen,
            _subscriptions: subscriptions,
        }
    }

    /// Each block's grip, from the field's last layout. A block whose first line is out of view
    /// has none.
    pub(crate) fn grips(&self, cx: &App) -> Vec<Grip> {
        let input = self.input.read(cx);
        let (Some(text), Some(line_height)) = (input.text_bounds(), input.line_height()) else {
            return Vec::new();
        };
        let view = input.input_bounds();
        let doc = &self.editor.read(cx).doc;
        let rows = input.display_map();
        let left = view.left() - px(GRIP_GUTTER);
        block_lines(&doc.overflow, doc.overflow_blocks())
            .into_iter()
            .filter_map(|(range, first, last)| {
                let start = rows.buffer_line_to_display_row_range(first)?.start;
                let end = rows.buffer_line_to_display_row_range(last)?.end;
                // The text's origin moves with the scroll position.
                let top = text.origin.y + line_height * start as f32;
                let bottom = text.origin.y + line_height * end as f32;
                (top >= view.top() - px(1.) && top < view.bottom()).then_some(Grip {
                    range,
                    left,
                    top,
                    bottom,
                })
            })
            .collect()
    }

    /// Show the grip of the block under the pointer.
    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let y = event.position.y;
        let hovered = self
            .grips(cx)
            .into_iter()
            .find(|g| g.top <= y && y < g.bottom)
            .map(|g| g.range);
        if hovered != self.hovered {
            self.hovered = hovered;
            cx.notify();
        }
    }

    fn render_grip(&self, grip: Grip, t: Theme, cx: &App) -> impl IntoElement {
        let text = self.editor.read(cx).doc.overflow[grip.range.clone()].to_string();
        let drag = OverflowDrag {
            range: grip.range.clone(),
            text,
        };
        anchored().position(point(grip.left, grip.top)).child(
            div()
                .id(("overflow-grip", grip.range.start))
                .px_1()
                .cursor_grab()
                .text_color(t.text_muted)
                .hover(|d| d.text_color(t.accent))
                .child("⠿")
                .on_drag(drag, move |drag: &OverflowDrag, _, _, cx| {
                    cx.new(|_| DragPreview {
                        text: drag.text.clone().into(),
                        theme: t,
                    })
                }),
        )
    }

    /// The field changed: store its text in the document.
    fn push(&mut self, cx: &mut Context<Self>) {
        let text = self.input.read(cx).value();
        let seen = self.seen_gen;
        // The field now matches the document, unless a change it hasn't shown is pending.
        self.seen_gen = self.editor.update(cx, |e, cx| {
            let in_sync = e.overflow_gen == seen;
            e.set_overflow(&text, cx);
            if in_sync { e.overflow_gen } else { seen }
        });
    }

    /// The document's overflow changed elsewhere: show it. After a stash the caret goes to the
    /// end, in view; otherwise the caret and scroll position stay. `set_value` emits no
    /// `Change`, and starts the field's undo history afresh.
    fn pull(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        cx.notify();
        let editor = self.editor.read(cx);
        let new_doc = editor.doc_gen != self.seen_doc_gen;
        if editor.overflow_gen == self.seen_gen && !new_doc {
            return;
        }
        self.seen_gen = editor.overflow_gen;
        self.seen_doc_gen = editor.doc_gen;
        let (text, appended) = (editor.doc.overflow.clone(), editor.overflow_appended);
        self.input.update(cx, |input, cx| {
            if new_doc {
                // A different document: even identical text must not keep the old undo
                // history, and the caret starts at the top.
                input.set_value(text, window, cx);
                input.set_selected_range(0..0, cx);
                input.set_scroll_offset(point(px(0.), px(0.)), cx);
                return;
            }
            if input.value().as_ref() == text {
                return;
            }
            let (caret, scroll) = (input.cursor(), input.scroll_offset());
            input.set_value(text, window, cx);
            if appended {
                let end = input.value().len();
                input.set_selected_range(end..end, cx);
                // Clamped to the bottom at the next layout.
                input.set_scroll_offset(point(px(0.), px(-1_000_000.)), cx);
            } else {
                let caret = caret.min(input.value().len());
                input.set_selected_range(caret..caret, cx);
                input.set_scroll_offset(scroll, cx);
            }
        });
    }

    /// Whether the Markdown preview hides the page.
    fn preview_open(&self, cx: &App) -> bool {
        self.workspace
            .upgrade()
            .is_some_and(|ws| ws.read(cx).preview)
    }

    /// With the preview open, the page can't take text: say so. Returns true then.
    fn refuse_under_preview(&self, cx: &mut Context<Self>) -> bool {
        let open = self.preview_open(cx);
        if open {
            self.editor
                .update(cx, |e, cx| e.set_notice(CLOSE_PREVIEW, cx));
        }
        open
    }

    /// Ctrl+Enter: move the field's selection, or the block at its cursor, to the page cursor,
    /// and focus the page. Not while the preview hides the page.
    fn use_block(&mut self, _: &OverflowUse, window: &mut Window, cx: &mut Context<Self>) {
        if self.refuse_under_preview(cx) {
            return;
        }
        let selected = self.input.read(cx).selected_range();
        let used = self
            .editor
            .update(cx, |e, cx| e.use_overflow_selection(selected, false, cx));
        if used {
            let page = self.editor.focus_handle(cx);
            window.focus(&page, cx);
        }
    }

    /// Ctrl+Shift+Enter: the same, but the text stays here and so does the focus.
    fn copy_block(&mut self, _: &OverflowCopy, _: &mut Window, cx: &mut Context<Self>) {
        if self.refuse_under_preview(cx) {
            return;
        }
        let selected = self.input.read(cx).selected_range();
        self.editor
            .update(cx, |e, cx| e.use_overflow_selection(selected, true, cx));
    }

    /// Esc: back to the page, or the preview while it is open (the panel stays open).
    fn focus_page(&mut self, _: &FocusPage, window: &mut Window, cx: &mut Context<Self>) {
        let main = match self.workspace.upgrade() {
            Some(ws) => ws.read(cx).main_focus(cx),
            None => self.editor.focus_handle(cx),
        };
        window.focus(&main, cx);
    }

    pub(crate) fn has_focus(&self, window: &Window, cx: &App) -> bool {
        self.input.focus_handle(cx).is_focused(window)
    }
}

impl Render for Scratchpad {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.editor.read(cx).theme;
        let grip = self
            .grips(cx)
            .into_iter()
            .find(|g| Some(&g.range) == self.hovered.as_ref());
        div()
            .id("scratchpad")
            .key_context("Scratchpad")
            .on_action(cx.listener(Self::use_block))
            .on_action(cx.listener(Self::copy_block))
            .on_action(cx.listener(Self::focus_page))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !*hovered && this.hovered.take().is_some() {
                    cx.notify();
                }
            }))
            .w(px(SCRATCHPAD_WIDTH))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(t.rule)
            .bg(t.panel)
            .text_sm()
            .child(
                div()
                    .px_3()
                    .pt_2()
                    .pb_1()
                    .font_family(crate::theme::SCRIPT_FONT)
                    .text_size(px(crate::theme::SCRIPT_SIZE))
                    .text_color(t.accent)
                    .child("Overflow"),
            )
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .pl(px(GRIP_GUTTER))
                    .pr_2()
                    .child(Textarea::new(&self.input).appearance(false).h(relative(1.))),
            )
            .children(grip.map(|grip| self.render_grip(grip, t, cx)))
            .child(
                div()
                    .px_3()
                    .py_2()
                    .text_xs()
                    .text_color(t.text_muted)
                    .child("ctrl+↵ or drag into the page to use"),
            )
    }
}

impl Workspace {
    /// The scratchpad's view, created on first use.
    pub(crate) fn ensure_scratchpad(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<Scratchpad> {
        let (editor, workspace) = (self.editor.clone(), cx.entity().downgrade());
        self.scratchpad
            .get_or_insert_with(|| cx.new(|cx| Scratchpad::new(editor, workspace, window, cx)))
            .clone()
    }

    /// Ctrl+Shift+\, vim `gX`: focus the scratchpad (opening the panel), or the page when the
    /// scratchpad has focus.
    pub(crate) fn toggle_overflow_focus(
        &mut self,
        _: &FocusOverflow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.scratchpad_focused(window, cx) {
            window.focus(&self.main_focus(cx), cx);
        } else {
            self.show_overflow = true;
            let input = self.ensure_scratchpad(window, cx).read(cx).input.clone();
            window.focus(&input.focus_handle(cx), cx);
        }
        cx.notify();
    }

    pub(crate) fn scratchpad_focused(&self, window: &Window, cx: &App) -> bool {
        self.scratchpad
            .as_ref()
            .is_some_and(|pad| pad.read(cx).has_focus(window, cx))
    }
}

#[cfg(test)]
mod tests {
    use super::block_lines;

    #[test]
    fn block_lines_count_the_lines_before_and_in_each_block() {
        let mut d = tulis_core::Document::new();
        d.overflow = "\none\ntwo\n\n\nthree\n\nfour\nfive\nsix".into();
        let lines: Vec<(usize, usize)> = block_lines(&d.overflow, d.overflow_blocks())
            .into_iter()
            .map(|(_, first, last)| (first, last))
            .collect();
        assert_eq!(lines, vec![(1, 2), (5, 5), (7, 9)]);
    }
}
