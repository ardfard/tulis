//! Snapshot-based undo/redo with coalescing of consecutive typing.

use std::time::{Duration, Instant};

pub const COALESCE_WINDOW: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditKind {
    /// A single-character insert or delete in paragraph `para`.
    Typing {
        para: usize,
    },
    Other,
}

pub struct History<S> {
    undo: Vec<S>,
    redo: Vec<S>,
    last: Option<(EditKind, Instant)>,
    limit: usize,
}

impl<S: Clone> History<S> {
    pub fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            last: None,
            limit: limit.max(1),
        }
    }

    /// Call before applying an edit. `before` produces the pre-edit state and only runs when a new
    /// undo step is needed.
    pub fn record(&mut self, kind: EditKind, now: Instant, before: impl FnOnce() -> S) {
        let coalesce = matches!(
            (kind, self.last),
            (EditKind::Typing { para }, Some((EditKind::Typing { para: last_para }, at)))
                if para == last_para && now.saturating_duration_since(at) < COALESCE_WINDOW
        );
        if !coalesce {
            self.undo.push(before());
            if self.undo.len() > self.limit {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last = Some((kind, now));
    }

    pub fn break_coalescing(&mut self) {
        self.last = None;
    }

    pub fn undo(&mut self, current: S) -> Option<S> {
        let previous = self.undo.pop()?;
        self.redo.push(current);
        self.last = None;
        Some(previous)
    }

    pub fn redo(&mut self, current: S) -> Option<S> {
        let next = self.redo.pop()?;
        self.undo.push(current);
        self.last = None;
        Some(next)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> String {
        v.to_string()
    }

    #[test]
    fn typing_in_one_paragraph_coalesces() {
        let mut h = History::new(10);
        let t0 = Instant::now();
        h.record(EditKind::Typing { para: 0 }, t0, || s(""));
        h.record(
            EditKind::Typing { para: 0 },
            t0 + Duration::from_millis(300),
            || s("a"),
        );
        assert_eq!(h.undo(s("ab")), Some(s("")));
        assert_eq!(h.undo(s("")), None);
    }

    #[test]
    fn a_pause_breaks_coalescing() {
        let mut h = History::new(10);
        let t0 = Instant::now();
        h.record(EditKind::Typing { para: 0 }, t0, || s(""));
        h.record(
            EditKind::Typing { para: 0 },
            t0 + Duration::from_secs(2),
            || s("a"),
        );
        assert_eq!(h.undo(s("ab")), Some(s("a")));
    }

    #[test]
    fn another_paragraph_or_explicit_break_starts_a_new_step() {
        let mut h = History::new(10);
        let t0 = Instant::now();
        h.record(EditKind::Typing { para: 0 }, t0, || s("0"));
        h.record(EditKind::Typing { para: 1 }, t0, || s("1"));
        h.break_coalescing();
        h.record(EditKind::Typing { para: 1 }, t0, || s("2"));
        assert_eq!(h.undo(s("3")), Some(s("2")));
        assert_eq!(h.undo(s("2")), Some(s("1")));
        assert_eq!(h.undo(s("1")), Some(s("0")));
    }

    #[test]
    fn other_edits_never_coalesce() {
        let mut h = History::new(10);
        let t0 = Instant::now();
        h.record(EditKind::Other, t0, || s("a"));
        h.record(EditKind::Other, t0, || s("b"));
        assert_eq!(h.undo(s("c")), Some(s("b")));
    }

    #[test]
    fn redo_restores_and_new_edit_clears_redo() {
        let mut h = History::new(10);
        let t0 = Instant::now();
        h.record(EditKind::Other, t0, || s("a"));
        assert_eq!(h.undo(s("b")), Some(s("a")));
        assert!(h.can_redo());
        assert_eq!(h.redo(s("a")), Some(s("b")));
        h.undo(s("b"));
        h.record(EditKind::Other, t0, || s("a"));
        assert!(!h.can_redo());
    }

    #[test]
    fn limit_drops_oldest_steps() {
        let mut h = History::new(2);
        let t0 = Instant::now();
        for v in ["a", "b", "c"] {
            h.record(EditKind::Other, t0, || s(v));
        }
        assert_eq!(h.undo(s("d")), Some(s("c")));
        assert_eq!(h.undo(s("c")), Some(s("b")));
        assert_eq!(h.undo(s("b")), None);
    }
}
