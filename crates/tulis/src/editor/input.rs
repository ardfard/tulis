//! Platform text input (typing and IME) over the document's flat text.

use std::ops::Range;

use gpui::{Bounds, Context, EntityInputHandler, Pixels, Point, UTF16Selection, Window, px, size};
use tulis_core::Selection;

use super::Editor;
use crate::utf16;

/// A UTF-16 range from the platform as an ordered byte range within `text` (conversion already
/// clamps each end to the text's length).
fn byte_range(text: &str, range_utf16: &Range<usize>) -> Range<usize> {
    let range = utf16::range_from_utf16(text, range_utf16);
    range.start.min(range.end)..range.start.max(range.end)
}

impl Editor {
    fn flat_range_of_selection(&self) -> Range<usize> {
        let r = self.selection.range();
        self.doc.flat_offset(r.start)..self.doc.flat_offset(r.end)
    }

    fn select_flat(&mut self, range: Range<usize>) {
        self.selection = Selection {
            anchor: self.doc.pos_from_flat(range.start),
            head: self.doc.pos_from_flat(range.end),
        };
    }
}

impl EntityInputHandler for Editor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let flat = self.doc.flat_text();
        let range = byte_range(&flat, &range_utf16);
        actual_range.replace(utf16::range_to_utf16(&flat, &range));
        Some(flat[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let flat = self.doc.flat_text();
        Some(UTF16Selection {
            range: utf16::range_to_utf16(&flat, &self.flat_range_of_selection()),
            reversed: self.selection.reversed(),
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let flat = self.doc.flat_text();
        self.marked
            .as_ref()
            .map(|r| utf16::range_to_utf16(&flat, r))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.vim_captures_text() {
            for ch in text.chars() {
                self.vim_key(tulis_core::vim::VimKey::Char(ch), window, cx);
            }
            return;
        }
        let flat = self.doc.flat_text();
        if let Some(range) = range_utf16
            .map(|r| byte_range(&flat, &r))
            .or(self.marked.clone())
        {
            self.select_flat(range);
        }
        self.marked = None;
        self.insert_text(text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.vim_captures_text() {
            return;
        }
        let flat = self.doc.flat_text();
        let range = range_utf16
            .map(|r| byte_range(&flat, &r))
            .or(self.marked.clone())
            .unwrap_or_else(|| self.flat_range_of_selection());
        self.select_flat(range.clone());
        self.insert_text(new_text, cx);
        let start = range.start;
        self.marked = (!new_text.is_empty()).then(|| start..start + new_text.len());
        if let Some(sel) = new_selected_range_utf16 {
            let sel = byte_range(new_text, &sel);
            self.select_flat(start + sel.start..start + sel.end);
        }
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let flat = self.doc.flat_text();
        let range = byte_range(&flat, &range_utf16);
        let layout = self.layout.as_ref()?;
        let origin = layout.window_point(self.doc.pos_from_flat(range.start))?;
        Some(Bounds::new(origin, size(px(2.), layout.line_height)))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let layout = self.layout.as_ref()?;
        let pos = self.doc.clamp(layout.pos_for_window_point(point));
        let flat = self.doc.flat_text();
        Some(utf16::offset_to_utf16(&flat, self.doc.flat_offset(pos)))
    }
}
