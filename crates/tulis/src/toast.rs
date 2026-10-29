//! Toasts: short notices shown above the footer that fade out after a few seconds.

use std::collections::VecDeque;
use std::time::Duration;

pub const TOAST_LIFETIME: Duration = Duration::from_secs(3);
/// At most this many toasts are stacked; a new one pushes out the oldest.
pub const MAX_TOASTS: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    pub id: u64,
    pub text: String,
}

#[derive(Debug, Default)]
pub struct ToastQueue {
    toasts: VecDeque<Toast>,
    next_id: u64,
}

impl ToastQueue {
    /// Show `text`; returns the new toast's id. Repeating the newest toast replaces it, so it
    /// restarts instead of stacking a duplicate.
    pub fn push(&mut self, text: impl Into<String>) -> u64 {
        let text = text.into();
        if self.toasts.back().is_some_and(|t| t.text == text) {
            self.toasts.pop_back();
        }
        self.next_id += 1;
        self.toasts.push_back(Toast {
            id: self.next_id,
            text,
        });
        while self.toasts.len() > MAX_TOASTS {
            self.toasts.pop_front();
        }
        self.next_id
    }

    /// Remove a toast (when its time is up). Unknown ids are ignored.
    pub fn remove(&mut self, id: u64) {
        self.toasts.retain(|t| t.id != id);
    }

    /// Showing toasts, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &Toast> {
        self.toasts.iter()
    }
}

/// Opacity at `progress` (0..=1) through a toast's lifetime: solid for 80%, then fading out.
pub fn toast_opacity(progress: f32) -> f32 {
    const SOLID: f32 = 0.8;
    if progress <= SOLID {
        1.
    } else {
        (1. - (progress - SOLID) / (1. - SOLID)).clamp(0., 1.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(q: &ToastQueue) -> Vec<&str> {
        q.iter().map(|t| t.text.as_str()).collect()
    }

    #[test]
    fn keeps_at_most_two_newest() {
        let mut q = ToastQueue::default();
        q.push("a");
        q.push("b");
        q.push("c");
        assert_eq!(texts(&q), vec!["b", "c"]);
    }

    #[test]
    fn repeating_the_newest_restarts_it() {
        let mut q = ToastQueue::default();
        q.push("a");
        let first = q.push("b");
        let second = q.push("b");
        assert_ne!(first, second);
        assert_eq!(texts(&q), vec!["a", "b"]);
        assert_eq!(q.iter().last().unwrap().id, second);
    }

    #[test]
    fn remove_by_id() {
        let mut q = ToastQueue::default();
        let a = q.push("a");
        q.remove(a);
        q.remove(999);
        assert!(q.iter().next().is_none());
    }

    #[test]
    fn opacity_is_solid_then_fades() {
        assert_eq!(toast_opacity(0.), 1.);
        assert_eq!(toast_opacity(0.8), 1.);
        assert!((toast_opacity(0.9) - 0.5).abs() < 1e-5);
        assert_eq!(toast_opacity(1.), 0.);
    }
}
