# Roadmap

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
- [ ] Hand albums and playlists to the TAW Player
- [ ] The shared panel crate for the family (see docs/ECOSYSTEM.md)
