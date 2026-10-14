# Handoff: TAW Library (LB-001)

Written 2026-10-14 when the family's work moved from one shared chat to a chat per app. Read CLAUDE.md, then this.

## Where it stands

- `main` is 0.2.0, released (Jupiter panel).
- Branch `claude/focused-ritchie-veg4m9`, unmerged: one commit (`c0bc732`), the Play key hands the album to the TAW Player (`taw-player --play <m3u8>`) when it's found next to the Library, in a sibling `taw-player…` folder, or on the PATH; otherwise the default player as before.

## In progress (2026-10-14, main chat)

Moving the Library to the TAW look, following the Player (TAW-Player `src/app.rs` is the model: plate(), settings drawer, fit_to_window, frameless window). Steps, ticked as they're pushed to this branch:

- [x] Pin TAW-Panel 0.3.0 (`1b8162d`), `hifi::install_fonts`, `Saved` gains `finish`, `simple`, `system_title_bar` (keep `panel` with `#[allow(dead_code)]`).
- [x] Plate: nameplate, window buttons, family switcher (current 0 = Library), Simple/Detailed, gear; frameless window in main.rs.
- [x] Views (`ui/views.rs`) redrawn with `hifi` tokens: sidebar, search, album list, album detail with Play in Player.
- [x] Settings drawer: look, Lookups, library folders, window, keys.
- [ ] Checkpoint `wip` builds; still to do: look at it under Xvfb and fix layout (album detail toward the mock-up: big title, chips, Play in Player + Show folder), then screenshots (test library: `tools/make-test-library.sh`), docs, CHANGELOG, push.

## Next

1. Move to the TAW look (TAW-Panel `hifi`, Spool Light Amber default); the reviewed mock-up is TAW-Panel `docs/finishes-review.html` (Library, Detailed and Simple).
2. Resizable panes and columns (the user asked for variable-width columns): drag a pane edge or column divider, double-click to fit, widths remembered per view, one flexible column (Title) takes the spare room, header right-click to show or hide columns. Build it in TAW-Panel so the Player can share it.
