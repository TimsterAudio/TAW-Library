# Handoff: TAW Library (LB-001)

Written 2026-10-14 when the family's work moved from one shared chat to a chat per app. Read CLAUDE.md, then this.

## Where it stands

- `main` is 0.2.0, released (Jupiter panel).
- Branch `claude/focused-ritchie-veg4m9`, unmerged: one commit (`c0bc732`), the Play key hands the album to the TAW Player (`taw-player --play <m3u8>`) when it's found next to the Library, in a sibling `taw-player…` folder, or on the PATH; otherwise the default player as before.

## The TAW look (2026-10-14)

Done on this branch, on TAW-Panel 0.3.0 like the other three: nameplate, family switcher, Simple/Detailed and Settings drawer as in the Player; search and Scan along the top; a sidebar with the views and drives; the album detail as in the mock-up (big title, tags, Play in Player, Show folder). The list pieces are in `src/ui/look.rs`. Checked under Xvfb with the test library in Spool Light and Dark, Simple and Detailed, Library and What's missing, at 1360 × 820 and 1280 × 720.

Not done: resizable panes and columns (below), and a release (0.3.0: move Unreleased into a version section, bump Cargo.toml and README, merge).

## Next

1. The user's review of the new look.
2. Resizable panes and columns (the user asked for variable-width columns): drag a pane edge or column divider, double-click to fit, widths remembered per view, one flexible column (Title) takes the spare room, header right-click to show or hide columns. Build it in TAW-Panel so the Player can share it.
