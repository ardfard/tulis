# Tulis Redesign, Part 1: Foundation + Look & Chrome

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task by task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move Tulis to `gpui-pre 0.3.10` + `gpui-component 0.8.1`, then give it the redesigned look: four theme presets (Midnight by default) with a monospace font, a centered column with edge fades, a bold `# ` title, dotted alternate markers, a paragraph-version bar, a header (counts, toggles, cheat sheet, auto-dim), a Markdown preview, a cheat sheet built from one command table, a footer (save, open, share, LAB, XYZ) and toasts. Every new command also gets a vim `g` binding.

**Architecture:** `tulis-core` gains pure modules: config fields, the vim `g`-view commands, `stats` (word/char counts and the title marker) and `share` (share text and intent URLs). The app gains `theme.rs` (presets, font fallback, gpui-component sync), `commands.rs` (the single command table that feeds the keymap, the cheat sheet and a vim drift test), `toast.rs`, `header_dim.rs`, `editor/markers.rs` (marker geometry), `chrome.rs` (header, footer, toasts, share) and `cheat_sheet.rs` (the dialog). The window root is `gpui_component::Root` wrapping `Workspace`. Editor-only state (theme, markers flag, toasts, cycled group) lives on `Editor`, so the editor's own tests run without gpui-component's globals. Workspace chrome uses gpui-component widgets, so workspace tests call `gpui_component::init` first.

**Tech Stack:** Rust 1.98 (edition 2024), `gpui-pre =0.3.10` (imported as `gpui`), `gpui-pre-platform =0.3.10` (imported as `gpui_platform`), `gpui-component 0.8.1`, `gpui-kit-assets 0.8.1`, plus the existing `tulis-core` dependencies.

**Spec:** `docs/superpowers/specs/2026-10-29-tulis-redesign-design.md`, sections 0 and A. Builds on `docs/superpowers/specs/2026-10-28-tulis-design.md`. Read both before starting a task.

## Global Constraints

- Version control is **jj**. Commit with `jj describe -m "<message>" && jj new`. Never run `git commit`.
- Every commit message ends with a blank line and then `Co-Authored-By: Claude <model> <noreply@anthropic.com>`, with your model name filled in (for example `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`).
- Run `cargo fmt --all` before every commit.
- No new clippy warnings. After Task 1, `cargo clippy --workspace --all-targets` reports exactly: `tulis-core (lib)` 3 warnings, `tulis (bin "tulis")` 3 warnings and `tulis (bin "tulis" test)` 4 warnings (all from v1 code). No count may go up. Dead-code warnings count too, so don't add an item before the task that uses it.
- Every task ends with `cargo test --workspace` passing. Task 1 must keep all 156 existing tests passing, plus the keymap test it adds.
- `tulis-core` must not depend on `gpui`.
- Positions are byte offsets into a paragraph's rendered text (ghosts included), always on char boundaries. Only the input handler converts to and from UTF-16.
- Don't search `.devenv` or `.direnv`.
- **gpui-pre API facts** (checked against `~/.cargo/registry/src/*/gpui-pre-0.3.10`):
  - `KeyBinding::new` rejects `secondary-` and panics (`src/keymap/binding.rs:35`). Use `KeyBinding::load(keys, Box<dyn Action>, Option<Rc<KeyBindingContextPredicate>>, false, None, cx)`, which resolves `secondary-` for the host OS.
  - There is no `Application::new`. Use `gpui_platform::application()` from `gpui-pre-platform` (`src/lib.rs:13` of that crate). `Application::with_assets(impl AssetSource)` registers icons (`src/app.rs:232`).
  - `Window::focus(&mut self, &FocusHandle, &mut App)` takes `cx` (`src/window.rs:2418`). `App::on_window_closed` callbacks take `(&mut App, WindowId)` (`src/app.rs:2886`). `ScrollHandle::max_offset()` returns a `Point<Pixels>` (`src/elements/div.rs:4328`).
  - `Window::dispatch_action(Box<dyn Action>, &mut App)` is deferred and goes to the focused element (`src/window.rs:2558`).
  - `linear_gradient(angle, from, to)`: 0° points up and angles go clockwise, so 180° runs top to bottom (`src/color.rs:865`). `outline(bounds, color, BorderStyle)` draws a 1px border (`src/window.rs:7888`). `PaintQuad::corner_radii` (`src/window.rs:7824`).
  - Derived `Debug` and `Clone` don't count as reading a field, but derived `PartialEq` does. That's why `Theme` can hold colors that later parts read.
- **gpui-component API facts** (checked against `~/.cargo/registry/src/*/gpui-component-0.8.1` and `gpui-base-0.8.1`):
  - `gpui_component::init(cx)` installs the theme, the root plugin (dialogs and notifications) and the input bindings (`src/lib.rs`). `gpui_component::Root` is `gpui_base::Root`: `Root::new(view, window, cx)`, `root.view() -> &AnyView` (`gpui-base/src/root.rs:175`).
  - `cx.theme()` and every widget panic unless `init` ran. Workspace tests must call `gpui_component::init` first. Editor tests don't need to.
  - Theme: `gpui_component::Theme::{global, change(mode, None, cx), update(cx, |t| …)}`, `ThemeMode::{Light, Dark}`. `Theme` derefs to `ThemeColor`, which has fields such as `background`, `foreground`, `border`, `primary`, `popover` and `overlay` (`src/theme/mod.rs`, `src/theme/theme_color.rs`).
  - Dialogs: `use gpui_component::WindowExt;` then `window.open_dialog(cx, |dialog, window, cx| dialog.title(..).w(px(..)).child(..))`, plus `window.has_active_dialog(cx)` and `window.close_dialog(cx)` (`src/window_ext.rs`, `src/dialog/dialog.rs`). These need a `Root` window. The builder runs again on every rebuild, so create entities before opening.
  - Buttons: `gpui_component::button::{Button, ButtonVariants}`, `Button::new(id).ghost().xsmall().icon(..).label(..).tooltip(..).selected(bool).on_click(|&ClickEvent, &mut Window, &mut App| ..)`. `xsmall` comes from `gpui_component::Sizable` and `selected` from `gpui_component::Selectable` (`src/button/button.rs`).
  - Input: `gpui_component::input::{Input, InputState, InputEvent}`, `InputState::new(window, cx).placeholder(..)`, `.value() -> SharedString`, `.set_value(v, window, cx)` (doesn't emit `Change`), `InputEvent::Change` (`gpui-base/src/input/base/state.rs`).
  - Markdown: `gpui_component::text::TextView::markdown(id, text).selectable(true)` (`src/text/compat.rs:68`).
  - Icons: the legacy `gpui_component::IconName` has the 101 default icons (`Eye`, `EyeOff`, `FileText`, `FolderOpen`, …). `gpui_kit_assets::IconName` is the full Lucide catalog. Icons outside the default set (`Keyboard`, `Save`, `Share2`) must be embedded with `gpui_kit_assets::icon_assets!` (`gpui-kit-assets-0.8.1/src/lib.rs:71`). There are no X or LinkedIn brand icons.
- Unknown API? Read those source files. Don't guess. `gpui-pre`'s `examples/` and gpui-component's `#[cfg(test)]` modules (for example `src/dialog/dialog.rs` tests) show working patterns.

## File Map

```
Cargo.toml                          drop [patch.crates-io] xattr (Task 1)
vendor/xattr/                       deleted (Task 1)
README.md                           config, keys, smoke checklist (Tasks 1, 13)
crates/tulis-core/src/
  lib.rs                            + pub mod share; pub mod stats;
  config.rs                         EditorConfig.theme, long_sentence_words, font default
  vim.rs                            VimCommand::{CycleTheme, ToggleMarkers, TogglePreview, CheatSheet, Lab, ToggleOverflow}
  stats.rs            (new)         Counts, document_counts, selection_counts, title_marker_len
  share.rs            (new)         ShareTarget, share_text, share_url, percent_encode, trim_to_chars
crates/tulis/
  Cargo.toml                        gpui-pre, gpui-pre-platform, gpui-component, gpui-kit-assets
  src/main.rs                       gpui_platform::application, assets, gpui_component::init, Root
  src/actions.rs                    + CycleTheme, ToggleMarkers, TogglePreview, ShowCheatSheet, OpenLab, ShareX, ShareLinkedIn
  src/keymap.rs                     bindings from the command table
  src/commands.rs     (new)         COMMANDS table, key_bindings, key_label, cheat_sheet rows
  src/theme.rs        (rewritten)   Theme, ThemeId presets, theme_from_config, resolve_font, apply_to_components
  src/toast.rs        (new)         ToastQueue, toast_opacity
  src/header_dim.rs   (new)         HeaderDim auto-dim state
  src/assets.rs       (new)         icon AssetSource (defaults + Keyboard, Save, Share2)
  src/chrome.rs       (new)         impl Workspace: header, footer, toasts, share, window title
  src/cheat_sheet.rs  (new)         CheatSheet view + Workspace::show_cheat_sheet
  src/runs.rs                       title font and marker, no alt underlines
  src/workspace.rs                  page column + fades, preview, handlers, tests
  src/editor/mod.rs                 theme, font_family, show_markers, toasts, cycled, EditorEvent
  src/editor/element.rs             GUTTER, line height 1.8, theme colors, marker painting
  src/editor/layout.rs              LayoutCache.version_handles
  src/editor/markers.rs (new)       marker geometry (pure)
  src/editor/features.rs            cycled group on cycle, next_version
  src/editor/actions.rs             focus fix, version-handle click
  src/editor/vim_exec.rs            new vim commands
  src/editor/ai.rs                  ai_trim_document is pub(crate)
  src/editor/tests.rs               new editor tests
```

---

### Task 1: Migrate to gpui-pre 0.3.10 and gpui-component 0.8.1

No behavior change. All 156 tests stay green, and the app still starts.

**Files:**
- Modify: `Cargo.toml`, `crates/tulis/Cargo.toml`, `crates/tulis/src/main.rs`, `crates/tulis/src/keymap.rs`, `crates/tulis/src/editor/actions.rs`, `crates/tulis/src/editor/element.rs`, `crates/tulis/src/workspace.rs`, `README.md`
- Delete: `vendor/xattr/`

**Interfaces:**
- Consumes: nothing new.
- Produces:
  - `gpui` now means `gpui-pre 0.3.10`. `gpui_platform::application() -> gpui::Application`.
  - `keymap::bind(cx: &mut App)` (same signature), built on `fn binding(keys: &str, action: impl Action, context: Option<&str>, cx: &App) -> KeyBinding`.
  - The window root is `gpui_component::Root` wrapping `Entity<Workspace>`.

- [ ] **Step 1: Write the failing keymap test**

Append to `crates/tulis/src/keymap.rs`:

```rust
#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    #[gpui::test]
    fn all_bindings_load(cx: &mut TestAppContext) {
        cx.update(|cx| {
            super::bind(cx);
            assert_eq!(cx.key_bindings().borrow().bindings().len(), 56);
        });
    }
}
```

- [ ] **Step 2: Swap the dependencies**

`crates/tulis/Cargo.toml`, `[dependencies]`: replace `gpui = "0.2.2"` with:

```toml
gpui = { package = "gpui-pre", version = "=0.3.10" }
gpui-component = "0.8.1"
gpui_platform = { package = "gpui-pre-platform", version = "=0.3.10", features = ["wayland", "x11", "font-kit"] }
```

`[dev-dependencies]`: replace the gpui line with:

```toml
gpui = { package = "gpui-pre", version = "=0.3.10", features = ["test-support"] }
```

In the workspace `Cargo.toml`, delete the comment `# gpui 0.2.2 pulls xattr 0.2.3, …`, the `[patch.crates-io]` header and the `xattr = { path = "vendor/xattr" }` line.

- [ ] **Step 3: Run the build (expect 5 errors)**

Run: `cargo test --workspace --no-run`
Expected: compile errors in `editor/actions.rs` (`window.focus` takes 2 args), `workspace.rs` (same), `editor/element.rs` (no field `height` on `Point<Pixels>`), `main.rs` (`Application::new` is gone, and the `on_window_closed` closure takes 2 args).

- [ ] **Step 4: Fix the five errors**

`crates/tulis/src/editor/actions.rs`, in `on_mouse_down`:

```rust
        window.focus(&self.focus_handle, cx);
```

`crates/tulis/src/workspace.rs`, `focus_editor`:

```rust
    pub fn focus_editor(&self, window: &mut Window, cx: &mut App) {
        let handle = self.editor.focus_handle(cx);
        window.focus(&handle, cx);
    }
```

`crates/tulis/src/editor/element.rs`, in `scroll_into_view`:

```rust
    offset.y = clamp_px(offset.y, -scroll.max_offset().y, px(0.));
```

Replace `crates/tulis/src/main.rs` with:

```rust
mod actions;
mod editor;
mod keymap;
mod runs;
mod theme;
mod utf16;
mod workspace;

use std::path::PathBuf;

use gpui::{App, Bounds, TitlebarOptions, WindowBounds, WindowOptions, prelude::*, px, size};
use gpui_component::Root;
use tulis_core::config::Config;

use workspace::Workspace;

fn main() {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    gpui_platform::application().run(move |cx: &mut App| {
        gpui_component::init(cx);
        let (config, warning) = Config::load(Config::default_path().as_deref());
        keymap::bind(cx);
        let bounds = Bounds::centered(None, size(px(1100.), px(820.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Tulis".into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let window = cx
            .open_window(options, |window, cx| {
                let workspace = cx.new(|cx| Workspace::new(config, warning, path, window, cx));
                workspace.update(cx, |ws, cx| ws.focus_editor(window, cx));
                cx.new(|cx| Root::new(workspace, window, cx))
            })
            .expect("failed to open the Tulis window");
        window.update(cx, |_, _, cx| cx.activate(true)).ok();
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
    });
}
```

- [ ] **Step 5: Run the tests (expect the keymap test to fail)**

Run: `cargo test --workspace`
Expected: the build succeeds and `keymap::tests::all_bindings_load` panics with `InvalidKeystrokeError { keystroke: "secondary-up" }`. Without the next step, the real app would panic the same way at startup.

- [ ] **Step 6: Build the bindings with `KeyBinding::load`**

In `crates/tulis/src/keymap.rs`, replace the imports and the top of `bind`:

```rust
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
```

Rewrite every entry mechanically: `KeyBinding::new("…", Action, ctx),` becomes `binding("…", Action, ctx, cx),`, where `ctx` is `ed` or `None`. Close with `];` and then `cx.bind_keys(bindings);` instead of `]);`. All 56 existing bindings stay, unchanged.

- [ ] **Step 7: Remove the vendored xattr if nothing needs it**

Run: `cargo tree -i xattr`
Expected: `error: package ID specification \`xattr\` did not match any packages`. If so, run `rm -r vendor/xattr` (and `rmdir vendor` if it's empty). If xattr is still in the tree, keep both the patch and the vendor directory, and report that.

In `README.md`, delete the paragraph that starts with ``"`vendor/xattr` is `xattr 0.2.3`…"`` and the sentence about `vendor/xattr` in the License section.

- [ ] **Step 8: Run the tests and clippy (expect pass)**

Run: `cargo test --workspace`
Expected: 46 app tests and 111 core tests pass.

Run: `cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: `tulis-core (lib)` 3, `tulis (bin "tulis")` 3, `tulis (bin "tulis" test)` 4. This is the baseline for all later tasks.

Run: `cargo build && timeout 5 target/debug/tulis /tmp/smoke.md; echo $?`
Expected: `124`. The window stayed open until the timeout, with no panic on stderr.

- [ ] **Step 9: Commit**

```bash
cargo fmt --all
jj describe -m "chore: migrate to gpui-pre 0.3.10 + gpui-component 0.8.1

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 2: Core config fields and vim `g` view commands

**Files:**
- Modify: `crates/tulis-core/src/config.rs`, `crates/tulis-core/src/vim.rs`, `crates/tulis/src/editor/vim_exec.rs`

**Interfaces:**
- Produces:
  - `EditorConfig { vim_mode, font_family, font_size, pub theme: String /* "midnight" */, pub long_sentence_words: usize /* 30 */ }`. The `font_family` default becomes `"IBM Plex Mono"`.
  - `VimCommand::{CycleTheme, ToggleMarkers, TogglePreview, CheatSheet, Lab, ToggleOverflow}`, fed by `gt`, `gm`, `gp`, `g?`, `gl` and `gx` in Normal and Visual mode when no operator is pending. These leave the mode unchanged.

- [ ] **Step 1: Write the failing tests**

In `config.rs` tests, change `defaults_match_spec`'s font assertion and add two lines:

```rust
        assert_eq!(c.editor.font_family, "IBM Plex Mono");
        assert_eq!(c.editor.font_size, 18.0);
        assert_eq!(c.editor.theme, "midnight");
        assert_eq!(c.editor.long_sentence_words, 30);
```

Add this test:

```rust
    #[test]
    fn theme_and_long_sentence_words_are_read() {
        let c = Config::parse("[editor]\ntheme = \"Sepia\"\nlong_sentence_words = 25\n").unwrap();
        assert_eq!(c.editor.theme, "Sepia");
        assert_eq!(c.editor.long_sentence_words, 25);
        assert_eq!(c.editor.font_family, "IBM Plex Mono");
    }
```

Append to the `vim.rs` tests module:

```rust
    #[test]
    fn g_view_commands() {
        let mut v = Vim::new();
        assert_eq!(feed(&mut v, "gt"), vec![CycleTheme]);
        assert_eq!(feed(&mut v, "gm"), vec![ToggleMarkers]);
        assert_eq!(feed(&mut v, "gp"), vec![TogglePreview]);
        assert_eq!(feed(&mut v, "g?"), vec![CheatSheet]);
        assert_eq!(feed(&mut v, "gl"), vec![Lab]);
        assert_eq!(feed(&mut v, "gx"), vec![ToggleOverflow]);
        assert_eq!(v.mode(), Mode::Normal);
    }

    #[test]
    fn g_view_commands_need_no_pending_operator_and_keep_visual_mode() {
        let mut v = Vim::new();
        assert_eq!(feed(&mut v, "dgt"), vec![]);
        assert_eq!(
            feed(&mut v, "w"),
            vec![Move {
                motion: WordForward,
                count: 1
            }]
        );
        feed(&mut v, "v");
        assert_eq!(feed(&mut v, "gm"), vec![ToggleMarkers]);
        assert_eq!(v.mode(), Mode::Visual);
        assert_eq!(feed(&mut v, "gz"), vec![]);
        assert_eq!(v.mode(), Mode::Visual);
    }
```

- [ ] **Step 2: Run them (expect fail)**

Run: `cargo test -p tulis-core config vim`
Expected: compile errors (no field `theme`, no variant `CycleTheme`).

- [ ] **Step 3: Implement**

`config.rs`: add the fields and their defaults:

```rust
pub struct EditorConfig {
    pub vim_mode: bool,
    pub font_family: String,
    pub font_size: f32,
    /// Theme preset name: "midnight", "paper", "sepia" or "black" (case-insensitive).
    pub theme: String,
    /// Sentences with more words than this are marked by the Lab's long-sentence check.
    pub long_sentence_words: usize,
}
```

```rust
            vim_mode: false,
            font_family: "IBM Plex Mono".into(),
            font_size: 18.0,
            theme: "midnight".into(),
            long_sentence_words: 30,
```

`vim.rs`: add the variants at the end of `VimCommand` (after `ToggleGhost`):

```rust
    /// `gt`: next theme preset.
    CycleTheme,
    /// `gm`: show or hide alternate and version markers.
    ToggleMarkers,
    /// `gp`: Markdown preview.
    TogglePreview,
    /// `g?`: cheat sheet.
    CheatSheet,
    /// `gl`: the Lab.
    Lab,
    /// `gx`: show or hide the Overflow panel.
    ToggleOverflow,
}

/// The view command for `g` + `c`, if there is one.
fn g_view_command(c: char) -> Option<VimCommand> {
    Some(match c {
        't' => VimCommand::CycleTheme,
        'm' => VimCommand::ToggleMarkers,
        'p' => VimCommand::TogglePreview,
        '?' => VimCommand::CheatSheet,
        'l' => VimCommand::Lab,
        'x' => VimCommand::ToggleOverflow,
        _ => return None,
    })
}
```

In `feed_prefixed`, add this arm before the `(']' | '[', 'a')` arm:

```rust
            ('g', c) if self.op.is_none() && g_view_command(c).is_some() => {
                self.reset();
                g_view_command(c).into_iter().collect()
            }
```

`crates/tulis/src/editor/vim_exec.rs`: the app's `match command` must stay exhaustive. Add this arm after `VimCommand::ToggleGhost => …` and keep the existing `let _ = window;`:

```rust
            // Wired up in Tasks 4 and 6.
            VimCommand::CycleTheme
            | VimCommand::ToggleMarkers
            | VimCommand::TogglePreview
            | VimCommand::CheatSheet
            | VimCommand::Lab
            | VimCommand::ToggleOverflow => {}
```

- [ ] **Step 4: Run them (expect pass)**

Run: `cargo test --workspace`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
jj describe -m "feat(core): theme and long_sentence_words config, vim g view commands

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 3: Core counts, title marker and share URLs

**Files:**
- Create: `crates/tulis-core/src/stats.rs`, `crates/tulis-core/src/share.rs`
- Modify: `crates/tulis-core/src/lib.rs`

**Interfaces:**
- Produces:
  - `stats::TITLE_MARKER: &str = "# "`, `stats::title_marker_len(text: &str) -> usize` (2 or 0)
  - `stats::Counts { pub words: usize, pub chars: usize }`, `Counts::of(&str) -> Counts`, `Counts::label(self, selected: bool) -> String` (`"N words · M chars"`, with `" selected"` appended for a selection)
  - `stats::document_counts(&Document) -> Counts`, `stats::selection_counts(&Document, Range<DocPos>) -> Counts`. Both leave out ghosts and the title marker.
  - `share::{X_LIMIT = 280, ShareTarget::{X, LinkedIn}, ShareUrl { url: String, trimmed: bool }}`
  - `share::share_text(&Document, Option<Range<DocPos>>) -> String`, `share::share_url(ShareTarget, &str) -> ShareUrl`, `share::percent_encode(&str) -> String`, `share::trim_to_chars(&str, usize) -> (&str, bool)`

- [ ] **Step 1: Write the modules with their tests, and stub the bodies**

Create `crates/tulis-core/src/stats.rs` with the code below, but make each function body `todo!()` at first. Do the same for `share.rs`. Then add `pub mod share;` and `pub mod stats;` to `lib.rs`, after `pub mod pos;`.

`crates/tulis-core/src/stats.rs`:

```rust
//! Word and character counts for the header, and the `# ` title marker.

use std::ops::Range;

use crate::{DocPos, Document};

/// A first paragraph starting with this renders as a bold title. The marker stays in the text.
pub const TITLE_MARKER: &str = "# ";

/// Byte length of the title marker at the start of `text` (0 when there is none).
pub fn title_marker_len(text: &str) -> usize {
    if text.starts_with(TITLE_MARKER) {
        TITLE_MARKER.len()
    } else {
        0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub words: usize,
    pub chars: usize,
}

impl Counts {
    /// Whitespace-separated words, and characters other than line breaks.
    pub fn of(text: &str) -> Self {
        Self {
            words: text.split_whitespace().count(),
            chars: text.chars().filter(|&c| c != '\n').count(),
        }
    }

    fn add(self, other: Self) -> Self {
        Self {
            words: self.words + other.words,
            chars: self.chars + other.chars,
        }
    }

    /// `"N words · M chars"`, with `" selected"` appended for a selection.
    pub fn label(self, selected: bool) -> String {
        let plural = |n: usize, one: &str, many: &str| {
            format!("{n} {}", if n == 1 { one } else { many })
        };
        let mut label = format!(
            "{} · {}",
            plural(self.words, "word", "words"),
            plural(self.chars, "char", "chars")
        );
        if selected {
            label.push_str(" selected");
        }
        label
    }
}

/// Counts for the whole document. Ghosts and the title marker are left out.
pub fn document_counts(doc: &Document) -> Counts {
    doc.paragraphs
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let text = p.export_text();
            let skip = if i == 0 { title_marker_len(&text) } else { 0 };
            Counts::of(&text[skip..])
        })
        .fold(Counts::default(), Counts::add)
}

/// Counts for `range`. Ghosts and the title marker are left out.
pub fn selection_counts(doc: &Document, range: Range<DocPos>) -> Counts {
    let mut start = range.start;
    if start.para == 0 {
        let marker = title_marker_len(&doc.paragraphs[0].text());
        start.offset = start.offset.max(marker);
    }
    if start >= range.end {
        return Counts::default();
    }
    Counts::of(&doc.export_text_in(start..range.end))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_words_and_chars() {
        assert_eq!(Counts::of("one two  three"), Counts { words: 3, chars: 14 });
        assert_eq!(Counts::of("héllo\nwörld"), Counts { words: 2, chars: 10 });
        assert_eq!(Counts::of(""), Counts::default());
    }

    #[test]
    fn labels_pluralize_and_mark_selections() {
        assert_eq!(Counts { words: 1, chars: 1 }.label(false), "1 word · 1 char");
        assert_eq!(
            Counts { words: 2, chars: 9 }.label(true),
            "2 words · 9 chars selected"
        );
    }

    #[test]
    fn document_counts_skip_ghosts_and_the_title_marker() {
        let mut doc = Document::from_texts(&["# My title", "I really love tea"]);
        doc.set_ghost(1, 2..9, true).unwrap();
        assert_eq!(document_counts(&doc), Counts { words: 5, chars: 18 });
    }

    #[test]
    fn selection_counts_skip_ghosts_and_the_title_marker() {
        let mut doc = Document::from_texts(&["# Title", "I really love tea"]);
        doc.set_ghost(1, 2..9, true).unwrap();
        let all = DocPos::new(0, 0)..DocPos::new(1, 17);
        assert_eq!(selection_counts(&doc, all), Counts { words: 4, chars: 15 });
        let marker_only = DocPos::new(0, 0)..DocPos::new(0, 2);
        assert_eq!(selection_counts(&doc, marker_only), Counts::default());
    }

    #[test]
    fn title_marker_needs_hash_and_space() {
        assert_eq!(title_marker_len("# Title"), 2);
        assert_eq!(title_marker_len("#Title"), 0);
        assert_eq!(title_marker_len("## Sub"), 0);
        assert_eq!(title_marker_len(""), 0);
    }
}
```

`crates/tulis-core/src/share.rs`:

```rust
//! Share to X or LinkedIn: the text to share and the intent URL.

use std::ops::Range;

use crate::storage::export_markdown;
use crate::{DocPos, Document};

/// X rejects longer posts.
pub const X_LIMIT: usize = 280;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShareTarget {
    X,
    LinkedIn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShareUrl {
    pub url: String,
    /// The text was cut to fit the target's limit.
    pub trimmed: bool,
}

/// What to share: the selection (paragraphs separated by a blank line) or, with no selection,
/// the whole document, as exported Markdown without the trailing newline.
pub fn share_text(doc: &Document, selection: Option<Range<DocPos>>) -> String {
    match selection {
        Some(range) if range.start != range.end => doc
            .export_text_in(range)
            .split('\n')
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n"),
        _ => export_markdown(doc).trim_end().to_string(),
    }
}

/// Percent-encode everything except RFC 3986 unreserved characters.
pub fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// At most `limit` characters of `text`, without trailing whitespace; true if it was cut.
pub fn trim_to_chars(text: &str, limit: usize) -> (&str, bool) {
    match text.char_indices().nth(limit) {
        Some((end, _)) => (text[..end].trim_end(), true),
        None => (text, false),
    }
}

pub fn share_url(target: ShareTarget, text: &str) -> ShareUrl {
    match target {
        ShareTarget::X => {
            let (text, trimmed) = trim_to_chars(text, X_LIMIT);
            ShareUrl {
                url: format!("https://x.com/intent/post?text={}", percent_encode(text)),
                trimmed,
            }
        }
        ShareTarget::LinkedIn => ShareUrl {
            url: format!(
                "https://www.linkedin.com/feed/?shareActive=true&text={}",
                percent_encode(text)
            ),
            trimmed: false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_encoding_keeps_unreserved_and_encodes_utf8() {
        assert_eq!(percent_encode("a-b_c.d~e"), "a-b_c.d~e");
        assert_eq!(percent_encode("tea & café\n"), "tea%20%26%20caf%C3%A9%0A");
    }

    #[test]
    fn x_url_trims_to_280_chars() {
        let short = share_url(ShareTarget::X, "Hello world");
        assert_eq!(short.url, "https://x.com/intent/post?text=Hello%20world");
        assert!(!short.trimmed);
        let long = "é".repeat(300);
        let url = share_url(ShareTarget::X, &long);
        assert!(url.trimmed);
        assert_eq!(url.url.matches("%C3%A9").count(), 280);
    }

    #[test]
    fn trimming_drops_trailing_whitespace_and_respects_exact_fit() {
        assert_eq!(trim_to_chars("abc def", 4), ("abc", true));
        assert_eq!(trim_to_chars("abcd", 4), ("abcd", false));
    }

    #[test]
    fn linkedin_url_is_never_trimmed() {
        let url = share_url(ShareTarget::LinkedIn, &"a".repeat(400));
        assert!(!url.trimmed);
        assert!(
            url.url
                .starts_with("https://www.linkedin.com/feed/?shareActive=true&text=aaa")
        );
    }

    #[test]
    fn share_text_uses_selection_or_whole_document_without_ghosts() {
        let mut doc = Document::from_texts(&["I really love tea", "", "Second."]);
        doc.set_ghost(0, 2..9, true).unwrap();
        assert_eq!(share_text(&doc, None), "I love tea\n\nSecond.");
        let range = DocPos::new(0, 9)..DocPos::new(2, 3);
        assert_eq!(share_text(&doc, Some(range)), "love tea\n\nSec");
        let caret = DocPos::new(0, 1)..DocPos::new(0, 1);
        assert_eq!(share_text(&doc, Some(caret)), "I love tea\n\nSecond.");
    }
}
```

- [ ] **Step 2: Run the tests (expect fail)**

Run: `cargo test -p tulis-core stats share`
Expected: the new tests panic with `not yet implemented`.

- [ ] **Step 3: Implement**

Replace the `todo!()` bodies with the bodies shown above.

- [ ] **Step 4: Run the tests (expect pass)**

Run: `cargo test --workspace`
Expected: all pass (10 new core tests).

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
jj describe -m "feat(core): word/char counts, title marker, share text and intent URLs

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---
### Task 4: Theme presets, font fallback, line height and theme cycling

**Files:**
- Rewrite: `crates/tulis/src/theme.rs`
- Modify: `crates/tulis/src/editor/mod.rs`, `crates/tulis/src/editor/element.rs`, `crates/tulis/src/workspace.rs`, `crates/tulis/src/actions.rs`, `crates/tulis/src/keymap.rs`, `crates/tulis/src/editor/vim_exec.rs`, `crates/tulis/src/editor/tests.rs`

**Interfaces:**
- Consumes: `EditorConfig.theme` and `EditorConfig.font_family` (Task 2), `VimCommand::CycleTheme` (Task 2).
- Produces:
  - `theme::Theme { id: ThemeId, dark: bool, background, panel, rule, banner, text, text_muted, accent, ai_accent, selection, cursor: Hsla, ghost_alpha: f32, fade_alpha: f32, mark_tint: Hsla, trim: Hsla }` (`Clone + Copy + Debug + PartialEq`)
  - `theme::ThemeId::{Midnight, Paper, Sepia, Black}` with `ALL`, `name()`, `from_name(&str) -> Option<ThemeId>`, `next()` and `theme() -> Theme`
  - `theme::theme_from_config(&str) -> (ThemeId, Option<String>)`, `theme::FONT_FALLBACKS`, `theme::platform_mono(&App) -> String`, `theme::resolve_font(configured: &str, installed: &[String], platform_mono: &str) -> String`, `theme::apply_to_components(&Theme, &mut App)`
  - `Editor.theme: Theme`, `Editor.font_family: SharedString` (pub(crate)), `Editor::cycle_theme(&mut self, cx)`
  - Action `CycleTheme`, bound to `secondary-k t` (window-wide). `Workspace::cycle_theme` is its handler.
  - `element::line_height` is now 1.8 × the font size.

- [ ] **Step 1: Write the failing tests**

The unit tests at the bottom of the new `theme.rs` (Step 3) cover presets, names, cycling, the config fallback and the font order. Add these to `crates/tulis/src/editor/tests.rs`:

```rust
#[gpui::test]
fn cycle_theme_advances_and_announces(cx: &mut TestAppContext) {
    use crate::theme::ThemeId;
    let (editor, cx) = editor_with(cx, &["x"]);
    assert_eq!(editor.read_with(cx, |e, _| e.theme.id), ThemeId::Midnight);
    editor.update(cx, |e, cx| e.cycle_theme(cx));
    assert_eq!(editor.read_with(cx, |e, _| e.theme.id), ThemeId::Paper);
    assert_eq!(
        editor.read_with(cx, |e, _| e.notice.clone()).as_deref(),
        Some("Theme: Paper")
    );
}

#[gpui::test]
fn vim_gt_cycles_theme(cx: &mut TestAppContext) {
    use crate::theme::ThemeId;
    let (editor, cx) = vim_editor(cx, &["x"]);
    type_str(&editor, cx, "gtgt");
    assert_eq!(editor.read_with(cx, |e, _| e.theme.id), ThemeId::Sepia);
}

#[gpui::test]
fn unknown_theme_in_config_falls_back_with_a_notice(cx: &mut TestAppContext) {
    use crate::theme::ThemeId;
    let mut config = Config::default();
    config.editor.theme = "neon".into();
    let (editor, cx) =
        cx.add_window_view(|_, cx| Editor::new(config, Document::from_texts(&["x"]), cx));
    assert_eq!(editor.read_with(cx, |e, _| e.theme.id), ThemeId::Midnight);
    assert_eq!(
        editor.read_with(cx, |e, _| e.notice.clone()).as_deref(),
        Some("Unknown theme \"neon\"; using Midnight")
    );
}
```

Add this to the `workspace.rs` tests module:

```rust
    #[gpui::test]
    fn cycling_the_theme_recolors_components(cx: &mut TestAppContext) {
        cx.update(gpui_component::init);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        let background = |cx: &mut gpui::VisualTestContext| {
            cx.update(|_, cx| gpui_component::Theme::global(cx).background)
        };
        let midnight = crate::theme::ThemeId::Midnight.theme();
        assert_eq!(background(cx), midnight.background);
        ws.update_in(cx, |ws, window, cx| {
            ws.cycle_theme(&crate::actions::CycleTheme, window, cx)
        });
        let paper = crate::theme::ThemeId::Paper.theme();
        assert_eq!(background(cx), paper.background);
        assert!(!cx.update(|_, cx| gpui_component::Theme::global(cx).mode.is_dark()));
    }
```

- [ ] **Step 2: Run them (expect fail)**

Run: `cargo test --workspace`
Expected: compile errors (`theme` field, `cycle_theme`, `ThemeId`, `CycleTheme`).

- [ ] **Step 3: Write `theme.rs`**

Replace `crates/tulis/src/theme.rs` with:

```rust
//! Theme presets (Midnight, Paper, Sepia, Black), font fallback, and syncing gpui-component.

use gpui::{App, Hsla, hsla, rgb};

/// Every color the editor and its chrome paint with.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    pub id: ThemeId,
    pub dark: bool,
    pub background: Hsla,
    /// Header, footer, panels and dialogs.
    pub panel: Hsla,
    /// Borders and separators.
    pub rule: Hsla,
    /// Error banner background.
    pub banner: Hsla,
    pub text: Hsla,
    pub text_muted: Hsla,
    pub accent: Hsla,
    pub ai_accent: Hsla,
    pub selection: Hsla,
    pub cursor: Hsla,
    /// Text alpha of ghosted text.
    pub ghost_alpha: f32,
    /// Text alpha of passages a Lab trim would cut.
    pub fade_alpha: f32,
    /// Background of Lab marks.
    pub mark_tint: Hsla,
    /// Strikethrough of v1 trim suggestions.
    pub trim: Hsla,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeId {
    #[default]
    Midnight,
    Paper,
    Sepia,
    Black,
}

fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

fn with_alpha(mut color: Hsla, alpha: f32) -> Hsla {
    color.a = alpha;
    color
}

impl ThemeId {
    pub const ALL: [ThemeId; 4] = [
        ThemeId::Midnight,
        ThemeId::Paper,
        ThemeId::Sepia,
        ThemeId::Black,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ThemeId::Midnight => "Midnight",
            ThemeId::Paper => "Paper",
            ThemeId::Sepia => "Sepia",
            ThemeId::Black => "Black",
        }
    }

    /// Case-insensitive; surrounding whitespace is ignored.
    pub fn from_name(name: &str) -> Option<Self> {
        let name = name.trim();
        Self::ALL
            .into_iter()
            .find(|id| id.name().eq_ignore_ascii_case(name))
    }

    pub fn next(self) -> Self {
        let ix = Self::ALL.iter().position(|&id| id == self).unwrap_or(0);
        Self::ALL[(ix + 1) % Self::ALL.len()]
    }

    pub fn theme(self) -> Theme {
        match self {
            ThemeId::Midnight => {
                let accent = hex(0x8b7cf6);
                Theme {
                    id: self,
                    dark: true,
                    background: hex(0x070b1f),
                    panel: hex(0x0c1230),
                    rule: hex(0x1d2448),
                    banner: hex(0x3a1730),
                    text: hex(0xe8cdb5),
                    text_muted: hex(0x8a8199),
                    accent,
                    ai_accent: hex(0xc49cff),
                    selection: with_alpha(accent, 0.32),
                    cursor: hex(0xf2b38a),
                    ghost_alpha: 0.06,
                    fade_alpha: 0.35,
                    mark_tint: with_alpha(hex(0xf2c94c), 0.18),
                    trim: hex(0xf87171),
                }
            }
            ThemeId::Paper => Theme {
                id: self,
                dark: false,
                background: hex(0xfbf8f1),
                panel: hex(0xf3efe6),
                rule: hex(0xe6e0d4),
                banner: hex(0xf6dcd2),
                text: hex(0x2b2a27),
                text_muted: hex(0x8a857a),
                accent: hsla(0.58, 0.35, 0.5, 1.),
                ai_accent: hsla(0.76, 0.55, 0.55, 1.),
                selection: hsla(0.11, 0.7, 0.6, 0.28),
                cursor: hex(0xb5522d),
                ghost_alpha: 0.12,
                fade_alpha: 0.35,
                mark_tint: hsla(0.14, 0.9, 0.6, 0.3),
                trim: hsla(0.0, 0.7, 0.5, 0.9),
            },
            ThemeId::Sepia => Theme {
                id: self,
                dark: false,
                background: hex(0xf4ecd8),
                panel: hex(0xebe0c6),
                rule: hex(0xdccfb0),
                banner: hex(0xeac9b5),
                text: hex(0x5b4636),
                text_muted: hex(0x9a8468),
                accent: hex(0xa0522d),
                ai_accent: hex(0x8e5ba8),
                selection: with_alpha(hex(0xc8a96e), 0.35),
                cursor: hex(0xa0522d),
                ghost_alpha: 0.12,
                fade_alpha: 0.35,
                mark_tint: with_alpha(hex(0xe0b040), 0.3),
                trim: hex(0xb03a2e),
            },
            ThemeId::Black => {
                let accent = hex(0x8b7cf6);
                Theme {
                    id: self,
                    dark: true,
                    background: hex(0x000000),
                    panel: hex(0x0b0b0b),
                    rule: hex(0x222222),
                    banner: hex(0x3a1010),
                    text: hex(0xd4d4d4),
                    text_muted: hex(0x7a7a7a),
                    accent,
                    ai_accent: hex(0xc49cff),
                    selection: with_alpha(accent, 0.3),
                    cursor: hex(0xe5e5e5),
                    ghost_alpha: 0.08,
                    fade_alpha: 0.35,
                    mark_tint: with_alpha(hex(0xf2c94c), 0.18),
                    trim: hex(0xf87171),
                }
            }
        }
    }
}

/// The preset named in config, or Midnight plus a notice when the name is unknown.
pub fn theme_from_config(name: &str) -> (ThemeId, Option<String>) {
    match ThemeId::from_name(name) {
        Some(id) => (id, None),
        None => (
            ThemeId::Midnight,
            Some(format!("Unknown theme \"{}\"; using Midnight", name.trim())),
        ),
    }
}

/// Monospace families tried, in order, after the configured one.
pub const FONT_FALLBACKS: [&str; 2] = ["IBM Plex Mono", "JetBrains Mono"];

/// The platform's monospace font, used when no preferred family is installed.
pub fn platform_mono(cx: &App) -> String {
    if cx.has_global::<gpui_component::Theme>() {
        // gpui-component already swapped its default for an installed family.
        return gpui_component::Theme::global(cx).mono_font_family.to_string();
    }
    if cfg!(target_os = "macos") {
        "Menlo".into()
    } else if cfg!(target_os = "windows") {
        "Consolas".into()
    } else {
        "DejaVu Sans Mono".into()
    }
}

/// The configured family if installed, else IBM Plex Mono, then JetBrains Mono, then
/// `platform_mono`.
pub fn resolve_font(configured: &str, installed: &[String], platform_mono: &str) -> String {
    std::iter::once(configured.trim())
        .chain(FONT_FALLBACKS)
        .find(|family| !family.is_empty() && installed.iter().any(|name| name == family))
        .unwrap_or(platform_mono)
        .to_string()
}

/// Make gpui-component's widgets use `theme`'s colors. Does nothing before
/// `gpui_component::init` (editor tests run without it).
pub fn apply_to_components(theme: &Theme, cx: &mut App) {
    use gpui_component::{Theme as ComponentTheme, ThemeMode};

    if !cx.has_global::<ComponentTheme>() {
        return;
    }
    let mode = if theme.dark {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    if ComponentTheme::global(cx).mode != mode {
        ComponentTheme::change(mode, None, cx);
    }
    let t = *theme;
    ComponentTheme::update(cx, |c| {
        let hover = with_alpha(t.accent, 0.14);
        c.background = t.background;
        c.foreground = t.text;
        c.border = t.rule;
        c.input = t.rule;
        c.ring = t.accent;
        c.caret = t.cursor;
        c.selection = t.selection;
        c.muted = t.panel;
        c.muted_foreground = t.text_muted;
        c.accent = hover;
        c.accent_foreground = t.text;
        c.primary = t.accent;
        c.primary_hover = with_alpha(t.accent, 0.85);
        c.primary_active = with_alpha(t.accent, 0.7);
        c.primary_foreground = t.background;
        c.secondary = t.panel;
        c.secondary_hover = hover;
        c.secondary_active = with_alpha(t.accent, 0.22);
        c.secondary_foreground = t.text;
        c.popover = t.panel;
        c.popover_foreground = t.text;
        c.list_hover = hover;
        c.list_active = with_alpha(t.accent, 0.22);
        c.list_active_border = t.accent;
        c.tab_bar = t.panel;
        c.tab = t.panel;
        c.tab_active = t.background;
        c.tab_foreground = t.text_muted;
        c.tab_active_foreground = t.accent;
        c.title_bar = t.panel;
        c.title_bar_border = t.rule;
        c.scrollbar = t.background;
        c.scrollbar_thumb = with_alpha(t.text_muted, 0.4);
        c.scrollbar_thumb_hover = with_alpha(t.text_muted, 0.6);
        c.overlay = with_alpha(gpui::black(), if t.dark { 0.55 } else { 0.25 });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_case_insensitively() {
        for id in ThemeId::ALL {
            assert_eq!(ThemeId::from_name(&id.name().to_uppercase()), Some(id));
        }
        assert_eq!(ThemeId::from_name(" sepia "), Some(ThemeId::Sepia));
        assert_eq!(ThemeId::from_name("solarized"), None);
    }

    #[test]
    fn next_cycles_through_all_presets() {
        let mut id = ThemeId::Midnight;
        let mut seen = Vec::new();
        for _ in 0..4 {
            seen.push(id);
            id = id.next();
        }
        assert_eq!(seen, ThemeId::ALL.to_vec());
        assert_eq!(id, ThemeId::Midnight);
    }

    #[test]
    fn presets_follow_the_spec() {
        let midnight = ThemeId::Midnight.theme();
        assert_eq!(midnight.background, hex(0x070b1f));
        assert_eq!(midnight.text, hex(0xe8cdb5));
        assert_eq!(midnight.accent, hex(0x8b7cf6));
        assert_eq!(midnight.ghost_alpha, 0.06);
        assert_eq!(ThemeId::Paper.theme().ghost_alpha, 0.12);
        assert_eq!(ThemeId::Black.theme().background, hex(0x000000));
        for id in ThemeId::ALL {
            let t = id.theme();
            assert_eq!(t.id, id);
            assert!((t.text.l - t.background.l).abs() > 0.4, "{id:?} lacks contrast");
        }
    }

    #[test]
    fn unknown_config_theme_falls_back_to_midnight_with_a_notice() {
        assert_eq!(theme_from_config("Paper"), (ThemeId::Paper, None));
        let (id, notice) = theme_from_config("neon");
        assert_eq!(id, ThemeId::Midnight);
        assert_eq!(notice.as_deref(), Some("Unknown theme \"neon\"; using Midnight"));
    }

    #[test]
    fn font_falls_back_in_order() {
        let installed = |names: &[&str]| names.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let platform = "DejaVu Sans Mono";
        assert_eq!(
            resolve_font("Fira Code", &installed(&["Fira Code", "IBM Plex Mono"]), platform),
            "Fira Code"
        );
        assert_eq!(
            resolve_font("Fira Code", &installed(&["JetBrains Mono", "IBM Plex Mono"]), platform),
            "IBM Plex Mono"
        );
        assert_eq!(
            resolve_font("Fira Code", &installed(&["JetBrains Mono"]), platform),
            "JetBrains Mono"
        );
        assert_eq!(resolve_font("Fira Code", &[], platform), platform);
        assert_eq!(resolve_font("", &installed(&["IBM Plex Mono"]), platform), "IBM Plex Mono");
    }
}
```

- [ ] **Step 4: Wire the theme into the editor**

`crates/tulis/src/editor/mod.rs`:
- Add `SharedString` to the `gpui` import. After `use layout::LayoutCache;`, add `use crate::theme::{self, Theme};`.
- Add these fields to `Editor`, after `scroll`:

```rust
    pub theme: Theme,
    /// The installed font family the page is set in (see `theme::resolve_font`).
    pub(crate) font_family: SharedString,
```

- In `Editor::new`, before the struct literal, add:

```rust
        let (theme_id, theme_notice) = theme::theme_from_config(&config.editor.theme);
        let font_family = theme::resolve_font(
            &config.editor.font_family,
            &cx.text_system().all_font_names(),
            &theme::platform_mono(cx),
        );
```

  In the literal, set `notice: theme_notice,` (instead of `None`) and add `theme: theme_id.theme(), font_family: font_family.into(),`.
- Add this method after `set_notice`:

```rust
    /// Switch to the next preset for this session and announce it.
    pub fn cycle_theme(&mut self, cx: &mut Context<Self>) {
        self.theme = self.theme.id.next().theme();
        theme::apply_to_components(&self.theme, cx);
        self.set_notice(format!("Theme: {}", self.theme.id.name()), cx);
    }
```

`crates/tulis/src/editor/element.rs`:
- Remove `use crate::theme;`.
- `line_height`: change `1.6` to `1.8`.
- In `build_layout`, use the editor's font and theme in the `RunStyle`:

```rust
    let style = RunStyle {
        font: font(editor.font_family.clone()),
        color: editor.theme.text,
        alt_underline: editor.theme.text_muted,
        ai_underline: editor.theme.accent,
        trim: editor.theme.trim,
        ghost_alpha: editor.theme.ghost_alpha,
    };
```

- `selection_quads`: change `theme::selection()` to `editor.theme.selection`. In `prepaint`, change `let mut color = theme::cursor();` to `let mut color = editor.theme.cursor;`.

`crates/tulis/src/editor/vim_exec.rs`: take `VimCommand::CycleTheme` out of the Task 2 placeholder arm and give it its own arm:

```rust
            VimCommand::CycleTheme => self.cycle_theme(cx),
```

- [ ] **Step 5: Wire the theme into the workspace, the action and the key**

`crates/tulis/src/actions.rs`: add `CycleTheme,` after `ToggleVim,`.

`crates/tulis/src/keymap.rs`: after the `ToggleVim` binding, add `binding("secondary-k t", CycleTheme, None, cx),` and change the test's expected count from 56 to 57.

`crates/tulis/src/workspace.rs`:
- Import `CycleTheme` with the other actions.
- In `Workspace::new`, right after `cx.observe(&editor, …).detach();`, add:

```rust
        let initial_theme = editor.read(cx).theme;
        theme::apply_to_components(&initial_theme, cx);
```

- Add this handler in the `impl Workspace` block that holds `toggle_overflow`, and register it with `.on_action(cx.listener(Self::cycle_theme))` in `render`, after the `toggle_overflow` listener:

```rust
    fn cycle_theme(&mut self, _: &CycleTheme, _: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| e.cycle_theme(cx));
    }
```

- Replace every color call. `render_status`, `render_overflow` and `render` each start with `let t = self.editor.read(cx).theme;` (in `render_status`, `let t = editor.theme;` after `let editor = …`). Then map `theme::paper()` → `t.background`, `theme::panel()` → `t.panel`, `theme::ink()` → `t.text`, `theme::muted()` → `t.text_muted`, `theme::rule()` → `t.rule` and `theme::banner()` → `t.banner`. Keep `use crate::theme;`, since `theme::apply_to_components` still uses it.

- [ ] **Step 6: Run the tests (expect pass)**

Run: `cargo test --workspace` and `cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: all tests pass, and the warning counts match the baseline.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all
jj describe -m "feat: theme presets (Midnight default), font fallback, 1.8 line height, Ctrl+K T / gt

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 5: The command table, keymap from it, and the cheat sheet

**Files:**
- Create: `crates/tulis/src/commands.rs`, `crates/tulis/src/cheat_sheet.rs`
- Modify: `crates/tulis/src/actions.rs`, `crates/tulis/src/keymap.rs`, `crates/tulis/src/main.rs`, `crates/tulis/src/workspace.rs`

**Interfaces:**
- Consumes: the `VimCommand` view variants (Task 2), `Theme` (Task 4).
- Produces:
  - Actions `ToggleMarkers`, `TogglePreview`, `ShowCheatSheet`, `OpenLab`, `ShareX`, `ShareLinkedIn` (handlers for most arrive in Tasks 6, 11 and 12. A bound action with no listener does nothing).
  - `commands::Group::{Editing, Alternates, Ghost, Overflow, Lab, View, File, Hidden}`, `Group::LISTED: [Group; 7]`, `Group::label(self) -> &'static str`
  - `commands::Command { id, label, group, keys: &'static [&'static str], context: Option<&'static str>, vim: Option<&'static str>, action: fn() -> Box<dyn Action> }`, `commands::COMMANDS: &[Command]`
  - `commands::key_bindings(&App) -> Vec<KeyBinding>`, `commands::key_label(&str) -> String`
  - `commands::CheatRow { id, label, key: String, vim }`, `commands::cheat_sheet(filter: &str) -> Vec<(Group, Vec<CheatRow>)>`
  - `cheat_sheet::CheatSheet { pub filter: Entity<InputState> }` with `new(Theme, &mut Window, &mut Context<Self>)` and `sections(&App)`
  - `Workspace::show_cheat_sheet(&mut self, &ShowCheatSheet, &mut Window, &mut Context<Self>)` and `Workspace.cheat_sheet: Option<Entity<CheatSheet>>`

- [ ] **Step 1: Write the failing tests**

The unit tests at the end of `commands.rs` (Step 3) are the drift guards. Every vim key in the table must make the vim machine produce that row's command, ids must be unique, key labels must be right, and the sheet must filter.

Replace the test in `keymap.rs` with:

```rust
#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use crate::actions::{CycleTheme, ToggleMarkers};
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
        });
    }
}
```

Add to the `workspace.rs` tests (a `Root` window, as in gpui-component's own dialog tests in `src/dialog/dialog.rs`):

```rust
    /// gpui-component widgets need their globals; some tests also press keys.
    fn init(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_component::init(cx);
            crate::keymap::bind(cx);
        });
    }

    #[gpui::test]
    fn cheat_sheet_opens_filters_and_closes_on_escape(cx: &mut TestAppContext) {
        use gpui::AppContext as _;
        use gpui_component::WindowExt;
        init(cx);
        let (root, cx) = cx.add_window_view(|window, cx| {
            let ws = cx.new(|cx| Workspace::new(Config::default(), None, None, window, cx));
            gpui_component::Root::new(ws, window, cx)
        });
        let ws = root.read_with(cx, |root, _| {
            root.view().clone().downcast::<Workspace>().unwrap()
        });
        ws.update_in(cx, |ws, window, cx| {
            ws.show_cheat_sheet(&crate::actions::ShowCheatSheet, window, cx)
        });
        cx.run_until_parked();
        assert!(cx.update(|window, cx| window.has_active_dialog(cx)));
        let sheet = ws.read_with(cx, |ws, _| ws.cheat_sheet.clone().unwrap());
        sheet.update_in(cx, |sheet, window, cx| {
            sheet
                .filter
                .update(cx, |input, cx| input.set_value("theme", window, cx));
            let sections = sheet.sections(cx);
            assert_eq!(sections.len(), 1);
            assert_eq!(sections[0].1[0].label, "Cycle theme");
        });
        cx.simulate_keystrokes("escape");
        cx.run_until_parked();
        assert!(!cx.update(|window, cx| window.has_active_dialog(cx)));
    }
```

Change `cycling_the_theme_recolors_components` (Task 4) to call `init(cx);` instead of `cx.update(gpui_component::init);`.

- [ ] **Step 2: Run them (expect fail)**

Run: `cargo test --workspace`
Expected: compile errors (`commands`, `cheat_sheet`, `show_cheat_sheet`).

- [ ] **Step 3: Write `commands.rs`**

`crates/tulis/src/actions.rs`: add after `CycleTheme,`:

```rust
        ToggleMarkers,
        TogglePreview,
        ShowCheatSheet,
        OpenLab,
        ShareX,
        ShareLinkedIn,
```

Create `crates/tulis/src/commands.rs`. All 57 bindings from Task 4's keymap are in it (same keys, same contexts), plus the new commands.

```rust
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
    cmd!("backspace", "Delete backward", Hidden, ["backspace"], ED, None, Backspace),
    cmd!("delete", "Delete forward", Hidden, ["delete"], ED, None, Delete),
    cmd!("left", "Left", Hidden, ["left"], ED, None, Left),
    cmd!("right", "Right", Hidden, ["right"], ED, None, Right),
    cmd!("up", "Up", Hidden, ["up"], ED, None, Up),
    cmd!("down", "Down", Hidden, ["down"], ED, None, Down),
    cmd!("select_left", "Select left", Hidden, ["shift-left"], ED, None, SelectLeft),
    cmd!("select_right", "Select right", Hidden, ["shift-right"], ED, None, SelectRight),
    cmd!("select_up", "Select up", Hidden, ["shift-up"], ED, None, SelectUp),
    cmd!("select_down", "Select down", Hidden, ["shift-down"], ED, None, SelectDown),
    cmd!("word_left", "Word left", Hidden, ["alt-left", "ctrl-left"], ED, None, WordLeft),
    cmd!("word_right", "Word right", Hidden, ["alt-right", "ctrl-right"], ED, None, WordRight),
    cmd!("select_word_left", "Select word left", Hidden, ["shift-alt-left", "ctrl-shift-left"], ED, None, SelectWordLeft),
    cmd!("select_word_right", "Select word right", Hidden, ["shift-alt-right", "ctrl-shift-right"], ED, None, SelectWordRight),
    cmd!("home", "Line start", Hidden, ["home"], ED, None, Home),
    cmd!("end", "Line end", Hidden, ["end"], ED, None, End),
    cmd!("select_home", "Select to line start", Hidden, ["shift-home"], ED, None, SelectHome),
    cmd!("select_end", "Select to line end", Hidden, ["shift-end"], ED, None, SelectEnd),
    cmd!("page_up", "Page up", Hidden, ["pageup"], ED, None, PageUp),
    cmd!("page_down", "Page down", Hidden, ["pagedown"], ED, None, PageDown),
    cmd!("doc_start", "Document start", Hidden, ["ctrl-home", "secondary-up"], ED, None, DocStart),
    cmd!("doc_end", "Document end", Hidden, ["ctrl-end", "secondary-down"], ED, None, DocEnd),
    cmd!("newline", "New paragraph", Hidden, ["enter", "shift-enter"], ED, None, Newline),
    // Editing
    cmd!("select_all", "Select all", Editing, ["secondary-a"], ED, None, SelectAll),
    cmd!("copy", "Copy", Editing, ["secondary-c"], ED, None, Copy),
    cmd!("cut", "Cut", Editing, ["secondary-x"], ED, None, Cut),
    cmd!("paste", "Paste", Editing, ["secondary-v"], ED, None, Paste),
    cmd!("undo", "Undo", Editing, ["secondary-z"], ED, Some("u"), Undo),
    cmd!("redo", "Redo", Editing, ["secondary-shift-z", "ctrl-y"], ED, Some("<C-r>"), Redo),
    cmd!("cancel", "Cancel / dismiss", Editing, ["escape"], ED, None, Cancel),
    cmd!("toggle_vim", "Toggle vim mode", Editing, ["secondary-alt-v"], ED, None, ToggleVim),
    // Alternates
    cmd!("make_alternate", "Make an alternate", Alternates, ["secondary-d"], ED, None, MakeAlternate),
    cmd!("cycle_next", "Next alternate", Alternates, ["alt-down"], ED, Some("]a"), CycleNext),
    cmd!("cycle_prev", "Previous alternate", Alternates, ["alt-up"], ED, Some("[a"), CyclePrev),
    cmd!("accept_alternate", "Keep this alternate", Alternates, ["secondary-enter"], ED, None, AcceptAlternate),
    cmd!("delete_alternate", "Delete this alternate", Alternates, ["secondary-shift-backspace"], ED, None, DeleteAlternate),
    cmd!("ai_alternates", "AI alternates", Alternates, ["secondary-j"], ED, None, AiAlternates),
    // Ghost
    cmd!("toggle_ghost", "Ghost / revive", Ghost, ["secondary-g"], ED, Some("gh"), ToggleGhost),
    // Overflow
    cmd!("send_to_overflow", "Send to Overflow", Overflow, ["secondary-shift-o"], ED, None, SendToOverflow),
    cmd!("toggle_overflow", "Show / hide Overflow", Overflow, ["secondary-\\"], None, Some("gx"), ToggleOverflowPanel),
    // Lab
    cmd!("open_lab", "The Lab", Lab, ["secondary-l"], None, Some("gl"), OpenLab),
    cmd!("ai_trim_paragraph", "AI trim: paragraph", Lab, ["secondary-shift-t"], ED, None, AiTrimParagraph),
    cmd!("ai_trim_document", "AI trim: document", Lab, ["secondary-shift-alt-t"], ED, None, AiTrimDocument),
    cmd!("accept_trim", "Accept a trim", Lab, ["tab"], ED, None, AcceptTrim),
    // View
    cmd!("cycle_theme", "Cycle theme", View, ["secondary-k t"], None, Some("gt"), CycleTheme),
    cmd!("toggle_markers", "Show / hide markers", View, ["secondary-."], None, Some("gm"), ToggleMarkers),
    cmd!("toggle_preview", "Markdown preview", View, ["secondary-shift-m"], None, Some("gp"), TogglePreview),
    cmd!("close_preview", "Close preview", Hidden, ["escape"], Some("Preview"), None, TogglePreview),
    cmd!("cheat_sheet", "Cheat sheet", View, ["secondary-?", "secondary-shift-/"], None, Some("g?"), ShowCheatSheet),
    // File
    cmd!("save", "Save", File, ["secondary-s"], None, None, Save),
    cmd!("save_as", "Save as", File, ["secondary-shift-s"], None, None, SaveAs),
    cmd!("open", "Open", File, ["secondary-o"], None, None, Open),
    cmd!("new", "New", File, ["secondary-n"], None, None, New),
    cmd!("share_x", "Share to X", File, [], None, None, ShareX),
    cmd!("share_linkedin", "Share to LinkedIn", File, [], None, None, ShareLinkedIn),
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
        keys.chars().flat_map(|c| vim.feed(VimKey::Char(c))).collect()
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
            _ => return None,
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
        let ctrl = if cfg!(target_os = "macos") { "Cmd" } else { "Ctrl" };
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
```

Run `cargo fmt` afterward. rustfmt leaves the long `cmd!` lines alone, which is fine.

Replace the body of `crates/tulis/src/keymap.rs` above its tests with:

```rust
//! Key bindings, generated from the command table. `secondary` is Cmd on macOS and Ctrl elsewhere.

use gpui::App;

pub fn bind(cx: &mut App) {
    let bindings = crate::commands::key_bindings(cx);
    cx.bind_keys(bindings);
}
```

`main.rs`: add `mod cheat_sheet;` and `mod commands;`.

- [ ] **Step 4: Write the cheat sheet**

Create `crates/tulis/src/cheat_sheet.rs`:

```rust
//! The cheat sheet: every listed command from the command table, grouped, with a filter box,
//! shown in a gpui-component dialog.

use gpui::{
    App, Context, Entity, Focusable, IntoElement, ParentElement, Render, SharedString, Styled,
    Subscription, Window, div, prelude::*, px,
};
use gpui_component::WindowExt;
use gpui_component::input::{Input, InputEvent, InputState};

use crate::actions::ShowCheatSheet;
use crate::commands::{CheatRow, Group, cheat_sheet};
use crate::theme::Theme;
use crate::workspace::Workspace;

pub struct CheatSheet {
    pub filter: Entity<InputState>,
    theme: Theme,
    _subscription: Subscription,
}

impl CheatSheet {
    pub fn new(theme: Theme, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let filter = cx.new(|cx| InputState::new(window, cx).placeholder("Filter commands…"));
        let subscription = cx.subscribe(&filter, |_, _, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        });
        Self {
            filter,
            theme,
            _subscription: subscription,
        }
    }

    /// The sections shown for the current filter text.
    pub fn sections(&self, cx: &App) -> Vec<(Group, Vec<CheatRow>)> {
        cheat_sheet(&self.filter.read(cx).value())
    }

    fn pill(&self, text: impl Into<SharedString>) -> impl IntoElement {
        let t = self.theme;
        div()
            .px_1()
            .rounded_sm()
            .border_1()
            .border_color(t.rule)
            .bg(t.background)
            .text_color(t.text)
            .child(text.into())
    }
}

impl Render for CheatSheet {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        let sections = self.sections(cx);
        div()
            .flex()
            .flex_col()
            .gap_3()
            .text_sm()
            .child(Input::new(&self.filter))
            .child(
                div()
                    .id("cheat-sheet-list")
                    .max_h(px(440.))
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .when(sections.is_empty(), |list| {
                        list.child(div().text_color(t.text_muted).child("No matching commands"))
                    })
                    .children(sections.into_iter().map(|(group, rows)| {
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().italic().text_color(t.accent).child(group.label()))
                            .children(rows.into_iter().map(|row| {
                                div()
                                    .id(row.id)
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(div().flex_1().child(row.label))
                                    .child(div().w(px(150.)).child(self.pill(row.key)))
                                    .child(div().w(px(48.)).when(!row.vim.is_empty(), |cell| {
                                        cell.child(self.pill(row.vim))
                                    }))
                            }))
                    })),
            )
    }
}

impl Workspace {
    /// Open the cheat sheet dialog, or close it if it is open. Needs a `Root` window (the app
    /// always has one; tests that skip `Root` must not call this).
    pub(crate) fn show_cheat_sheet(
        &mut self,
        _: &ShowCheatSheet,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.root::<gpui_component::Root>().flatten().is_none() {
            return;
        }
        if window.has_active_dialog(cx) {
            window.close_dialog(cx);
            return;
        }
        let theme = self.editor.read(cx).theme;
        let sheet = cx.new(|cx| CheatSheet::new(theme, window, cx));
        self.cheat_sheet = Some(sheet.clone());
        // The builder runs on every rebuild; focus the filter only the first time.
        let first_build = std::cell::Cell::new(true);
        window.open_dialog(cx, move |dialog, window, cx| {
            if first_build.replace(false) {
                let filter = sheet.read(cx).filter.focus_handle(cx);
                window.focus(&filter, cx);
            }
            dialog
                .title("Keyboard shortcuts")
                .w(px(600.))
                .child(sheet.clone())
        });
    }
}
```

`crates/tulis/src/workspace.rs`:
- Add the field `pub(crate) cheat_sheet: Option<Entity<crate::cheat_sheet::CheatSheet>>,` to `Workspace` and initialize it to `cheat_sheet: None,`.
- Import `ShowCheatSheet`, and register `.on_action(cx.listener(Self::show_cheat_sheet))` in `render`.

- [ ] **Step 5: Run the tests (expect pass)**

Run: `cargo test --workspace` and `cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: all pass, with no new warnings.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
jj describe -m "feat: command table drives keymap and cheat sheet dialog (Ctrl+? / g?)

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 6: View commands: markers toggle, Lab placeholder, vim dispatch

**Files:**
- Modify: `crates/tulis/src/editor/mod.rs`, `crates/tulis/src/editor/ai.rs`, `crates/tulis/src/editor/vim_exec.rs`, `crates/tulis/src/workspace.rs`, `crates/tulis/src/editor/tests.rs`

**Interfaces:**
- Consumes: the actions from Task 5 and the `VimCommand` variants from Task 2.
- Produces:
  - `Editor.show_markers: bool` (default `true`), `Editor::toggle_markers(&mut self, cx)` (toasts "Markers shown" or "Markers hidden")
  - `Editor::open_lab(&mut self, &mut Window, cx)`. Until Part 4 this runs the v1 document trim (`ai_trim_document`, now `pub(crate)`).
  - `Workspace::toggle_markers` and `Workspace::open_lab` handlers (`pub(crate)`), and `Workspace::toggle_overflow` becomes `pub(crate)`.
  - Vim: `gm` → `toggle_markers`, `gl` → `open_lab`, and `gp`/`g?`/`gx` → `window.dispatch_action(TogglePreview / ShowCheatSheet / ToggleOverflowPanel)`.

- [ ] **Step 1: Write the failing tests**

Add to `crates/tulis/src/editor/tests.rs` (`with_mock` and `vim_editor` already exist there):

```rust
#[gpui::test]
fn vim_gm_toggles_markers(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["x"]);
    assert!(editor.read_with(cx, |e, _| e.show_markers));
    type_str(&editor, cx, "gm");
    assert!(!editor.read_with(cx, |e, _| e.show_markers));
    assert_eq!(
        editor.read_with(cx, |e, _| e.notice.clone()).as_deref(),
        Some("Markers hidden")
    );
}

#[gpui::test]
fn vim_gl_runs_the_document_trim_until_the_lab_exists(cx: &mut TestAppContext) {
    let (editor, cx) = vim_editor(cx, &["I really love tea."]);
    with_mock(&editor, cx, vec![r#"["really "]"#]);
    type_str(&editor, cx, "gl");
    cx.run_until_parked();
    assert_eq!(editor.read_with(cx, |e, _| e.trims.len()), 1);
}
```

Add to the `workspace.rs` tests:

```rust
    #[gpui::test]
    fn vim_gx_toggles_the_overflow_panel(cx: &mut TestAppContext) {
        init(cx);
        let mut config = Config::default();
        config.editor.vim_mode = true;
        let (ws, cx) = cx.add_window_view(move |window, cx| {
            Workspace::new(config, None, None, window, cx)
        });
        ws.update_in(cx, |ws, window, cx| ws.focus_editor(window, cx));
        cx.run_until_parked();
        let editor = ws.read_with(cx, |ws, _| ws.editor.clone());
        editor.update_in(cx, |e, window, cx| {
            e.replace_text_in_range(None, "g", window, cx);
            e.replace_text_in_range(None, "x", window, cx);
        });
        cx.run_until_parked();
        assert!(ws.read_with(cx, |ws, _| ws.show_overflow));
    }
```

- [ ] **Step 2: Run them (expect fail)**

Run: `cargo test --workspace`
Expected: compile error (no field `show_markers`).

- [ ] **Step 3: Implement**

`editor/mod.rs`: add the field after `theme`:

```rust
    /// Alternate and version markers are drawn (Ctrl+. / vim `gm`).
    pub show_markers: bool,
```

Initialize it with `show_markers: true,`. Then add these methods after `cycle_theme`:

```rust
    pub fn toggle_markers(&mut self, cx: &mut Context<Self>) {
        self.show_markers = !self.show_markers;
        let state = if self.show_markers {
            "Markers shown"
        } else {
            "Markers hidden"
        };
        self.set_notice(state, cx);
    }

    /// The Lab arrives with the Lab plan; until then this runs the v1 document trim.
    pub fn open_lab(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ai_trim_document(&crate::actions::AiTrimDocument, window, cx);
    }
```

`editor/ai.rs`: make `ai_trim_document` `pub(crate)` instead of `pub(super)`.

`editor/vim_exec.rs`: change the import to `use crate::actions::{ShowCheatSheet, ToggleOverflowPanel, TogglePreview, ToggleVim};`. Replace the placeholder arm with the arms below and delete `let _ = window;`:

```rust
            VimCommand::ToggleMarkers => self.toggle_markers(cx),
            VimCommand::Lab => self.open_lab(window, cx),
            // Window-level commands: the workspace handles these actions.
            VimCommand::TogglePreview => window.dispatch_action(Box::new(TogglePreview), cx),
            VimCommand::CheatSheet => window.dispatch_action(Box::new(ShowCheatSheet), cx),
            VimCommand::ToggleOverflow => {
                window.dispatch_action(Box::new(ToggleOverflowPanel), cx)
            }
```

`workspace.rs`: import `OpenLab` and `ToggleMarkers`, make `toggle_overflow` `pub(crate)`, and add these handlers next to `cycle_theme`:

```rust
    pub(crate) fn toggle_markers(
        &mut self,
        _: &ToggleMarkers,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.editor.update(cx, |e, cx| e.toggle_markers(cx));
    }

    pub(crate) fn open_lab(&mut self, _: &OpenLab, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, cx| e.open_lab(window, cx));
    }
```

Register them in `render` with `.on_action(cx.listener(Self::toggle_markers))` and `.on_action(cx.listener(Self::open_lab))`.

- [ ] **Step 4: Run the tests (expect pass)**

Run: `cargo test --workspace` and clippy as before.
Expected: all pass, with no new warnings.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
jj describe -m "feat: markers toggle (Ctrl+. / gm), Lab placeholder (Ctrl+L / gl), vim gp g? gx

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---
### Task 7: Toasts replace the status-bar notice

**Files:**
- Create: `crates/tulis/src/toast.rs`, `crates/tulis/src/chrome.rs`
- Modify: `crates/tulis/src/editor/mod.rs`, `crates/tulis/src/workspace.rs`, `crates/tulis/src/main.rs`, `crates/tulis/src/editor/tests.rs`

**Interfaces:**
- Produces:
  - `toast::{TOAST_LIFETIME = 3s, MAX_TOASTS = 2, Toast { id: u64, text: String }, ToastQueue}` with `push(text) -> u64`, `remove(id)` and `iter()`, plus `toast::toast_opacity(progress: f32) -> f32`
  - `Editor.toasts: ToastQueue` replaces `Editor.notice`. `Editor::set_notice(text, cx)` (same signature) pushes a toast and removes it after `TOAST_LIFETIME` with a background timer. `#[cfg(test)] Editor::last_notice(&self) -> Option<&str>` returns the newest toast still showing.
  - `Workspace::render_toasts(&self, &App) -> impl IntoElement` (in `chrome.rs`) shows the toasts centered above the bottom edge, each fading out with `with_animation`.
  - `end_edit` no longer clears notices. They expire on their own.

- [ ] **Step 1: Write the failing tests**

`toast.rs` unit tests (Step 3) cover the queue. In `crates/tulis/src/editor/tests.rs`, replace every `e.notice.clone()` with `e.last_notice().map(str::to_string)`. This also covers the tests from Tasks 4 and 6.

Add to the `workspace.rs` tests:

```rust
    #[gpui::test]
    fn toasts_stack_two_and_expire(cx: &mut TestAppContext) {
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        let editor = ws.read_with(cx, |ws, _| ws.editor.clone());
        editor.update(cx, |e, cx| {
            e.set_notice("one", cx);
            e.set_notice("two", cx);
            e.set_notice("three", cx);
        });
        let shown = |cx: &mut gpui::VisualTestContext| {
            editor.read_with(cx, |e, _| {
                e.toasts.iter().map(|t| t.text.clone()).collect::<Vec<_>>()
            })
        };
        assert_eq!(shown(cx), vec!["two", "three"]);
        cx.run_until_parked();
        cx.executor().advance_clock(crate::toast::TOAST_LIFETIME);
        cx.run_until_parked();
        assert!(shown(cx).is_empty());
    }
```

- [ ] **Step 2: Run them (expect fail)**

Run: `cargo test --workspace`
Expected: compile errors (`last_notice`, `toasts`).

- [ ] **Step 3: Write `toast.rs`**

```rust
//! Toasts: short notices shown above the footer that fade out after a few seconds.

use std::collections::VecDeque;
use std::time::Duration;

pub const TOAST_LIFETIME: Duration = Duration::from_secs(3);
/// At most this many toasts are stacked; a new one pushes out the oldest.
pub const MAX_TOASTS: usize = 2;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Toast {
    pub id: u64,
    pub text: String,
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

    /// Showing toasts, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &Toast> {
        self.toasts.iter()
    }
}

/// Opacity at `progress` (0..=1) through a toast's lifetime: solid for 80%, then fading out.
pub fn toast_opacity(progress: f32) -> f32 {
    const SOLID: f32 = 0.8;
    if progress <= SOLID {
        1.
    } else {
        (1. - (progress - SOLID) / (1. - SOLID)).clamp(0., 1.)
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
    fn opacity_is_solid_then_fades() {
        assert_eq!(toast_opacity(0.), 1.);
        assert_eq!(toast_opacity(0.8), 1.);
        assert!((toast_opacity(0.9) - 0.5).abs() < 1e-5);
        assert_eq!(toast_opacity(1.), 0.);
    }
}
```

- [ ] **Step 4: Route notices through toasts**

`main.rs`: add `mod chrome;` and `mod toast;`.

`editor/mod.rs`:
- Import: `use crate::toast::{TOAST_LIFETIME, ToastQueue};`.
- Replace the field `pub notice: Option<String>,` with `pub toasts: ToastQueue,`.
- In `Editor::new`, make the struct literal `let mut editor = Self { … toasts: ToastQueue::default(), … };`, then follow it with:

```rust
        if let Some(notice) = theme_notice {
            editor.set_notice(notice, cx);
        }
        editor
```

- Replace `set_notice`, and add `last_notice` after it:

```rust
    /// Show a toast; it is removed after `TOAST_LIFETIME`.
    pub fn set_notice(&mut self, notice: impl Into<String>, cx: &mut Context<Self>) {
        let id = self.toasts.push(notice);
        cx.spawn(async move |this, cx| {
            cx.background_executor().timer(TOAST_LIFETIME).await;
            this.update(cx, |e, cx| {
                e.toasts.remove(id);
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// The newest toast still showing (tests).
    #[cfg(test)]
    pub fn last_notice(&self) -> Option<&str> {
        self.toasts.iter().last().map(|t| t.text.as_str())
    }
```

- In `end_edit`, delete `self.notice = None;`.
- In `insert_text`, replace `self.notice = Some(DISCARD_NOTICE.into());` with `self.set_notice(DISCARD_NOTICE, cx);`.

Create `crates/tulis/src/chrome.rs`:

```rust
//! The workspace's chrome: header, footer, toasts, and the share commands.

use gpui::{Animation, AnimationExt, App, IntoElement, ParentElement, Styled, div, prelude::*, px};

use crate::toast::{TOAST_LIFETIME, toast_opacity};
use crate::workspace::Workspace;

/// Distance of the toast stack from the window's bottom edge (above the footer).
const TOAST_BOTTOM: f32 = 56.;

impl Workspace {
    /// Toasts, centered above the bottom edge, newest last.
    pub(crate) fn render_toasts(&self, cx: &App) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let t = editor.theme;
        div()
            .absolute()
            .left_0()
            .right_0()
            .bottom(px(TOAST_BOTTOM))
            .flex()
            .flex_col()
            .items_center()
            .gap_1()
            .children(editor.toasts.iter().map(|toast| {
                div()
                    .id(("toast", toast.id))
                    .px_3()
                    .py_1()
                    .rounded_md()
                    .bg(t.panel)
                    .border_1()
                    .border_color(t.rule)
                    .text_xs()
                    .text_color(t.text)
                    .child(toast.text.clone())
                    .with_animation(
                        ("toast-fade", toast.id),
                        Animation::new(TOAST_LIFETIME),
                        |el, progress| el.opacity(toast_opacity(progress)),
                    )
            }))
    }
}
```

`workspace.rs`:
- In `render_status`, delete `.child(editor.notice.clone().unwrap_or_default())`.
- In `render`, add `.relative()` right after `div()` on the root, and add `.child(self.render_toasts(cx))` as the last child.

- [ ] **Step 5: Run the tests (expect pass)**

Run: `cargo test --workspace` and clippy.
Expected: all pass, with no new warnings.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
jj describe -m "feat: toasts (max 2, fade after 3s) replace the status-bar notice

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 8: Header (counts, badges, markers toggle, cheat sheet button, auto-dim), icons, window title

**Files:**
- Create: `crates/tulis/src/header_dim.rs`, `crates/tulis/src/assets.rs`
- Modify: `crates/tulis/Cargo.toml`, `crates/tulis/src/main.rs`, `crates/tulis/src/chrome.rs`, `crates/tulis/src/workspace.rs`, `crates/tulis/src/editor/mod.rs`, `crates/tulis/src/editor/vim_exec.rs`

**Interfaces:**
- Consumes: `stats::{document_counts, selection_counts}` (Task 3), `Editor.show_markers` (Task 6), `Workspace::show_cheat_sheet` (Task 5).
- Produces:
  - `header_dim::{DIM_AFTER = 2s, HeaderDim}` with `on_key(Instant)`, `on_mouse_move() -> bool`, `is_dimmed(Instant) -> bool` and `next_change(Instant) -> Option<Duration>`
  - `editor::EditorEvent::Typed`, where `Editor: EventEmitter<EditorEvent>`. It's emitted when a single character is typed.
  - `assets::Assets: AssetSource`: gpui-component's default icons plus Keyboard, Save and Share2
  - `Workspace.{header_dim, dim_timer, window_title}`, `Workspace::{typed, header_status(&App) -> (String, Vec<String>), render_header, sync_window_title}`
  - `chrome::{HEADER_HEIGHT = 40., mode_label(Mode) -> &'static str}`
  - The status bar is gone. The file name and dirty dot move to the window title (`"essay.md • — Tulis"`). Vim mode, `alt i/n`/`ver i/n` and `AI…` become header badges.

- [ ] **Step 1: Write the failing tests**

`header_dim.rs` unit tests are in Step 3, and so is the `assets.rs` test. In the `workspace.rs` tests module, add `init(cx);` as the first line of every `#[gpui::test]` function that lacks it (the header renders gpui-component buttons). Then add:

```rust
    #[gpui::test]
    fn header_shows_counts_selection_counts_and_vim_badge(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        ws.update_in(cx, |ws, window, cx| {
            ws.editor.update(cx, |e, cx| {
                e.replace_text_in_range(None, "# Title here", window, cx);
            });
            assert_eq!(ws.header_status(cx).0, "2 words · 10 chars");
            ws.editor.update(cx, |e, cx| {
                e.selection = Selection {
                    anchor: DocPos::new(0, 2),
                    head: DocPos::new(0, 7),
                };
                e.toggle_vim(&crate::actions::ToggleVim, window, cx);
            });
            let (counts, badges) = ws.header_status(cx);
            assert_eq!(counts, "1 word · 5 chars selected");
            assert_eq!(badges, vec!["NORMAL".to_string()]);
        });
    }

    #[gpui::test]
    fn typing_starts_a_dim_burst_and_mouse_move_ends_it(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        let editor = ws.read_with(cx, |ws, _| ws.editor.clone());
        editor.update_in(cx, |e, window, cx| e.replace_text_in_range(None, "a", window, cx));
        assert!(ws.read_with(cx, |ws, _| ws.dim_timer.is_some()));
        ws.update(cx, |ws, _| assert!(ws.header_dim.on_mouse_move()));
    }
```

- [ ] **Step 2: Run them (expect fail)**

Run: `cargo test --workspace`
Expected: compile errors (`header_status`, `dim_timer`, `header_dim`).

- [ ] **Step 3: Write `header_dim.rs` and `assets.rs`**

`crates/tulis/src/header_dim.rs`:

```rust
//! Auto-dim for the header: it fades out after 2s of continuous typing and comes back when
//! typing pauses for 2s or the mouse moves.

use std::time::{Duration, Instant};

pub const DIM_AFTER: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HeaderDim {
    /// First keystroke of the current typing burst.
    burst_start: Option<Instant>,
    last_key: Option<Instant>,
}

impl HeaderDim {
    pub fn on_key(&mut self, now: Instant) {
        let paused = self
            .last_key
            .is_none_or(|last| now.duration_since(last) >= DIM_AFTER);
        if paused || self.burst_start.is_none() {
            self.burst_start = Some(now);
        }
        self.last_key = Some(now);
    }

    /// Returns true if this ended a typing burst (the header may need repainting).
    pub fn on_mouse_move(&mut self) -> bool {
        let had_burst = self.burst_start.is_some();
        *self = Self::default();
        had_burst
    }

    pub fn is_dimmed(&self, now: Instant) -> bool {
        match (self.burst_start, self.last_key) {
            (Some(start), Some(last)) => {
                now.duration_since(last) < DIM_AFTER && now.duration_since(start) >= DIM_AFTER
            }
            _ => false,
        }
    }

    /// How long until `is_dimmed` may change, if it can change without further input.
    pub fn next_change(&self, now: Instant) -> Option<Duration> {
        let (start, last) = (self.burst_start?, self.last_key?);
        let since_last = now.duration_since(last);
        if since_last >= DIM_AFTER {
            return None;
        }
        if self.is_dimmed(now) {
            Some(DIM_AFTER - since_last)
        } else {
            Some(DIM_AFTER.saturating_sub(now.duration_since(start)))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// Type a key every 200ms from `from` for `count` keys; returns the time of the last key.
    fn type_keys(dim: &mut HeaderDim, from: Instant, count: u64) -> Instant {
        let mut t = from;
        for i in 0..count {
            t = from + ms(200 * i);
            dim.on_key(t);
        }
        t
    }

    #[test]
    fn dims_after_two_seconds_of_typing() {
        let t0 = Instant::now();
        let mut dim = HeaderDim::default();
        let last = type_keys(&mut dim, t0, 5); // 0..800ms
        assert!(!dim.is_dimmed(last));
        let last = type_keys(&mut dim, t0, 12); // 0..2200ms
        assert!(dim.is_dimmed(last));
    }

    #[test]
    fn comes_back_after_a_two_second_pause() {
        let t0 = Instant::now();
        let mut dim = HeaderDim::default();
        let last = type_keys(&mut dim, t0, 12);
        assert!(dim.is_dimmed(last + ms(1900)));
        assert!(!dim.is_dimmed(last + ms(2000)));
        assert_eq!(dim.next_change(last + ms(1500)), Some(ms(500)));
        assert_eq!(dim.next_change(last + ms(2000)), None);
    }

    #[test]
    fn a_pause_starts_a_new_burst() {
        let t0 = Instant::now();
        let mut dim = HeaderDim::default();
        let last = type_keys(&mut dim, t0, 12);
        dim.on_key(last + ms(2500));
        assert!(!dim.is_dimmed(last + ms(2600)));
        assert_eq!(dim.next_change(last + ms(2600)), Some(ms(1900)));
    }

    #[test]
    fn mouse_move_undims() {
        let t0 = Instant::now();
        let mut dim = HeaderDim::default();
        let last = type_keys(&mut dim, t0, 12);
        assert!(dim.on_mouse_move());
        assert!(!dim.is_dimmed(last));
        assert!(!dim.on_mouse_move());
        assert_eq!(dim.next_change(last), None);
    }
}
```

`crates/tulis/Cargo.toml`: add `gpui-kit-assets = "0.8.1"` to `[dependencies]`.

`crates/tulis/src/assets.rs`. `gpui_kit_assets::Assets::load` returns `Err` for unknown paths, so the extras are tried first. The macro is documented in `gpui-kit-assets-0.8.1/src/lib.rs:59-91`.

```rust
//! Icon assets: gpui-component's default icons plus the few extra Lucide icons Tulis uses.

use std::borrow::Cow;

use gpui::{AssetSource, Result, SharedString};

gpui_kit_assets::icon_assets!(ExtraIcons, [Keyboard, Save, Share2]);

pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        match ExtraIcons.load(path)? {
            Some(data) => Ok(Some(data)),
            None => gpui_kit_assets::Assets.load(path),
        }
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        let mut names = ExtraIcons.list(path)?;
        names.extend(gpui_kit_assets::Assets.list(path)?);
        names.sort();
        names.dedup();
        Ok(names)
    }
}

#[cfg(test)]
mod tests {
    use gpui::AssetSource;
    use gpui_kit_assets::IconName;

    use super::Assets;

    #[test]
    fn extra_and_default_icons_load() {
        for icon in [IconName::Keyboard, IconName::Save, IconName::Share2, IconName::Eye] {
            let path = icon.path();
            assert!(Assets.load(&path).unwrap().is_some(), "{path}");
        }
    }
}
```

`main.rs`: add `mod assets;` and `mod header_dim;`. Start the app with the assets:

```rust
    gpui_platform::application()
        .with_assets(assets::Assets)
        .run(move |cx: &mut App| {
```

- [ ] **Step 4: Emit `Typed` from the editor**

`editor/mod.rs`, after `pub type Snapshot …`:

```rust
/// Events the workspace listens for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorEvent {
    /// A character was typed into the page (drives the header's auto-dim).
    Typed,
}

impl gpui::EventEmitter<EditorEvent> for Editor {}
```

In `insert_text`, right after `kind` is computed:

```rust
        if matches!(kind, EditKind::Typing { .. }) {
            cx.emit(EditorEvent::Typed);
        }
```

`editor/vim_exec.rs`: make `toggle_vim` `pub(crate)` (the header test calls it).

- [ ] **Step 5: Header in `chrome.rs`; replace the status bar**

Replace the imports at the top of `chrome.rs` with:

```rust
use std::time::Instant;

use gpui::{
    Animation, AnimationExt, App, ClickEvent, Context, IntoElement, ParentElement, Styled, Window,
    div, prelude::*, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{IconName, Selectable, Sizable};
use tulis_core::alternates::AltLevel;
use tulis_core::stats;
use tulis_core::vim::Mode;

use crate::actions::{ShowCheatSheet, ToggleMarkers};
use crate::toast::{TOAST_LIFETIME, toast_opacity};
use crate::workspace::{Workspace, file_name};

pub const HEADER_HEIGHT: f32 = 40.;
```

Add this at module level:

```rust
pub(crate) fn mode_label(mode: Mode) -> &'static str {
    match mode {
        Mode::Normal => "NORMAL",
        Mode::Insert => "INSERT",
        Mode::Visual => "VISUAL",
        Mode::VisualLine => "V-LINE",
    }
}
```

Add these methods to the `impl Workspace` block in `chrome.rs`:

```rust
    /// Called when the editor reports typing: track the burst and schedule the repaint that
    /// dims (or undims) the header.
    pub(crate) fn typed(&mut self, cx: &mut Context<Self>) {
        self.header_dim.on_key(Instant::now());
        self.schedule_dim_repaint(cx);
    }

    fn schedule_dim_repaint(&mut self, cx: &mut Context<Self>) {
        let Some(delay) = self.header_dim.next_change(Instant::now()) else {
            self.dim_timer = None;
            return;
        };
        self.dim_timer = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(delay).await;
            this.update(cx, |ws, cx| {
                cx.notify();
                ws.schedule_dim_repaint(cx);
            })
            .ok();
        }));
    }

    /// The left part of the header: counts, then the vim, alternate and AI badges.
    pub(crate) fn header_status(&self, cx: &App) -> (String, Vec<String>) {
        let editor = self.editor.read(cx);
        let range = editor.selection.range();
        let counts = if range.start == range.end {
            stats::document_counts(&editor.doc).label(false)
        } else {
            stats::selection_counts(&editor.doc, range).label(true)
        };
        let mut badges = Vec::new();
        if let Some(mode) = editor.vim_mode() {
            badges.push(mode_label(mode).to_string());
        }
        if let Some(info) = editor.doc.alt_info(editor.cursor()) {
            let label = match info.level {
                AltLevel::Word => "alt",
                AltLevel::Paragraph => "ver",
            };
            badges.push(format!("{label} {}/{}", info.index + 1, info.count));
        }
        if editor.ai_task.is_some() {
            badges.push("AI…".into());
        }
        (counts, badges)
    }

    pub(crate) fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let t = editor.theme;
        let show_markers = editor.show_markers;
        let (counts, badges) = self.header_status(cx);
        let dimmed = self.header_dim.is_dimmed(Instant::now());
        let header = div()
            .id("header")
            .h(px(HEADER_HEIGHT))
            .flex_none()
            .px_4()
            .flex()
            .flex_row()
            .items_center()
            .text_xs()
            .text_color(t.text_muted)
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(counts)
                    .children(badges.into_iter().map(|badge| {
                        div()
                            .px_1()
                            .rounded_sm()
                            .border_1()
                            .border_color(t.rule)
                            .text_color(t.accent)
                            .child(badge)
                    })),
            )
            .child(
                div().flex().flex_row().items_center().gap_1().child(
                    Button::new("toggle-markers")
                        .ghost()
                        .xsmall()
                        .icon(if show_markers {
                            IconName::Eye
                        } else {
                            IconName::EyeOff
                        })
                        .selected(show_markers)
                        .tooltip("Markers (Ctrl+.)")
                        .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                            ws.toggle_markers(&ToggleMarkers, window, cx)
                        })),
                ),
            )
            .child(
                div().flex_1().flex().flex_row().justify_end().child(
                    Button::new("cheat-sheet")
                        .ghost()
                        .xsmall()
                        .icon(gpui_kit_assets::IconName::Keyboard)
                        .tooltip("Cheat sheet (Ctrl+?)")
                        .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                            ws.show_cheat_sheet(&ShowCheatSheet, window, cx)
                        })),
                ),
            );
        if dimmed {
            header
                .with_animation(
                    "header-dim",
                    Animation::new(std::time::Duration::from_millis(400)),
                    |el, progress| el.opacity(1. - progress),
                )
                .into_any_element()
        } else {
            header.into_any_element()
        }
    }

    /// Keep the window title in step with the file name and unsaved state.
    pub(crate) fn sync_window_title(&mut self, window: &mut Window, cx: &App) {
        let name = self
            .path
            .as_deref()
            .map(file_name)
            .unwrap_or_else(|| "Untitled".into());
        let dirty = if self.editor.read(cx).dirty {
            " •"
        } else {
            ""
        };
        let title = format!("{name}{dirty} — Tulis");
        if title != self.window_title {
            window.set_window_title(&title);
            self.window_title = title;
        }
    }
```

The header test also needs `Workspace::toggle_markers` (Task 6) to be `pub(crate)`. It already is.

`workspace.rs`:
- Delete `render_status`, and with it the imports of `AltLevel` and `Mode`.
- Imports: add `MouseMoveEvent`, `Subscription` and `Task` to the `gpui` import; change `use crate::editor::Editor;` to `use crate::editor::{Editor, EditorEvent};`; add `use crate::header_dim::HeaderDim;`.
- Add these fields to `Workspace`:

```rust
    pub(crate) header_dim: HeaderDim,
    /// Repaints the header when its auto-dim state is due to change.
    pub(crate) dim_timer: Option<Task<()>>,
    pub(crate) window_title: String,
    _subscriptions: Vec<Subscription>,
```

- In `Workspace::new`, replace `cx.observe(&editor, |_, _, cx| cx.notify()).detach();` with:

```rust
        let subscriptions = vec![
            cx.observe(&editor, |_, _, cx| cx.notify()),
            cx.subscribe(&editor, |ws, _, event: &EditorEvent, cx| match event {
                EditorEvent::Typed => ws.typed(cx),
            }),
        ];
```

  Then initialize the new fields: `header_dim: HeaderDim::default(), dim_timer: None, window_title: String::new(), _subscriptions: subscriptions,`.
- In `render`, rename `_window` to `window` and call `self.sync_window_title(window, cx);` first. On the root `div()`, add:

```rust
            .on_mouse_move(cx.listener(|ws, _: &MouseMoveEvent, _, cx| {
                if ws.header_dim.on_mouse_move() {
                    cx.notify();
                }
            }))
```

  Add `.child(self.render_header(cx))` as the first child (before the banner), and delete `.child(self.render_status(cx))`.
- Update the module doc comment: "The root view: header, page, overflow panel, footer, toasts and file commands."

- [ ] **Step 6: Run the tests (expect pass)**

Run: `cargo test --workspace` and clippy.
Expected: all pass, with no new warnings.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all
jj describe -m "feat: header with counts, badges, markers toggle, cheat sheet button and auto-dim

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 9: Writing column, edge fades and the bold `# ` title

**Files:**
- Modify: `crates/tulis/src/runs.rs`, `crates/tulis/src/editor/element.rs`, `crates/tulis/src/editor/mod.rs`, `crates/tulis/src/workspace.rs`

**Interfaces:**
- Consumes: `stats::title_marker_len` (Task 3).
- Produces:
  - `RunStyle.title_font: Font` (bold), `build_runs(text_len, spans, trims, marked, title_marker: usize, style)`. The bytes `0..title_marker` are drawn at alpha 0, and every run of a title paragraph uses `title_font`.
  - `editor::GUTTER: Pixels = px(24.)` (re-exported from `element`). The element lays text at `bounds.origin.x + GUTTER` with width `bounds.width − 2·GUTTER`, and `LayoutCache.origin` is that text origin.
  - `workspace::{COLUMN_WIDTH = 640., FADE_HEIGHT = 48.}`, `Workspace::render_page(&self, &App)`. The text column is 640px wide and centered, with 64px vertical padding and 48px fades at the top and bottom.

- [ ] **Step 1: Write the failing tests**

In `runs.rs` tests: pass `0` as the new `title_marker` argument in the three existing `build_runs` calls (`build_runs(10, &spans, &[7..9], None, 0, &style())` and so on). Import `FontWeight`, and give `style()` a bold title font:

```rust
    fn style() -> RunStyle {
        let mut title_font = font("Test");
        title_font.weight = FontWeight::BOLD;
        RunStyle {
            font: font("Test"),
            title_font,
            color: hsla(0., 0., 0., 1.),
            alt_underline: hsla(0.5, 0.5, 0.5, 1.),
            ai_underline: hsla(0.75, 0.5, 0.5, 1.),
            trim: hsla(0., 1., 0.5, 1.),
            ghost_alpha: 0.1,
        }
    }

    #[test]
    fn title_is_bold_and_its_marker_invisible() {
        let spans = vec![Span {
            range: 0..7,
            ghost: false,
            alt: None,
        }];
        let runs = build_runs(7, &spans, &[], None, 2, &style());
        assert_eq!(runs.iter().map(|r| r.len).collect::<Vec<_>>(), vec![2, 5]);
        assert_eq!(runs[0].color.a, 0.);
        assert_eq!(runs[1].color.a, 1.);
        assert!(runs.iter().all(|r| r.font.weight == FontWeight::BOLD));
        let plain = build_runs(7, &spans, &[], None, 0, &style());
        assert!(plain.iter().all(|r| r.font.weight != FontWeight::BOLD));
    }
```

Add to the `workspace.rs` tests:

```rust
    #[gpui::test]
    fn text_column_is_640px_wide_and_starts_after_the_gutter(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        cx.simulate_resize(gpui::size(gpui::px(1000.), gpui::px(700.)));
        cx.run_until_parked();
        let (width, x) = ws.read_with(cx, |ws, cx| {
            let layout = ws.editor.read(cx).layout.as_ref().expect("painted");
            (layout.width, layout.origin.x)
        });
        assert_eq!(width, gpui::px(super::COLUMN_WIDTH));
        // Centered: (1000 - 640) / 2 = 180.
        assert_eq!(x, gpui::px(180.));
    }
```

- [ ] **Step 2: Run them (expect fail)**

Run: `cargo test --workspace`
Expected: compile errors (`title_font`, the `build_runs` arity, `COLUMN_WIDTH`).

- [ ] **Step 3: Title runs**

`runs.rs`: add the field after `font`:

```rust
    /// The font of a `# ` title paragraph (bold).
    pub title_font: Font,
```

Change the signature and the start of `build_runs`:

```rust
pub fn build_runs(
    text_len: usize,
    spans: &[Span],
    trims: &[Range<usize>],
    marked: Option<Range<usize>>,
    title_marker: usize,
    style: &RunStyle,
) -> Vec<TextRun> {
    let title_marker = title_marker.min(text_len);
    let mut cuts = vec![0, text_len, title_marker];
```

After the ghost alpha adjustment:

```rust
            // The title's `# ` stays in the text but is not drawn.
            if b <= title_marker {
                color.a = 0.;
            }
```

In the `TextRun` literal:

```rust
                font: if title_marker > 0 {
                    style.title_font.clone()
                } else {
                    style.font.clone()
                },
```

Update the module doc comment to mention the title marker.

- [ ] **Step 4: Gutter and title in the element**

`element.rs`:
- Add `FontWeight` to the `gpui` import and `use tulis_core::stats::title_marker_len;`.
- After `EditorPrepaint`'s `IntoElement` impl:

```rust
/// Horizontal space kept free on each side of the text, for the paragraph-version bar.
pub(crate) const GUTTER: Pixels = px(24.);
```

- In `build_layout`, before `let style = …`:

```rust
    let mut title_font = font(editor.font_family.clone());
    title_font.weight = FontWeight::BOLD;
```

  Add `title_font,` to the `RunStyle` literal. In the paragraph loop, compute the marker and pass it:

```rust
        let title_marker = if i == 0 { title_marker_len(&text) } else { 0 };
        let runs = build_runs(text_len, &p.spans(), &trims, marked_here, title_marker, &style);
```

- `request_layout`'s measure closure:

```rust
                let width = known.width.unwrap_or(match available.width {
                    AvailableSpace::Definite(width) => width,
                    _ => px(640.) + GUTTER * 2.,
                });
                let text_width = (width - GUTTER * 2.).max(px(1.));
                let layout =
                    build_layout(editor.read(cx), text_width, point(px(0.), px(0.)), window);
                size(width, layout.total_height())
```

- `prepaint`:

```rust
        let text_origin = point(bounds.origin.x + GUTTER, bounds.origin.y);
        let text_width = (bounds.size.width - GUTTER * 2.).max(px(1.));
        let layout = build_layout(editor, text_width, text_origin, window);
```

- `paint`: paint each line at the layout's text origin:

```rust
        let text_origin = prepaint.layout.origin;
        for para in &prepaint.layout.paras {
            let origin = point(text_origin.x, text_origin.y + para.top);
```

`editor/mod.rs`: next to `use element::EditorElement;`, add `pub(crate) use element::GUTTER;`.

- [ ] **Step 5: Column and fades in the workspace**

`workspace.rs`:
- Add `Hsla`, `linear_color_stop` and `linear_gradient` to the `gpui` import. Change the editor import to `use crate::editor::{Editor, EditorEvent, GUTTER};`.
- Add these before `impl Render for Workspace`:

```rust
/// The writing column's text width (the element adds `GUTTER` on each side).
pub(crate) const COLUMN_WIDTH: f32 = 640.;
/// Space above the first and below the last line, inside the scroll view.
const COLUMN_PADDING_Y: f32 = 64.;
/// Height of the fades over the scroll view's top and bottom edges.
pub(crate) const FADE_HEIGHT: f32 = 48.;

/// A fade from the background color (at the edge) to transparent, over `FADE_HEIGHT`.
fn edge_fade(top: bool, background: Hsla) -> impl IntoElement {
    let clear = Hsla {
        a: 0.,
        ..background
    };
    // Angle 180° runs top to bottom, 0° bottom to top; the first stop sits at the start.
    let angle = if top { 180. } else { 0. };
    div()
        .absolute()
        .left_0()
        .right_0()
        .h(px(FADE_HEIGHT))
        .map(|d| if top { d.top_0() } else { d.bottom_0() })
        .bg(linear_gradient(
            angle,
            linear_color_stop(background, 0.),
            linear_color_stop(clear, 1.),
        ))
}

impl Workspace {
    /// The scrollable page: a centered column with fades over its top and bottom edges.
    fn render_page(&self, cx: &App) -> impl IntoElement {
        let editor = self.editor.read(cx);
        let (scroll, background) = (editor.scroll.clone(), editor.theme.background);
        div()
            .relative()
            .flex_1()
            .h_full()
            .child(
                div()
                    .id("editor-scroll")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&scroll)
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .justify_center()
                            .py(px(COLUMN_PADDING_Y))
                            .px(px(32.))
                            .child(
                                div()
                                    .w_full()
                                    .max_w(px(COLUMN_WIDTH) + GUTTER * 2.)
                                    .child(self.editor.clone()),
                            ),
                    ),
            )
            .child(edge_fade(true, background))
            .child(edge_fade(false, background))
    }
}
```

- In `render`, replace the whole `div().id("editor-scroll")…` child of the middle row with `.child(self.render_page(cx))`, and delete the now-unused `let scroll = …` line.

The fades are plain divs without listeners, so they register no hitbox. Wheel and clicks still reach the scroll view and the editor under them.

- [ ] **Step 6: Run the tests (expect pass)**

Run: `cargo test --workspace` and clippy.
Expected: all pass, with no new warnings.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all
jj describe -m "feat: 640px column with edge fades, bold # title with hidden marker

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---
### Task 10: Markers: dotted underlines, version bar + handle, cycled-group box

**Files:**
- Create: `crates/tulis/src/editor/markers.rs`
- Modify: `crates/tulis/src/runs.rs`, `crates/tulis/src/editor/element.rs`, `crates/tulis/src/editor/layout.rs`, `crates/tulis/src/editor/mod.rs`, `crates/tulis/src/editor/features.rs`, `crates/tulis/src/editor/actions.rs`, `crates/tulis/src/editor/tests.rs`

**Interfaces:**
- Consumes: `Editor.show_markers` (Task 6), `GUTTER` (Task 9), `Theme.{accent, text_muted, background}` (Task 4).
- Produces:
  - `editor::markers::{DOT, DOT_STEP, BAR_OFFSET, BAR_WIDTH, HANDLE_WIDTH, HANDLE_HEIGHT}` and the pure helpers `split_by_lines(&[usize], len, Range<usize>) -> Vec<(usize, Range<usize>)>`, `dot_xs(f32, f32) -> Vec<f32>`, `underline_y(line_top, line_height, font_size) -> Pixels`, `version_bar(text_left, para_top, height) -> Bounds<Pixels>`, `version_handle(..) -> Bounds<Pixels>`, `handle_dots(Bounds) -> [Bounds; 3]`, `group_box(x0, x1, line_top, line_height) -> Bounds<Pixels>` and `handle_at(&[(usize, Bounds)], Point) -> Option<usize>`
  - `LayoutCache.version_handles: Vec<(usize, Bounds<Pixels>)>`, filled while painting (empty when markers are hidden)
  - `editor::CycledGroup { para_id: ParaId, range: Range<usize> }`, `Editor.cycled: Option<CycledGroup>`. It's set by `cycle()` and `next_version()`, and cleared by any edit, undo/redo, `load`, or a selection whose head leaves the range.
  - `Editor::next_version(&mut self, para: usize, cx)`: a click on a `⋮` handle shows the paragraph's next version. This is a placeholder until Part 2's sidebar.
  - `RunStyle` loses `alt_underline` and `ai_underline`. Alternates are no longer underlined by text runs.

- [ ] **Step 1: Write the failing tests**

The `markers.rs` unit tests are in Step 3. `runs.rs` tests: delete `alt_underline` and `ai_underline` from `style()`. In `runs_cover_text_and_carry_styles`, replace the two underline assertions with `assert!(runs.iter().all(|r| r.underline.is_none()));`.

Add to `crates/tulis/src/editor/tests.rs`:

```rust
#[gpui::test]
fn cycling_boxes_the_group_until_the_cursor_leaves(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["I love tea"]);
    editor.update(cx, |e, _| {
        e.doc.make_alt(0, 2..6).unwrap();
        e.doc.edit_alt(0, 1, 0..4, "adore");
        e.selection = Selection::caret(DocPos::new(0, 3));
    });
    editor.update_in(cx, |e, window, cx| e.cycle_next(&CycleNext, window, cx));
    let cycled = editor.read_with(cx, |e, _| e.cycled.clone()).unwrap();
    assert_eq!(cycled.range, 2..6);
    editor.update_in(cx, |e, window, cx| e.left(&Left, window, cx));
    assert!(editor.read_with(cx, |e, _| e.cycled.is_some()));
    editor.update_in(cx, |e, window, cx| e.home(&Home, window, cx));
    assert!(editor.read_with(cx, |e, _| e.cycled.is_none()));
}

#[gpui::test]
fn clicking_the_version_handle_shows_the_next_version(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Draft one."]);
    editor.update_in(cx, |e, window, cx| {
        e.make_alternate(&MakeAlternate, window, cx);
        e.replace_text_in_range(None, " Two.", window, cx);
    });
    cx.simulate_resize(gpui::size(gpui::px(800.), gpui::px(600.)));
    cx.run_until_parked();
    let handle = editor.read_with(cx, |e, _| {
        let layout = e.layout.as_ref().expect("painted");
        assert_eq!(layout.version_handles.len(), 1);
        layout.version_handles[0].1
    });
    let before = editor.read_with(cx, |e, _| e.doc.paragraphs[0].active);
    cx.simulate_click(handle.center(), gpui::Modifiers::none());
    let after = editor.read_with(cx, |e, _| e.doc.paragraphs[0].active);
    assert_ne!(before, after);
    assert!(editor.read_with(cx, |e, _| e.cycled.is_some()));
}

#[gpui::test]
fn hidden_markers_record_no_handles(cx: &mut TestAppContext) {
    let (editor, cx) = editor_with(cx, &["Draft one."]);
    editor.update_in(cx, |e, window, cx| {
        e.make_alternate(&MakeAlternate, window, cx);
        e.toggle_markers(cx);
    });
    cx.simulate_resize(gpui::size(gpui::px(800.), gpui::px(600.)));
    cx.run_until_parked();
    let handles = editor.read_with(cx, |e, _| e.layout.as_ref().unwrap().version_handles.len());
    assert_eq!(handles, 0);
}
```

- [ ] **Step 2: Run them (expect fail)**

Run: `cargo test --workspace`
Expected: compile errors (`cycled`, `version_handles`, `markers`).

- [ ] **Step 3: Write `editor/markers.rs`**

```rust
//! Marker geometry: dotted underlines under alternate groups, the paragraph-version bar and its
//! handle, and the box around a group being cycled. All pure, so it is unit-tested.

use std::ops::Range;

use gpui::{Bounds, Pixels, Point, point, px, size};

/// Dot size and spacing of the dotted underline.
pub const DOT: f32 = 1.;
pub const DOT_STEP: f32 = 3.;
/// The version bar sits this far left of the text column.
pub const BAR_OFFSET: f32 = 12.;
pub const BAR_WIDTH: f32 = 1.5;
/// The handle's clickable area.
pub const HANDLE_WIDTH: f32 = 12.;
pub const HANDLE_HEIGHT: f32 = 20.;

/// Split `range` of a paragraph into its pieces on each visual line, as `(line, start..end)`.
/// `line_starts` is `ParaLayout::line_starts`; `len` is the paragraph's byte length.
pub fn split_by_lines(
    line_starts: &[usize],
    len: usize,
    range: Range<usize>,
) -> Vec<(usize, Range<usize>)> {
    let mut out = Vec::new();
    for (k, &start) in line_starts.iter().enumerate() {
        let end = line_starts.get(k + 1).copied().unwrap_or(len);
        let s = range.start.max(start);
        let e = range.end.min(end);
        if s < e {
            out.push((k, s..e));
        }
    }
    out
}

/// Left edges of the dots of a dotted line from `x0` to `x1` (every `DOT_STEP`, each `DOT` wide).
pub fn dot_xs(x0: f32, x1: f32) -> Vec<f32> {
    let mut xs = Vec::new();
    let mut x = x0;
    while x + DOT <= x1 + 0.01 {
        xs.push(x);
        x += DOT_STEP;
    }
    xs
}

/// Top of the dotted underline on a line whose top is `line_top`: just under the glyphs, which
/// are vertically centered in the line.
pub fn underline_y(line_top: Pixels, line_height: Pixels, font_size: Pixels) -> Pixels {
    line_top + (line_height + font_size) / 2. + px(1.)
}

/// The version bar for a paragraph whose top is `para_top` (window coordinates) and which is
/// `height` tall; `text_left` is the text column's left edge.
pub fn version_bar(text_left: Pixels, para_top: Pixels, height: Pixels) -> Bounds<Pixels> {
    Bounds::new(
        point(text_left - px(BAR_OFFSET) - px(BAR_WIDTH / 2.), para_top),
        size(px(BAR_WIDTH), height),
    )
}

/// The `⋮` handle's clickable area, centered on the bar at the paragraph's mid-height.
pub fn version_handle(text_left: Pixels, para_top: Pixels, height: Pixels) -> Bounds<Pixels> {
    let center = point(text_left - px(BAR_OFFSET), para_top + height / 2.);
    Bounds::new(
        point(
            center.x - px(HANDLE_WIDTH / 2.),
            center.y - px(HANDLE_HEIGHT / 2.),
        ),
        size(px(HANDLE_WIDTH), px(HANDLE_HEIGHT)),
    )
}

/// The three 2×2 dots that draw `⋮` inside `handle`.
pub fn handle_dots(handle: Bounds<Pixels>) -> [Bounds<Pixels>; 3] {
    let c = handle.center();
    [-5., -1., 3.].map(|dy| Bounds::new(point(c.x - px(1.), c.y + px(dy)), size(px(2.), px(2.))))
}

/// The cycled-group box around one line's piece of a group, from `x0` to `x1` on a line whose
/// top is `line_top`.
pub fn group_box(x0: Pixels, x1: Pixels, line_top: Pixels, line_height: Pixels) -> Bounds<Pixels> {
    Bounds::from_corners(
        point(x0 - px(2.), line_top + px(2.)),
        point(x1 + px(2.), line_top + line_height - px(2.)),
    )
}

/// The handle under `position`, as a paragraph index.
pub fn handle_at(handles: &[(usize, Bounds<Pixels>)], position: Point<Pixels>) -> Option<usize> {
    handles
        .iter()
        .find(|(_, bounds)| bounds.contains(&position))
        .map(|(para, _)| *para)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_split_across_wrapped_lines() {
        let starts = [0, 10, 20];
        assert_eq!(split_by_lines(&starts, 25, 2..5), vec![(0, 2..5)]);
        assert_eq!(
            split_by_lines(&starts, 25, 8..22),
            vec![(0, 8..10), (1, 10..20), (2, 20..22)]
        );
        assert_eq!(split_by_lines(&starts, 25, 10..10), vec![]);
        assert_eq!(split_by_lines(&[0], 5, 0..5), vec![(0, 0..5)]);
    }

    #[test]
    fn dots_every_three_pixels_within_the_span() {
        assert_eq!(dot_xs(10., 20.), vec![10., 13., 16., 19.]);
        assert_eq!(dot_xs(0., 0.5), Vec::<f32>::new());
        assert_eq!(dot_xs(0., 1.), vec![0.]);
    }

    #[test]
    fn underline_sits_below_centered_glyphs() {
        assert_eq!(underline_y(px(100.), px(36.), px(20.)), px(129.));
    }

    #[test]
    fn bar_and_handle_sit_left_of_the_column() {
        let bar = version_bar(px(200.), px(50.), px(72.));
        assert_eq!(bar.origin, point(px(187.25), px(50.)));
        assert_eq!(bar.size, size(px(1.5), px(72.)));
        let handle = version_handle(px(200.), px(50.), px(72.));
        assert_eq!(handle.center(), point(px(188.), px(86.)));
        assert_eq!(handle_at(&[(3, handle)], point(px(188.), px(90.))), Some(3));
        assert_eq!(handle_at(&[(3, handle)], point(px(200.), px(90.))), None);
        let dots = handle_dots(handle);
        assert_eq!(dots[1].origin, point(px(187.), px(85.)));
    }

    #[test]
    fn group_box_pads_the_piece() {
        let b = group_box(px(10.), px(50.), px(0.), px(36.));
        assert_eq!(b.origin, point(px(8.), px(2.)));
        assert_eq!(b.size, size(px(44.), px(32.)));
    }
}
```

Register it in `editor/mod.rs` with `mod markers;` (after `pub mod layout;`).

- [ ] **Step 4: Editor state for the cycled group**

`editor/mod.rs`:
- After `TrimSuggestion`:

```rust
/// The group (or paragraph, for versions) just cycled; boxed until the cursor leaves it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CycledGroup {
    pub para_id: ParaId,
    pub range: Range<usize>,
}
```

- Add the field `pub cycled: Option<CycledGroup>,` after `alt_edit`, initialized to `cycled: None,`.
- Set `self.cycled = None;` in `load` (next to `self.alt_edit = None;`), in `begin_edit` (right after `self.history.record(…)`) and in `after_history` (next to `self.alt_edit = None;`).
- In `set_selection`, after the `finish_alt_edit` check:

```rust
        if !self.in_cycled(self.selection.head) {
            self.cycled = None;
        }
```

- Add next to `finish_alt_edit`:

```rust
    /// Whether `pos` lies in the cycled group (edges included). False when there is none.
    fn in_cycled(&self, pos: DocPos) -> bool {
        self.cycled.as_ref().is_some_and(|c| {
            self.doc.para_index(c.para_id) == Some(pos.para)
                && c.range.start <= pos.offset
                && pos.offset <= c.range.end
        })
    }
```

`editor/features.rs`: import `CycledGroup` (`use super::{AltEdit, CycledGroup, Editor, TrimSuggestion};`). Replace the end of `cycle`, from `self.begin_edit(…)` on, and add `next_version` after it:

```rust
        self.begin_edit(EditKind::Other, head.para..head.para + 1);
        let info = self.doc.cycle(head, forward);
        if let Some(info) = &info {
            let offset = match info.level {
                AltLevel::Word => info.span.end,
                AltLevel::Paragraph => head.offset.min(info.span.end),
            };
            self.selection = Selection::caret(DocPos::new(head.para, offset));
        }
        self.end_edit(cx);
        self.cycled = info.map(|info| CycledGroup {
            para_id: self.doc.paragraphs[head.para].id,
            range: info.span,
        });
    }

    /// Show the next version of paragraph `para` (a click on its `⋮` handle).
    pub(crate) fn next_version(&mut self, para: usize, cx: &mut Context<Self>) {
        self.finish_alt_edit();
        let count = self.doc.paragraphs[para].versions.len();
        if count < 2 {
            return;
        }
        self.begin_edit(EditKind::Other, para..para + 1);
        let p = &mut self.doc.paragraphs[para];
        p.active = (p.active + 1) % count;
        let (id, len) = (p.id, p.len());
        self.selection = Selection::caret(DocPos::new(para, 0));
        self.end_edit(cx);
        self.cycled = Some(CycledGroup {
            para_id: id,
            range: 0..len,
        });
    }
```

`editor/actions.rs`, in `on_mouse_down`, right after the `let Some(layout) = … else { return; };`:

```rust
        if let Some(para) = super::markers::handle_at(&layout.version_handles, event.position)
            && para < self.doc.paragraphs.len()
        {
            self.next_version(para, cx);
            return;
        }
```

- [ ] **Step 5: Paint the markers**

`editor/layout.rs`: import `Bounds`, and add this field to `LayoutCache`:

```rust
    /// Clickable `⋮` handles of paragraphs with versions: (paragraph index, window bounds).
    pub version_handles: Vec<(usize, Bounds<Pixels>)>,
```

The test helper `cache()` sets `version_handles: Vec::new(),`.

`runs.rs`: remove the `alt_underline` and `ai_underline` fields and the `let alt = …` line. Replace the underline computation with:

```rust
            // Alternates are marked with painted dots (see `editor::markers`), not underlines.
            let underline = in_marked.then(|| UnderlineStyle {
                color: Some(style.color),
                thickness: px(1.),
                wavy: false,
            });
```

`element.rs`:
- Add `BorderStyle` and `outline` to the `gpui` import, and `use super::markers::{self, DOT};`.
- In `build_layout`, remove the `alt_underline` and `ai_underline` lines from the `RunStyle`, and add `version_handles: Vec::new(),` to the `LayoutCache` literal.
- Add after `build_layout`:

```rust
/// Dotted underlines under alternate groups, version bars with their handles (recorded in
/// `layout.version_handles` for clicks), and the box around a group just cycled.
fn marker_quads(editor: &Editor, layout: &mut LayoutCache) -> Vec<PaintQuad> {
    let t = &editor.theme;
    let lh = layout.line_height;
    let font_size = px(editor.config.editor.effective_font_size());
    let left = layout.origin.x;
    let mut quads = Vec::new();
    for (ix, (p, pl)) in editor.doc.paragraphs.iter().zip(&layout.paras).enumerate() {
        let para_top = layout.origin.y + pl.top;
        for span in p.spans() {
            let Some(mark) = span.alt else { continue };
            let color = if mark.ai { t.accent } else { t.text_muted };
            for (k, r) in markers::split_by_lines(&pl.line_starts, pl.line.len(), span.range) {
                let x0 = left + pl.x_in_line(k, r.start);
                let x1 = left + pl.x_in_line(k, r.end);
                let y = markers::underline_y(para_top + lh * k as f32, lh, font_size);
                for x in markers::dot_xs(f32::from(x0), f32::from(x1)) {
                    let dot = Bounds::new(point(px(x), y), size(px(DOT), px(DOT)));
                    quads.push(fill(dot, color));
                }
            }
        }
        if p.versions.len() > 1 {
            let height = pl.height(lh);
            let mut bar = t.accent;
            bar.a *= 0.6;
            quads.push(fill(markers::version_bar(left, para_top, height), bar));
            let handle = markers::version_handle(left, para_top, height);
            let c = handle.center();
            let gap = Bounds::new(point(c.x - px(2.), c.y - px(7.)), size(px(4.), px(14.)));
            quads.push(fill(gap, t.background));
            quads.extend(markers::handle_dots(handle).map(|dot| fill(dot, t.accent)));
            layout.version_handles.push((ix, handle));
        }
    }
    if let Some(cycled) = &editor.cycled
        && let Some(ix) = editor.doc.para_index(cycled.para_id)
        && let Some(pl) = layout.paras.get(ix)
    {
        let mut color = t.text_muted;
        color.a *= 0.4;
        let range = cycled.range.clone();
        for (k, r) in markers::split_by_lines(&pl.line_starts, pl.line.len(), range) {
            let line_top = layout.origin.y + pl.top + lh * k as f32;
            let x0 = left + pl.x_in_line(k, r.start);
            let x1 = left + pl.x_in_line(k, r.end);
            let bounds = markers::group_box(x0, x1, line_top, lh);
            quads.push(outline(bounds, color, BorderStyle::Solid).corner_radii(px(3.)));
        }
    }
    quads
}
```

- Add `markers: Vec<PaintQuad>,` to `EditorPrepaint`. In `prepaint`, make `layout` mutable and compute the markers after the selection:

```rust
        let mut layout = build_layout(editor, text_width, text_origin, window);
        let lh = layout.line_height;
        let selection = selection_quads(editor, &layout);
        let markers = if editor.show_markers {
            marker_quads(editor, &mut layout)
        } else {
            Vec::new()
        };
```

  Add `markers,` to the returned `EditorPrepaint`.
- In `paint`, after the text loop and before the cursor:

```rust
        for quad in prepaint.markers {
            window.paint_quad(quad);
        }
```

The bar and handle sit 12px left of the text, inside the 24px `GUTTER`, so they fall within the editor element's bounds and its `on_mouse_down` sees clicks on the handle.

- [ ] **Step 6: Run the tests (expect pass)**

Run: `cargo test --workspace` and clippy.
Expected: all pass, with no new warnings.

- [ ] **Step 7: Commit**

```bash
cargo fmt --all
jj describe -m "feat: dotted alternate markers, version bar with handle, cycled-group box

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 11: Markdown preview

**Files:**
- Modify: `crates/tulis/src/workspace.rs`, `crates/tulis/src/chrome.rs`

**Interfaces:**
- Consumes: `TogglePreview` and the `close_preview` binding (`escape` in the `Preview` context, Task 5), `storage::export_markdown`.
- Produces:
  - `Workspace.preview: Option<SharedString>`, a snapshot of `export_markdown(doc)` taken when the preview opens (shown options, no ghosts). `Workspace.preview_focus: FocusHandle`.
  - `Workspace::toggle_preview(&mut self, &TogglePreview, &mut Window, &mut Context<Self>)` (`pub(crate)`). On open it finishes alternate editing and focuses the preview. On close it focuses the page.
  - `Workspace::render_main` shows the page or the preview (`gpui_component::text::TextView::markdown`, selectable, read-only, inside the same centered column).
  - A header toggle button (`IconName::FileText`), selected while previewing.

- [ ] **Step 1: Write the failing test**

Add to the `workspace.rs` tests:

```rust
    #[gpui::test]
    fn preview_shows_export_markdown_and_escape_closes_it(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        ws.update_in(cx, |ws, window, cx| {
            ws.editor.update(cx, |e, cx| {
                e.replace_text_in_range(None, "I really love tea\nBye.", window, cx);
                e.doc.set_ghost(0, 2..9, true).unwrap();
            });
            ws.toggle_preview(&crate::actions::TogglePreview, window, cx);
        });
        assert_eq!(
            ws.read_with(cx, |ws, _| ws.preview.clone()).as_deref(),
            Some("I love tea\n\nBye.\n")
        );
        cx.run_until_parked();
        cx.simulate_keystrokes("escape");
        assert!(ws.read_with(cx, |ws, _| ws.preview.is_none()));
    }
```

- [ ] **Step 2: Run it (expect fail)**

Run: `cargo test --workspace preview_shows`
Expected: compile error (no `toggle_preview`).

- [ ] **Step 3: Implement**

`workspace.rs`:
- Imports: add `AnyElement`, `FocusHandle` and `SharedString` to `gpui`; add `use gpui_component::text::TextView;`; import `TogglePreview`.
- Add the fields `pub(crate) preview: Option<SharedString>,` and `preview_focus: FocusHandle,`, initialized to `preview: None, preview_focus: cx.focus_handle(),`.
- Add to the `impl Workspace` that holds `render_page`:

```rust
    /// Swap the page for a read-only Markdown preview (export text: shown options, no ghosts),
    /// or back.
    pub(crate) fn toggle_preview(
        &mut self,
        _: &TogglePreview,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.preview.take().is_some() {
            self.focus_editor(window, cx);
        } else {
            self.editor.update(cx, |e, _| {
                e.finish_alt_edit();
            });
            let md = storage::export_markdown(&self.editor.read(cx).doc);
            self.preview = Some(md.into());
            window.focus(&self.preview_focus, cx);
        }
        cx.notify();
    }

    fn render_preview(&self, md: SharedString) -> impl IntoElement {
        div()
            .id("md-preview")
            .key_context("Preview")
            .track_focus(&self.preview_focus)
            .flex_1()
            .h_full()
            .overflow_y_scroll()
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_center()
                    .py(px(COLUMN_PADDING_Y))
                    .px(px(32.))
                    .child(
                        div()
                            .w_full()
                            .max_w(px(COLUMN_WIDTH))
                            .child(TextView::markdown("md-preview-text", md).selectable(true)),
                    ),
            )
    }

    /// The page, or the Markdown preview while it is open.
    fn render_main(&self, cx: &App) -> AnyElement {
        match self.preview.clone() {
            Some(md) => self.render_preview(md).into_any_element(),
            None => self.render_page(cx).into_any_element(),
        }
    }
```

- In `render`, change `.child(self.render_page(cx))` to `.child(self.render_main(cx))`, and register `.on_action(cx.listener(Self::toggle_preview))`.

`chrome.rs`:
- Change the import to `use crate::actions::{ShowCheatSheet, ToggleMarkers, TogglePreview};`.
- In `render_header`, add `let previewing = self.preview.is_some();`. The center group becomes:

```rust
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new("toggle-markers")
                            .ghost()
                            .xsmall()
                            .icon(if show_markers {
                                IconName::Eye
                            } else {
                                IconName::EyeOff
                            })
                            .selected(show_markers)
                            .tooltip("Markers (Ctrl+.)")
                            .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                                ws.toggle_markers(&ToggleMarkers, window, cx)
                            })),
                    )
                    .child(
                        Button::new("toggle-preview")
                            .ghost()
                            .xsmall()
                            .icon(IconName::FileText)
                            .selected(previewing)
                            .tooltip("Markdown preview (Ctrl+Shift+M)")
                            .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                                ws.toggle_preview(&TogglePreview, window, cx)
                            })),
                    ),
            )
```

- [ ] **Step 4: Run the tests (expect pass)**

Run: `cargo test --workspace` and clippy.
Expected: all pass, with no new warnings.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
jj describe -m "feat: Markdown preview (Ctrl+Shift+M / gp, Esc closes)

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 12: Footer (save, open, share to X/LinkedIn, LAB, XYZ) and sharing

**Files:**
- Modify: `crates/tulis/src/chrome.rs`, `crates/tulis/src/workspace.rs`

**Interfaces:**
- Consumes: `share::{share_text, share_url, ShareTarget}` (Task 3), the actions `ShareX` and `ShareLinkedIn` (Task 5), `Workspace::{open_lab, toggle_overflow}` (Task 6), and the icons `Save` and `Share2` (Task 8).
- Produces:
  - `Workspace::share(&mut self, ShareTarget, &mut Context<Self>)`. It finishes alternate editing, takes `share_text` of the selection (or the whole document), and toasts "Nothing to share yet" if that's empty. Otherwise it writes the text to the clipboard, calls `cx.open_url(url)` and toasts "Copied — opening the browser", or "Copied — trimmed to 280 characters for X" when trimmed.
  - `Workspace::{share_x, share_linkedin}` action handlers, and `Workspace::render_footer`. `chrome::FOOTER_HEIGHT = 44.`
  - `Workspace::{save, open}` become `pub(crate)` (the footer buttons call them).

- [ ] **Step 1: Write the failing tests**

Add to the `workspace.rs` tests. `TestAppContext::opened_url` records `open_url` calls (`gpui-pre-0.3.10/src/app/test_app.rs:280`).

```rust
    #[gpui::test]
    fn share_copies_markdown_and_opens_the_intent_url(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        ws.update_in(cx, |ws, window, cx| {
            ws.editor.update(cx, |e, cx| {
                e.replace_text_in_range(None, "Tea & cake\nMore", window, cx);
                e.selection = Selection::caret(DocPos::new(0, 0));
            });
            ws.share(tulis_core::share::ShareTarget::X, cx);
        });
        assert_eq!(
            cx.read_from_clipboard().and_then(|c| c.text()).as_deref(),
            Some("Tea & cake\n\nMore")
        );
        assert_eq!(
            cx.opened_url().as_deref(),
            Some("https://x.com/intent/post?text=Tea%20%26%20cake%0A%0AMore")
        );
        ws.update_in(cx, |ws, window, cx| {
            ws.editor.update(cx, |e, _| {
                e.selection = Selection {
                    anchor: DocPos::new(0, 0),
                    head: DocPos::new(0, 3),
                };
            });
            ws.share_linkedin(&crate::actions::ShareLinkedIn, window, cx);
        });
        assert_eq!(
            cx.opened_url().as_deref(),
            Some("https://www.linkedin.com/feed/?shareActive=true&text=Tea")
        );
        let notice = ws.read_with(cx, |ws, cx| {
            ws.editor.read(cx).last_notice().map(str::to_string)
        });
        assert_eq!(notice.as_deref(), Some("Copied — opening the browser"));
    }

    #[gpui::test]
    fn sharing_an_empty_document_only_toasts(cx: &mut TestAppContext) {
        init(cx);
        let (ws, cx) = cx.add_window_view(|window, cx| {
            Workspace::new(Config::default(), None, None, window, cx)
        });
        ws.update(cx, |ws, cx| ws.share(tulis_core::share::ShareTarget::X, cx));
        assert_eq!(cx.opened_url(), None);
        let notice = ws.read_with(cx, |ws, cx| {
            ws.editor.read(cx).last_notice().map(str::to_string)
        });
        assert_eq!(notice.as_deref(), Some("Nothing to share yet"));
    }
```

- [ ] **Step 2: Run them (expect fail)**

Run: `cargo test --workspace share`
Expected: compile errors (no method `share`).

- [ ] **Step 3: Implement**

`chrome.rs`. The imports become:

```rust
use gpui::{
    Animation, AnimationExt, App, ClickEvent, ClipboardItem, Context, IntoElement, ParentElement,
    Styled, Window, div, prelude::*, px,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::{IconName, Selectable, Sizable};
use tulis_core::alternates::AltLevel;
use tulis_core::share::{self, ShareTarget};
use tulis_core::stats;
use tulis_core::vim::Mode;

use crate::actions::{
    Open, OpenLab, Save, ShareLinkedIn, ShareX, ShowCheatSheet, ToggleMarkers, ToggleOverflowPanel,
    TogglePreview,
};
```

Add `pub const FOOTER_HEIGHT: f32 = 44.;` after `HEADER_HEIGHT`, and these methods to the `impl Workspace`:

```rust
    /// Copy the selection (or the whole document) as Markdown and open the target's share page.
    pub(crate) fn share(&mut self, target: ShareTarget, cx: &mut Context<Self>) {
        self.editor.update(cx, |e, _| {
            e.finish_alt_edit();
        });
        let editor = self.editor.read(cx);
        let text = share::share_text(&editor.doc, Some(editor.selection.range()));
        if text.trim().is_empty() {
            self.editor
                .update(cx, |e, cx| e.set_notice("Nothing to share yet", cx));
            return;
        }
        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        let url = share::share_url(target, &text);
        cx.open_url(&url.url);
        let notice = if url.trimmed {
            "Copied — trimmed to 280 characters for X"
        } else {
            "Copied — opening the browser"
        };
        self.editor.update(cx, |e, cx| e.set_notice(notice, cx));
    }

    pub(crate) fn share_x(&mut self, _: &ShareX, _: &mut Window, cx: &mut Context<Self>) {
        self.share(ShareTarget::X, cx);
    }

    pub(crate) fn share_linkedin(
        &mut self,
        _: &ShareLinkedIn,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.share(ShareTarget::LinkedIn, cx);
    }

    /// Save/Open/Share on the left, LAB in the middle, XYZ (Overflow) on the right.
    pub(crate) fn render_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.editor.read(cx).theme;
        let overflow_open = self.show_overflow;
        let icon_button = |id: &'static str, tooltip: &'static str| {
            Button::new(id).ghost().xsmall().tooltip(tooltip)
        };
        div()
            .id("footer")
            .h(px(FOOTER_HEIGHT))
            .flex_none()
            .px_4()
            .flex()
            .flex_row()
            .items_center()
            .text_xs()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(
                        icon_button("save", "Save (Ctrl+S)")
                            .icon(gpui_kit_assets::IconName::Save)
                            .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                                ws.save(&Save, window, cx)
                            })),
                    )
                    .child(
                        icon_button("open", "Open (Ctrl+O)")
                            .icon(IconName::FolderOpen)
                            .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                                ws.open(&Open, window, cx)
                            })),
                    )
                    .child(
                        icon_button("share-x", "Share to X")
                            .icon(gpui_kit_assets::IconName::Share2)
                            .label("X")
                            .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                                ws.share_x(&ShareX, window, cx)
                            })),
                    )
                    .child(
                        icon_button("share-linkedin", "Share to LinkedIn")
                            .icon(gpui_kit_assets::IconName::Share2)
                            .label("in")
                            .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                                ws.share_linkedin(&ShareLinkedIn, window, cx)
                            })),
                    ),
            )
            .child(
                div()
                    .id("lab")
                    .px_3()
                    .py_0p5()
                    .rounded_md()
                    .border_1()
                    .border_dashed()
                    .border_color(t.accent)
                    .text_color(t.accent)
                    .cursor_pointer()
                    .hover(|s| s.bg(t.panel))
                    .child("LAB")
                    .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                        ws.open_lab(&OpenLab, window, cx)
                    })),
            )
            .child(
                div().flex_1().flex().flex_row().justify_end().child(
                    div()
                        .id("xyz")
                        .px_3()
                        .py_0p5()
                        .rounded_md()
                        .border_1()
                        .border_color(if overflow_open { t.accent } else { t.rule })
                        .text_color(if overflow_open { t.accent } else { t.text_muted })
                        .cursor_pointer()
                        .hover(|s| s.bg(t.panel))
                        .child("XYZ")
                        .on_click(cx.listener(|ws, _: &ClickEvent, window, cx| {
                            ws.toggle_overflow(&ToggleOverflowPanel, window, cx)
                        })),
                ),
            )
    }
```

`workspace.rs`:
- Make `save` and `open` `pub(crate)`.
- In `render`, register `.on_action(cx.listener(Self::share_x))` and `.on_action(cx.listener(Self::share_linkedin))`, and add `.child(self.render_footer(cx))` just before `.child(self.render_toasts(cx))`.

`cx.open_url` returns `()`, so a failed browser launch can't be detected. The clipboard copy has already happened, and the toast says so.

- [ ] **Step 4: Run the tests (expect pass)**

Run: `cargo test --workspace` and clippy.
Expected: all pass, with no new warnings.

- [ ] **Step 5: Commit**

```bash
cargo fmt --all
jj describe -m "feat: footer with save, open, share to X/LinkedIn, LAB and XYZ

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

### Task 13: README, smoke checklist and final verification

**Files:**
- Modify: `README.md`

**Interfaces:** none (docs only).

- [ ] **Step 1: Update the README**

- Keys table: add `Ctrl+K T` (cycle theme), `Ctrl+.` (markers), `Ctrl+Shift+M` (Markdown preview, Esc closes), `Ctrl+?` (cheat sheet), `Ctrl+L` (the Lab; runs the document trim until Part 4) and `Ctrl+\` (Overflow; unchanged). Mention the footer buttons.
- Vim line: add `gt gm gp g? gl gx`.
- Config block:

```toml
[editor]
vim_mode = false
font_family = "IBM Plex Mono"   # falls back to IBM Plex Mono, JetBrains Mono, then the platform mono font
font_size = 18
theme = "midnight"              # midnight | paper | sepia | black (Ctrl+K T cycles for the session)
long_sentence_words = 30        # used by the Lab (Part 4)
```

- Smoke checklist: replace the first item, "status bar shows name, word count", with "header shows word/char counts; window title shows the file name and • when unsaved". Change "underline on alternates; `alt i/n` in status bar" to "dotted underline on alternates; `alt i/n` badge in the header". Add:

```markdown
- [ ] Midnight theme on first start; Ctrl+K T / `gt` cycles Midnight → Paper → Sepia → Black with a toast; buttons and dialogs follow the theme
- [ ] Unknown `theme` in config shows a toast and uses Midnight
- [ ] Text column is 640px and centered; text fades out under the top and bottom edges while scrolling
- [ ] A first paragraph starting with `# ` is bold and its `# ` is invisible (but the cursor still moves over it)
- [ ] Alternates show a dotted underline (accent when AI-written); paragraphs with versions show a bar left of the column with a `⋮` handle; clicking the handle shows the next version
- [ ] Alt+↓ draws a faint box around the cycled group until the cursor leaves it
- [ ] Ctrl+. / `gm` hides and shows all markers
- [ ] Header fades after ~2s of continuous typing and returns on mouse move or a 2s pause
- [ ] Ctrl+Shift+M / `gp` shows the Markdown preview without ghosts; Esc returns
- [ ] Ctrl+? / `g?` / keyboard icon opens the cheat sheet; typing filters; Esc closes
- [ ] Footer: Save and Open work; Share X/in copy the text and open the browser (X trims to 280 with a toast); LAB runs the trim; XYZ toggles Overflow
- [ ] Toasts appear centered above the footer, at most two, and fade after ~3s
- [ ] Vim mode shows a NORMAL/INSERT badge next to the counts
```

- [ ] **Step 2: Full verification**

Run: `cargo fmt --all --check && cargo test --workspace && cargo clippy --workspace --all-targets 2>&1 | grep generated`
Expected: formatting is clean, all tests pass (about 88 app and 124 core) and the clippy counts match the Task 1 baseline.

Run: `cargo build && timeout 5 target/debug/tulis /tmp/smoke.md; echo $?`
Expected: `124`, with no panic on stderr.

Then walk through the new smoke-checklist items by hand. Report any that fail rather than ticking them.

- [ ] **Step 3: Commit**

```bash
jj describe -m "docs: README keys, config and smoke checklist for redesign part 1

Co-Authored-By: Claude <model> <noreply@anthropic.com>" && jj new
```

---

## Decisions this plan makes where the spec is silent or ambiguous

- **Platform crate.** gpui-pre ships without a platform backend. The app adds `gpui-pre-platform` (`gpui_platform::application()`) with the `wayland`, `x11` and `font-kit` features.
- **Key bindings.** `KeyBinding::new` panics on `secondary-` in gpui-pre, so every binding is built with `KeyBinding::load`. Without this, Task 1's app would crash at startup.
- **Toasts are Tulis's own**, not gpui-component notifications (which stack top-right). They sit centered above the footer, at most 2, and a repeat of the newest toast restarts it.
- **Share failure.** `cx.open_url` can't report failure, so the toast always says "Copied — opening the browser". X text is cut to its first 280 characters (trailing whitespace dropped, no ellipsis). A selection's paragraphs are joined with a blank line.
- **Counts** leave out the title's `# ` and line breaks. "1 word" and "1 char" are singular.
- **Status-bar contents** move as follows: the file name and dirty dot go to the window title, and vim mode, `alt i/n`/`ver i/n` and `AI…` become header badges.
- **Placeholders until later parts.** The `⋮` handle shows the next version (Part 2 opens the sidebar). LAB, Ctrl+L and `gl` run the v1 document trim (Part 4 replaces it). XYZ and `gx` toggle the existing Overflow panel (Part 3 replaces it).
- **Ctrl+?** is bound as both `secondary-?` and `secondary-shift-/`, because keyboards report `?` differently.
- **The cycled-group box** also covers whole paragraphs when versions are cycled.
- **Vim view commands** (`gt gm gp g? gl gx`) keep Visual mode. Part 2's selection commands are what return to Normal.
- **The gutter.** The editor element reserves 24px on each side, so the version bar and handle lie inside it and receive clicks.
- **The preview** renders the Markdown with gpui-component's `TextView`, from a snapshot taken when it opens.
- **The markers toggle** is session-only and on by default. It isn't in config.
