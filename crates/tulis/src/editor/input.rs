//! Platform input handler. Read-only for now: editing arrives with the editing commands.

use std::ops::Range;

use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window};

use super::Editor;
use crate::utf16::{offset_to_utf16, range_from_utf16, range_to_utf16};

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.doc.flat_text();
        let range = range_from_utf16(&text, &range);
        *adjusted_range = Some(range_to_utf16(&text, &range));
        text.get(range).map(str::to_string)
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let text = self.doc.flat_text();
        let a = self.doc.flat_offset(self.selection.anchor);
        let h = self.doc.flat_offset(self.selection.head);
        Some(UTF16Selection {
            range: range_to_utf16(&text, &(a.min(h)..a.max(h))),
            reversed: h < a,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let text = self.doc.flat_text();
        self.marked.as_ref().map(|r| range_to_utf16(&text, r))
    }

    fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        _text: &str,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        _new_text: &str,
        _new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let layout = self.layout.as_ref()?;
        let text = self.doc.flat_text();
        let start = self
            .doc
            .pos_from_flat(range_from_utf16(&text, &range_utf16).start);
        let origin = layout.window_point(start)?;
        Some(Bounds::new(
            origin,
            gpui::size(gpui::px(2.), layout.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        let flat = self.doc.flat_offset(layout.pos_for_window_point(point));
        Some(offset_to_utf16(&self.doc.flat_text(), flat))
    }
}
