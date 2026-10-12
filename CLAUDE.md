# Music Box: working conventions

Music Box is a native music library manager by Timster's Audio Works, written in Rust with egui (via eframe) for the panel. Read `docs/ARCHITECTURE.md` before you change code, and `docs/DESIGN.md` before you change the panel. The panel follows Drone Box's design language, as EQ Box does. `docs/ECOSYSTEM.md` covers how it fits with Spool, EQ Box and SP-10.

## Hard rules

- **Native, not HTML.** No web views or browser shells.
- **Never change a music file without a plan the user has reviewed.** Until stage 3, nothing writes to music files at all. When writing comes: copy is the default, every change is logged and can be undone, and tags Music Box doesn't use are kept.
- **Never delete a music file.** Duplicates go to a Review folder.
- **No network requests unless Lookups is switched on.** Fonts and the icon are built into the binary.
- **The library engine stays free of UI code.** Everything in `crates/mb-core` is plain Rust and unit tested.
- **Never break the database or saved settings.** Schema changes go through a migration keyed on `meta.schema`. `Saved` in `ui/app.rs` uses `#[serde(default)]`: add fields with defaults; never rename or repurpose one.
- **Keep the panel code in step with EQ Box.** `ui/theme.rs` and `ui/widgets.rs` come from EQ Box; a change to the shared look goes to both until the family has one panel crate.

## Checks before pushing

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo clippy --workspace --target x86_64-pc-windows-gnu -- -D warnings   # Windows still compiles
```

For panel changes, look at the real app: build the test library (`tools/make-test-library.sh`), scan it into a test database (see README), and screenshot the panel under Xvfb at 1600 × 900 and at a laptop size such as 1280 × 720.

## Workflow

- Work on a feature branch. `main` always holds a working version.
- Open a pull request when the user asks, or merge when they ask.
- For every user-visible change, add a line under **Unreleased** in `CHANGELOG.md`, and tick off `ROADMAP.md`.
- For a release: move Unreleased into a section headed `## X.Y.Z – YYYY-MM-DD`, and bump `version` in `Cargo.toml` and the version in `README.md`. Merging that to `main` publishes the release with the Linux and Windows packages (`.github/workflows/release.yml`).
