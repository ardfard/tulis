//! Turn a paragraph's spans, a trim preview's cuts, IME marked range and `# ` title marker into
//! GPUI text runs.

use std::ops::Range;

use gpui::{Font, Hsla, TextRun, UnderlineStyle, px};
use tulis_core::Span;

pub struct RunStyle {
    pub font: Font,
    /// The font of a `# ` title paragraph (bold).
    pub title_font: Font,
    pub color: Hsla,
    /// The `# ` marker's color while the caret sits on it (otherwise it is not drawn).
    pub marker: Hsla,
    pub ghost_alpha: f32,
    /// Alpha factor of a trim preview's active cuts.
    pub fade_alpha: f32,
}

pub fn build_runs(
    text_len: usize,
    spans: &[Span],
    faded: &[Range<usize>],
    marked: Option<Range<usize>>,
    title_marker: usize,
    caret_on_marker: bool,
    style: &RunStyle,
) -> Vec<TextRun> {
    let title_marker = title_marker.min(text_len);
    let mut cuts = vec![0, text_len, title_marker];
    for span in spans {
        cuts.extend([span.range.start, span.range.end]);
    }
    for range in faded.iter().chain(marked.iter()) {
        cuts.extend([range.start.min(text_len), range.end.min(text_len)]);
    }
    cuts.sort_unstable();
    cuts.dedup();
    cuts.windows(2)
        .filter(|w| w[0] < w[1])
        .map(|w| {
            let (a, b) = (w[0], w[1]);
            let span = spans
                .iter()
                .find(|s| s.range.start <= a && b <= s.range.end);
            let ghost = span.is_some_and(|s| s.ghost);
            let in_marked = marked.as_ref().is_some_and(|m| m.start <= a && b <= m.end);
            let fade = faded.iter().any(|t| t.start <= a && b <= t.end);
            let mut color = style.color;
            if ghost {
                color.a *= style.ghost_alpha;
            }
            if fade {
                color.a *= style.fade_alpha;
            }
            // The title's `# ` stays in the text but is not drawn, unless the caret is on it
            // (then the block cursor needs something under it).
            if b <= title_marker {
                if caret_on_marker {
                    color = style.marker;
                } else {
                    color.a = 0.;
                }
            }
            // Alternates are marked with painted dots (see `editor::markers`), not underlines.
            let underline = in_marked.then(|| UnderlineStyle {
                color: Some(style.color),
                thickness: px(1.),
                wavy: false,
            });
            TextRun {
                len: b - a,
                font: if title_marker > 0 {
                    style.title_font.clone()
                } else {
                    style.font.clone()
                },
                color,
                background_color: None,
                underline,
                strikethrough: None,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use gpui::{FontWeight, font, hsla};
    use tulis_core::AltMark;

    use super::*;

    fn style() -> RunStyle {
        let mut title_font = font("Test");
        title_font.weight = FontWeight::BOLD;
        RunStyle {
            font: font("Test"),
            title_font,
            color: hsla(0., 0., 0., 1.),
            fade_alpha: 0.35,
            marker: hsla(0., 0., 0.5, 0.5),
            ghost_alpha: 0.1,
        }
    }

    #[test]
    fn runs_cover_text_and_carry_styles() {
        let spans = vec![
            Span {
                range: 0..2,
                ghost: false,
                alt: None,
            },
            Span {
                range: 2..6,
                ghost: false,
                alt: Some(AltMark { ai: true }),
            },
            Span {
                range: 6..10,
                ghost: true,
                alt: None,
            },
        ];
        let runs = build_runs(10, &spans, &[7..9], None, 0, false, &style());
        assert_eq!(
            runs.iter().map(|r| r.len).collect::<Vec<_>>(),
            vec![2, 4, 1, 2, 1]
        );
        assert!(runs.iter().all(|r| r.underline.is_none()));
        assert!((runs[2].color.a - 0.1).abs() < 1e-6);
        // A trim preview's cut is faded, not struck through.
        assert!(runs.iter().all(|r| r.strikethrough.is_none()));
        assert!((runs[3].color.a - 0.1 * 0.35).abs() < 1e-6);
        assert!((runs[4].color.a - 0.1).abs() < 1e-6);
    }

    #[test]
    fn title_is_bold_and_its_marker_invisible() {
        let spans = vec![Span {
            range: 0..7,
            ghost: false,
            alt: None,
        }];
        let runs = build_runs(7, &spans, &[], None, 2, false, &style());
        assert_eq!(runs.iter().map(|r| r.len).collect::<Vec<_>>(), vec![2, 5]);
        assert_eq!(runs[0].color.a, 0.);
        assert_eq!(runs[1].color.a, 1.);
        assert!(runs.iter().all(|r| r.font.weight == FontWeight::BOLD));
        let plain = build_runs(7, &spans, &[], None, 0, false, &style());
        assert!(plain.iter().all(|r| r.font.weight != FontWeight::BOLD));
    }

    #[test]
    fn title_marker_shows_muted_while_the_caret_is_on_it() {
        let spans = vec![Span {
            range: 0..7,
            ghost: false,
            alt: None,
        }];
        let hidden = build_runs(7, &spans, &[], None, 2, false, &style());
        assert_eq!(hidden[0].color.a, 0.);
        let shown = build_runs(7, &spans, &[], None, 2, true, &style());
        assert_eq!(shown[0].color, style().marker);
        assert!(shown[0].color.a > 0.);
        // The title text itself is untouched.
        assert_eq!(shown[1].color.a, 1.);
    }

    #[test]
    fn marked_text_is_underlined() {
        let spans = vec![Span {
            range: 0..5,
            ghost: false,
            alt: None,
        }];
        let runs = build_runs(5, &spans, &[], Some(1..3), 0, false, &style());
        assert_eq!(
            runs.iter().map(|r| r.len).collect::<Vec<_>>(),
            vec![1, 2, 2]
        );
        assert!(runs[1].underline.is_some());
        assert!(runs[2].underline.is_none());
    }

    #[test]
    fn empty_text_has_no_runs() {
        assert!(build_runs(0, &[], &[], None, 0, false, &style()).is_empty());
    }
}
