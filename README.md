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
| `Ctrl+K T` | Cycle theme (Midnight, Paper, Sepia, Black) for the session |
| `Ctrl+.` | Show/hide alternate and version markers |
| `Ctrl+Shift+M` | Markdown preview (`Esc` closes) |
| `Ctrl+?` | Cheat sheet |
| `Ctrl+L` | The Lab (runs the document trim until Part 4) |
| `Ctrl+S` / `Ctrl+Shift+S` / `Ctrl+O` / `Ctrl+N` / `Ctrl+Q` | Save / Save as / Open / New / Quit |

Vim mode: `h j k l w b e 0 ^ $ gg G { }` with counts; `i a I A o O`; `d c y` + motion, `dd cc yy`,
`x p P u Ctrl+R`; `v`/`V`; `]a`/`[a` cycle alternates; `gh` toggles ghost; `gt gm gp g? gl gx`
(theme, markers, preview, cheat sheet, Lab, Overflow).

The footer has Save, Open, Share to X and LinkedIn, LAB and XYZ (Overflow) buttons.

## Config

`~/.config/tulis/config.toml` (macOS: `~/Library/Application Support/tulis/config.toml`):

```toml
[editor]
vim_mode = false
font_family = "IBM Plex Mono"   # falls back to IBM Plex Mono, JetBrains Mono, then the platform mono font
font_size = 18
theme = "midnight"              # midnight | paper | sepia | black (Ctrl+K T cycles for the session)
long_sentence_words = 30        # used by the Lab (Part 4)

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

### Manual smoke checklist

- [ ] Window opens; text wraps in a centered column; header shows word/char counts; window title shows the file name and • when unsaved
- [ ] Typing, Enter, Backspace/Delete, arrows (Up/Down follow wrapped lines), Home/End, PageUp/Down
- [ ] Mouse click, drag-select, double-click word, triple-click paragraph
- [ ] Copy/cut/paste, undo/redo (typing undoes in chunks)
- [ ] IME composition (e.g. Japanese/Chinese input method) shows underlined marked text and commits
- [ ] Caret stays visible when typing past the bottom; scrolling with the wheel works
- [ ] `Ctrl+D` → type → `Alt+↑/↓` swaps; dotted underline on alternates; `alt i/n` badge in the header
- [ ] `Ctrl+G` ghosts to ~10% and back; ghosted text absent from saved `.md`
- [ ] Overflow: send, panel toggle, click to insert, × to remove, "keep after insert"
- [ ] Save/Open round-trips alternates, ghosts, overflow; external edit shows drift notice
- [ ] Unsaved-changes prompt on New/Open/Quit/window close
- [ ] AI alternates (violet underline) and trim (red strike, Tab/Esc) with a real key; missing key notice without
- [ ] Vim mode: motions, operators, visual, `]a`, `gh`; `vim_mode = true` starts in Normal
- [ ] A missing font family falls back to a readable font
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

## License

[MIT](LICENSE).
