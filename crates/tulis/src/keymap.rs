//! Key bindings, generated from the command table. `secondary` is Cmd on macOS and Ctrl elsewhere.

use gpui::App;

pub fn bind(cx: &mut App) {
    let bindings = crate::commands::key_bindings(cx);
    cx.bind_keys(bindings);
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use crate::actions::{CycleTheme, OpenAlternates, SendToOverflow, ToggleGhost, ToggleMarkers};
    use crate::commands::COMMANDS;

    #[gpui::test]
    fn every_table_key_is_bound(cx: &mut TestAppContext) {
        cx.update(|cx| {
            super::bind(cx);
            let keymap = cx.key_bindings();
            let keymap = keymap.borrow();
            let expected: usize = COMMANDS.iter().map(|c| c.keys.len()).sum();
            assert_eq!(keymap.bindings().len(), expected);
            assert_eq!(keymap.bindings_for_action(&CycleTheme).count(), 1);
            assert_eq!(keymap.bindings_for_action(&ToggleMarkers).count(), 1);
            // New keys plus the old ones they replace.
            assert_eq!(keymap.bindings_for_action(&OpenAlternates).count(), 2);
            assert_eq!(keymap.bindings_for_action(&ToggleGhost).count(), 2);
            assert_eq!(keymap.bindings_for_action(&SendToOverflow).count(), 2);
        });
    }

    /// gpui shows (in the context menu and every `Kbd`) the binding that wins, which is the last
    /// one bound: it must be the table's first key, not the older secondary one.
    #[gpui::test]
    fn the_first_table_key_takes_precedence(cx: &mut TestAppContext) {
        use gpui::{Action, Focusable, KeyBinding};

        use crate::actions::AiAlternates;

        cx.update(super::bind);
        let (editor, cx) = crate::editor::tests::editor_with(cx, &["I love tea"]);
        cx.update(|window, cx| {
            let focus = editor.read(cx).focus_handle(cx);
            window.focus(&focus, cx);
        });
        cx.run_until_parked();
        let checks: [(&dyn Action, &str); 4] = [
            (&OpenAlternates, "secondary-shift-a"),
            (&AiAlternates, "secondary-shift-g"),
            (&ToggleGhost, "secondary-/"),
            (&SendToOverflow, "secondary-shift-x"),
        ];
        cx.update(|window, cx| {
            let focus = editor.read(cx).focus_handle(cx);
            for (action, keys) in checks {
                let expected =
                    KeyBinding::load(keys, action.boxed_clone(), None, false, None, cx).unwrap();
                let winner = window
                    .highest_precedence_binding_for_action_in(action, &focus)
                    .unwrap_or_else(|| panic!("{} is bound", action.name()));
                assert_eq!(
                    winner.keystrokes(),
                    expected.keystrokes(),
                    "{}",
                    action.name()
                );
            }
        });
    }
}
