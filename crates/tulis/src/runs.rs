//! Turn a paragraph's spans, trim suggestions, IME marked range and `# ` title marker into GPUI
//! text runs.

use std::ops::Range;

use gpui::{Font, Hsla, StrikethroughStyle, TextRun, UnderlineStyle, px};
use tulis_core::Span;

pub struct RunStyle {
    pub font: Font,
    /// The font of a `# ` title paragraph (bold).
    pub title_font: Font,
    pub color: Hsla,
    pub alt_underline: Hsla,
    pub ai_underline: Hsla,
    pub trim: Hsla,
    pub ghost_alpha: f32,
}

pub fn build_runs(
    text_len: usize,
    spans: &[Span],
    trims: &[Range<usize>],
    marked: Option<Range<usize>>,
    title_marker: usize,
    style: &RunStyle,
) -> Vec<TextRun> {
    let title_marker = title_marker.min(text_len);
    let mut cuts = vec![0, text_len, title_marker];
    for span in spans {
        cuts.extend([span.range.start, span.range.end]);
    }
    for range in trims.iter().chain(marked.iter()) {
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
            let alt = span.and_then(|s| s.alt);
            let in_marked = marked.as_ref().is_some_and(|m| m.start <= a && b <= m.end);
            let trimmed = trims.iter().any(|t| t.start <= a && b <= t.end);
            let mut color = style.color;
            if ghost {
                color.a *= style.ghost_alpha;
            }
            // The title's `# ` stays in the text but is not drawn.
            if b <= title_marker {
                color.a = 0.;
            }
            let underline = if in_marked {
                Some(UnderlineStyle {
                    color: Some(style.color),
                    thickness: px(1.),
                    wavy: false,
                })
            } else {
                alt.map(|mark| UnderlineStyle {
                    color: Some(if mark.ai {
                        style.ai_underline
                    } else {
                        style.alt_underline
                    }),
                    thickness: px(1.),
                    wavy: false,
                })
            };
            let strikethrough = trimmed.then(|| StrikethroughStyle {
                thickness: px(1.5),
                color: Some(style.trim),
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
                strikethrough,
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
            alt_underline: hsla(0.5, 0.5, 0.5, 1.),
            ai_underline: hsla(0.75, 0.5, 0.5, 1.),
            trim: hsla(0., 1., 0.5, 1.),
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
        let runs = build_runs(10, &spans, &[7..9], None, 0, &style());
        assert_eq!(
            runs.iter().map(|r| r.len).collect::<Vec<_>>(),
            vec![2, 4, 1, 2, 1]
        );
        assert!(runs[0].underline.is_none());
        assert_eq!(
            runs[1].underline.as_ref().unwrap().color,
            Some(hsla(0.75, 0.5, 0.5, 1.))
        );
        assert!((runs[2].color.a - 0.1).abs() < 1e-6);
        assert!(runs[3].strikethrough.is_some());
        assert!(runs[4].strikethrough.is_none());
    }

    #[test]
    fn title_is_bold_and_its_marker_invisible() {
        let spans = vec![Span {
            range: 0..7,
            ghost: false,
            alt: None,
        }];
        let runs = build_runs(7, &spans, &[], None, 2, &style());
        assert_eq!(runs.iter().map(|r| r.len).collect::<Vec<_>>(), vec![2, 5]);
        assert_eq!(runs[0].color.a, 0.);
        assert_eq!(runs[1].color.a, 1.);
        assert!(runs.iter().all(|r| r.font.weight == FontWeight::BOLD));
        let plain = build_runs(7, &spans, &[], None, 0, &style());
        assert!(plain.iter().all(|r| r.font.weight != FontWeight::BOLD));
    }

    #[test]
    fn marked_text_is_underlined() {
        let spans = vec![Span {
            range: 0..5,
            ghost: false,
            alt: None,
        }];
        let runs = build_runs(5, &spans, &[], Some(1..3), 0, &style());
        assert_eq!(
            runs.iter().map(|r| r.len).collect::<Vec<_>>(),
            vec![1, 2, 2]
        );
        assert!(runs[1].underline.is_some());
        assert!(runs[2].underline.is_none());
    }

    #[test]
    fn empty_text_has_no_runs() {
        assert!(build_runs(0, &[], &[], None, 0, &style()).is_empty());
    }
}
