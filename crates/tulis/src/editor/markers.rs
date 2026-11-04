//! Marker geometry: dotted underlines under alternate groups, the paragraph-version bar and its
//! version dots, and the box around a group being cycled. All pure, so it is unit-tested.

use std::ops::Range;

use gpui::{Bounds, Pixels, Point, point, px, size};

/// Dot size and spacing of the dotted underline.
pub const DOT: f32 = 1.;
pub const DOT_STEP: f32 = 3.;
/// The version bar's left edge sits this far left of the text column.
pub const BAR_OFFSET: f32 = 14.;
pub const BAR_WIDTH: f32 = 2.;
/// The version dots' column: its left edge this far left of the text column, as wide as the
/// shown dot.
pub const DOTS_OFFSET: f32 = 29.;
pub const DOT_SHOWN: f32 = 7.;
pub const DOT_OTHER: f32 = 5.;
/// Space between one version dot and the next.
pub const DOTS_GAP: f32 = 5.;
/// Width of a version dot's clickable area, centered on the dot.
pub const DOT_HIT_WIDTH: f32 = 14.;

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
        point(text_left - px(BAR_OFFSET), para_top),
        size(px(BAR_WIDTH), height),
    )
}

/// One dot per version, top to bottom, the shown one (`active`) larger. The first dot is
/// centered on the paragraph's first line. Returns each dot's bounds and its clickable area.
pub fn version_dots(
    text_left: Pixels,
    para_top: Pixels,
    line_height: Pixels,
    count: usize,
    active: usize,
) -> Vec<(Bounds<Pixels>, Bounds<Pixels>)> {
    let center_x = text_left - px(DOTS_OFFSET) + px(DOT_SHOWN / 2.);
    let first = if active == 0 { DOT_SHOWN } else { DOT_OTHER };
    let mut top = para_top + line_height / 2. - px(first / 2.);
    (0..count)
        .map(|i| {
            let d = if i == active { DOT_SHOWN } else { DOT_OTHER };
            let dot = Bounds::new(point(center_x - px(d / 2.), top), size(px(d), px(d)));
            let hit = Bounds::new(
                point(center_x - px(DOT_HIT_WIDTH / 2.), top - px(DOTS_GAP / 2.)),
                size(px(DOT_HIT_WIDTH), px(d + DOTS_GAP)),
            );
            top += px(d + DOTS_GAP);
            (dot, hit)
        })
        .collect()
}

/// The cycled-group box around one line's piece of a group, from `x0` to `x1` on a line whose
/// top is `line_top`.
pub fn group_box(x0: Pixels, x1: Pixels, line_top: Pixels, line_height: Pixels) -> Bounds<Pixels> {
    Bounds::from_corners(
        point(x0 - px(2.), line_top + px(2.)),
        point(x1 + px(2.), line_top + line_height - px(2.)),
    )
}

/// The version dot under `position`, as `(paragraph index, version index)`.
pub fn version_dot_at(
    dots: &[(usize, usize, Bounds<Pixels>)],
    position: Point<Pixels>,
) -> Option<(usize, usize)> {
    dots.iter()
        .find(|(_, _, hit)| hit.contains(&position))
        .map(|(para, version, _)| (*para, *version))
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
    fn bar_and_dots_sit_left_of_the_column() {
        let bar = version_bar(px(200.), px(50.), px(72.));
        assert_eq!(bar.origin, point(px(186.), px(50.)));
        assert_eq!(bar.size, size(px(2.), px(72.)));
        let dots = version_dots(px(200.), px(50.), px(24.), 3, 1);
        let bounds: Vec<_> = dots.iter().map(|(dot, _)| dot).collect();
        // Small, shown (larger), small; the first centered on the first line.
        assert_eq!(bounds[0].origin, point(px(172.), px(59.5)));
        assert_eq!(bounds[0].size, size(px(5.), px(5.)));
        assert_eq!(bounds[1].origin, point(px(171.), px(69.5)));
        assert_eq!(bounds[1].size, size(px(7.), px(7.)));
        assert_eq!(bounds[2].origin.y, px(81.5));
        // The shown dot's column edge is 29 left of the text.
        assert_eq!(bounds[1].left(), px(171.));
        let hits: Vec<_> = dots
            .iter()
            .enumerate()
            .map(|(i, (_, hit))| (4, i, *hit))
            .collect();
        assert_eq!(version_dot_at(&hits, bounds[2].center()), Some((4, 2)));
        assert_eq!(version_dot_at(&hits, bounds[0].center()), Some((4, 0)));
        assert_eq!(version_dot_at(&hits, point(px(200.), px(60.))), None);
    }

    #[test]
    fn group_box_pads_the_piece() {
        let b = group_box(px(10.), px(50.), px(0.), px(36.));
        assert_eq!(b.origin, point(px(8.), px(2.)));
        assert_eq!(b.size, size(px(44.), px(32.)));
    }
}
