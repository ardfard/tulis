//! Vim mode: a key-by-key state machine that produces abstract editor commands.

/// Upper bound for any count the machine produces, whether typed or multiplied.
pub const MAX_COUNT: usize = 10_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VimKey {
    Char(char),
    Escape,
    Ctrl(char),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Visual,
    VisualLine,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    Up,
    Down,
    WordForward,
    WordBackward,
    WordEnd,
    LineStart,
    FirstNonBlank,
    LineEnd,
    DocStart,
    DocEnd,
    ParagraphForward,
    ParagraphBackward,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operator {
    Delete,
    Change,
    Yank,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertAt {
    Cursor,
    After,
    LineStart,
    LineEnd,
    ParagraphBelow,
    ParagraphAbove,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VimCommand {
    Move {
        motion: Motion,
        count: usize,
    },
    Operate {
        op: Operator,
        motion: Motion,
        count: usize,
    },
    OperateParagraphs {
        op: Operator,
        count: usize,
    },
    OperateSelection {
        op: Operator,
        linewise: bool,
    },
    DeleteChars {
        count: usize,
    },
    Put {
        before: bool,
        count: usize,
    },
    EnterInsert(InsertAt),
    BeginVisual,
    EndVisual,
    ExitInsert,
    Undo {
        count: usize,
    },
    Redo {
        count: usize,
    },
    CycleAlt {
        forward: bool,
        count: usize,
    },
    ToggleGhost,
}

#[derive(Debug, Default)]
pub struct Vim {
    mode: Mode,
    count: Option<usize>,
    op: Option<(Operator, Option<usize>)>,
    prefix: Option<char>,
}

fn motion_for(c: char) -> Option<Motion> {
    Some(match c {
        'h' => Motion::Left,
        'l' => Motion::Right,
        'j' => Motion::Down,
        'k' => Motion::Up,
        'w' => Motion::WordForward,
        'b' => Motion::WordBackward,
        'e' => Motion::WordEnd,
        '0' => Motion::LineStart,
        '^' => Motion::FirstNonBlank,
        '$' => Motion::LineEnd,
        'G' => Motion::DocEnd,
        '}' => Motion::ParagraphForward,
        '{' => Motion::ParagraphBackward,
        _ => return None,
    })
}

fn operator_for(c: char) -> Option<Operator> {
    Some(match c {
        'd' => Operator::Delete,
        'c' => Operator::Change,
        'y' => Operator::Yank,
        _ => return None,
    })
}

impl Vim {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        self.reset();
    }

    pub fn is_visual(&self) -> bool {
        matches!(self.mode, Mode::Visual | Mode::VisualLine)
    }

    pub fn feed(&mut self, key: VimKey) -> Vec<VimCommand> {
        if self.mode == Mode::Insert {
            if key == VimKey::Escape {
                self.mode = Mode::Normal;
                return vec![VimCommand::ExitInsert];
            }
            return Vec::new();
        }
        match key {
            VimKey::Escape => {
                self.reset();
                if self.is_visual() {
                    self.mode = Mode::Normal;
                    vec![VimCommand::EndVisual]
                } else {
                    Vec::new()
                }
            }
            VimKey::Ctrl('r') => {
                let count = self.count();
                self.reset();
                vec![VimCommand::Redo { count }]
            }
            VimKey::Ctrl(_) => {
                self.reset();
                Vec::new()
            }
            VimKey::Char(c) => self.feed_char(c),
        }
    }

    fn reset(&mut self) {
        self.count = None;
        self.op = None;
        self.prefix = None;
    }

    fn count(&self) -> usize {
        self.count
            .unwrap_or(1)
            .saturating_mul(self.op.and_then(|(_, c)| c).unwrap_or(1))
            .min(MAX_COUNT)
    }

    fn feed_char(&mut self, c: char) -> Vec<VimCommand> {
        if let Some(prefix) = self.prefix.take() {
            return self.feed_prefixed(prefix, c);
        }
        if let Some(digit) = c.to_digit(10) {
            if digit != 0 || self.count.is_some() {
                let n = self
                    .count
                    .unwrap_or(0)
                    .saturating_mul(10)
                    .saturating_add(digit as usize)
                    .min(MAX_COUNT);
                self.count = Some(n);
                return Vec::new();
            }
        }
        if let Some(motion) = motion_for(c) {
            return self.motion(motion);
        }
        if matches!(c, 'g' | ']' | '[') {
            self.prefix = Some(c);
            return Vec::new();
        }
        let visual = self.is_visual();
        let linewise = self.mode == Mode::VisualLine;
        if let Some(op) = operator_for(c) {
            if visual {
                self.reset();
                self.mode = if op == Operator::Change {
                    Mode::Insert
                } else {
                    Mode::Normal
                };
                return vec![VimCommand::OperateSelection { op, linewise }];
            }
            return match self.op {
                Some((pending, _)) if pending == op => {
                    let count = self.count();
                    self.reset();
                    if op == Operator::Change {
                        self.mode = Mode::Insert;
                    }
                    vec![VimCommand::OperateParagraphs { op, count }]
                }
                Some(_) => {
                    self.reset();
                    Vec::new()
                }
                None => {
                    self.op = Some((op, self.count.take()));
                    Vec::new()
                }
            };
        }
        if self.op.is_some() {
            self.reset();
            return Vec::new();
        }
        let count = self.count();
        self.reset();
        match c {
            'x' if visual => {
                self.mode = Mode::Normal;
                vec![VimCommand::OperateSelection {
                    op: Operator::Delete,
                    linewise,
                }]
            }
            'x' => vec![VimCommand::DeleteChars { count }],
            'p' => vec![VimCommand::Put {
                before: false,
                count,
            }],
            'P' => vec![VimCommand::Put {
                before: true,
                count,
            }],
            'u' => vec![VimCommand::Undo { count }],
            'v' => self.toggle_visual(Mode::Visual),
            'V' => self.toggle_visual(Mode::VisualLine),
            'i' | 'a' | 'I' | 'A' | 'o' | 'O' if !visual => {
                self.mode = Mode::Insert;
                let at = match c {
                    'i' => InsertAt::Cursor,
                    'a' => InsertAt::After,
                    'I' => InsertAt::LineStart,
                    'A' => InsertAt::LineEnd,
                    'o' => InsertAt::ParagraphBelow,
                    _ => InsertAt::ParagraphAbove,
                };
                vec![VimCommand::EnterInsert(at)]
            }
            _ => Vec::new(),
        }
    }

    fn feed_prefixed(&mut self, prefix: char, c: char) -> Vec<VimCommand> {
        match (prefix, c) {
            ('g', 'g') => self.motion(Motion::DocStart),
            ('g', 'h') if self.op.is_none() => {
                self.reset();
                if self.is_visual() {
                    self.mode = Mode::Normal;
                    vec![VimCommand::ToggleGhost, VimCommand::EndVisual]
                } else {
                    vec![VimCommand::ToggleGhost]
                }
            }
            (']' | '[', 'a') if self.op.is_none() => {
                let count = self.count();
                self.reset();
                vec![VimCommand::CycleAlt {
                    forward: prefix == ']',
                    count,
                }]
            }
            _ => {
                self.reset();
                Vec::new()
            }
        }
    }

    fn toggle_visual(&mut self, target: Mode) -> Vec<VimCommand> {
        if self.mode == target {
            self.mode = Mode::Normal;
            vec![VimCommand::EndVisual]
        } else if self.is_visual() {
            self.mode = target;
            Vec::new()
        } else {
            self.mode = target;
            vec![VimCommand::BeginVisual]
        }
    }

    fn motion(&mut self, motion: Motion) -> Vec<VimCommand> {
        let count = self.count();
        let op = self.op.map(|(op, _)| op);
        self.reset();
        match op {
            Some(op) => {
                if op == Operator::Change {
                    self.mode = Mode::Insert;
                }
                vec![VimCommand::Operate { op, motion, count }]
            }
            None => vec![VimCommand::Move { motion, count }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Motion::*;
    use super::Operator::*;
    use super::VimCommand::*;
    use super::*;

    fn feed(vim: &mut Vim, keys: &str) -> Vec<VimCommand> {
        keys.chars()
            .flat_map(|c| vim.feed(VimKey::Char(c)))
            .collect()
    }

    #[test]
    fn motions_with_counts() {
        let mut v = Vim::new();
        assert_eq!(
            feed(&mut v, "3w"),
            vec![Move {
                motion: WordForward,
                count: 3
            }]
        );
        assert_eq!(
            feed(&mut v, "10j"),
            vec![Move {
                motion: Down,
                count: 10
            }]
        );
        assert_eq!(
            feed(&mut v, "0"),
            vec![Move {
                motion: LineStart,
                count: 1
            }]
        );
        assert_eq!(
            feed(&mut v, "$G{}"),
            vec![
                Move {
                    motion: LineEnd,
                    count: 1
                },
                Move {
                    motion: DocEnd,
                    count: 1
                },
                Move {
                    motion: ParagraphBackward,
                    count: 1
                },
                Move {
                    motion: ParagraphForward,
                    count: 1
                },
            ]
        );
    }

    #[test]
    fn operator_counts_multiply() {
        let mut v = Vim::new();
        assert_eq!(
            feed(&mut v, "2d3w"),
            vec![Operate {
                op: Delete,
                motion: WordForward,
                count: 6
            }]
        );
    }

    #[test]
    fn doubled_operator_is_paragraph_wise() {
        let mut v = Vim::new();
        assert_eq!(
            feed(&mut v, "dd"),
            vec![OperateParagraphs {
                op: Delete,
                count: 1
            }]
        );
        assert_eq!(
            feed(&mut v, "3yy"),
            vec![OperateParagraphs { op: Yank, count: 3 }]
        );
        assert_eq!(v.mode(), Mode::Normal);
        assert_eq!(
            feed(&mut v, "cc"),
            vec![OperateParagraphs {
                op: Change,
                count: 1
            }]
        );
        assert_eq!(v.mode(), Mode::Insert);
    }

    #[test]
    fn change_enters_insert_and_escape_leaves() {
        let mut v = Vim::new();
        assert_eq!(
            feed(&mut v, "cw"),
            vec![Operate {
                op: Change,
                motion: WordForward,
                count: 1
            }]
        );
        assert_eq!(v.mode(), Mode::Insert);
        assert!(feed(&mut v, "hello").is_empty());
        assert_eq!(v.feed(VimKey::Escape), vec![ExitInsert]);
        assert_eq!(v.mode(), Mode::Normal);
    }

    #[test]
    fn insert_commands() {
        let mut v = Vim::new();
        assert_eq!(feed(&mut v, "A"), vec![EnterInsert(InsertAt::LineEnd)]);
        assert_eq!(v.mode(), Mode::Insert);
        v.feed(VimKey::Escape);
        assert_eq!(
            feed(&mut v, "o"),
            vec![EnterInsert(InsertAt::ParagraphBelow)]
        );
    }

    #[test]
    fn prefixed_commands() {
        let mut v = Vim::new();
        assert_eq!(
            feed(&mut v, "gg"),
            vec![Move {
                motion: DocStart,
                count: 1
            }]
        );
        assert_eq!(feed(&mut v, "gh"), vec![ToggleGhost]);
        assert_eq!(
            feed(&mut v, "2]a"),
            vec![CycleAlt {
                forward: true,
                count: 2
            }]
        );
        assert_eq!(
            feed(&mut v, "[a"),
            vec![CycleAlt {
                forward: false,
                count: 1
            }]
        );
        assert_eq!(
            feed(&mut v, "dgg"),
            vec![Operate {
                op: Delete,
                motion: DocStart,
                count: 1
            }]
        );
        assert!(feed(&mut v, "gz").is_empty());
    }

    #[test]
    fn visual_mode() {
        let mut v = Vim::new();
        assert_eq!(feed(&mut v, "v"), vec![BeginVisual]);
        assert_eq!(v.mode(), Mode::Visual);
        assert_eq!(
            feed(&mut v, "w"),
            vec![Move {
                motion: WordForward,
                count: 1
            }]
        );
        assert_eq!(
            feed(&mut v, "d"),
            vec![OperateSelection {
                op: Delete,
                linewise: false
            }]
        );
        assert_eq!(v.mode(), Mode::Normal);
        assert_eq!(feed(&mut v, "V"), vec![BeginVisual]);
        assert_eq!(
            feed(&mut v, "y"),
            vec![OperateSelection {
                op: Yank,
                linewise: true
            }]
        );
        feed(&mut v, "v");
        assert_eq!(v.feed(VimKey::Escape), vec![EndVisual]);
        assert_eq!(v.mode(), Mode::Normal);
        feed(&mut v, "v");
        assert_eq!(feed(&mut v, "gh"), vec![ToggleGhost, EndVisual]);
        assert_eq!(v.mode(), Mode::Normal);
    }

    #[test]
    fn invalid_sequences_reset() {
        let mut v = Vim::new();
        assert!(feed(&mut v, "dx").is_empty());
        assert_eq!(feed(&mut v, "x"), vec![DeleteChars { count: 1 }]);
        assert!(feed(&mut v, "dc").is_empty());
        assert_eq!(
            feed(&mut v, "w"),
            vec![Move {
                motion: WordForward,
                count: 1
            }]
        );
    }

    #[test]
    fn huge_counts_saturate_and_cap() {
        let mut v = Vim::new();
        let nines = "9".repeat(20);
        assert!(feed(&mut v, &nines).is_empty());
        assert!(feed(&mut v, "d").is_empty());
        assert!(feed(&mut v, "2").is_empty());
        assert_eq!(
            feed(&mut v, "w"),
            vec![Operate {
                op: Delete,
                motion: WordForward,
                count: MAX_COUNT
            }]
        );
    }

    #[test]
    fn undo_redo_and_put() {
        let mut v = Vim::new();
        assert_eq!(feed(&mut v, "u"), vec![Undo { count: 1 }]);
        assert_eq!(v.feed(VimKey::Ctrl('r')), vec![Redo { count: 1 }]);
        assert_eq!(
            feed(&mut v, "2p"),
            vec![Put {
                before: false,
                count: 2
            }]
        );
        assert_eq!(
            feed(&mut v, "P"),
            vec![Put {
                before: true,
                count: 1
            }]
        );
    }
}
