//! Snapshot-based undo/redo with coalescing of consecutive typing. A step can also carry an op
//! (`O`) for state its snapshots leave out, which undo and redo replay instead of restoring.

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

/// A step: the state on one side of an edit, and the op the edit carried.
struct Step<S, O> {
    state: S,
    op: Option<O>,
}

pub struct History<S, O = ()> {
    /// Undo steps (the state before each edit), each with the id it got when pushed.
    undo: Vec<(u64, Step<S, O>)>,
    /// Redo steps (the state after each undone edit).
    redo: Vec<Step<S, O>>,
    last: Option<(EditKind, Instant)>,
    limit: usize,
    next_id: u64,
}

impl<S, O> History<S, O> {
    pub fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            last: None,
            limit: limit.max(1),
            next_id: 0,
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
            self.push_undo(Step {
                state: before(),
                op: None,
            });
            if self.undo.len() > self.limit {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last = Some((kind, now));
    }

    fn push_undo(&mut self, step: Step<S, O>) {
        self.undo.push((self.next_id, step));
        self.next_id += 1;
    }

    /// Attach `op` to the newest undo step (call after `record` and the edit).
    pub fn set_op(&mut self, op: O) {
        if let Some((_, step)) = self.undo.last_mut() {
            step.op = Some(op);
        }
    }

    /// The id of the newest undo step. Every pushed step (a recorded edit, or a redo) gets a new
    /// id, so this tells whether the newest step is still one seen earlier.
    pub fn newest_undo_id(&self) -> Option<u64> {
        self.undo.last().map(|(id, _)| *id)
    }

    pub fn break_coalescing(&mut self) {
        self.last = None;
    }

    pub fn undo(&mut self, current: S) -> Option<S> {
        self.undo_with(current, |op| op)
    }

    /// Step back: returns the state before the newest edit. `replay` undoes the edit's op and
    /// returns the op its redo step keeps.
    pub fn undo_with(
        &mut self,
        current: S,
        replay: impl FnOnce(Option<O>) -> Option<O>,
    ) -> Option<S> {
        let (_, previous) = self.undo.pop()?;
        self.redo.push(Step {
            state: current,
            op: replay(previous.op),
        });
        self.last = None;
        Some(previous.state)
    }

    pub fn redo(&mut self, current: S) -> Option<S> {
        self.redo_with(current, |op| op)
    }

    /// Step forward: returns the state after the newest undone edit. `replay` redoes the
    /// edit's op and returns the op its undo step keeps.
    pub fn redo_with(
        &mut self,
        current: S,
        replay: impl FnOnce(Option<O>) -> Option<O>,
    ) -> Option<S> {
        let next = self.redo.pop()?;
        self.push_undo(Step {
            state: current,
            op: replay(next.op),
        });
        self.last = None;
        Some(next.state)
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
        let mut h = History::<String>::new(10);
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
        let mut h = History::<String>::new(10);
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
        let mut h = History::<String>::new(10);
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
        let mut h = History::<String>::new(10);
        let t0 = Instant::now();
        h.record(EditKind::Other, t0, || s("a"));
        h.record(EditKind::Other, t0, || s("b"));
        assert_eq!(h.undo(s("c")), Some(s("b")));
    }

    #[test]
    fn redo_restores_and_new_edit_clears_redo() {
        let mut h = History::<String>::new(10);
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
    fn every_pushed_step_gets_a_new_id() {
        let mut h = History::<String>::new(10);
        let t0 = Instant::now();
        assert_eq!(h.newest_undo_id(), None);
        h.record(EditKind::Other, t0, || s("a"));
        let first = h.newest_undo_id();
        h.undo(s("b"));
        h.record(EditKind::Other, t0, || s("a"));
        assert_ne!(h.newest_undo_id(), first);
        let second = h.newest_undo_id();
        h.record(EditKind::Typing { para: 0 }, t0, || s("c"));
        let typing = h.newest_undo_id();
        assert_ne!(typing, second);
        h.record(EditKind::Typing { para: 0 }, t0, || s("d")); // coalesces
        assert_eq!(h.newest_undo_id(), typing);
        h.undo(s("e"));
        h.redo(s("c"));
        assert_ne!(h.newest_undo_id(), typing);
    }

    #[test]
    fn ops_travel_with_their_step_and_replay_can_change_them() {
        let mut h: History<String, &str> = History::new(10);
        let t0 = Instant::now();
        h.record(EditKind::Other, t0, || s("a"));
        h.set_op("stash");
        h.record(EditKind::Other, t0, || s("b"));
        let mut seen = Vec::new();
        let undone = h.undo_with(s("c"), |op| {
            seen.push(op);
            op
        });
        assert_eq!(undone, Some(s("b")));
        let undone = h.undo_with(s("b"), |op| {
            seen.push(op);
            Some("changed")
        });
        assert_eq!(undone, Some(s("a")));
        assert_eq!(seen, vec![None, Some("stash")]);
        seen.clear();
        h.redo_with(s("a"), |op| {
            seen.push(op);
            op
        });
        h.undo_with(s("b"), |op| {
            seen.push(op);
            op
        });
        assert_eq!(seen, vec![Some("changed"), Some("changed")]);
    }

    #[test]
    fn limit_drops_oldest_steps() {
        let mut h = History::<String>::new(2);
        let t0 = Instant::now();
        for v in ["a", "b", "c"] {
            h.record(EditKind::Other, t0, || s(v));
        }
        assert_eq!(h.undo(s("d")), Some(s("c")));
        assert_eq!(h.undo(s("c")), Some(s("b")));
        assert_eq!(h.undo(s("b")), None);
    }
}
