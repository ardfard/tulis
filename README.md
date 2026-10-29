# Tulis

A writing app for playing with your words: keep alternates inline, ghost passages to judge a
paragraph without them, stash text in an overflow panel, and ask an AI for alternatives or cuts
(never rewrites). Built with Rust and [GPUI](https://www.gpui.rs/).

## Run

```bash
cargo run --release -p tulis -- path/to/essay.md
```

Files are plain Markdown (`essay.md`). Alternates, ghosts and overflow live in a sidecar next to it
(`essay.md.tulis.json`). If the Markdown is edited elsewhere, Tulis keeps the metadata of every
paragraph that still matches and tells you how many paragraphs lost theirs.

## Keys

`Ctrl` is `Cmd` on macOS.

| Key | Action |
|---|---|
| `Ctrl+D` | Make an alternate of the selection (then type the new wording), or a new version of the paragraph |
| `Alt+↑` / `Alt+↓` | Cycle alternates or paragraph versions |
| `Ctrl+Enter` | Accept: keep only the shown alternate/version |
| `Ctrl+Shift+Backspace` | Delete the shown alternate/version |
| `Ctrl+G` | Ghost / revive the selection or paragraph |
| `Ctrl+Shift+O` | Move the selection or paragraph to overflow |
| `Ctrl+\` | Show/hide the overflow panel |
| `Ctrl+J` | AI alternates for the selection or paragraph |
| `Ctrl+Shift+T` / `Ctrl+Shift+Alt+T` | AI trim suggestions for the paragraph / document; `Tab` accepts (ghosts), `Esc` dismisses |
| `Ctrl+Alt+V` | Toggle vim mode |
| `Ctrl+S` / `Ctrl+Shift+S` / `Ctrl+O` / `Ctrl+N` / `Ctrl+Q` | Save / Save as / Open / New / Quit |

Vim mode: `h j k l w b e 0 ^ $ gg G { }` with counts; `i a I A o O`; `d c y` + motion, `dd cc yy`,
`x p P u Ctrl+R`; `v`/`V`; `]a`/`[a` cycle alternates; `gh` toggles ghost.

## Config

`~/.config/tulis/config.toml` (macOS: `~/Library/Application Support/tulis/config.toml`):

```toml
[editor]
vim_mode = false
font_family = "Iowan Old Style"
font_size = 18

[ai]
provider = "openai"                         # any OpenAI-compatible endpoint, or "anthropic"
model = "openrouter/auto"                   # any OpenRouter model id, e.g. "anthropic/…"
api_key_env = "OPENROUTER_API_KEY"          # the key is read from this environment variable
base_url = "https://openrouter.ai/api/v1"   # e.g. "http://localhost:11434/v1" for Ollama
# Anthropic directly: provider = "anthropic", model = "claude-haiku-5-5",
#                     api_key_env = "ANTHROPIC_API_KEY", base_url = ""
num_alternates = 3
```

## Development

```bash
cargo test --workspace
```

`vendor/xattr` is `xattr 0.2.3` with a one-line Linux fix. GPUI 0.2.2 depends on it indirectly, and
the published version no longer compiles against current `libc`.

### Manual smoke checklist

- [ ] Window opens; text wraps in a centered column; status bar shows name, word count
- [ ] Typing, Enter, Backspace/Delete, arrows (Up/Down follow wrapped lines), Home/End, PageUp/Down
- [ ] Mouse click, drag-select, double-click word, triple-click paragraph
- [ ] Copy/cut/paste, undo/redo (typing undoes in chunks)
- [ ] IME composition (e.g. Japanese/Chinese input method) shows underlined marked text and commits
- [ ] Caret stays visible when typing past the bottom; scrolling with the wheel works
- [ ] `Ctrl+D` → type → `Alt+↑/↓` swaps; underline on alternates; `alt i/n` in status bar
- [ ] `Ctrl+G` ghosts to ~10% and back; ghosted text absent from saved `.md`
- [ ] Overflow: send, panel toggle, click to insert, × to remove, "keep after insert"
- [ ] Save/Open round-trips alternates, ghosts, overflow; external edit shows drift notice
- [ ] Unsaved-changes prompt on New/Open/Quit/window close
- [ ] AI alternates (violet underline) and trim (red strike, Tab/Esc) with a real key; missing key notice without
- [ ] Vim mode: motions, operators, visual, `]a`, `gh`; `vim_mode = true` starts in Normal
- [ ] A missing font family falls back to a readable font
