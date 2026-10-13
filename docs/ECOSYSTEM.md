# The Timster's Audio Works family

| App | What it is | Built as | Repo |
|---|---|---|---|
| **Music Box** | Library manager: finds, checks and (later) organises your music | Native Rust / egui | Music-Box |
| **Spool** | Player: gapless, bit-perfect where it can | One HTML file in the browser | Spool |
| **EQ Box** | System-wide 10-band parametric EQ | Native Rust / egui | EQ-Box |
| **SP-10** | Spectrum analyser (the "Analyser Box") | Native Rust / egui, a second program in EQ-Box | EQ-Box |
| Drone Box, Drum Box | Instruments | HTML | their own repos |

## How the playback chain fits together today

```
Music Box ──(album as a playlist)──► player ──► system output ──► EQ Box ──► speakers
                                                                     └──► SP-10 listens
```

EQ Box works at the system level (a PipeWire sink on Linux, an audio-engine effect on Windows), so it already sits after any player with no coupling. SP-10 listens to an output and needs nothing from the others.

## Where they need to meet

1. **Music Box → Spool.** Today Play writes an `.m3u8` playlist and opens it in the default player. A browser page can't be handed files by another program: Spool would need the user to pick the folder again each time, which is the boundary in `WHAT_SPOOL_IS.md`. So the real hand-off depends on a decision about Spool:
   - **Spool stays in the browser:** Music Box opens the album's folder and the user drags it into Spool. Simple, but not joined up.
   - **Spool goes native** (Rust / egui, like EQ Box): `spool --play album.m3u8`, a shared look for free, and Spool can read Music Box's library database (read-only) to browse the whole collection. The browser's limits (remembering folders, writing logs, Brave quirks) go away. Its Web Audio engine (gapless, bit-perfect handling, meters) would need porting; symphonia and cpal are the usual Rust choices.
2. **One panel look.** Done: the shared `taw-panel` crate (TimsterAudio/TAW-Panel), pinned by commit in each app.
3. **Shared conventions** (already common to EQ Box and Music Box): version and changelog format, the release workflow, `#[serde(default)]` settings, no network unless switched on, and settings and data in each OS's normal app-data folder.
4. **Later:** one download with all four, and a small launcher. Not needed until the hand-offs exist.
