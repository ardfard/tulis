# Design handoff: improving Tulis in pen.dev

Read this first if you are picking up the design work. It tells you what exists, how to connect to
pen.dev, how the board is built, and what is known to be out of date.

## What this is

Tulis is a desktop writing app (Rust + GPUI). `docs/design/tulis-design.pen` is a pen.dev board that
documents the app's design and every state it can be in. **The app's code is the source of truth**;
the board documents it. If you change the design, the Rust has to change to match. The `.pen` file does
not generate code.

## Read these, in this order

| File | Why |
|---|---|
| `docs/design/tulis-design.pen` | The board. Open it in pen.dev. |
| `docs/design/previews/*.png` | Quick-look image of each section (half scale). Slightly stale, see "Known gaps". |
| `docs/superpowers/specs/2026-10-29-tulis-redesign-design.md` | The intent behind each screen: exact keys, vim keys, behavior, metrics. |
| `docs/superpowers/specs/2026-10-28-tulis-design.md` | The original v1 spec (document model, storage, AI). |
| `crates/tulis/src/theme.rs` | The real colors of the four themes. |
| `crates/tulis/src/commands.rs` | The real shortcut table (keys, vim keys, cheat-sheet rows). |
| `README.md` | Keys, config and the manual smoke checklist. |

## Connecting to pen.dev (this part is fiddly)

1. **Point the tool at the desktop app.** In `~/.claude.json`, the `pencil` MCP server must run with
   `--app desktop`. A Cursor-extension setup runs with `--app cursor` and will say
   "transport not connected to app: cursor" if Cursor is not open. After changing the config, type
   `/mcp` and reconnect `pencil`, or restart Claude Code. Note the config points at the app's temporary
   mount folder (`/tmp/.mount_Pen-…`), which changes every time the desktop app is relaunched.
2. **Open the file in the desktop app.** File → Open → `docs/design/tulis-design.pen`. The tool only
   works on the file that is open in the editor (`get_app_state` shows it).
3. **Save explicitly.** Edits made through the tool live in the app until saved. Press save in the app,
   or run: `printf 'save()\nexit()\n' | pen interactive --app desktop`.
4. **Sign in once.** `pen login` must be run in a real terminal. The prompt does not work from Claude
   Code's `!` shell. `pen status` should say "Active".
5. **No desktop app?** Headless mode works for building: `pen interactive --out <file.pen>`. It needs
   the same login. You will not see the canvas live.

Before editing, read the skill files the tool serves (`read_skill()`, then `pen-schema.md` and
`execute.md`); they define what properties exist. Unsupported things (percent sizes, margins, dotted
strokes, padding on text) will error.

## How the board is built

- **Sections** are top-level frames with a dark backing: `Section 00: Overview and tokens` (principles,
  token swatches for all four themes, typography, metrics, anatomy, index, keyboard model), then
  `01 Themes`, `02 Page states`, `03 Alternates sidebar and context menu`, `04 Overflow scratchpad`,
  `05 The Lab`, `06 Chrome, overlays and feedback`.
- **Every screen** is a `Spec: <title>` frame: a title, a line of keyboard and vim shortcuts, a note, then
  a 1140 × 720 `Window` (header 44, body 628, footer 48; writing column 640 wide, 27px line pitch).
- **Reusable components** at the document root: `Header` and `Footer`. Screens use them as instances and
  override pieces by descendant name (`Counts`, `Vim Badge`, `Vim Mode`, `Status Badge`,
  `Preview Toggle`, `Markers Toggle`, `LAB Button`, `XYZ Button`, …). Names are unique inside a
  component, so use names, not ids.
- **Themes** use one variable axis, `mode`: `midnight` (default), `paper`, `sepia`, `black`. Put
  `theme: {mode: "paper"}` on a frame to re-theme everything inside it, including instances (pass the theme
  on the `ref` too).
- **Variables** (all color tokens are variables; never hard-code a theme color): `bg`, `panel`, `rule`,
  `text`, `muted`, `accent`, `ai`, `selection`, `tint`, `cutbg`; number `ghost-alpha`; strings
  `font-mono` (IBM Plex Mono), `font-ui` (Inter), `font-script` (Caveat). Section backing and the spec
  annotation text use fixed colors on purpose (they are not part of the app).
- **Text with mixed styling is built from pieces.** pen.dev text nodes cannot be partly styled, so a page
  "line" is a horizontal frame of small text nodes (normal, underlined alternate, violet AI alternate,
  faded at 35%, ghost at 6–12%, tinted mark, selection, caret). A paragraph is a vertical frame of lines
  with hand-chosen line breaks, at most about 64 characters per line in the monospace font.
- **Overlays** (popover, bar, menu, dialog, toasts, banner) are absolutely positioned children of the
  `Window` frame.

### Practical tips

- In the `execute` tool, helper functions do **not** persist between calls (only ids and values do). Resend
  any helpers each call. Give every node a human-readable `name`.
- Moving nodes in the same call as measuring them gives stale bounds. Measure in a separate call.
- Run the overflow check from the build: visit every node, compare its bounds with its parent's, and flag
  anything sticking out by more than about 3px. (Ignore the version bar, ⋮ handle and fade strips; they
  sit outside their parent on purpose.)
- Take screenshots of single windows, not whole sections, when checking details. Section screenshots are
  too small to read.

## Known gaps

- **The board lags the app on four points.** The app has moved on since the board was drawn:
  1. Line height is now **1.6×** the font size (configurable as `editor.line_height`); the board uses 1.8
     (27px pitch).
  2. The title's `# ` marker now **hangs in the left margin**, so the title text lines up with the body.
     The board draws titles without a marker.
  3. The alternates sidebar's text field, "✦ Ask AI" row and key hint sit **directly under the option
     list** (the board already shows this; the app was fixed to match).
  4. The handwriting labels use **Caveat**, which the app now bundles (`crates/tulis/assets/fonts`).
- **Approximations forced by pen.dev:** alternates use a solid underline (the app draws dotted ones), the
  dashed LAB button border is solid, and the 48px edge fades are three stepped translucent strips.
- **Marker column in the sidebar:** the board shows ✦ in the left marker column for AI options and the
  accent dot for the shown option. The app shows the dot/· on the left and ✦ on the right. Decide which
  is right and make both agree.
- **The previews are stale.** They were exported before a small fix (sidebar placeholder wrapping) and
  before the points above. Re-export from pen.dev when you finish.
- **Sample text** in the board is placeholder copy ("On Editing in Layers"), not real content.

## When you improve the design

1. Change the board. Keep tokens as variables and keep the `Spec:` annotations (title, keys, note) in
   step with the behavior.
2. If a change affects the app (colors, spacing, copy, layout), update the Rust too:
   - colors and theme values: `crates/tulis/src/theme.rs`;
   - the column, markers and text layout: `crates/tulis/src/editor/element.rs` and `editor/layout.rs`;
   - sidebar, Overflow panel, Lab popover and bar, cheat sheet: `sidebar.rs`, `scratchpad.rs`,
     `lab_popover.rs`, `editor/lab.rs`, `cheat_sheet.rs`;
   - chrome (header, footer, toasts): `chrome.rs`, `toast.rs`;
   - shortcuts: `commands.rs` (one table drives the keymap, vim bindings and cheat sheet).
3. Keep the tests green: `cargo test --workspace`, plus `cargo fmt --all --check`.
4. Save the `.pen` file and re-export the previews.

**Do not inject keyboard input into windows or screenshot a window picked by its title** when verifying the
running app. The user usually has their own Tulis window open. If you need to look at the app, launch your
own instance and capture only the window with your process id.
