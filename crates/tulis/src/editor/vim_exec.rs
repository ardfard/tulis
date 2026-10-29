//! Executes vim commands against the editor.

use std::ops::Range;

use gpui::{Context, Window};
use tulis_core::history::EditKind;
use tulis_core::motion as mv;
use tulis_core::vim::{InsertAt, Mode, Motion, Operator, Vim, VimCommand, VimKey};
use tulis_core::{DocPos, Selection};

use super::Editor;
use crate::actions::ToggleVim;

impl Editor {
    pub(crate) fn vim_key(&mut self, key: VimKey, window: &mut Window, cx: &mut Context<Self>) {
        let Some(vim) = self.vim.as_mut() else { return };
        let commands = vim.feed(key);
        for command in commands {
            self.exec_vim(command, window, cx);
        }
        self.clamp_for_normal_mode();
        cx.notify();
    }

    pub(super) fn toggle_vim(&mut self, _: &ToggleVim, _: &mut Window, cx: &mut Context<Self>) {
        self.vim = if self.vim.is_some() {
            None
        } else {
            Some(Vim::new())
        };
        self.clamp_for_normal_mode();
        let state = if self.vim.is_some() {
            "Vim mode on"
        } else {
            "Vim mode off"
        };
        self.set_notice(state, cx);
    }

    /// In Normal mode the cursor sits on a character, never past the end of a non-empty paragraph.
    fn clamp_for_normal_mode(&mut self) {
        if self.vim_mode() != Some(Mode::Normal) {
            return;
        }
        let head = self.selection.head;
        let len = self.doc.paragraphs[head.para].len();
        if len > 0 && head.offset >= len {
            let pos = mv::prev_grapheme(&self.doc, DocPos::new(head.para, len));
            self.selection = Selection::caret(pos);
        }
    }

    /// The next grapheme, but never across a paragraph boundary.
    fn next_in_para(&self, pos: DocPos) -> DocPos {
        let next = mv::next_grapheme(&self.doc, pos);
        if next.para == pos.para { next } else { pos }
    }

    fn motion_target(&self, from: DocPos, motion: Motion, count: usize) -> DocPos {
        let mut pos = from;
        for _ in 0..count.max(1) {
            pos = match motion {
                Motion::Left if pos.offset == 0 => pos,
                Motion::Left => mv::prev_grapheme(&self.doc, pos),
                Motion::Right => self.next_in_para(pos),
                Motion::Up | Motion::Down => {
                    let lines = if motion == Motion::Up { -1 } else { 1 };
                    match self.current_layout() {
                        Some(layout) => {
                            let goal = self.goal_x.unwrap_or_else(|| layout.x_for(from));
                            layout.vertical(pos, lines, goal)
                        }
                        None => pos,
                    }
                }
                Motion::WordForward => mv::next_word_start(&self.doc, pos),
                Motion::WordBackward => mv::prev_word_start(&self.doc, pos),
                Motion::WordEnd => mv::word_end(&self.doc, pos),
                Motion::LineStart => DocPos::new(pos.para, self.line_bounds(pos).0),
                Motion::FirstNonBlank => {
                    let (start, end) = self.line_bounds(pos);
                    let text = self.doc.paragraphs[pos.para].text();
                    let line = &text[start..end];
                    DocPos::new(pos.para, start + line.len() - line.trim_start().len())
                }
                Motion::LineEnd => DocPos::new(pos.para, self.line_bounds(pos).1),
                Motion::DocStart => DocPos::default(),
                Motion::DocEnd => DocPos::new(self.doc.paragraphs.len() - 1, 0),
                Motion::ParagraphForward => mv::next_paragraph(&self.doc, pos),
                Motion::ParagraphBackward => mv::prev_paragraph(&self.doc, pos),
            };
        }
        pos
    }

    /// Range and register text for whole paragraphs `first..=last`. For `change`, the (now empty)
    /// paragraph is kept to type into.
    fn paragraph_span(&self, first: usize, last: usize, change: bool) -> (Range<DocPos>, String) {
        let n = self.doc.paragraphs.len();
        let last = last.min(n - 1);
        let text = (first..=last)
            .map(|i| self.doc.paragraphs[i].text())
            .collect::<Vec<_>>()
            .join("\n");
        let last_len = self.doc.paragraphs[last].len();
        let range = if change {
            DocPos::new(first, 0)..DocPos::new(last, last_len)
        } else if last + 1 < n {
            DocPos::new(first, 0)..DocPos::new(last + 1, 0)
        } else if first > 0 {
            DocPos::new(first - 1, self.doc.paragraphs[first - 1].len())
                ..DocPos::new(last, last_len)
        } else {
            DocPos::new(0, 0)..self.doc.end_pos()
        };
        (range, text)
    }

    fn apply_operator(
        &mut self,
        op: Operator,
        range: Range<DocPos>,
        register: String,
        linewise: bool,
        cx: &mut Context<Self>,
    ) {
        self.register = register;
        self.register_linewise = linewise;
        match op {
            Operator::Yank => self.set_selection(Selection::caret(range.start), cx),
            Operator::Delete | Operator::Change => self.delete_range(range, EditKind::Other, cx),
        }
    }

    fn exec_vim(&mut self, command: VimCommand, window: &mut Window, cx: &mut Context<Self>) {
        let head = self.selection.head;
        match command {
            VimCommand::Move { motion, count } => {
                let goal = self.goal_x;
                let target = self.motion_target(head, motion, count);
                self.move_to(target, false, cx);
                if matches!(motion, Motion::Up | Motion::Down) {
                    self.goal_x = goal.or_else(|| self.current_layout().map(|l| l.x_for(head)));
                }
            }
            VimCommand::Operate { op, motion, count } => {
                let motion = if op == Operator::Change && motion == Motion::WordForward {
                    Motion::WordEnd
                } else {
                    motion
                };
                let target = self.motion_target(head, motion, count);
                let range = if target < head {
                    target..head
                } else if motion == Motion::WordEnd {
                    head..self.next_in_para(target)
                } else {
                    head..target
                };
                let register = self.doc.text_in(range.clone());
                self.apply_operator(op, range, register, false, cx);
            }
            VimCommand::OperateParagraphs { op, count } => {
                let (range, text) = self.paragraph_span(
                    head.para,
                    head.para + count.max(1) - 1,
                    op == Operator::Change,
                );
                self.apply_operator(op, range, text, true, cx);
            }
            VimCommand::OperateSelection { op, linewise } => {
                let selected = self.selection.range();
                let (range, text) = if linewise {
                    self.paragraph_span(
                        selected.start.para,
                        selected.end.para,
                        op == Operator::Change,
                    )
                } else {
                    let range = selected.start..self.next_in_para(selected.end);
                    let text = self.doc.text_in(range.clone());
                    (range, text)
                };
                self.selection = Selection::caret(selected.start);
                self.apply_operator(op, range, text, linewise, cx);
            }
            VimCommand::DeleteChars { count } => {
                let mut end = head;
                for _ in 0..count.max(1) {
                    end = self.next_in_para(end);
                }
                if end > head {
                    let register = self.doc.text_in(head..end);
                    self.apply_operator(Operator::Delete, head..end, register, false, cx);
                }
            }
            VimCommand::Put { before, count } => self.vim_put(before, count, cx),
            VimCommand::EnterInsert(at) => self.vim_enter_insert(at, cx),
            VimCommand::BeginVisual => self.selection = Selection::caret(head),
            VimCommand::EndVisual => self.set_selection(Selection::caret(head), cx),
            VimCommand::ExitInsert => {
                self.finish_alt_edit();
                let head = self.selection.head;
                if head.offset > 0 {
                    let pos = mv::prev_grapheme(&self.doc, head);
                    self.set_selection(Selection::caret(pos), cx);
                }
            }
            VimCommand::Undo { count } => self.undo_steps(count, cx),
            VimCommand::Redo { count } => self.redo_steps(count, cx),
            VimCommand::CycleAlt { forward, count } => {
                for _ in 0..count.max(1) {
                    self.cycle(forward, cx);
                }
            }
            VimCommand::ToggleGhost => self.toggle_ghost_here(cx),
        }
        let _ = window;
    }

    fn vim_put(&mut self, before: bool, count: usize, cx: &mut Context<Self>) {
        if self.register.is_empty() {
            return;
        }
        let head = self.selection.head;
        let separator = if self.register_linewise { "\n" } else { "" };
        let text = vec![self.register.clone(); count.max(1)].join(separator);
        if self.register_linewise {
            let len = self.doc.paragraphs[head.para].len();
            let (pos, insert, target) = if before {
                (DocPos::new(head.para, 0), format!("{text}\n"), head.para)
            } else {
                (
                    DocPos::new(head.para, len),
                    format!("\n{text}"),
                    head.para + 1,
                )
            };
            self.selection = Selection::caret(pos);
            self.insert_text(&insert, cx);
            self.selection = Selection::caret(DocPos::new(target, 0));
        } else {
            let pos = if before {
                head
            } else {
                self.next_in_para(head)
            };
            self.selection = Selection::caret(pos);
            self.insert_text(&text, cx);
            let end = self.selection.head;
            self.selection = Selection::caret(mv::prev_grapheme(&self.doc, end));
        }
    }

    fn vim_enter_insert(&mut self, at: InsertAt, cx: &mut Context<Self>) {
        let head = self.selection.head;
        match at {
            InsertAt::Cursor => {}
            InsertAt::After => {
                let pos = self.next_in_para(head);
                self.move_to(pos, false, cx);
            }
            InsertAt::LineStart => {
                let pos = self.motion_target(head, Motion::FirstNonBlank, 1);
                self.move_to(pos, false, cx);
            }
            InsertAt::LineEnd => {
                let pos = self.motion_target(head, Motion::LineEnd, 1);
                self.move_to(pos, false, cx);
            }
            InsertAt::ParagraphBelow => {
                let len = self.doc.paragraphs[head.para].len();
                self.selection = Selection::caret(DocPos::new(head.para, len));
                self.insert_text("\n", cx);
            }
            InsertAt::ParagraphAbove => {
                self.selection = Selection::caret(DocPos::new(head.para, 0));
                self.insert_text("\n", cx);
                self.selection = Selection::caret(DocPos::new(head.para, 0));
            }
        }
    }
}
