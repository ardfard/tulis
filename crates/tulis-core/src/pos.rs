//! Positions and selections in a document.

use std::ops::Range;

/// A cursor position: paragraph index and byte offset into its rendered text.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocPos {
    pub para: usize,
    pub offset: usize,
}

impl DocPos {
    pub fn new(para: usize, offset: usize) -> Self {
        Self { para, offset }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    pub anchor: DocPos,
    pub head: DocPos,
}

impl Selection {
    pub fn caret(pos: DocPos) -> Self {
        Self {
            anchor: pos,
            head: pos,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// The selected range with its ends ordered.
    pub fn range(&self) -> Range<DocPos> {
        if self.reversed() {
            self.head..self.anchor
        } else {
            self.anchor..self.head
        }
    }

    pub fn reversed(&self) -> bool {
        self.head < self.anchor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_order_by_paragraph_then_offset() {
        assert!(DocPos::new(0, 9) < DocPos::new(1, 0));
        assert!(DocPos::new(1, 2) < DocPos::new(1, 3));
    }

    #[test]
    fn range_is_ordered_for_reversed_selection() {
        let sel = Selection {
            anchor: DocPos::new(1, 4),
            head: DocPos::new(0, 2),
        };
        assert!(sel.reversed());
        assert_eq!(sel.range(), DocPos::new(0, 2)..DocPos::new(1, 4));
    }
}
