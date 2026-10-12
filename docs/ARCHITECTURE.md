# Architecture

```
            ┌──────────────── UI thread (eframe / egui) ────────────────┐
            │ ui/app.rs    top strip, drives column, footer, polling    │
            │ ui/views.rs  Library, What's Missing, later views         │
            │ ui/parts.rs  lists, chips, lights   ui/widgets.rs  keys…  │
            └──────┬──────────────────────▲─────────────────▲───────────┘
                   │ start / stop         │ Progress        │ textures
            ┌──────▼───────┐   ┌──────────┴─────┐   ┌───────┴────────┐
            │ scan thread  │   │ drive watcher  │   │ artwork loader │
            │ own Db conn  │   │ every 4 s      │   │ decode, shrink │
            └──────┬───────┘   └────────────────┘   └────────────────┘
                   ▼
            library.db (SQLite, WAL) ◄── Library::load after each batch
```

## Modules

- **`crates/mb-core`**: the library engine, with no UI code.
- **the app** (the root package, `src/`): the panel and the threads around it.

| File | What it does |
|---|---|
| `mb-core/src/volumes.rs` | Finds drives and gives each a lasting id: the filesystem UUID on Linux (from `/proc/self/mountinfo` and `/dev/disk/by-uuid`), the volume serial on Windows, the share for network drives. Paths are stored relative to their drive. |
| `mb-core/src/excludes.rs` | What scans leave out: system and hidden folders, DAW project folders (found by their project files), and the rules the user can change. |
| `mb-core/src/tags.rs` | Reads tags and audio properties with lofty. Read-only. Embedded artwork is measured during scans and only decoded for display. |
| `mb-core/src/db.rs` | The SQLite schema and queries: volumes, scan locations, exclude rules, tracks, folder artwork. |
| `mb-core/src/scan.rs` | Walks each drive, reads new or changed files, and forgets files that have gone (only under folders that were scanned completely). Batches are committed every 400 files or 2 seconds, so the panel fills in as it goes. A cancelled scan keeps what it read. |
| `mb-core/src/library.rs` | Builds albums and artists from the tracks, works out what each album is missing, and suggests artist merges. Albums are grouped by drive, album folder (CD1/CD2 folders join their parent) and album name, so copies on two drives stay two albums for the Dupes view to find. |
| `src/workers.rs` | The scan thread, the drive watcher and the artwork loader. |
| `src/ui/*` | The panel. `theme.rs` and `widgets.rs` come from EQ Box. |

## Rules the code keeps

- Music files are opened read-only. Nothing in stage 1 writes to them.
- The scanner doesn't follow symbolic links, and on Linux doesn't cross into other drives mounted inside the one it's scanning (those are scanned as their own drives).
- The panel never waits on a drive: drive listing, scanning and artwork all happen on other threads.

## Tests

`cargo test` checks mountinfo parsing and paths, exclude rules, the database (moved drives, forgetting files only under a rescanned folder), a full scan of generated WAV files with tags (first scan, unchanged rescan, a deleted file), album grouping (CD1/CD2, compilations, untagged folders, folder art), merge suggestions and what's missing.
