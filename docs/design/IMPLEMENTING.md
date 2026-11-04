# Implementing the board in code

How to prompt an AI agent to turn `tulis-design.pen` into Rust. pen.dev only exports HTML, and
Tulis is Rust + GPUI, so the board is a spec the agent reads and implements by hand. Read
[HANDOFF.md](HANDOFF.md) for how the board is built.

## Before you prompt

1. Save the board in pen.dev. The agent reads the saved or open file.
2. Keep the pen.dev desktop app open on `docs/design/tulis-design.pen`, so the agent's `pencil` MCP
   can read nodes and take screenshots.
3. Commit the design on its own (`jj commit -m "design: …"`), so code changes are separate commits.

## Base prompt

Paste this, then fill in the last three lines from the table below. One piece per session.

```
You're implementing a design change in Tulis (Rust + GPUI). The design lives in
docs/design/tulis-design.pen (open in pen.dev; use the pencil MCP to read it).
Read docs/design/HANDOFF.md first. It maps board sections to source files and
lists the rules.

Rules:
- The board is a spec, not code. Translate it into GPUI by hand; match the
  existing code style in the files you touch.
- Colors come from theme.rs, never hard-coded. Board variables map 1:1:
  bg, panel, rule, text, muted, accent, ai, selection, tint, cutbg.
  Motion variables (motion-fast/base/slow/hold, in ms) map to named constants.
- Shortcuts live only in commands.rs (one table drives keymap, vim and cheat sheet).
- Read the exact Spec frames listed below: their title, "Spec Keys" and
  "Spec Note" texts are the behavior contract. Take a screenshot of each Window
  with the pencil tool and read sizes/spacing from the nodes with Get, don't guess.
- Where the board approximates (solid underline instead of dotted, stepped
  fades, solid LAB border), follow the spec text, not the drawing.
- If the code has to differ from the board, list every difference in your final
  message instead of silently picking one.
- Done means: cargo test --workspace and cargo fmt --all --check pass, and you
  launched YOUR OWN Tulis instance and captured only its window (by pid) to
  compare against the board. Never send input to or screenshot my window.
- Use jj. One commit for this piece, message in the repo's style.

This piece: <PIECE>
Spec frames to read: <FRAMES>
Files likely touched: <FILES>
```

## Pieces, in order

| # | `<PIECE>` | `<FRAMES>` | `<FILES>` |
|---|---|---|---|
| 1 | Header: file name and unsaved dot on the left; markers and preview toggles together in one panel; counts and cheat sheet button on the right | `Header` component, `Spec: Midnight (default)`, `Spec: Selection` | `chrome.rs` |
| 2 | Footer: Save, Open, Share ▾ opening X / LinkedIn icon buttons; Lab with its `Ctrl L` key; Overflow with stash count | `Footer` component, `Spec: Toasts and sharing`, `Spec: Stash with the panel closed` | `chrome.rs`, `scratchpad.rs`, `assets.rs` (X and LinkedIn SVGs) |
| 3 | Version dots beside the paragraph bar instead of `⋮`; sidebar marker column (● / · on the left, ✦ on the right) | `Spec: Markers: alternates, AI options, paragraph versions`, `Spec: Paragraph tab`, `Spec: Word tab` | `editor/markers.rs`, `editor/element.rs`, `sidebar.rs` |
| 4 | Key pills in the sidebar, context menu, Overflow, Lab bar and cheat sheet; toasts centered over the footer; the save prompt names the file | Overlays in sections 03–06, `Spec: Errors and unsaved-changes prompt` | `sidebar.rs`, `cheat_sheet.rs`, `editor/lab.rs`, `scratchpad.rs`, `toast.rs`, `workspace.rs` |
| 5 | Paragraph map, shown only when the document is taller than the window | `Spec: Long document: paragraph map` | `editor/element.rs`, `editor/layout.rs` |
| 6 | Motion tokens and easing helpers, then the version swap | Section 07 tokens row, `Spec: Motion: Paragraph version swap` | `theme.rs` (or a new `motion.rs`), `editor/element.rs`, `editor/markers.rs` |
| 7 | Lab scan line, replacing the dimmed popover | `Spec: Motion: Lab run: the scan line`, `Spec: Request running: the scan line` | `editor/lab.rs`, `lab_popover.rs`, `runs.rs` |
| 8 | Smooth jumps and chrome auto-dim timing | `Spec: Motion: Jumps scroll smoothly`, `Spec: Motion: Chrome steps back while you type` | `editor/element.rs`, `header_dim.rs`, `chrome.rs` |

## Tips

- Name the exact Spec frames. "Read `Spec: Motion: Lab run: the scan line`" works better than
  "add the scan line".
- For the motion pieces (6–8), ask for a plan before code: "Before coding, tell me how you'll drive
  animation frames in GPUI (timer and `cx.notify`) and how a key press jumps to the end state."
  GPUI has no CSS transitions, and this is where agents tend to go wrong.
- When a piece lands, ask the agent to update the board's Spec text if the code had to differ, and
  to re-export that section's preview into `docs/design/previews/`.
