# Handoff: TAW Library (LB-001)

Written 2026-10-14 when the family's work moved from one shared chat to a chat per app. Read CLAUDE.md, then this.

## Where it stands

- `main` is 0.2.0, released (Jupiter panel).
- Branch `claude/focused-ritchie-veg4m9`, unmerged: one commit (`c0bc732`), the Play key hands the album to the TAW Player (`taw-player --play <m3u8>`) when it's found next to the Library, in a sibling `taw-player…` folder, or on the PATH; otherwise the default player as before.

## Next

1. Move to the TAW look (TAW-Panel `hifi`, Spool Light Amber default); the reviewed mock-up is TAW-Panel `docs/finishes-review.html` (Library, Detailed and Simple).
2. Resizable panes and columns (the user asked for variable-width columns): drag a pane edge or column divider, double-click to fit, widths remembered per view, one flexible column (Title) takes the spare room, header right-click to show or hide columns. Build it in TAW-Panel so the Player can share it.
