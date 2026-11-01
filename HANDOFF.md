# Handoff: TAW Library (LB-001)

Read CLAUDE.md, then this. TAW-Panel's `docs/TRACKER.md` has where the whole family stands.

## Where it stands (2026-11-01)

- **0.3.2 on `main`**: Linen, TAW-Panel 0.5.0, one Library at a time.
- **0.4.0 on branch `claude/resizable-list`**, not merged yet: resizable panes and columns on TAW-Panel 0.6.0 (`table`, made in this session on TAW-Panel's branch of the same name, commit `efbded4`). Merge TAW-Panel's branch to its `main` first, with a merge commit or fast-forward (never a squash), so the pinned commit is on `main`; then merge this one, which publishes 0.4.0.
  - Library view: the artists pane and the album pane have draggable edges (the albums list keeps at least 372 px); What's missing has a draggable tiles pane.
  - Columns: albums (art, Year, Album, Needs), tracks (#, Title, Format when mixed, Time) and What's missing (Artist, Album, Year, Missing, Where it is). Album is the flexible column. Right-click the titles to show or hide, double-click a divider to fit.
  - Saved in `Saved::layout` (`table::Layout`).
  - In line with the other three apps: the 66 px nameplate strip (row of keys at 46), window buttons at 6 px, `hifi::RADIUS` for the search field, view list and tiles.
  - Checked under Xvfb at 1600 × 900 and 1280 × 720 with the test library, Library and What's missing, dragging panes and dividers, fitting a column, and widths kept after closing and reopening.

## Next

1. The user's review of 0.4.0, and of the look.
2. 0.5.0: stage 2, MusicBrainz and Cover Art Archive lookups, off until Lookups is switched on, staged as proposals (nothing written to files). The user said yes to it on 2026-11-01.
