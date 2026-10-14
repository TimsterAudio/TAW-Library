# Roadmap

## Pipeline (proposed 2026-10-14)

The stages below, release by release. Sizes: S = one session, M = two or three, L = a release of its own. Briefs from the family session go in `docs/briefs/`; TAW-Panel's `docs/TRACKER.md` tracks the family as a whole.

| Release | What | Needs | Size |
|---|---|---|---|
| 0.3.x | Fixes from the user's review of the TAW look | The review | S |
| 0.4.0 | Resizable panes and columns, widths remembered per view | TAW-Panel 0.5.0 | S–M |
| 0.5.0 | Stage 2: MusicBrainz and Cover Art Archive lookups, staged as proposals, off until Lookups is switched on | — | L |
| 0.5.x | AcoustID fingerprints for untagged files (Chromaprint bundled for both platforms) | — | M |
| 0.6.0 | Stage 3: tag writing, with the change log and undo. The first release that writes to music files: test on copies | — | L |
| 0.7.0 | Stage 4a: duplicates, with keep suggestions and the Review folder | — | L |
| 0.8.0 | Stage 4b: the organise plan, copy (default) or move, each copy checked, with undo | — | L |

## Stage 1: scan and report (read-only) — v0.1
- [x] Drives by volume ID, offline drives kept, new drives noticed
- [x] Scanning with incremental rescans and exclude rules (system, DAW projects, sample packs)
- [x] Library view: artists, albums, album details, artwork, tracks
- [x] What's Missing view
- [x] Artist merge suggestions
- [x] Play an album in the default player

## Stage 2: online lookups (off until switched on)
- [ ] MusicBrainz lookups by album (rate-limited to one request a second)
- [ ] Cover Art Archive artwork
- [ ] AcoustID fingerprints for files with no tags (needs Chromaprint)
- [ ] Results staged as proposals; nothing written yet

## Stage 3: tag writing
- [ ] Edit the core fields (artist, album artist, album, year, disc, track, title, artwork)
- [ ] Apply staged lookups
- [ ] Every change logged, with undo; tags the Library doesn't use are kept

## Stage 4: duplicates and the organise plan
- [ ] Duplicates: identical files, same audio with other tags, same recording in another format
- [ ] Keep suggestions (lossless, bitrate, tag completeness); others to a Review folder
- [ ] Artist merges
- [ ] The plan: now / plan / new Music folder, with a name pattern
- [ ] Copy (default) or move, each copy checked against its original, with undo

## Later
- [x] Hand albums and playlists to the TAW Player (albums; playlists when the Library has them)
- [x] The shared panel crate for the family: TAW-Panel (0.2.0)
- [x] The TAW look (TAW-Panel `hifi`): Spool or Porcelain, Light, Dark or Automatic; Simple and Detailed; own title strip and window buttons
- [ ] Resizable panes and columns (drag to resize, double-click to fit, widths remembered)
