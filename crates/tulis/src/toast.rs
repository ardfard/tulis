//! Toasts: short notices shown above the footer that fade out after a few seconds.

use std::collections::VecDeque;
use std::time::Duration;

pub const TOAST_LIFETIME: Duration = Duration::from_secs(3);
/// A toast is solid until its last 20%, then fades out over `TOAST_FADE`.
pub const TOAST_FADE_AT: Duration = Duration::from_millis(2400);
pub const TOAST_FADE: Duration = Duration::from_millis(600);
/// At most this many toasts are stacked; a new one pushes out the oldest.
pub const MAX_TOASTS: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    pub id: u64,
    pub text: String,
    /// Set once the fade-out has begun; only then does the toast animate.
    pub fading: bool,
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
            fading: false,
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

    /// Begin a toast's fade-out. Unknown ids (removed or replaced) are ignored.
    pub fn start_fade(&mut self, id: u64) {
        if let Some(t) = self.toasts.iter_mut().find(|t| t.id == id) {
            t.fading = true;
        }
    }

    /// Showing toasts, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &Toast> {
        self.toasts.iter()
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
    fn fade_starts_in_the_last_fifth_and_only_for_the_right_toast() {
        assert_eq!(TOAST_FADE_AT + TOAST_FADE, TOAST_LIFETIME);
        assert_eq!(
            TOAST_FADE_AT.as_millis(),
            TOAST_LIFETIME.as_millis() * 4 / 5
        );
        let mut q = ToastQueue::default();
        let a = q.push("a");
        assert!(q.iter().all(|t| !t.fading));
        q.start_fade(999);
        assert!(q.iter().all(|t| !t.fading));
        q.start_fade(a);
        assert!(q.iter().next().unwrap().fading);
        // Repeating the text restarts the toast: the replacement is solid again.
        q.push("a");
        assert!(!q.iter().next().unwrap().fading);
    }
}
