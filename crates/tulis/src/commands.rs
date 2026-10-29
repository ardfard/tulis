//! The one command table: key bindings, vim keys and cheat-sheet rows all come from here.

use std::rc::Rc;

use gpui::{Action, App, KeyBinding, KeyBindingContextPredicate};

use crate::actions::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    Editing,
    Alternates,
    Ghost,
    Overflow,
    Lab,
    View,
    File,
    /// Bound but not listed (cursor movement and secondary bindings).
    Hidden,
}

impl Group {
    /// Cheat-sheet sections, in order.
    pub const LISTED: [Group; 7] = [
        Group::Editing,
        Group::Alternates,
        Group::Ghost,
        Group::Overflow,
        Group::Lab,
        Group::View,
        Group::File,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Group::Editing => "Editing",
            Group::Alternates => "Alternates",
            Group::Ghost => "Ghost",
            Group::Overflow => "Overflow",
            Group::Lab => "Lab",
            Group::View => "View",
            Group::File => "File",
            Group::Hidden => "",
        }
    }
}

pub struct Command {
    /// Stable id (used by tests, the vim mapping and as the cheat-sheet row id).
    pub id: &'static str,
    pub label: &'static str,
    pub group: Group,
    /// GPUI keystrokes; the first is the one the cheat sheet shows.
    pub keys: &'static [&'static str],
    /// Key context: `Some("Editor")` for page commands, `None` for window-wide ones.
    pub context: Option<&'static str>,
    /// The vim Normal-mode keys, as typed (`<C-r>` is Ctrl+R).
    pub vim: Option<&'static str>,
    pub action: fn() -> Box<dyn Action>,
}

const ED: Option<&str> = Some("Editor");

macro_rules! cmd {
    ($id:literal, $label:literal, $group:ident, [$($key:literal),*], $ctx:expr, $vim:expr, $action:ty) => {
        Command {
            id: $id,
            label: $label,
            group: Group::$group,
            keys: &[$($key),*],
            context: $ctx,
            vim: $vim,
            action: || Box::new(<$action>::default()),
        }
    };
}

pub static COMMANDS: &[Command] = &[
    // Movement and plain editing keys: bound, not listed.
    cmd!(
        "backspace",
        "Delete backward",
        Hidden,
        ["backspace"],
        ED,
        None,
        Backspace
    ),
    cmd!(
        "delete",
        "Delete forward",
        Hidden,
        ["delete"],
        ED,
        None,
        Delete
    ),
    cmd!("left", "Left", Hidden, ["left"], ED, None, Left),
    cmd!("right", "Right", Hidden, ["right"], ED, None, Right),
    cmd!("up", "Up", Hidden, ["up"], ED, None, Up),
    cmd!("down", "Down", Hidden, ["down"], ED, None, Down),
    cmd!(
        "select_left",
        "Select left",
        Hidden,
        ["shift-left"],
        ED,
        None,
        SelectLeft
    ),
    cmd!(
        "select_right",
        "Select right",
        Hidden,
        ["shift-right"],
        ED,
        None,
        SelectRight
    ),
    cmd!(
        "select_up",
        "Select up",
        Hidden,
        ["shift-up"],
        ED,
        None,
        SelectUp
    ),
    cmd!(
        "select_down",
        "Select down",
        Hidden,
        ["shift-down"],
        ED,
        None,
        SelectDown
    ),
    cmd!(
        "word_left",
        "Word left",
        Hidden,
        ["alt-left", "ctrl-left"],
        ED,
        None,
        WordLeft
    ),
    cmd!(
        "word_right",
        "Word right",
        Hidden,
        ["alt-right", "ctrl-right"],
        ED,
        None,
        WordRight
    ),
    cmd!(
        "select_word_left",
        "Select word left",
        Hidden,
        ["shift-alt-left", "ctrl-shift-left"],
        ED,
        None,
        SelectWordLeft
    ),
    cmd!(
        "select_word_right",
        "Select word right",
        Hidden,
        ["shift-alt-right", "ctrl-shift-right"],
        ED,
        None,
        SelectWordRight
    ),
    cmd!("home", "Line start", Hidden, ["home"], ED, None, Home),
    cmd!("end", "Line end", Hidden, ["end"], ED, None, End),
    cmd!(
        "select_home",
        "Select to line start",
        Hidden,
        ["shift-home"],
        ED,
        None,
        SelectHome
    ),
    cmd!(
        "select_end",
        "Select to line end",
        Hidden,
        ["shift-end"],
        ED,
        None,
        SelectEnd
    ),
    cmd!("page_up", "Page up", Hidden, ["pageup"], ED, None, PageUp),
    cmd!(
        "page_down",
        "Page down",
        Hidden,
        ["pagedown"],
        ED,
        None,
        PageDown
    ),
    cmd!(
        "doc_start",
        "Document start",
        Hidden,
        ["ctrl-home", "secondary-up"],
        ED,
        None,
        DocStart
    ),
    cmd!(
        "doc_end",
        "Document end",
        Hidden,
        ["ctrl-end", "secondary-down"],
        ED,
        None,
        DocEnd
    ),
    cmd!(
        "newline",
        "New paragraph",
        Hidden,
        ["enter", "shift-enter"],
        ED,
        None,
        Newline
    ),
    // Editing
    cmd!(
        "select_all",
        "Select all",
        Editing,
        ["secondary-a"],
        ED,
        None,
        SelectAll
    ),
    cmd!("copy", "Copy", Editing, ["secondary-c"], ED, None, Copy),
    cmd!("cut", "Cut", Editing, ["secondary-x"], ED, None, Cut),
    cmd!("paste", "Paste", Editing, ["secondary-v"], ED, None, Paste),
    cmd!(
        "undo",
        "Undo",
        Editing,
        ["secondary-z"],
        ED,
        Some("u"),
        Undo
    ),
    cmd!(
        "redo",
        "Redo",
        Editing,
        ["secondary-shift-z", "ctrl-y"],
        ED,
        Some("<C-r>"),
        Redo
    ),
    cmd!(
        "cancel",
        "Cancel / dismiss",
        Editing,
        ["escape"],
        ED,
        None,
        Cancel
    ),
    cmd!(
        "toggle_vim",
        "Toggle vim mode",
        Editing,
        ["secondary-alt-v"],
        ED,
        None,
        ToggleVim
    ),
    // Alternates
    cmd!(
        "make_alternate",
        "Make an alternate",
        Alternates,
        ["secondary-d"],
        ED,
        None,
        MakeAlternate
    ),
    cmd!(
        "cycle_next",
        "Next alternate",
        Alternates,
        ["alt-down"],
        ED,
        Some("]a"),
        CycleNext
    ),
    cmd!(
        "cycle_prev",
        "Previous alternate",
        Alternates,
        ["alt-up"],
        ED,
        Some("[a"),
        CyclePrev
    ),
    cmd!(
        "accept_alternate",
        "Keep this alternate",
        Alternates,
        ["secondary-enter"],
        ED,
        None,
        AcceptAlternate
    ),
    cmd!(
        "delete_alternate",
        "Delete this alternate",
        Alternates,
        ["secondary-shift-backspace"],
        ED,
        None,
        DeleteAlternate
    ),
    cmd!(
        "ai_alternates",
        "AI alternates",
        Alternates,
        ["secondary-j"],
        ED,
        None,
        AiAlternates
    ),
    // Ghost
    cmd!(
        "toggle_ghost",
        "Ghost / revive",
        Ghost,
        ["secondary-g"],
        ED,
        Some("gh"),
        ToggleGhost
    ),
    // Overflow
    cmd!(
        "send_to_overflow",
        "Send to Overflow",
        Overflow,
        ["secondary-shift-o"],
        ED,
        None,
        SendToOverflow
    ),
    cmd!(
        "toggle_overflow",
        "Show / hide Overflow",
        Overflow,
        ["secondary-\\"],
        None,
        Some("gx"),
        ToggleOverflowPanel
    ),
    // Lab
    cmd!(
        "open_lab",
        "The Lab",
        Lab,
        ["secondary-l"],
        None,
        Some("gl"),
        OpenLab
    ),
    cmd!(
        "ai_trim_paragraph",
        "AI trim: paragraph",
        Lab,
        ["secondary-shift-t"],
        ED,
        None,
        AiTrimParagraph
    ),
    cmd!(
        "ai_trim_document",
        "AI trim: document",
        Lab,
        ["secondary-shift-alt-t"],
        ED,
        None,
        AiTrimDocument
    ),
    cmd!(
        "accept_trim",
        "Accept a trim",
        Lab,
        ["tab"],
        ED,
        None,
        AcceptTrim
    ),
    // View
    cmd!(
        "cycle_theme",
        "Cycle theme",
        View,
        ["secondary-k t"],
        None,
        Some("gt"),
        CycleTheme
    ),
    cmd!(
        "toggle_markers",
        "Show / hide markers",
        View,
        ["secondary-."],
        None,
        Some("gm"),
        ToggleMarkers
    ),
    cmd!(
        "toggle_preview",
        "Markdown preview",
        View,
        ["secondary-shift-m"],
        None,
        Some("gp"),
        TogglePreview
    ),
    cmd!(
        "close_preview",
        "Close preview",
        Hidden,
        ["escape"],
        Some("Preview"),
        None,
        TogglePreview
    ),
    cmd!(
        "cheat_sheet",
        "Cheat sheet",
        View,
        ["secondary-?", "secondary-shift-/"],
        None,
        Some("g?"),
        ShowCheatSheet
    ),
    // File
    cmd!("save", "Save", File, ["secondary-s"], None, None, Save),
    cmd!(
        "save_as",
        "Save as",
        File,
        ["secondary-shift-s"],
        None,
        None,
        SaveAs
    ),
    cmd!("open", "Open", File, ["secondary-o"], None, None, Open),
    cmd!("new", "New", File, ["secondary-n"], None, None, New),
    cmd!("share_x", "Share to X", File, [], None, None, ShareX),
    cmd!(
        "share_linkedin",
        "Share to LinkedIn",
        File,
        [],
        None,
        None,
        ShareLinkedIn
    ),
    cmd!("quit", "Quit", File, ["secondary-q"], None, None, Quit),
];

/// Every key binding in the table. `KeyBinding::load` (unlike `new`) resolves `secondary-`
/// to Cmd or Ctrl for the host OS.
pub fn key_bindings(cx: &App) -> Vec<KeyBinding> {
    COMMANDS
        .iter()
        .flat_map(|c| {
            let context = c.context.map(|ctx| {
                Rc::new(KeyBindingContextPredicate::parse(ctx).expect("valid key context"))
            });
            c.keys.iter().map(move |keys| {
                KeyBinding::load(keys, (c.action)(), context.clone(), false, None, cx)
                    .unwrap_or_else(|e| panic!("bad keystroke {keys:?}: {e:?}"))
            })
        })
        .collect()
}

/// Display form of a GPUI keystroke sequence: `"secondary-shift-m"` → `"Ctrl+Shift+M"`,
/// `"secondary-k t"` → `"Ctrl+K T"`.
pub fn key_label(keys: &str) -> String {
    keys.split(' ')
        .map(|stroke| {
            let parts: Vec<&str> = stroke.split('-').collect();
            // A trailing "-" means the key itself is "-" (e.g. "ctrl--").
            let (mods, key) = match parts.as_slice() {
                [mods @ .., "", ""] => (mods, "-"),
                [mods @ .., key] => (mods, *key),
                [] => (&[][..], ""),
            };
            let mut out: Vec<String> = mods
                .iter()
                .map(|m| {
                    match *m {
                        "secondary" if cfg!(target_os = "macos") => "Cmd",
                        "secondary" | "ctrl" => "Ctrl",
                        "cmd" => "Cmd",
                        "alt" => "Alt",
                        "shift" => "Shift",
                        other => other,
                    }
                    .to_string()
                })
                .collect();
            out.push(match key {
                "up" => "↑".into(),
                "down" => "↓".into(),
                "left" => "←".into(),
                "right" => "→".into(),
                "enter" => "Enter".into(),
                "escape" => "Esc".into(),
                "backspace" => "Backspace".into(),
                "tab" => "Tab".into(),
                other => other.to_uppercase(),
            });
            out.join("+")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// A command shown in the cheat sheet: label, key label (or "—") and vim keys.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheatRow {
    pub id: &'static str,
    pub label: &'static str,
    pub key: String,
    pub vim: &'static str,
}

/// The cheat sheet's sections, filtered (case-insensitively) by label, key or vim keys.
/// Empty sections are left out.
pub fn cheat_sheet(filter: &str) -> Vec<(Group, Vec<CheatRow>)> {
    let filter = filter.trim().to_lowercase();
    Group::LISTED
        .into_iter()
        .filter_map(|group| {
            let rows: Vec<CheatRow> = COMMANDS
                .iter()
                .filter(|c| c.group == group)
                .map(|c| CheatRow {
                    id: c.id,
                    label: c.label,
                    key: c.keys.first().map_or_else(|| "—".into(), |k| key_label(k)),
                    vim: c.vim.unwrap_or(""),
                })
                .filter(|row| {
                    filter.is_empty()
                        || row.label.to_lowercase().contains(&filter)
                        || row.key.to_lowercase().contains(&filter)
                        || row.vim.to_lowercase().contains(&filter)
                })
                .collect();
            (!rows.is_empty()).then_some((group, rows))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use tulis_core::vim::{Vim, VimCommand, VimKey};

    use super::*;

    /// Feed vim keys written as in the table (`<C-r>` is Ctrl+R).
    fn feed(keys: &str) -> Vec<VimCommand> {
        let mut vim = Vim::new();
        if let Some(c) = keys.strip_prefix("<C-").and_then(|k| k.strip_suffix('>')) {
            return vim.feed(VimKey::Ctrl(c.chars().next().unwrap()));
        }
        keys.chars()
            .flat_map(|c| vim.feed(VimKey::Char(c)))
            .collect()
    }

    /// Which table command a vim command performs.
    fn vim_id(command: &VimCommand) -> Option<&'static str> {
        Some(match command {
            VimCommand::Undo { .. } => "undo",
            VimCommand::Redo { .. } => "redo",
            VimCommand::CycleAlt { forward: true, .. } => "cycle_next",
            VimCommand::CycleAlt { forward: false, .. } => "cycle_prev",
            VimCommand::ToggleGhost => "toggle_ghost",
            VimCommand::CycleTheme => "cycle_theme",
            VimCommand::ToggleMarkers => "toggle_markers",
            VimCommand::TogglePreview => "toggle_preview",
            VimCommand::CheatSheet => "cheat_sheet",
            VimCommand::Lab => "open_lab",
            VimCommand::ToggleOverflow => "toggle_overflow",
            // Pure motions and editing grammar: no command-table row.
            VimCommand::Move { .. }
            | VimCommand::Operate { .. }
            | VimCommand::OperateParagraphs { .. }
            | VimCommand::OperateSelection { .. }
            | VimCommand::DeleteChars { .. }
            | VimCommand::Put { .. }
            | VimCommand::EnterInsert(_)
            | VimCommand::BeginVisual
            | VimCommand::EndVisual
            | VimCommand::ExitInsert => return None,
        })
    }

    #[test]
    fn vim_keys_in_the_table_do_what_the_table_says() {
        for c in COMMANDS {
            if let Some(keys) = c.vim {
                let ids: Vec<_> = feed(keys).iter().filter_map(vim_id).collect();
                assert_eq!(ids, vec![c.id], "vim keys {keys:?} of {}", c.id);
            }
        }
    }

    /// The other direction: every user-facing vim command (the exhaustive `vim_id` match above
    /// fails to compile for a new variant) has a table row that lists vim keys.
    #[test]
    fn user_facing_vim_commands_have_table_rows_with_vim_keys() {
        let commands = [
            VimCommand::Undo { count: 1 },
            VimCommand::Redo { count: 1 },
            VimCommand::CycleAlt {
                forward: true,
                count: 1,
            },
            VimCommand::CycleAlt {
                forward: false,
                count: 1,
            },
            VimCommand::ToggleGhost,
            VimCommand::CycleTheme,
            VimCommand::ToggleMarkers,
            VimCommand::TogglePreview,
            VimCommand::CheatSheet,
            VimCommand::Lab,
            VimCommand::ToggleOverflow,
        ];
        for command in &commands {
            let id = vim_id(command).expect("user-facing command has an id");
            let row = COMMANDS.iter().find(|c| c.id == id);
            assert!(
                row.is_some_and(|c| c.vim.is_some()),
                "{command:?} maps to {id}, which needs a table row with vim keys"
            );
        }
    }

    #[test]
    fn ids_are_unique() {
        let mut seen = HashSet::new();
        for c in COMMANDS {
            assert!(seen.insert(c.id), "duplicate id {}", c.id);
        }
        let theme = COMMANDS.iter().find(|c| c.id == "cycle_theme").unwrap();
        assert_eq!(theme.keys, &["secondary-k t"]);
    }

    #[test]
    fn key_labels() {
        let ctrl = if cfg!(target_os = "macos") {
            "Cmd"
        } else {
            "Ctrl"
        };
        assert_eq!(key_label("secondary-shift-m"), format!("{ctrl}+Shift+M"));
        assert_eq!(key_label("secondary-k t"), format!("{ctrl}+K T"));
        assert_eq!(key_label("alt-up"), "Alt+↑");
        assert_eq!(key_label("secondary-\\"), format!("{ctrl}+\\"));
        assert_eq!(key_label("escape"), "Esc");
        assert_eq!(key_label("ctrl--"), "Ctrl+-");
    }

    #[test]
    fn cheat_sheet_lists_groups_in_order_and_filters() {
        let all = cheat_sheet("");
        let groups: Vec<Group> = all.iter().map(|(g, _)| *g).collect();
        assert_eq!(groups, Group::LISTED.to_vec());
        let theme = cheat_sheet("THEME");
        assert_eq!(theme.len(), 1);
        assert_eq!(theme[0].0, Group::View);
        assert_eq!(theme[0].1[0].vim, "gt");
        assert_eq!(cheat_sheet("gx")[0].1[0].label, "Show / hide Overflow");
        assert_eq!(cheat_sheet("linkedin")[0].1[0].key, "—");
        assert!(cheat_sheet("zzzz").is_empty());
    }
}
