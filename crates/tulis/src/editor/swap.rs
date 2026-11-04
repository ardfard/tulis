//! The paragraph version swap: a tint marks the paragraph, the old text fades out (ease-in),
//! the new text fades in (ease-out), then the tint holds and fades. Hovering a version bar or
//! its dots fades the same tint in as a preview. Jumps the app makes for you glide to their
//! place and land under the same tint.

use std::time::{Duration, Instant};

use gpui::{Context, Pixels};
use tulis_core::{ParaId, Paragraph};

use super::Editor;
use crate::motion::{self, BASE, FAST, HOLD, SLOW};

/// A version swap in progress.
#[derive(Clone, Debug)]
pub(crate) struct Swap {
    pub para_id: ParaId,
    /// The paragraph as it was, painted while it fades out.
    pub old: Paragraph,
    pub start: Instant,
}

/// When the tint starts to fade: after both fades and the hold.
const TINT_OUT_AT: Duration = Duration::from_millis(2 * 120 + 500);

impl Swap {
    /// Text alpha of the old version (gone after `FAST`).
    pub fn old_alpha(&self, now: Instant) -> f32 {
        1. - motion::ease_in(motion::progress(self.start, Duration::ZERO, FAST, now))
    }

    /// Text alpha of the new version (fades in after the old one is gone).
    pub fn new_alpha(&self, now: Instant) -> f32 {
        motion::ease_out(motion::progress(self.start, FAST, FAST, now))
    }

    /// Whether the old version is still being painted.
    pub fn showing_old(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.start) < FAST
    }

    pub fn tint_alpha(&self, now: Instant) -> f32 {
        let fade_in = motion::ease_out(motion::progress(self.start, Duration::ZERO, FAST, now));
        let fade_out = motion::ease_in(motion::progress(self.start, TINT_OUT_AT, BASE, now));
        fade_in * (1. - fade_out)
    }

    pub fn done(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.start) >= TINT_OUT_AT + BASE
    }
}

/// A jump gliding from one scroll offset to another (`motion::SLOW`, ease-out).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ScrollAnim {
    pub from: Pixels,
    pub to: Pixels,
    pub start: Instant,
    /// The offset set last frame; anything else means the user scrolled, which ends the glide.
    pub last: Pixels,
}

impl ScrollAnim {
    pub fn offset_at(&self, now: Instant) -> Pixels {
        let p = motion::ease_out(motion::progress(self.start, Duration::ZERO, SLOW, now));
        self.from + (self.to - self.from) * p
    }

    pub fn done(&self, now: Instant) -> bool {
        now.saturating_duration_since(self.start) >= SLOW
    }
}

/// The tint on where a jump lands: in with the scroll, held, then faded like a swap's.
pub(crate) fn arrive_tint_alpha(start: Instant, now: Instant) -> f32 {
    let fade_in = motion::ease_out(motion::progress(start, Duration::ZERO, FAST, now));
    let fade_out = motion::ease_in(motion::progress(start, SLOW + HOLD, BASE, now));
    fade_in * (1. - fade_out)
}

pub(crate) fn arrive_tint_done(start: Instant, now: Instant) -> bool {
    now.saturating_duration_since(start) >= SLOW + HOLD + BASE
}

/// Hovering a paragraph's version bar or dots.
#[derive(Clone, Copy, Debug)]
pub(crate) struct VersionHover {
    pub para: usize,
    pub start: Instant,
}

impl VersionHover {
    pub fn tint_alpha(&self, now: Instant) -> f32 {
        motion::ease_out(motion::progress(self.start, Duration::ZERO, FAST, now))
    }
}

// The hold is part of `TINT_OUT_AT`; keep them in step.
const _: () = assert!(TINT_OUT_AT.as_millis() == 2 * FAST.as_millis() + HOLD.as_millis());

impl Editor {
    /// Paragraph `para` just switched versions from `old`: start the swap (unless the OS asks for
    /// reduced motion, which makes it an instant swap).
    pub(crate) fn start_swap(&mut self, para: usize, old: Paragraph, cx: &mut Context<Self>) {
        if cx.reduce_motion() {
            return;
        }
        self.swap = Some(Swap {
            para_id: self.doc.paragraphs[para].id,
            old,
            start: Instant::now(),
        });
    }

    /// The next scroll to the caret is a jump to paragraph `para` the app makes for you: it
    /// glides there and tints where it lands.
    pub(crate) fn smooth_jump(&mut self, para: usize) {
        self.jump_request = Some(para);
    }

    /// A key was pressed: every running animation jumps to its end state.
    pub(crate) fn settle_motion(&mut self, cx: &mut Context<Self>) {
        let mut settled = self.swap.take().is_some() | self.lab_arrived.take().is_some();
        if let Some(anim) = self.scroll_anim.take() {
            let mut offset = self.scroll.offset();
            offset.y = anim.to;
            self.scroll.set_offset(offset);
            settled = true;
        }
        if settled | self.arrive_tint.take().is_some() {
            cx.notify();
        }
    }

    /// The version hover changed to `para` (or none).
    pub(crate) fn set_version_hover(&mut self, para: Option<usize>, cx: &mut Context<Self>) {
        if self.version_hover.map(|h| h.para) == para {
            return;
        }
        self.version_hover = para.map(|para| VersionHover {
            para,
            start: Instant::now(),
        });
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_swap_fades_out_then_in_and_the_tint_holds() {
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let swap = Swap {
            para_id: ParaId(1),
            old: Paragraph::new(ParaId(1), "old"),
            start,
        };
        assert_eq!(swap.old_alpha(at(0)), 1.);
        assert_eq!(swap.new_alpha(at(60)), 0.);
        assert!(swap.showing_old(at(119)) && !swap.showing_old(at(120)));
        assert!(swap.old_alpha(at(120)).abs() < 1e-3);
        assert!((swap.new_alpha(at(240)) - 1.).abs() < 1e-3);
        assert!((swap.tint_alpha(at(500)) - 1.).abs() < 1e-3);
        assert!(swap.tint_alpha(at(840)) < 1.);
        assert!(swap.tint_alpha(at(940)).abs() < 1e-3);
        assert!(!swap.done(at(900)) && swap.done(at(940)));
    }
}
