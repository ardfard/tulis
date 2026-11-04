//! Auto-dim for the chrome (header, footer, toasts): it fades out after 2s of continuous typing
//! and comes back when typing pauses for 2s or the mouse moves.

use std::time::{Duration, Instant};

use crate::motion;

pub const DIM_AFTER: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeaderDim {
    /// First keystroke of the current typing burst.
    burst_start: Option<Instant>,
    last_key: Option<Instant>,
}

impl HeaderDim {
    pub fn on_key(&mut self, now: Instant) {
        let paused = self
            .last_key
            .is_none_or(|last| now.duration_since(last) >= DIM_AFTER);
        if paused || self.burst_start.is_none() {
            self.burst_start = Some(now);
        }
        self.last_key = Some(now);
    }

    /// Returns true if this ended a typing burst (the header may need repainting).
    pub fn on_mouse_move(&mut self) -> bool {
        let had_burst = self.burst_start.is_some();
        *self = Self::default();
        had_burst
    }

    pub fn is_dimmed(&self, now: Instant) -> bool {
        match (self.burst_start, self.last_key) {
            (Some(start), Some(last)) => {
                now.duration_since(last) < DIM_AFTER && now.duration_since(start) >= DIM_AFTER
            }
            _ => false,
        }
    }

    /// How long until `is_dimmed` may change, if it can change without further input.
    pub fn next_change(&self, now: Instant) -> Option<Duration> {
        let (start, last) = (self.burst_start?, self.last_key?);
        let since_last = now.duration_since(last);
        if since_last >= DIM_AFTER {
            return None;
        }
        if self.is_dimmed(now) {
            Some(DIM_AFTER - since_last)
        } else {
            Some(DIM_AFTER.saturating_sub(now.duration_since(start)))
        }
    }
}

/// The chrome's opacity as it fades between shown and dimmed: out over `motion::SLOW`
/// (ease-in), back over `motion::BASE` (ease-out), from wherever it was.
#[derive(Clone, Copy, Debug)]
pub struct ChromeFade {
    dimmed: bool,
    since: Instant,
    from: f32,
}

impl Default for ChromeFade {
    fn default() -> Self {
        Self {
            dimmed: false,
            since: Instant::now(),
            from: 1.,
        }
    }
}

impl ChromeFade {
    /// Head toward `dimmed` (a change restarts the fade from the current opacity).
    pub fn set(&mut self, dimmed: bool, now: Instant) {
        if dimmed != self.dimmed {
            *self = Self {
                dimmed,
                since: now,
                from: self.opacity(now),
            };
        }
    }

    pub fn opacity(&self, now: Instant) -> f32 {
        if self.dimmed {
            let p = motion::progress(self.since, Duration::ZERO, motion::SLOW, now);
            self.from * (1. - motion::ease_in(p))
        } else {
            let p = motion::progress(self.since, Duration::ZERO, motion::BASE, now);
            self.from + (1. - self.from) * motion::ease_out(p)
        }
    }

    /// Still fading (another frame is needed).
    pub fn fading(&self, now: Instant) -> bool {
        let duration = if self.dimmed {
            motion::SLOW
        } else {
            motion::BASE
        };
        now.saturating_duration_since(self.since) < duration
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn chrome_fades_out_slowly_and_back_quickly_from_where_it_was() {
        let start = Instant::now();
        let at = |ms| start + Duration::from_millis(ms);
        let mut fade = ChromeFade::default();
        fade.set(true, at(0));
        assert_eq!(fade.opacity(at(0)), 1.);
        assert!(fade.fading(at(399)) && !fade.fading(at(400)));
        assert!(fade.opacity(at(400)).abs() < 1e-3);
        fade.set(true, at(500));
        assert!(fade.opacity(at(500)).abs() < 1e-3);
        // Back, from halfway through a fade out.
        let mut fade = ChromeFade::default();
        fade.set(true, at(0));
        let mid = fade.opacity(at(200));
        fade.set(false, at(200));
        assert!((fade.opacity(at(200)) - mid).abs() < 1e-3);
        assert!((fade.opacity(at(400)) - 1.).abs() < 1e-3);
    }

    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// Type a key every 200ms from `from` for `count` keys; returns the time of the last key.
    fn type_keys(dim: &mut HeaderDim, from: Instant, count: u64) -> Instant {
        let mut t = from;
        for i in 0..count {
            t = from + ms(200 * i);
            dim.on_key(t);
        }
        t
    }

    #[test]
    fn dims_after_two_seconds_of_typing() {
        let t0 = Instant::now();
        let mut dim = HeaderDim::default();
        let last = type_keys(&mut dim, t0, 5); // 0..800ms
        assert!(!dim.is_dimmed(last));
        let last = type_keys(&mut dim, t0, 12); // 0..2200ms
        assert!(dim.is_dimmed(last));
    }

    #[test]
    fn comes_back_after_a_two_second_pause() {
        let t0 = Instant::now();
        let mut dim = HeaderDim::default();
        let last = type_keys(&mut dim, t0, 12);
        assert!(dim.is_dimmed(last + ms(1900)));
        assert!(!dim.is_dimmed(last + ms(2000)));
        assert_eq!(dim.next_change(last + ms(1500)), Some(ms(500)));
        assert_eq!(dim.next_change(last + ms(2000)), None);
    }

    #[test]
    fn a_pause_starts_a_new_burst() {
        let t0 = Instant::now();
        let mut dim = HeaderDim::default();
        let last = type_keys(&mut dim, t0, 12);
        dim.on_key(last + ms(2500));
        assert!(!dim.is_dimmed(last + ms(2600)));
        assert_eq!(dim.next_change(last + ms(2600)), Some(ms(1900)));
    }

    #[test]
    fn mouse_move_undims() {
        let t0 = Instant::now();
        let mut dim = HeaderDim::default();
        let last = type_keys(&mut dim, t0, 12);
        assert!(dim.on_mouse_move());
        assert!(!dim.is_dimmed(last));
        assert!(!dim.on_mouse_move());
        assert_eq!(dim.next_change(last), None);
    }
}
