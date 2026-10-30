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
| `Ctrl+Shift+A` (or `Ctrl+D`) | Alternatives sidebar for the selection or the word under the cursor: Word/Selection, Sentence and Paragraph tabs; `↑/↓` or `j/k` preview, `Enter` keeps, `Delete` or `dd` removes, `i`/`o` type a new option, `Ctrl+Z`/`Ctrl+Shift+Z` undo/redo, `Esc` closes (so does `Esc` on the page when there is nothing else to cancel). The page dims around the target while the sidebar has focus |
| `Alt+↑` / `Alt+↓` | Cycle alternates or paragraph versions |
| `Ctrl+Enter` | Accept: keep only the shown alternate/version |
| `Ctrl+Shift+Backspace` | Delete the shown alternate/version |
| `Ctrl+/` (or `Ctrl+G`) | Ghost / revive the selection or paragraph |
| `Ctrl+Shift+X` (or `Ctrl+Shift+O`) | Stash the selection or paragraph in overflow |
| `Ctrl+\` | Show/hide the overflow panel |
| `Ctrl+Shift+G` (or `Ctrl+J`) | AI alternatives for the selection, else the paragraph (as versions); an open sidebar follows to that target. In the sidebar (or its "✦ Ask AI" row): for the sidebar's target |
| `Ctrl+Shift+T` / `Ctrl+Shift+Alt+T` | AI trim suggestions for the paragraph / document; `Tab` accepts (ghosts), `Esc` dismisses |
| `Ctrl+Alt+V` | Toggle vim mode |
| `Ctrl+K T` | Cycle theme (Midnight, Paper, Sepia, Black) for the session |
| `Ctrl+.` | Show/hide alternate and version markers |
| `Ctrl+Shift+M` | Markdown preview (`Esc` closes) |
| `Ctrl+?` | Cheat sheet |
| Right-click (or `Shift+F10` / the Menu key, at the caret) | Menu: Alternatives / AI alternatives for the selection (or the word under the pointer), Ghost it / Revive it, Stash this in Overflow |
| `Ctrl+L` | The Lab (runs the document trim until Part 4) |
| `Ctrl+S` / `Ctrl+Shift+S` / `Ctrl+O` / `Ctrl+N` / `Ctrl+Q` | Save / Save as / Open / New / Quit |

Vim mode: `h j k l w b e 0 ^ $ gg G { }` with counts; `i a I A o O`; `d c y` + motion, `dd cc yy`,
`x p P u Ctrl+R`; `v`/`V`; `]a`/`[a` cycle alternates; `gh` toggles ghost; `ga` sidebar, `gA` AI alternatives, `gs` stash (in Visual mode these act on the selection); `gt gm gp g? gl gx`
(theme, markers, preview, cheat sheet, Lab, Overflow). In `V` (VisualLine) mode `ga gA gs gh` act on whole paragraphs.

Sentence tab: sentences end at `.`, `!` or `?` followed by a space or the end of the paragraph.
Known gaps: a sentence ending in an abbreviation such as `etc.` or `e.g.` runs on into the next one;
other dotted abbreviations end a sentence ("The U.S. economy" splits after "U.S."); CJK `。` does
not end a sentence.

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
- [ ] `Alt+↑/↓` swaps a group's options; dotted underline on alternates; `alt i/n` badge in the header
- [ ] `Ctrl+/` (and `Ctrl+G`) ghosts to ~10% and back; ghosted text absent from saved `.md`
- [ ] Overflow: send, panel toggle, click to insert, × to remove, "keep after insert"
- [ ] Save/Open round-trips alternates, ghosts, overflow; external edit shows drift notice
- [ ] Unsaved-changes prompt on New/Open/Quit/window close
- [ ] AI alternates (violet underline) and trim (red strike, Tab/Esc) with a real key; missing key notice without
- [ ] Vim mode: motions, operators, visual, `]a`, `gh`, `ga` (sidebar), `gA` (AI alternatives), `gs` (stash), also from `v`/`V` (`V` acts on whole paragraphs); `vim_mode = true` starts in Normal
- [ ] A missing font family falls back to a readable font
- [ ] Midnight theme on first start; Ctrl+K T / `gt` cycles Midnight → Paper → Sepia → Black with a toast; buttons and dialogs follow the theme
- [ ] Unknown `theme` in config shows a toast and uses Midnight
- [ ] Text column is 640px and centered; text fades out under the top and bottom edges while scrolling
- [ ] A first paragraph starting with `# ` is bold and its `# ` is invisible (but the cursor still moves over it)
- [ ] Alternates show a dotted underline (accent when AI-written); paragraphs with versions show a bar left of the column with a `⋮` handle; clicking the handle opens the sidebar on its Paragraph tab
- [ ] Alt+↓ draws a faint box around the cycled group until the cursor leaves it
- [ ] Ctrl+. / `gm` hides and shows all markers
- [ ] Header fades after ~2s of continuous typing and returns on mouse move or a 2s pause
- [ ] Ctrl+Shift+M / `gp` shows the Markdown preview without ghosts; Esc returns
- [ ] Ctrl+? / `g?` / keyboard icon opens the cheat sheet; typing filters; Esc closes
- [ ] Footer: Save and Open work; Share X/in copy the text and open the browser (X trims to 280 with a toast); LAB runs the trim; XYZ toggles Overflow
- [ ] Toasts appear centered above the footer, at most two, and fade after ~3s
- [ ] Vim mode shows a NORMAL/INSERT badge next to the counts
- [ ] Ctrl+Shift+A / Ctrl+D / `ga` opens a 240px sidebar on the left; while it has focus the page dims except the target paragraph; tabs read Word (Selection with a selection), Sentence, Paragraph in italic accent
- [ ] Rows show `●` for the shown option, `·` for the others and `✦` for AI-written ones; long options clip at 3 lines
- [ ] j/k or arrows preview options in the page; one Ctrl+Z after closing undoes the whole preview session
- [ ] Typing in "type another way to say it…" + Enter adds and shows the option; opening and closing without adding leaves no group behind
- [ ] Adding on the Sentence tab over a word group toasts "Inner alternates merged — Ctrl+Z to restore"
- [ ] "✦ Ask AI" (or Ctrl+Shift+G in the sidebar) appends AI options to the tab's target
- [ ] Delete / `dd` removes the highlighted option but never the last; Esc closes and keeps what is shown
- [ ] Deleting the target paragraph (or typing inside the target) closes the sidebar
- [ ] Right-click a word: the menu targets it (or the selection); Ghost it turns into Revive it on ghosted text; Stash moves it to Overflow
- [ ] Ctrl+Z / Ctrl+Shift+Z work while the sidebar has focus; Esc on the page (with no AI request, trims or alternate edit pending) closes the sidebar
- [ ] Shift+F10 or the Menu key opens the right-click menu at the caret

## License

[MIT](LICENSE).
