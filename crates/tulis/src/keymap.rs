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
}
