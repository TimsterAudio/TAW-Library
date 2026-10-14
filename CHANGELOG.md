# Changelog

## Unreleased

- Play sends the album to the TAW Player when it's installed, and to the default music player when it isn't.
- The TAW look (TAW-Panel 0.3.0), as the Player, Equaliser and Analyser: Spool's look by default (Light, Amber) with Spool's seven colour themes, or Porcelain; Light, Dark or Automatic.
- New layout: search across artists and albums along the top with Scan drives (and, in Detailed, Add folder, Exclude and Lookups); a sidebar with the views and drives; the album with big artwork, its title, what it needs, and Play in Player.
- Simple view hides Duplicates, Organise, Exclude, Lookups, merge suggestions and the detail of what's missing; it keeps search, Scan and Play.
- The family switcher starts the Player, Equaliser and Analyser when they're installed.
- The Library draws its own title strip and window buttons; Settings has "Use the system title bar", Add folder and Exclude.

## 0.2.0 – 2026-10-13

Music Box is now **Timster's Audio Works Library** (LB-001), one of the family's hi-fi separates with the Player, Equaliser and Analyser.

- New name on the panel, window and program (`taw-library`).
- The library database moves to the family's folder (`Timster's Audio Works\Library` on Windows, `~/.local/share/timsters-audio-works/library` on Linux). A Music Box library is moved there automatically the first time, with nothing rescanned. Panel settings start fresh.
- `TAW_LIBRARY_DB` replaces `MUSIC_BOX_DB`.
- The panel comes from the shared TAW-Panel crate. It looks the same.

## 0.1.0 – 2026-10-13

The first version: stage 1, scan and report. Music Box only reads your music.

- Finds internal, USB and network drives on Windows and Linux by volume ID, keeps offline drives, and notices drives being plugged in.
- Scans for MP3, FLAC, WAV, M4A, AIFF, OGG, Opus, APE and WavPack, reading only new or changed files on a rescan.
- Leaves out system folders, hidden folders, DAW project folders, and folders named Samples, Sample Packs, Splice or Podcasts (these four can be removed), plus any folder you exclude.
- Library view: artists, albums with what each is missing, album details with artwork and tracks. CD1/CD2 folders make one album.
- What's Missing view: artwork, small artwork, year, album artist, track numbers, disc numbers and files with no tags.
- "Beatles" / "The Beatles" style merge suggestions.
- Play sends an album to the default music player as a playlist.
- Black and Silver panels, from EQ Box.
