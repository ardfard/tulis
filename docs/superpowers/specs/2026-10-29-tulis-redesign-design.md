# Tulis Redesign — Design Spec

Date: 2026-10-29
Status: Approved in conversation; pending written-spec review
Builds on: `docs/superpowers/specs/2026-10-28-tulis-design.md` (v1). Everything in v1 still holds
unless this document changes it.

## Summary

Bring Tulis close to the Write_On demo (the reference video, analyzed frame by frame) while keeping
Tulis's own choices where the user wanted them:

- **Foundation:** move to `gpui-pre 0.3.10` + `gpui-component 0.8.1`. Add four theme presets
  (Midnight is the default) and a monospace default font.
- **A. Look & chrome:** writing column with edge fades, dotted alternate markers, a paragraph-version
  bar, near-invisible ghosts, a header (counts, markers and Markdown toggles, cheat sheet) and a
  footer (save/open/share, LAB, XYZ), plus toasts.
- **B. Alternates sidebar:** Word/Sentence/Paragraph tabs, type-to-add, AI options, sentence-level
  alternates, a right-click context menu and Write_On's shortcuts.
- **C. Overflow scratchpad:** a free-text side panel; Ctrl+Enter or drag to move text into the page.
- **D. The Lab:** trim levels with a faded-word preview (Make the cuts / Walk through / Done) and six
  guide checks that produce marks or AI corrections.

Every new command has a vim Normal-mode equivalent.

## Non-goals

- Nested alternates (word alternates inside sentence options). A sentence alternate merges the
  word alternates inside it, and undo restores them.
- Rich text beyond a bold `# ` title paragraph.
- Vim inside the Overflow scratchpad.
- Live theme editing, or custom theme files beyond the presets.

## 0. Foundation

### 0.1 Dependency migration
- `crates/tulis`: replace `gpui = "0.2.2"` with `gpui = { package = "gpui-pre", version = "=0.3.10" }`
  (the dev-dependency keeps `features = ["test-support"]`), and add `gpui-component = "0.8.1"`.
  A spike showed exactly 5 compile errors in today's code: a `Point` used where `Size.height` is
  expected, two methods that gained a parameter, and the `Application::new`/`run` changes.
- Remove `vendor/xattr` and the `[patch.crates-io]` entry if `gpui-pre`'s dependency tree no
  longer contains `xattr`. The spike's lockfile had none.
- `main`: call `gpui_component::init(cx)`, and wrap the window's root view in `gpui_component::Root`.
- This lands first, as its own step, with all existing tests passing and no behavior change.

### 0.2 Themes
- `tulis_core::config::EditorConfig` gains `theme: String` (default `"midnight"`) and
  `long_sentence_words: usize` (default 30, used by the Lab). The `font_family` default becomes
  `"IBM Plex Mono"`.
- App module `theme.rs` becomes a `Theme` struct with presets **Midnight**, **Paper**, **Sepia**
  and **Black**. Each preset defines:
  - Editor colors: `background`, `text`, `text_muted`, `accent`, `ai_accent`, `selection`,
    `cursor`, `ghost_alpha`, `fade_alpha`, `mark_tint` and `trim`.
  - The matching gpui-component theme colors, so widgets agree with the editor.
- Midnight uses a deep navy background (≈ `#070b1f`), warm peach text (≈ `#e8cdb5`) and a purple
  accent (≈ `#8b7cf6`); ghost_alpha is 0.06. Paper is today's light look (ghost_alpha 0.12). Sepia
  is a warm brown-on-cream theme. Black is `#000` with light-grey text.
- An unknown theme name falls back to Midnight and shows a toast.
- **Ctrl+K T** (vim `gt`) cycles presets for the session and toasts the theme name.
- **Font fallback:** if `font_family` is not installed, try IBM Plex Mono, then JetBrains Mono,
  then the platform monospace font.
- Line height becomes 1.8× the font size.

## A. Look & chrome

### A.1 Writing column
- Centered, max width 640px, with ~64px vertical padding inside the scroll view.
- Top and bottom fades: a 48px gradient from the background color to transparent, painted over the
  scroll viewport's edges.
- **Title:** if the first paragraph's text starts with `# `, it renders bold and the `# ` is drawn at
  0 alpha. The `# ` stays in the text and in the Markdown. Cursor movement treats it as normal text.

### A.2 Markers
All markers are theme-colored and hidden while the markers toggle is off.
- **Alternate groups (Alt segments):** a dotted underline (1px dots every 3px), drawn with
  `paint_quad` per segment line run. The color is `accent` when the shown option is AI-written,
  otherwise `text_muted`.
- **Paragraphs with more than one version:** a 1.5px vertical bar 12px left of the column, the
  paragraph's height, in `accent` at 60% alpha. A `⋮` handle sits at mid-height; clicking it opens
  the sidebar on its Paragraph tab.
- **Ghosts:** text alpha is `ghost_alpha`.
- **Group being cycled:** while the cursor is in a group just cycled, a 1px rounded box in
  `text_muted` at 40% alpha, until the cursor leaves the group.

### A.3 Header
- **Left:** `N words · M chars`, or `N words · M chars selected` when there is a selection. Ghosts
  are excluded.
- **Center:** the markers toggle (Ctrl+. / vim `gm`) and the Markdown preview toggle
  (Ctrl+Shift+M / vim `gp`). The preview swaps the page for a read-only, scrollable view of
  `storage::export_markdown(doc)`, with the shown options resolved and ghosts removed. Press the
  toggle again or Esc to return.
- **Right:** a keyboard icon that opens the cheat sheet (Ctrl+? / vim `g?`).
- **Auto-dim:** the header fades to 0 alpha after 2s of typing and comes back when the mouse moves
  or typing pauses for 2s.

### A.4 Cheat sheet
A modal (gpui-component dialog) listing every command, grouped (Editing, Alternates, Ghost,
Overflow, Lab, View, File), with columns for the normal key and the vim key, and a filter box.
The list is generated from one command table shared with the keymap and the vim bindings
(`commands.rs`), so it can't drift.

### A.5 Footer
- **Left:** icon buttons for Save, Open, Share to X and Share to LinkedIn.
  - Sharing works on the selection, or the whole document if there is none, as exported Markdown.
  - It copies that text to the clipboard, then opens
    `https://x.com/intent/post?text=<urlencoded>` (trimmed to 280 characters, with a toast if trimmed)
    or `https://www.linkedin.com/feed/?shareActive=true&text=<urlencoded>` via `cx.open_url`.
- **Center:** a dashed-border **LAB** button (Ctrl+L / vim `gl`).
- **Right:** an **XYZ** button that toggles Overflow (Ctrl+\\ / vim `gx`).

### A.6 Toasts
The status-bar notice is replaced by toasts: small, centered above the footer, fading after 3s,
with at most 2 stacked. Every `set_notice` becomes a toast. The bottom status bar is removed;
counts move to the header, and the vim mode appears as a small badge next to the counts when vim
is on.

## B. Alternates sidebar, sentence alternates, context menu

### B.1 Sentence detection (tulis-core)
`pub fn sentence_range_at(text: &str, offset: usize) -> Range<usize>`, plus
`pub fn sentence_ranges(text: &str) -> Vec<Range<usize>>`.
- A sentence ends after `.`, `!` or `?` and any closing `"` `'` `”` `’` `)` `]`, when followed by
  whitespace or the end of the text.
- These abbreviations (case-insensitive) don't end a sentence: `e.g.`, `i.e.`, `etc.`, `vs.`,
  `Mr.`, `Mrs.`, `Ms.`, `Dr.`, `St.`, `No.`.
- Leading whitespace is excluded from the range; trailing whitespace before the next sentence is
  excluded too.
- Text with no terminator is one sentence.
- Unit tests cover terminators, closing quotes, abbreviations, multibyte text, and offsets on
  whitespace.

### B.2 Scopes
For the cursor (or selection):
- **Word:** the Alt group under the cursor (`alt_at`), else the word under it (`word_range_at`).
  With a non-empty single-paragraph selection, the tab is labelled **Selection** and its range is
  exactly the selection.
- **Sentence:** `sentence_range_at`, expanded with `expand_to_alt_boundaries`.
- **Paragraph:** the paragraph's versions.

Creating a Sentence- or Selection-scope group over existing groups merges them, as `make_alt`
already does. The editor detects this (any Alt segment strictly inside the range) and toasts
"Inner alternates merged — Ctrl+Z to restore".

### B.3 Core additions (tulis-core)
- `Document::add_option(para, range, text: String) -> Result<Range<usize>, OpError>` creates the
  group if needed (`ensure_alt`), appends a Human option and makes it active. Returns the group span.
- `Document::set_active_option(para, seg, index)` and `Document::set_active_version(para, index)`.
- `Document::remove_option_at(para, seg, index)` and `Document::remove_version(para, index)`.
  Removing the last-but-one option collapses the group to plain text, as `delete_option` does today.
- `Paragraph::options_at(seg) -> Vec<(String, Source)>` and
  `Paragraph::version_texts() -> Vec<(String, Source)>` for listing.
- All of these are unit-tested.

### B.4 Sidebar
- **Width and opening:** a left panel, 240px wide. It opens via Ctrl+Shift+A (and the old Ctrl+D),
  vim `ga`, the context menu, or the `⋮` handle.
- **Initial tab:** Selection/Word when there is a selection or a group under the cursor; otherwise
  Word.
- **Tabs:** a gpui-component `Tabs` header with Word/Selection, Sentence and Paragraph, in an
  italic/script style using the theme's accent. Switching tabs retargets the list.
- **List rows:**
  - If the target is an existing group (or a paragraph with versions), there is one row per option
    or version in stored order; option 0 is the original. Otherwise there is a single row with the
    target's current text.
  - The shown row has a `●` in the accent color; others show `·`. AI-written rows show `✦` in
    `ai_accent`.
  - A row's text is the option text truncated to 3 lines.
- **Text-field row:** the last row is a gpui-component `Input` with placeholder "type another way
  to say it…". Enter adds the text (B.3 `add_option`, or a new version on the Paragraph tab) and
  shows it.
- **"✦ Ask AI" row:** Ctrl+Shift+G (old Ctrl+J kept) / vim `gA` runs v1's AI alternates for this
  target, with options or versions appended as AI.
- **Keys while the sidebar has focus:**
  - Up/Down or `j`/`k` move the highlight and live-preview the option in the page (set active).
  - Enter or click keeps it and returns focus to the page.
  - Delete, or vim `dd` on a highlighted row, removes that option (never the last remaining one).
  - `i`/`o` focus the text-field row.
  - Esc closes, keeping the shown option.
  - Previewing records **one** undo step per sidebar session, not one per arrow press.
- **No empty groups:** opening the sidebar on a target without a group creates nothing. A group is
  created only on the first add (typed or AI).
- **Dimming:** while open, the page dims to 35% alpha except the target's paragraph.
- The sidebar closes when the target paragraph is deleted.

### B.5 Inline cycling
Alt+↑/↓ and vim `]a`/`[a` stay as in v1. They cycle the innermost group at the cursor, falling back
to versions. A cycled group shows the A.2 box.

### B.6 Context menu
Right-click opens a gpui-component context menu. It targets the selection if there is one;
otherwise the word under the pointer, and the cursor moves there.

| Item | Key | Vim (Normal/Visual) |
|---|---|---|
| Alternatives for selection | Ctrl+Shift+A | `ga` |
| AI alternatives for selection | Ctrl+Shift+G | `gA` |
| Ghost it / Revive it (label follows state) | Ctrl+/ | `gh` (existing) |
| Stash this in Overflow | Ctrl+Shift+X | `gs` |

The old keys Ctrl+D, Ctrl+J, Ctrl+G and Ctrl+Shift+O keep working. In vim Visual mode these
commands act on the selection and return to Normal.

## C. Overflow scratchpad

### C.1 Model and storage
- `Document.overflow: Vec<OverflowItem>` becomes `Document.overflow: String`. `OverflowItem`,
  `OverflowId`, `to_overflow`, `paragraph_to_overflow`, `insert_overflow` and `remove_overflow` are
  replaced by:
  - `stash(range: Range<DocPos>) -> bool`: export text of the range, appended after a blank line;
    the range is deleted. With an empty range, the cursor's paragraph is used.
  - `overflow_blocks() -> Vec<Range<usize>>`: blocks split on blank lines.
  - `take_overflow(range: Range<usize>) -> String`: removes that byte range and normalizes the
    blank lines around it.
- **Sidecar `FORMAT_VERSION` = 2.** A version 1 sidecar is accepted: its `overflow` items are
  joined with `"\n\n"` in order. The next save writes version 2. `Document::validate` applies to
  both; its OverflowId check is dropped, since overflow has no ids any more.
- Unit tests: stash, blocks, take, v1→v2 migration, and round trip.

### C.2 Panel
- **Layout:** a right panel, 260px wide, toggled by XYZ / Ctrl+\\ / vim `gx`. It contains an italic
  "Overflow" title, a multi-line gpui-component `Input` bound to `doc.overflow`, and the footer hint
  "ctrl+↵ or drag into the page to use".
- **Editing:** edits update `doc.overflow` and mark the document dirty. They don't enter the
  page's undo history; the Input has its own undo.
- **Ctrl+Enter** moves the Input's selection, or the block under its cursor, to the page cursor
  (removing it from overflow) and focuses the page. **Ctrl+Shift+Enter** inserts a copy and keeps
  the text in overflow. The page side is one undo step.
- **Drag:** each block shows a `⠿` grip on hover. Dragging a grip onto the page inserts the block
  at the drop position (via `LayoutCache::pos_for_window_point`) and removes it from overflow.
  Ctrl held while dropping copies instead.
- **Focus:** Ctrl+Shift+\\ / vim `gX` toggles focus between page and scratchpad, opening the panel
  if needed. Esc in the scratchpad focuses the page.
- **Stash** (B.6) appends without opening the panel and toasts "Stashed to Overflow".

## D. The Lab

### D.1 Popover
- **Opening:** LAB / Ctrl+L / vim `gl` opens a gpui-component popover anchored above the LAB
  button. Ctrl+Shift+T (v1) opens it scoped to the cursor's paragraph, and v1's Ctrl+Shift+Alt+T
  opens it scoped to the whole document (same as Ctrl+L with no selection).
- **Scope:** the selection (one or more paragraphs, by paragraph id) if there is one, else the
  whole document. The header line reads "The Lab guide…", with a right-aligned "what each idea
  does" link that expands one-line descriptions under each check.
- **Contents:** six check rows, then a row of five trim buttons. Arrows or `j`/`k` plus Enter
  operate it, and Esc closes it.

### D.2 Trim levels
- **Levels:** `Original` (0), `Slight trim` (10%), `Tighten more` (20%), `Even sharper` (30%) and
  `Cut in half` (50%).
- **Requests:** choosing a level runs one request per in-scope, non-empty paragraph.
  `prompts::trim_request` gains `target_percent: u8`, asking for exact passages whose combined
  words are ≈ that share. Results go through `parse_trim` + `locate_trims` as in v1. Results are
  cached per (level, paragraph id, paragraph text). `Original` clears the preview.
- **Preview state:** `TrimPreview { level, cuts: Vec<TrimSuggestion>, kept: HashSet<usize>,
  walk: Option<usize> }`. It is view state; editing a paragraph drops its cuts, as v1 does.
- **Rendering:** active cuts use `fade_alpha` (≈ 0.35 of the text alpha), distinct from ghosts.
- **Bar** above the footer:
  - Text: `"<Level> <before> → <after> words · −<pct>%"`, where `after` is the export word count
    minus words in active cuts, and `pct` is the real reduction (rounded).
  - A second line: "Faded words would go. Click one to keep it."
  - Buttons: **Make the cuts**, **Walk through**, **Done**.
- **Keeping:** clicking an active cut, or Enter while the cursor is in one, marks it kept and draws
  it normally. Clicking a kept cut reactivates it.
- **Make the cuts:** `set_ghost(true)` on every active cut, in one undo step, then closes the
  preview and toasts "Cut N passages (ghosted — Ctrl+Z to undo)".
- **Walk through:** sets `walk = Some(0)`, scrolls to and highlights the current cut, and the bar
  shows "Cut i of n".
  - `y`/Enter ghosts that cut; `n`/Backspace keeps it. Either advances.
  - Esc ends the walk, leaving the preview for the remaining cuts.
  - Plain `y`/`n` keys are captured only while walking, without needing vim.
- **Done:** clears the preview with no changes.
- **Tab:** moves to the next active cut while a preview is showing (replacing v1's Tab = accept).

### D.3 Guide checks
Marks are view state: `Mark { para_id, range, check: Check, note: String }`. They render as a
`mark_tint` background, and editing a paragraph drops its marks. The bar shows
`"<n> marked · <check label>"` plus **Clear**. With the cursor inside a mark, the bar's second
line shows its note. Tab or vim `]m`/`[m` jumps to the next or previous mark. Ctrl+Shift+G inside a
mark targets the mark's range.

| Check | Implementation |
|---|---|
| Fix punctuation and typos | AI per paragraph: `prompts::fix_request` returns a JSON array of `{"original": "...", "fixed": "..."}` with the exact original substring. Each located fix becomes an AI alternate (`add_ai_alternates`) on that range. The bar shows "N fixes proposed — Alt+↑/↓ to compare" instead of marks. |
| Mark the weakest sentences | AI: `prompts::mark_request(kind, paragraph)` returns `[{"text": "...", "note": "..."}]`, located like trims (exact substring in non-ghost text) |
| Mark sentences that run long | **Local:** `sentence_ranges` with word count > `long_sentence_words`; note "N words" |
| Mark convoluted sentences | AI (`mark_request`) |
| Mark words that don't fit the tone | AI (`mark_request`, with a short excerpt of the whole document as context) |
| Mark hedges and filler | AI (`mark_request`) |

New prompt builders and parsers live in `tulis-core::ai::prompts`, each with unit tests
(parsing, exact-substring location, rejection of invented text).

### D.4 Cancellation
All Lab requests share an `Arc<AtomicBool>` cancel flag. The background loop checks it before
each paragraph request. Esc (or Done/Clear/closing the popover while running) sets it and drops
the task. This also fixes v1's deferred gap where cancelling a document trim didn't stop the
remaining requests.

## Keymap summary (new or changed)

| Command | Key | Vim |
|---|---|---|
| Cycle theme | Ctrl+K T | `gt` |
| Toggle markers | Ctrl+. | `gm` |
| Markdown preview | Ctrl+Shift+M | `gp` |
| Cheat sheet | Ctrl+? | `g?` |
| Alternatives sidebar | Ctrl+Shift+A (Ctrl+D) | `ga` |
| AI alternatives | Ctrl+Shift+G (Ctrl+J) | `gA` |
| Ghost / revive | Ctrl+/ (Ctrl+G) | `gh` |
| Stash in Overflow | Ctrl+Shift+X (Ctrl+Shift+O) | `gs` |
| Toggle Overflow | Ctrl+\\ | `gx` |
| Focus page ↔ Overflow | Ctrl+Shift+\\ | `gX` |
| Overflow block → page | Ctrl+Enter (copy: Ctrl+Shift+Enter) | — |
| The Lab | Ctrl+L (paragraph: Ctrl+Shift+T) | `gl` |
| Next/prev mark | Tab | `]m` / `[m` |

The vim state machine (`tulis-core::vim`) gains these `g`-prefixed commands as `VimCommand`
variants. `]m`/`[m` join `]a`/`[a`.

## Error handling

- Every AI failure (missing key, HTTP, parse, refusal) toasts the v1 message, and the document is
  unchanged. Partial Lab failures toast "N of M paragraphs failed".
- A missing font falls back silently. An unknown theme toasts and uses Midnight.
- Share without network: the clipboard copy still succeeds, and if `open_url` fails, a toast says
  "Copied — couldn't open the browser".

## Testing

- **tulis-core unit tests:** sentence detection; the B.3 option/version ops; overflow string ops
  and the v1→v2 sidecar migration; trim targets; fix/mark prompt parsing and location; long-sentence
  marks; the new vim commands.
- **App `#[gpui::test]`s:**
  - Theme cycling.
  - Sidebar add/preview/keep/delete with a single undo step.
  - The context-menu commands as editor methods.
  - Stash and Ctrl+Enter round trip.
  - Lab trim preview counts, keep, Make the cuts, walk-through and Done, using `MockProvider`.
  - The long-sentence check.
  - Cancel stopping the remaining requests.
- Each plan task ends with `cargo test --workspace` green. A manual smoke checklist (README) is
  extended with the visual items: markers, fades, sidebar, menu, Lab, drag.

## Delivery

Four plans, built in order. Each one leaves the app releasable.
1. **Foundation + A:** migration, themes and font, column and markers, header and footer, cheat
   sheet, share, toasts.
2. **B:** sentence detection, core option ops, sidebar, context menu, new vim commands.
3. **C:** overflow model and sidecar v2, scratchpad panel, Ctrl+Enter, drag.
4. **D:** the Lab, with trim levels, guide checks and cancellation.
