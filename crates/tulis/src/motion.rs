//! Motion tokens and easing. Motion is used only where the text itself changes (a version swap,
//! the Lab reading the page, a jump, the chrome stepping back); everything else appears at once.
//!
//! GPUI has no transitions: an animation is a start `Instant` kept in state. Painting computes
//! its progress from the clock and requests another frame while it runs. A key press jumps every
//! running animation to its end state, and the OS reduced-motion setting skips them.

use std::time::{Duration, Instant};

/// Crossfade of swapped text, hover tint in, one scan-line step.
pub const FAST: Duration = Duration::from_millis(120);
/// Cut words fading to ghost, chrome coming back, a tint fading out.
pub const BASE: Duration = Duration::from_millis(200);
/// Scroll jumps, chrome dimming.
pub const SLOW: Duration = Duration::from_millis(400);
/// How long the swap tint stays before it fades.
pub const HOLD: Duration = Duration::from_millis(500);

/// `cubic-bezier(x1, y1, x2, y2)` at `t` (0..=1), solved for x by Newton steps then bisection.
pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32, t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    let coord = |a: f32, b: f32, s: f32| {
        let u = 1. - s;
        3. * u * u * s * a + 3. * u * s * s * b + s * s * s
    };
    let slope = |a: f32, b: f32, s: f32| {
        let u = 1. - s;
        3. * u * u * a + 6. * u * s * (b - a) + 3. * s * s * (1. - b)
    };
    let mut s = t;
    for _ in 0..8 {
        let dx = coord(x1, x2, s) - t;
        let d = slope(x1, x2, s);
        if dx.abs() < 1e-5 || d.abs() < 1e-6 {
            break;
        }
        s = (s - dx / d).clamp(0., 1.);
    }
    if (coord(x1, x2, s) - t).abs() > 1e-3 {
        let (mut lo, mut hi) = (0., 1.);
        for _ in 0..30 {
            s = (lo + hi) / 2.;
            if coord(x1, x2, s) < t {
                lo = s;
            } else {
                hi = s;
            }
        }
    }
    coord(y1, y2, s)
}

/// For everything that arrives: new text, a tint, a scroll, the chrome coming back.
pub fn ease_out(t: f32) -> f32 {
    cubic_bezier(0.2, 0., 0., 1., t)
}

/// For everything that leaves: old text, a tint fading, the chrome dimming.
pub fn ease_in(t: f32) -> f32 {
    cubic_bezier(0.4, 0., 1., 1., t)
}

/// Linear progress (0..=1) of a `duration` that starts `delay` after `start`.
pub fn progress(start: Instant, delay: Duration, duration: Duration, now: Instant) -> f32 {
    let elapsed = now.saturating_duration_since(start);
    if elapsed < delay {
        return 0.;
    }
    if duration.is_zero() {
        return 1.;
    }
    ((elapsed - delay).as_secs_f32() / duration.as_secs_f32()).min(1.)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn easings_run_from_zero_to_one() {
        for ease in [ease_out, ease_in] {
            assert!(ease(0.).abs() < 1e-3);
            assert!((ease(1.) - 1.).abs() < 1e-3);
            let mut last = 0.;
            for i in 1..=20 {
                let v = ease(i as f32 / 20.);
                assert!(v >= last - 1e-4, "monotonic");
                last = v;
            }
        }
        // Ease-out does most of the travel early; ease-in late.
        assert!(ease_out(0.3) > 0.6);
        assert!(ease_in(0.3) < 0.2);
        assert!((cubic_bezier(0., 0., 1., 1., 0.37) - 0.37).abs() < 1e-3);
    }

    #[test]
    fn progress_waits_for_the_delay_and_stops_at_one() {
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        assert_eq!(progress(start, FAST, FAST, at(100)), 0.);
        assert!((progress(start, FAST, FAST, at(180)) - 0.5).abs() < 1e-3);
        assert_eq!(progress(start, FAST, FAST, at(500)), 1.);
        assert_eq!(progress(start, Duration::ZERO, Duration::ZERO, at(0)), 1.);
    }
}
