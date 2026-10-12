# Music Box

**Timster's Audio Works – Music Box**: a native music library manager for Windows and Linux, in the same panel style as EQ Box and Drone Box.

Status: **design stage.** There is no app yet, only a clickable mock-up for review in `docs/mockups/` (open `music-box-mockup.html` in a browser).

## What it's for

- Scan every local, removable and network drive for music (MP3, FLAC, WAV, M4A, AIFF; no video or pictures) and keep a database of it. Drives are recognised by volume ID, not drive letter. A drive that's unplugged stays in the library, marked offline.
- Show what's missing: artwork (or artwork that's too small), year, album artist, disc and track numbers, files with no tags at all.
- Look up missing details and artwork online, only when you turn Lookups on (MusicBrainz, Cover Art Archive, AcoustID).
- Find duplicates at three levels (identical file, same audio with other tags, same recording in another format) and suggest which copy to keep.
- Suggest artist merges ("Beatles" into "The Beatles").
- Plan a clean Music folder (`{Album Artist}\{Year} - {Album}\{Disc}{Track} {Title}`) and show the plan before anything changes.

## Rules agreed so far

- **Native, not HTML.** Rust with egui, sharing the panel code, fonts and release workflow with EQ Box.
- **Nothing changes on disk until you review a plan and press Apply.** Copy is the default. Each copy is checked against its original. Every change is logged and can be undone. Duplicates go to a Review folder; nothing is deleted.
- **The fields that matter:** artist, album artist, album, year, disc, track, title, time and artwork. Every other tag in a file is kept as it is.
- **No network requests unless Lookups is on.** This is the one deliberate exception to the family's offline rule, and it's off by default.
- **Exclude rules** keep sample packs, DAW projects, system sounds and podcasts out of the library.

## Planned stages

1. Scan and report (read-only): database, What's missing, duplicates.
2. Online lookups, staged as proposals.
3. Tag writing, with the undo log.
4. The organise plan and Apply.
