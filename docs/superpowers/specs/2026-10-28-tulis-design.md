# Tulis — Design Spec

Date: 2026-10-28
Status: Approved for planning

## Summary

Tulis is a desktop writing app in Rust and GPUI. It is modeled on Jason Fried's
"Write_On". Its idea is *alternative control* rather than version control: the
writer keeps multiple versions of words, sentences and paragraphs inline, cycles
between them, dims ("ghosts") text to judge a passage without it, parks text in
an overflow panel, and asks an LLM for alternatives or for cuts it would suggest.
An optional vim mode covers navigation and basic editing.

## Goals (v1)

1. Inline alternates at word/selection level and at paragraph level, cycled from the keyboard.
2. Ghosting and reviving text, with ghosts left out of export and word count.
3. An overflow panel for stashed text.
4. AI alternatives, stored as options in the same alternate group and marked as AI-made.
5. AI trim: the model flags suggested cuts, never rewrites, and accepting a cut ghosts it.
6. Markdown file plus a JSON sidecar holding the full model.
7. An optional vim mode (Normal / Insert / Visual) with a core set of motions and operators.
8. Pluggable LLM provider: Anthropic and OpenAI-compatible.

## Non-goals (v1)

- Rich formatting (bold/italic/headings rendering). The text is treated as plain prose paragraphs.
- Collaboration, sync, cloud storage.
- Vim text objects, `.` repeat, named registers, macros, `:` commands, search.
- Spell check.
- Multiple documents in tabs (one document per window).

## Architecture

Cargo workspace with two crates:

| Crate | Depends on GPUI | Contents |
|---|---|---|
| `tulis-core` | no | document model, edit operations, undo history, storage (Markdown + sidecar), vim state machine, AI providers and prompts, config |
| `tulis` (bin) | yes | window setup, editor view + custom text element, overflow panel, status bar, keymap, wiring of async AI requests |

`tulis-core` must compile and test without GPUI. All logic that can be expressed
without a window lives there. The app crate is a thin rendering and input layer.

## 1. Document model (`tulis-core::model`)

```
Document
├── paragraphs: Vec<Paragraph>
└── overflow:   Vec<OverflowItem>

Paragraph    { id: ParaId, versions: Vec<ParaVersion>, active: usize }
ParaVersion  { segments: Vec<Segment>, source: Source }
Segment      { kind: SegKind, ghost: bool }
SegKind      = Plain(String)
             | Alt { options: Vec<AltOption>, active: usize }
AltOption    { text: String, source: Source }
OverflowItem { id: OverflowId, text: String, created_at: DateTime, from_paragraph: Option<ParaId> }
Source       = Human | Ai { provider: String, model: String, kind: AiKind }
AiKind       = Rewrite | Trim
```

Invariants:

- Every `Paragraph` has at least one version, and `active` is a valid index.
- Every `Alt` has at least one option, and `active` is a valid index.
- Nesting is limited to two levels: paragraph versions contain segments, and alt options are flat strings.
- No empty `Plain` segments, except a lone empty `Plain` in an empty paragraph.
- Adjacent `Plain` segments with the same `ghost` flag are merged (normalization runs after every operation).
- Text never contains `\n`. Paragraph breaks are the boundaries between `Paragraph`s.

**Rendered text** of a paragraph is the concatenation of each segment's visible
text (the `Plain` string, or the active alt option's text), *including* ghosts.
**Export text** is the same with ghosted segments dropped.

**Positions.** `DocPos { para: usize, offset: usize }`, where `offset` is a byte
offset into the paragraph's rendered text and always lies on a char boundary.
`Paragraph::locate(offset) -> (segment index, offset in segment)` maps to the
segment, and its inverse maps back. Positions use bytes (matching GPUI's
`TextLayout`). The input handler converts to and from UTF-16 where GPUI requires it.

**Operations** (all pure functions or methods on `Document`, unit-tested):

- `insert(pos, text)`: inserts into the segment at `pos`. Text containing `\n` splits the paragraph.
  At a boundary between a `Plain` and an `Alt`, the text goes into the `Plain` side, so typing
  right after an alternate doesn't silently extend it. If there's no `Plain` on that side,
  one is created. To edit an alternate's text, place the cursor strictly inside it.
- `delete(range)`: deletes across segments and paragraphs. Segments that become empty are removed,
  an `Alt` whose active option becomes empty is removed, and paragraphs are joined when the range
  spans a paragraph boundary. The joined paragraph keeps only the active version of each side.
- `make_alt(range)`: the selection must lie in one paragraph. Segments in the range are flattened to
  a single string, which becomes option 1 (`Human`); a copy becomes option 2, and option 2 is active.
  A selection that already lies exactly on one `Alt` adds a new option (a copy of the active one)
  to that group instead.
- `make_para_version(para)`: clones the active version as a new active version.
- `cycle(pos, dir)`: if `pos` is inside an `Alt` or at either edge of one (for cycling and
  accepting, edges count as inside; the preceding `Alt` wins if two are adjacent), cycles its options. Otherwise, if the paragraph has
  more than one version, cycles versions. Otherwise does nothing. Wraps around.
- `accept(pos)`: collapses the `Alt` at `pos` to a `Plain` of its active option, or drops every
  paragraph version except the active one.
- `delete_option(pos)`: removes the active option or version. Removing the last remaining one
  collapses the group to `Plain`.
- `toggle_ghost(range | para)`: splits segments at the range edges and flips the `ghost` flag. If
  every segment in the range is already ghosted it revives them, otherwise it ghosts them all.
- `to_overflow(range | para)`: removes the text (export text, active versions only) and pushes an
  `OverflowItem`.
- `insert_overflow(id, pos, remove: bool)`
- `add_ai_options(target, texts, source)`: appends options to an existing or newly created `Alt`
  (or new paragraph versions) and makes the first new one active.

**Undo** (`tulis-core::history`): a snapshot stack of `(Document, Selection)`.
Each operation pushes a snapshot first. Consecutive single-character inserts and
deletes in the same paragraph are coalesced into one undo step until there's a
pause of over 1s, a cursor jump, or a non-typing operation. Redo stack is cleared on
a new edit. Depth limit 500.

## 2. Storage (`tulis-core::storage`)

Files: `essay.md` and `essay.md.tulis.json`.

- `essay.md` is the export: each paragraph's export text, paragraphs separated by
  `\n\n`, with a trailing newline. Paragraphs whose export text is empty are skipped.
- The sidecar is JSON holding `{ format_version: 1, md_hash: <sha256 hex of the .md bytes written>, document: Document }`.

**Save:** write the `.md` to a temp file in the same directory, fsync, and rename. Then write
the sidecar the same way. Save is only successful if both succeed.

**Open:**

1. Read the `.md`. Split it into paragraphs on blank lines (one or more empty or whitespace-only
   lines). Within a paragraph, single newlines are joined with a space.
2. If there's no sidecar: build a plain document from the paragraphs.
3. If there's a sidecar and `md_hash` matches: load the sidecar document.
4. If there's a sidecar and the hash doesn't match (**drift**): build a plain document from the `.md`.
   Then, for each `.md` paragraph, if a sidecar paragraph with identical export text exists and hasn't
   already been used, use that sidecar paragraph (keeping its metadata) instead. Overflow is always
   restored from the sidecar. Report drift: "essay.md changed outside Tulis; N paragraphs lost
   alternates/ghosts".
5. An unreadable or unknown-version sidecar is treated as missing, and a warning is reported.

## 3. Editor (`tulis` app crate)

**Window layout:** a centered writing column (max width 680px, configurable serif
font), a toggleable overflow panel on the right (about 280px), and a bottom status bar.

**Status bar:** file name and dirty marker; word count (export text); `alt i/n` when the
cursor is in an alternate group, or `ver i/n` on a multi-version paragraph; vim mode
(`NORMAL`/`INSERT`/`VISUAL`) when enabled; the AI state (`AI…` while in flight); and the last
notice or error message.

**Custom text element:**

- For each paragraph, builds `TextRun`s from segments: normal color; ghost at 10% alpha; a dotted
  underline on `Alt` segments; a dotted underline in a different tint when the active option is
  AI-sourced; and a red strikethrough overlay on trim suggestions.
- Shapes each paragraph with the GPUI text system into wrapped lines at the column width, and caches
  the layouts per paragraph (invalidated when the paragraph or the width changes).
- Paints the selection, then the text, then the cursor (a blinking bar in Insert, a block in vim Normal).
- Mouse: click to place the cursor, drag to select, double-click to select a word, triple-click
  to select a paragraph. Hit-testing uses the cached layouts.
- Implements `EntityInputHandler`, which covers text insertion and IME marked text (rendered underlined).
- Scrolls to keep the cursor visible.

**Selection:** `{ anchor: DocPos, head: DocPos }`. It may span paragraphs. Operations that need
a single paragraph (`make_alt`) report "selection must be within one paragraph" when it doesn't.

**Keymap** (`secondary` = Ctrl on Linux, Cmd on macOS):

| Key | Action |
|---|---|
| arrows, Home/End, PageUp/Down (+Shift to extend) | movement; Up/Down move by visual line |
| `alt-left/right` | word movement |
| `secondary-d` | make alternate (selection) / new paragraph version (no selection) |
| `alt-up` / `alt-down` | cycle alternate or version |
| `secondary-enter` | accept |
| `secondary-shift-backspace` | delete active option/version |
| `secondary-g` | toggle ghost (selection or paragraph) |
| `secondary-shift-o` | send to overflow |
| `secondary-\` | toggle overflow panel |
| `secondary-j` | AI alternates |
| `secondary-shift-t` | AI trim (paragraph) |
| `secondary-shift-alt-t` | AI trim (document) |
| `tab` | accept the trim suggestion under the cursor, or the next one (when suggestions exist) |
| `escape` | dismiss trim suggestions / cancel in-flight AI / (vim) to Normal |
| `secondary-z` / `secondary-shift-z` | undo / redo |
| `secondary-s` / `secondary-shift-s` / `secondary-o` / `secondary-n` | save / save as / open / new |
| `secondary-alt-v` | toggle vim mode |
| `secondary-c` / `secondary-x` / `secondary-v` / `secondary-a` | copy / cut / paste / select all (copy uses rendered text) |

**Overflow panel:** a list of items showing a preview (first ~3 lines), with the newest
item at the top. Clicking an item inserts it at the cursor. A small "×" on each item removes
it, and a "keep in overflow" checkbox at the top of the panel decides whether items are removed
after inserting (default: removed).

**Trim suggestions** are view state, not saved: `Vec<{ para_id, byte range }>`. They're
drawn as overlays and cleared for a paragraph when that paragraph is edited. Accepting one
ghosts that range (an undoable operation).

**Dialogs:** file open/save use GPUI's platform prompts (`prompt_for_paths`,
`prompt_for_new_path`). The unsaved-changes confirmation uses `window.prompt`.

## 4. Vim mode (`tulis-core::vim`)

A pure state machine: `Vim::feed(key) -> Vec<VimCommand>`, plus a `mode()` accessor.
`VimCommand` values are abstract (`Move(Motion, count)`, `Delete(Motion)`, `EnterInsert(Where)`,
`Yank(...)`, `Put(Before|After)`, `Undo`, `Redo`, `CycleAlt(dir)`, `ToggleGhost`,
`SetMode(...)`, ...). The editor carries them out using the same motion and edit API as the
normal keymap.

- Modes: Normal, Insert, Visual (charwise), VisualLine.
- Motions: `h j k l` (j/k by visual line), `w b e`, `0 ^ $`, `gg G`, `{ }` (paragraphs); counts.
- Insert entry: `i a I A o O`.
- Operators: `d c y` combined with any motion; `dd cc yy` (paragraph-wise, since a paragraph is one
  logical line); `x`, `p P`, `u`, `ctrl-r`.
- Visual: motions extend the selection; `d c y x` act on it; `secondary-*` app actions use it.
- App extras in Normal: `]a`/`[a` cycle alternates, `gh` toggles ghost.
- A single unnamed register, which is separate from the system clipboard.
- In Normal and Visual mode, text from `EntityInputHandler` goes to `Vim::feed` instead of being
  inserted. `Esc` → Normal. Bindings that start with `secondary-` and `alt-` keep working in every mode.
- Toggle: `secondary-alt-v`, initial state from config.

## 5. AI (`tulis-core::ai`)

```rust
pub trait LlmProvider: Send + Sync {
    fn complete(&self, req: &CompletionRequest) -> Result<String, AiError>;
    fn name(&self) -> &str;   // "anthropic" | "openai"
    fn model(&self) -> &str;
}
CompletionRequest { system: String, user: String, max_tokens: u32 }
```

- `AnthropicProvider`: `POST https://api.anthropic.com/v1/messages` with the headers `x-api-key`
  and `anthropic-version: 2023-06-01`; takes the text of the first content block.
- `OpenAiCompatProvider`: `POST {base_url}/chat/completions` with `Authorization: Bearer`; takes
  `choices[0].message.content`.
- HTTP via `ureq` (blocking), 30s timeout. The app runs `complete` on the GPUI background executor.
  Cancelling drops the task, and its result is ignored.
- `AiError`: `MissingKey(env_var)`, `Http(status, body excerpt)`, `Network(msg)`, `Parse(msg)`.

**Alternates prompt** (`prompts::alternates`): the system prompt asks for N alternative phrasings of
the target text that fit the surrounding paragraph, keeping meaning and voice, and returned as a
JSON array of strings. The user message contains the paragraph's export text and the target text.
Parsing: find the first `[`…last `]` and parse it as JSON. If that fails, use non-empty lines with
list markers stripped. Empty results and duplicates of the original are dropped.

**Trim prompt** (`prompts::trim`): the system prompt asks for exact substrings that could be
deleted to tighten the prose, never rewriting, returned as a JSON array of strings. Validation: each
substring must appear in the paragraph's non-ghost rendered text. It is mapped to its first
occurrence that doesn't overlap an earlier match, and anything that doesn't match is dropped.
Document-level trim runs one request per non-empty paragraph, in sequence.

**Staleness:** each request records `(para_id, the target's rendered text)`. When applying the
result, if the paragraph is gone or its text differs, it is discarded with a notice.

## 6. Config (`tulis-core::config`)

Path: `$XDG_CONFIG_HOME/tulis/config.toml` (or `~/.config/tulis/config.toml`; on macOS
`~/Library/Application Support/tulis/config.toml`, via the `dirs` crate).

```toml
[editor]
vim_mode = false
font_family = "Iowan Old Style"
font_size = 18

[ai]
provider = "anthropic"            # "anthropic" | "openai"
model = "claude-haiku-5-5"
api_key_env = "ANTHROPIC_API_KEY"
base_url = ""                     # required for "openai"
num_alternates = 3
```

Every field has a default. A missing file means defaults. A malformed file means defaults plus one
status-bar warning. API keys are only read from the named environment variable.

## 7. Error handling

| Situation | Behavior |
|---|---|
| Missing API key | status: "Set $ANTHROPIC_API_KEY to use AI"; nothing else changes |
| AI HTTP/network/parse error | status message; document unchanged |
| Stale AI result | discarded; status notice |
| Save fails | error banner; dirty flag kept; previous files intact (atomic rename) |
| Open fails | error banner; current document untouched |
| Sidecar drift / corrupt sidecar | load with reconciliation; status notice |
| Quit / New / Open with unsaved changes | Save / Discard / Cancel prompt |

## 8. Testing

- `tulis-core` is developed test-first. Unit tests cover: segment normalization; `locate` and its
  inverse, including multibyte text; insert and delete across segments and paragraphs; every
  alternate operation; ghost toggling; overflow; export; Markdown parsing; sidecar round-trip; drift
  reconciliation; undo coalescing; the vim parser (keystrokes → commands); request building for
  both providers (against fixtures); response parsing; trim validation.
- A `MockProvider` covers the app's AI flow without touching the network.
- `tulis`: `gpui::TestAppContext` tests for editor actions (typing, selection, alternates,
  ghost, undo) where possible, plus a manual smoke checklist in the README (rendering, IME,
  scrolling, dialogs).
