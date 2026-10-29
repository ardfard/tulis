//! Key bindings. `secondary` is Cmd on macOS and Ctrl elsewhere.

use std::rc::Rc;

use gpui::{Action, App, KeyBinding, KeyBindingContextPredicate};

use crate::actions::*;

/// `KeyBinding::new` only accepts explicit modifiers in gpui-pre; `load` resolves `secondary-`
/// to Cmd or Ctrl for the host OS.
fn binding(keys: &str, action: impl Action, context: Option<&str>, cx: &App) -> KeyBinding {
    let context =
        context.map(|c| Rc::new(KeyBindingContextPredicate::parse(c).expect("valid key context")));
    KeyBinding::load(keys, Box::new(action), context, false, None, cx)
        .unwrap_or_else(|e| panic!("bad keystroke {keys:?}: {e:?}"))
}

pub fn bind(cx: &mut App) {
    let ed = Some("Editor");
    let bindings = [
        binding("backspace", Backspace, ed, cx),
        binding("delete", Delete, ed, cx),
        binding("left", Left, ed, cx),
        binding("right", Right, ed, cx),
        binding("up", Up, ed, cx),
        binding("down", Down, ed, cx),
        binding("shift-left", SelectLeft, ed, cx),
        binding("shift-right", SelectRight, ed, cx),
        binding("shift-up", SelectUp, ed, cx),
        binding("shift-down", SelectDown, ed, cx),
        binding("alt-left", WordLeft, ed, cx),
        binding("alt-right", WordRight, ed, cx),
        binding("ctrl-left", WordLeft, ed, cx),
        binding("ctrl-right", WordRight, ed, cx),
        binding("shift-alt-left", SelectWordLeft, ed, cx),
        binding("shift-alt-right", SelectWordRight, ed, cx),
        binding("ctrl-shift-left", SelectWordLeft, ed, cx),
        binding("ctrl-shift-right", SelectWordRight, ed, cx),
        binding("home", Home, ed, cx),
        binding("end", End, ed, cx),
        binding("shift-home", SelectHome, ed, cx),
        binding("shift-end", SelectEnd, ed, cx),
        binding("pageup", PageUp, ed, cx),
        binding("pagedown", PageDown, ed, cx),
        binding("ctrl-home", DocStart, ed, cx),
        binding("ctrl-end", DocEnd, ed, cx),
        binding("secondary-up", DocStart, ed, cx),
        binding("secondary-down", DocEnd, ed, cx),
        binding("enter", Newline, ed, cx),
        binding("shift-enter", Newline, ed, cx),
        binding("secondary-a", SelectAll, ed, cx),
        binding("secondary-c", Copy, ed, cx),
        binding("secondary-x", Cut, ed, cx),
        binding("secondary-v", Paste, ed, cx),
        binding("secondary-z", Undo, ed, cx),
        binding("secondary-shift-z", Redo, ed, cx),
        binding("ctrl-y", Redo, ed, cx),
        binding("secondary-d", MakeAlternate, ed, cx),
        binding("alt-up", CyclePrev, ed, cx),
        binding("alt-down", CycleNext, ed, cx),
        binding("secondary-enter", AcceptAlternate, ed, cx),
        binding("secondary-shift-backspace", DeleteAlternate, ed, cx),
        binding("secondary-g", ToggleGhost, ed, cx),
        binding("secondary-shift-o", SendToOverflow, ed, cx),
        binding("secondary-\\", ToggleOverflowPanel, None, cx),
        binding("secondary-j", AiAlternates, ed, cx),
        binding("secondary-shift-t", AiTrimParagraph, ed, cx),
        binding("secondary-shift-alt-t", AiTrimDocument, ed, cx),
        binding("tab", AcceptTrim, ed, cx),
        binding("escape", Cancel, ed, cx),
        binding("secondary-alt-v", ToggleVim, ed, cx),
        binding("secondary-k t", CycleTheme, None, cx),
        binding("secondary-s", Save, None, cx),
        binding("secondary-shift-s", SaveAs, None, cx),
        binding("secondary-o", Open, None, cx),
        binding("secondary-n", New, None, cx),
        binding("secondary-q", Quit, None, cx),
    ];
    cx.bind_keys(bindings);
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    #[gpui::test]
    fn all_bindings_load(cx: &mut TestAppContext) {
        cx.update(|cx| {
            super::bind(cx);
            assert_eq!(cx.key_bindings().borrow().bindings().len(), 57);
        });
    }
}
