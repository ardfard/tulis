//! Marker geometry: dotted underlines under alternate groups, the paragraph-version bar and its
//! handle, and the box around a group being cycled. All pure, so it is unit-tested.

use std::ops::Range;

use gpui::{Bounds, Pixels, Point, point, px, size};

/// Dot size and spacing of the dotted underline.
pub const DOT: f32 = 1.;
pub const DOT_STEP: f32 = 3.;
/// The version bar sits this far left of the text column.
pub const BAR_OFFSET: f32 = 12.;
pub const BAR_WIDTH: f32 = 1.5;
/// The handle's clickable area.
pub const HANDLE_WIDTH: f32 = 12.;
pub const HANDLE_HEIGHT: f32 = 20.;

/// Split `range` of a paragraph into its pieces on each visual line, as `(line, start..end)`.
/// `line_starts` is `ParaLayout::line_starts`; `len` is the paragraph's byte length.
pub fn split_by_lines(
    line_starts: &[usize],
    len: usize,
    range: Range<usize>,
) -> Vec<(usize, Range<usize>)> {
    let mut out = Vec::new();
    for (k, &start) in line_starts.iter().enumerate() {
        let end = line_starts.get(k + 1).copied().unwrap_or(len);
        let s = range.start.max(start);
        let e = range.end.min(end);
        if s < e {
            out.push((k, s..e));
        }
    }
    out
}

/// Left edges of the dots of a dotted line from `x0` to `x1` (every `DOT_STEP`, each `DOT` wide).
pub fn dot_xs(x0: f32, x1: f32) -> Vec<f32> {
    let mut xs = Vec::new();
    let mut x = x0;
    while x + DOT <= x1 + 0.01 {
        xs.push(x);
        x += DOT_STEP;
    }
    xs
}

/// Top of the dotted underline on a line whose top is `line_top`: just under the glyphs, which
/// are vertically centered in the line.
pub fn underline_y(line_top: Pixels, line_height: Pixels, font_size: Pixels) -> Pixels {
    line_top + (line_height + font_size) / 2. + px(1.)
}

/// The version bar for a paragraph whose top is `para_top` (window coordinates) and which is
/// `height` tall; `text_left` is the text column's left edge.
pub fn version_bar(text_left: Pixels, para_top: Pixels, height: Pixels) -> Bounds<Pixels> {
    Bounds::new(
        point(text_left - px(BAR_OFFSET) - px(BAR_WIDTH / 2.), para_top),
        size(px(BAR_WIDTH), height),
    )
}

/// The `⋮` handle's clickable area, centered on the bar at the paragraph's mid-height.
pub fn version_handle(text_left: Pixels, para_top: Pixels, height: Pixels) -> Bounds<Pixels> {
    let center = point(text_left - px(BAR_OFFSET), para_top + height / 2.);
    Bounds::new(
        point(
            center.x - px(HANDLE_WIDTH / 2.),
            center.y - px(HANDLE_HEIGHT / 2.),
        ),
        size(px(HANDLE_WIDTH), px(HANDLE_HEIGHT)),
    )
}

/// The three 2×2 dots that draw `⋮` inside `handle`.
pub fn handle_dots(handle: Bounds<Pixels>) -> [Bounds<Pixels>; 3] {
    let c = handle.center();
    [-5., -1., 3.].map(|dy| Bounds::new(point(c.x - px(1.), c.y + px(dy)), size(px(2.), px(2.))))
}

/// The cycled-group box around one line's piece of a group, from `x0` to `x1` on a line whose
/// top is `line_top`.
pub fn group_box(x0: Pixels, x1: Pixels, line_top: Pixels, line_height: Pixels) -> Bounds<Pixels> {
    Bounds::from_corners(
        point(x0 - px(2.), line_top + px(2.)),
        point(x1 + px(2.), line_top + line_height - px(2.)),
    )
}

/// The handle under `position`, as a paragraph index.
pub fn handle_at(handles: &[(usize, Bounds<Pixels>)], position: Point<Pixels>) -> Option<usize> {
    handles
        .iter()
        .find(|(_, bounds)| bounds.contains(&position))
        .map(|(para, _)| *para)
}

/// Whether a paragraph spanning `top..top + height` touches the viewport `view` (a `(top,
/// bottom)` pair, with `margin` of slack). `None` means the viewport is unknown: draw everything.
pub fn intersects_viewport(
    top: Pixels,
    height: Pixels,
    view: Option<(Pixels, Pixels)>,
    margin: Pixels,
) -> bool {
    view.is_none_or(|(view_top, view_bottom)| {
        top + height >= view_top - margin && top <= view_bottom + margin
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paragraphs_outside_the_viewport_are_culled() {
        let view = Some((px(100.), px(500.)));
        let m = px(10.);
        assert!(intersects_viewport(px(200.), px(50.), view, m));
        assert!(intersects_viewport(px(80.), px(30.), view, m)); // straddles the top
        assert!(intersects_viewport(px(490.), px(50.), view, m)); // straddles the bottom
        assert!(!intersects_viewport(px(0.), px(50.), view, m)); // above
        assert!(!intersects_viewport(px(600.), px(50.), view, m)); // below
        assert!(intersects_viewport(px(900.), px(50.), None, m)); // unknown viewport
    }

    #[test]
    fn ranges_split_across_wrapped_lines() {
        let starts = [0, 10, 20];
        assert_eq!(split_by_lines(&starts, 25, 2..5), vec![(0, 2..5)]);
        assert_eq!(
            split_by_lines(&starts, 25, 8..22),
            vec![(0, 8..10), (1, 10..20), (2, 20..22)]
        );
        assert_eq!(split_by_lines(&starts, 25, 10..10), vec![]);
        assert_eq!(split_by_lines(&[0], 5, 0..5), vec![(0, 0..5)]);
    }

    #[test]
    fn dots_every_three_pixels_within_the_span() {
        assert_eq!(dot_xs(10., 20.), vec![10., 13., 16., 19.]);
        assert_eq!(dot_xs(0., 0.5), Vec::<f32>::new());
        assert_eq!(dot_xs(0., 1.), vec![0.]);
    }

    #[test]
    fn underline_sits_below_centered_glyphs() {
        assert_eq!(underline_y(px(100.), px(36.), px(20.)), px(129.));
    }

    #[test]
    fn bar_and_handle_sit_left_of_the_column() {
        let bar = version_bar(px(200.), px(50.), px(72.));
        assert_eq!(bar.origin, point(px(187.25), px(50.)));
        assert_eq!(bar.size, size(px(1.5), px(72.)));
        let handle = version_handle(px(200.), px(50.), px(72.));
        assert_eq!(handle.center(), point(px(188.), px(86.)));
        assert_eq!(handle_at(&[(3, handle)], point(px(188.), px(90.))), Some(3));
        assert_eq!(handle_at(&[(3, handle)], point(px(200.), px(90.))), None);
        let dots = handle_dots(handle);
        assert_eq!(dots[1].origin, point(px(187.), px(85.)));
    }

    #[test]
    fn group_box_pads_the_piece() {
        let b = group_box(px(10.), px(50.), px(0.), px(36.));
        assert_eq!(b.origin, point(px(8.), px(2.)));
        assert_eq!(b.size, size(px(44.), px(32.)));
    }
}
