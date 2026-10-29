//! The custom element that shapes and paints the document.

use gpui::{
    App, AvailableSpace, BorderStyle, Bounds, ElementId, ElementInputHandler, Entity, FontWeight,
    GlobalElementId, InspectorElementId, LayoutId, PaintQuad, Pixels, Point, ScrollHandle, Style,
    TextAlign, Window, fill, font, outline, point, prelude::*, px, relative, size,
};

use super::Editor;
use super::layout::{LayoutCache, ParaLayout, clamp_px};
use super::markers::{self, DOT};
use crate::runs::{RunStyle, build_runs};
use tulis_core::stats::title_marker_len;

pub struct EditorElement {
    editor: Entity<Editor>,
}

impl EditorElement {
    pub fn new(editor: Entity<Editor>) -> Self {
        Self { editor }
    }
}

pub struct EditorPrepaint {
    layout: LayoutCache,
    selection: Vec<PaintQuad>,
    markers: Vec<PaintQuad>,
    cursor: Option<PaintQuad>,
}

impl IntoElement for EditorElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Horizontal space kept free on each side of the text, for the paragraph-version bar.
pub(crate) const GUTTER: Pixels = px(24.);

pub(crate) fn line_height(editor: &Editor) -> Pixels {
    px(editor.config.editor.effective_font_size() * 1.8)
}

/// Shape every paragraph at `width`. GPUI caches shaped lines between frames, so this is cheap.
fn build_layout(
    editor: &Editor,
    width: Pixels,
    origin: Point<Pixels>,
    window: &mut Window,
) -> LayoutCache {
    let font_size = px(editor.config.editor.effective_font_size());
    let lh = line_height(editor);
    let mut title_font = font(editor.font_family.clone());
    title_font.weight = FontWeight::BOLD;
    let style = RunStyle {
        font: font(editor.font_family.clone()),
        title_font,
        color: editor.theme.text,
        trim: editor.theme.trim,
        marker: {
            let mut c = editor.theme.text_muted;
            c.a *= 0.5;
            c
        },
        ghost_alpha: editor.theme.ghost_alpha,
    };
    let marked = editor.marked.clone().map(|r| {
        (
            editor.doc.pos_from_flat(r.start),
            editor.doc.pos_from_flat(r.end),
        )
    });
    let mut top = px(0.);
    let mut paras = Vec::with_capacity(editor.doc.paragraphs.len());
    for (i, p) in editor.doc.paragraphs.iter().enumerate() {
        let text = p.text();
        let trims: Vec<_> = editor
            .trims
            .iter()
            .filter(|t| t.para_id == p.id)
            .map(|t| t.range.clone())
            .collect();
        let marked_here =
            marked.and_then(|(a, b)| (a.para == i && b.para == i).then(|| a.offset..b.offset));
        let text_len = text.len();
        let title_marker = if i == 0 { title_marker_len(&text) } else { 0 };
        let caret_on_marker = i == 0
            && editor.selection.head.para == 0
            && editor.selection.head.offset < title_marker;
        let runs = build_runs(
            text_len,
            &p.spans(),
            &trims,
            marked_here,
            title_marker,
            caret_on_marker,
            &style,
        );
        let line = window
            .text_system()
            .shape_text(text.into(), font_size, &runs, Some(width), None)
            .ok()
            .and_then(|lines| lines.into_iter().next())
            .unwrap_or_default();
        let para = ParaLayout::new(top, line, text_len);
        top += para.height(lh) + lh * 0.6;
        paras.push(para);
    }
    LayoutCache {
        origin,
        width,
        line_height: lh,
        paras,
        version_handles: Vec::new(),
    }
}

/// Dotted underlines under alternate groups, version bars with their handles (recorded in
/// `layout.version_handles` for clicks), and the box around a group just cycled.
fn marker_quads(
    editor: &Editor,
    layout: &mut LayoutCache,
    view: Option<(Pixels, Pixels)>,
) -> Vec<PaintQuad> {
    let t = &editor.theme;
    let lh = layout.line_height;
    let font_size = px(editor.config.editor.effective_font_size());
    let left = layout.origin.x;
    let mut quads = Vec::new();
    for (ix, (p, pl)) in editor.doc.paragraphs.iter().zip(&layout.paras).enumerate() {
        let para_top = layout.origin.y + pl.top;
        if !markers::intersects_viewport(para_top, pl.height(lh), view, lh) {
            continue;
        }
        for span in p.spans() {
            let Some(mark) = span.alt else { continue };
            let color = if mark.ai { t.accent } else { t.text_muted };
            for (k, r) in markers::split_by_lines(&pl.line_starts, pl.line.len(), span.range) {
                let x0 = left + pl.x_in_line(k, r.start);
                let x1 = left + pl.x_in_line(k, r.end);
                let y = markers::underline_y(para_top + lh * k as f32, lh, font_size);
                for x in markers::dot_xs(f32::from(x0), f32::from(x1)) {
                    let dot = Bounds::new(point(px(x), y), size(px(DOT), px(DOT)));
                    quads.push(fill(dot, color));
                }
            }
        }
        if p.versions.len() > 1 {
            let height = pl.height(lh);
            let mut bar = t.accent;
            bar.a *= 0.6;
            quads.push(fill(markers::version_bar(left, para_top, height), bar));
            let handle = markers::version_handle(left, para_top, height);
            let c = handle.center();
            let gap = Bounds::new(point(c.x - px(2.), c.y - px(7.)), size(px(4.), px(14.)));
            quads.push(fill(gap, t.background));
            quads.extend(markers::handle_dots(handle).map(|dot| fill(dot, t.accent)));
            layout.version_handles.push((ix, handle));
        }
    }
    if let Some(cycled) = &editor.cycled
        && let Some(ix) = editor.doc.para_index(cycled.para_id)
        && let Some(pl) = layout.paras.get(ix)
    {
        let mut color = t.text_muted;
        color.a *= 0.4;
        let range = cycled.range.clone();
        for (k, r) in markers::split_by_lines(&pl.line_starts, pl.line.len(), range) {
            let line_top = layout.origin.y + pl.top + lh * k as f32;
            let x0 = left + pl.x_in_line(k, r.start);
            let x1 = left + pl.x_in_line(k, r.end);
            let bounds = markers::group_box(x0, x1, line_top, lh);
            quads.push(outline(bounds, color, BorderStyle::Solid).corner_radii(px(3.)));
        }
    }
    quads
}

fn selection_quads(editor: &Editor, layout: &LayoutCache) -> Vec<PaintQuad> {
    let range = editor.selection.range();
    if range.start == range.end || layout.paras.is_empty() {
        return Vec::new();
    }
    let lh = layout.line_height;
    let mut quads = Vec::new();
    let last_para = range.end.para.min(layout.paras.len() - 1);
    for para_ix in range.start.para..=last_para {
        let p = &layout.paras[para_ix];
        let len = p.line.len();
        let sel_start = if para_ix == range.start.para {
            range.start.offset
        } else {
            0
        };
        let sel_end = if para_ix == range.end.para {
            range.end.offset.min(len)
        } else {
            len
        };
        let continues = para_ix < range.end.para;
        for k in 0..p.line_count() {
            let (line_start, line_end) = p.line_range(k);
            let last_line = k + 1 == p.line_count();
            let s = sel_start.max(line_start);
            let e = sel_end.min(line_end);
            let newline_marker = continues && last_line && e == len;
            if s > e || (s == e && !newline_marker) {
                continue;
            }
            let x0 = p.x_in_line(k, s);
            let mut x1 = p.x_in_line(k, e);
            if newline_marker {
                x1 += px(editor.config.editor.effective_font_size() * 0.4);
            } else if e == line_end && !last_line {
                x1 = layout.width;
            }
            let top = layout.origin.y + p.top + lh * k as f32;
            quads.push(fill(
                Bounds::from_corners(
                    point(layout.origin.x + x0, top),
                    point(layout.origin.x + x1, top + lh),
                ),
                editor.theme.selection,
            ));
        }
    }
    quads
}

/// Adjust `scroll` so the caret is visible. Returns true if the offset changed.
fn scroll_into_view(scroll: &ScrollHandle, caret: Point<Pixels>, line_height: Pixels) -> bool {
    let viewport = scroll.bounds();
    if viewport.size.height <= px(0.) {
        return false;
    }
    let top = viewport.origin.y + line_height;
    let bottom = viewport.origin.y + viewport.size.height - line_height;
    let mut offset = scroll.offset();
    if caret.y < top {
        offset.y += top - caret.y;
    } else if caret.y + line_height > bottom {
        offset.y -= caret.y + line_height - bottom;
    } else {
        return false;
    }
    offset.y = clamp_px(offset.y, -scroll.max_offset().y, px(0.));
    scroll.set_offset(offset);
    true
}

impl Element for EditorElement {
    type RequestLayoutState = ();
    type PrepaintState = Option<EditorPrepaint>;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let editor = self.editor.clone();
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let layout_id =
            window.request_measured_layout(style, move |known, available, window, cx| {
                let width = known.width.unwrap_or(match available.width {
                    AvailableSpace::Definite(width) => width,
                    // No content-based minimum: the column may shrink with the window.
                    AvailableSpace::MinContent => GUTTER * 2.,
                    AvailableSpace::MaxContent => px(640.) + GUTTER * 2.,
                });
                let text_width = (width - GUTTER * 2.).max(px(1.));
                let layout =
                    build_layout(editor.read(cx), text_width, point(px(0.), px(0.)), window);
                size(width, layout.total_height())
            });
        (layout_id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let editor = self.editor.read(cx);
        let text_origin = point(bounds.origin.x + GUTTER, bounds.origin.y);
        let text_width = (bounds.size.width - GUTTER * 2.).max(px(1.));
        let mut layout = build_layout(editor, text_width, text_origin, window);
        let lh = layout.line_height;
        let selection = selection_quads(editor, &layout);
        let markers = if editor.show_markers {
            // Cull to the scroll viewport (known once it has been laid out).
            let viewport = editor.scroll.bounds();
            let view = (viewport.size.height > px(0.))
                .then(|| (viewport.origin.y, viewport.origin.y + viewport.size.height));
            marker_quads(editor, &mut layout, view)
        } else {
            Vec::new()
        };
        let block = editor.vim_captures_text();
        let cursor = layout.window_point(editor.selection.head).map(|caret| {
            let width = if block {
                px(editor.config.editor.effective_font_size() * 0.55)
            } else {
                px(2.)
            };
            let mut color = editor.theme.cursor;
            if block {
                color.a = 0.45;
            }
            fill(Bounds::new(caret, size(width, lh)), color)
        });
        let wants_autoscroll = editor.autoscroll;
        let caret = layout.window_point(editor.selection.head);
        let scroll = editor.scroll.clone();
        if wants_autoscroll {
            if let Some(caret) = caret {
                if scroll_into_view(&scroll, caret, lh) {
                    window.request_animation_frame();
                }
            }
            self.editor
                .update(cx, |editor, _| editor.autoscroll = false);
        }
        Some(EditorPrepaint {
            layout,
            selection,
            markers,
            cursor,
        })
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(prepaint) = prepaint.take() else {
            return;
        };
        let focus_handle = self.editor.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );
        for quad in prepaint.selection {
            window.paint_quad(quad);
        }
        let lh = prepaint.layout.line_height;
        let text_origin = prepaint.layout.origin;
        for para in &prepaint.layout.paras {
            let origin = point(text_origin.x, text_origin.y + para.top);
            para.line
                .paint(origin, lh, TextAlign::Left, None, window, cx)
                .ok();
        }
        for quad in prepaint.markers {
            window.paint_quad(quad);
        }
        if focus_handle.is_focused(window) {
            if let Some(cursor) = prepaint.cursor {
                window.paint_quad(cursor);
            }
        }
        let layout = prepaint.layout;
        self.editor
            .update(cx, |editor, _| editor.layout = Some(layout));
    }
}
