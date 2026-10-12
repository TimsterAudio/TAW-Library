//! Scanning drives into the database. Read-only: files are listed and their
//! tags read, nothing more. Files that haven't changed since the last scan
//! (same size and date) aren't read again.

use crate::db::{Db, Known, Result, VolumeRow};
use crate::excludes::{is_daw_marker, Excludes, Skip};
use crate::volumes::{self, Kind, Volume};
use crate::{is_audio, tags, FOLDER_ART_NAMES, IMAGE_EXTS};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Shared with the panel while a scan runs.
#[derive(Default)]
pub struct Progress {
    pub running: AtomicBool,
    pub cancel: AtomicBool,
    pub folders: AtomicU64,
    /// Music files found, changed or not.
    pub found: AtomicU64,
    /// New or changed files whose tags were read.
    pub read: AtomicU64,
    pub removed: AtomicU64,
    pub errors: AtomicU64,
    /// Bumped after each batch is saved, so the panel knows to reload.
    pub saved: AtomicU64,
    /// The drive being scanned.
    pub current: Mutex<String>,
}

impl Progress {
    pub fn reset(&self) {
        for a in [
            &self.folders,
            &self.found,
            &self.read,
            &self.removed,
            &self.errors,
        ] {
            a.store(0, Relaxed);
        }
        self.cancel.store(false, Relaxed);
    }
}

/// One drive to scan, and which folders on it.
#[derive(Clone, Debug)]
pub struct Job {
    pub volume: String,
    pub label: String,
    pub mount: PathBuf,
    /// Folders inside the drive; "" means the whole drive.
    pub roots: Vec<String>,
}

/// Whether a known drive is there right now.
pub fn is_online(v: &VolumeRow, mounted: &[Volume]) -> bool {
    match v.kind {
        Kind::Folder | Kind::Network if !mounted.iter().any(|m| m.id == v.id) => v.mount.is_dir(),
        _ => mounted.iter().any(|m| m.id == v.id),
    }
}

/// The scan jobs for every drive that's there now.
pub fn plan(db: &Db, mounted: &[Volume]) -> Result<Vec<Job>> {
    Ok(db
        .volumes()?
        .into_iter()
        .filter(|v| is_online(v, mounted))
        .filter_map(|v| {
            let roots = if v.scan {
                vec![String::new()]
            } else {
                v.locations.clone()
            };
            (!roots.is_empty()).then_some(Job {
                volume: v.id,
                label: v.label,
                mount: v.mount,
                roots,
            })
        })
        .collect())
}

#[derive(Debug, Default, PartialEq)]
pub struct Summary {
    pub found: u64,
    pub read: u64,
    pub removed: u64,
    pub errors: u64,
    pub cancelled: bool,
}

/// Runs the jobs. `gen` marks everything seen in this scan (use the time).
pub fn run(db: &Db, jobs: &[Job], ex: &Excludes, progress: &Progress, gen: i64) -> Result<Summary> {
    let mut hits = vec![0i64; ex.rules.len()];
    let mut cancelled = false;
    for job in jobs {
        *progress.current.lock().unwrap() = job.label.clone();
        let mut s = Scanner {
            db,
            ex,
            progress,
            gen,
            job,
            known: db.known(&job.volume)?,
            hits: &mut hits,
            pending: 0,
            last_commit: Instant::now(),
            #[cfg(unix)]
            dev: None,
        };
        db.conn.execute_batch("BEGIN")?;
        let mut complete_roots = Vec::new();
        for root in &job.roots {
            if s.walk_root(root)? {
                complete_roots.push(root.clone());
            } else {
                cancelled = true;
                break;
            }
        }
        db.conn.execute_batch("COMMIT")?;
        for root in &complete_roots {
            let n = db.remove_unseen(&job.volume, root, gen)?;
            progress.removed.fetch_add(n as u64, Relaxed);
        }
        if !cancelled {
            db.set_scanned(&job.volume, crate::now_secs())?;
        }
        progress.saved.fetch_add(1, Relaxed);
        if cancelled {
            break;
        }
    }
    if !cancelled {
        for (r, h) in ex.rules.iter().zip(&hits) {
            db.set_rule_hits(r.id, *h)?;
        }
    }
    Ok(Summary {
        found: progress.found.load(Relaxed),
        read: progress.read.load(Relaxed),
        removed: progress.removed.load(Relaxed),
        errors: progress.errors.load(Relaxed),
        cancelled,
    })
}

struct Scanner<'a> {
    db: &'a Db,
    ex: &'a Excludes,
    progress: &'a Progress,
    gen: i64,
    job: &'a Job,
    known: HashMap<String, Known>,
    hits: &'a mut Vec<i64>,
    pending: usize,
    last_commit: Instant,
    /// The drive's device, so the walk doesn't wander onto other drives
    /// mounted inside it.
    #[cfg(unix)]
    dev: Option<u64>,
}

fn mtime_secs(m: &std::fs::Metadata) -> i64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn ext_of(name: &str) -> Option<&str> {
    Path::new(name).extension().and_then(|e| e.to_str())
}

impl Scanner<'_> {
    /// Returns false if the scan was cancelled.
    fn walk_root(&mut self, root: &str) -> Result<bool> {
        let start = volumes::join(&self.job.mount, root);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            self.dev = std::fs::metadata(&self.job.mount).ok().map(|m| m.dev());
        }
        let mut stack = vec![(start, root.to_string())];
        while let Some((dir, rel)) = stack.pop() {
            if self.progress.cancel.load(Relaxed) {
                return Ok(false);
            }
            self.dir(&dir, &rel, &mut stack)?;
            if self.pending >= 400 || self.last_commit.elapsed() > Duration::from_secs(2) {
                self.db.conn.execute_batch("COMMIT; BEGIN")?;
                self.pending = 0;
                self.last_commit = Instant::now();
                self.progress.saved.fetch_add(1, Relaxed);
            }
        }
        Ok(true)
    }

    fn dir(&mut self, dir: &Path, rel: &str, stack: &mut Vec<(PathBuf, String)>) -> Result<()> {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return Ok(()); // no permission, or gone
        };
        self.progress.folders.fetch_add(1, Relaxed);
        let entries: Vec<_> = rd.flatten().collect();
        let names: Vec<String> = entries
            .iter()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        // A DAW project's audio is stems and bounces: leave the whole folder.
        if names.iter().any(|n| is_daw_marker(n)) {
            return Ok(());
        }
        let child = |name: &str| {
            if rel.is_empty() {
                name.to_string()
            } else {
                format!("{rel}/{name}")
            }
        };
        let mut art: Option<(usize, String)> = None;
        let mut subdirs = Vec::new();
        for (e, name) in entries.iter().zip(&names) {
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                let path = e.path();
                match self.ex.skip_dir(&path, name, rel.is_empty()) {
                    Some(Skip::Rule(i)) => self.hits[i] += 1,
                    Some(_) => {}
                    None => {
                        #[cfg(unix)]
                        {
                            use std::os::unix::fs::MetadataExt;
                            if let (Some(dev), Ok(m)) = (self.dev, e.metadata()) {
                                if m.dev() != dev {
                                    continue;
                                }
                            }
                        }
                        subdirs.push((path, child(name)));
                    }
                }
                continue;
            }
            if !ft.is_file() {
                continue;
            }
            let Some(ext) = ext_of(name) else { continue };
            if is_audio(ext) {
                self.file(e, &child(name), rel, ext)?;
            } else if IMAGE_EXTS.iter().any(|x| x.eq_ignore_ascii_case(ext)) {
                let stem = Path::new(name)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if let Some(rank) = FOLDER_ART_NAMES.iter().position(|n| *n == stem) {
                    if art.as_ref().is_none_or(|(r, _)| rank < *r) {
                        art = Some((rank, name.clone()));
                    }
                }
            }
        }
        if let Some((_, name)) = art {
            if let Ok(sz) = imagesize::size(dir.join(&name)) {
                self.db.set_folder_art(
                    &self.job.volume,
                    rel,
                    &name,
                    (sz.width as u32, sz.height as u32),
                    self.gen,
                )?;
            }
        }
        // Walk sub-folders in name order (the stack pops from the end).
        subdirs.sort_by_key(|d| std::cmp::Reverse(d.1.to_lowercase()));
        stack.extend(subdirs);
        Ok(())
    }

    fn file(&mut self, e: &std::fs::DirEntry, rel_path: &str, dir: &str, ext: &str) -> Result<()> {
        let Ok(meta) = e.metadata() else {
            return Ok(());
        };
        self.progress.found.fetch_add(1, Relaxed);
        let (size, mtime) = (meta.len() as i64, mtime_secs(&meta));
        if let Some(k) = self.known.get(rel_path) {
            if k.size == size && k.mtime == mtime {
                self.db.mark_seen(k.id, self.gen)?;
                return Ok(());
            }
        }
        let mut format = ext.to_uppercase();
        if format == "AIF" {
            format = "AIFF".into();
        }
        let info = tags::read(&e.path());
        if info.is_err() {
            self.progress.errors.fetch_add(1, Relaxed);
        }
        self.db.upsert_track(
            &self.job.volume,
            rel_path,
            dir,
            size,
            mtime,
            &format,
            info.as_ref().map_err(|s| s.as_str()),
            self.gen,
        )?;
        self.progress.read.fetch_add(1, Relaxed);
        self.pending += 1;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::excludes::{Rule, RuleKind};
    use lofty::config::WriteOptions;
    use lofty::tag::{Accessor, ItemKey, Tag, TagExt, TagType};

    /// A short silent 16-bit WAV file.
    pub fn write_wav(path: &Path, secs: f32) {
        let rate = 44_100u32;
        let frames = (rate as f32 * secs) as u32;
        let data_len = frames * 4;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36 + data_len).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&rate.to_le_bytes());
        b.extend_from_slice(&(rate * 4).to_le_bytes());
        b.extend_from_slice(&4u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&data_len.to_le_bytes());
        b.resize(b.len() + data_len as usize, 0);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b).unwrap();
    }

    pub struct T<'a> {
        pub artist: &'a str,
        pub album_artist: Option<&'a str>,
        pub album: &'a str,
        pub title: &'a str,
        pub track: u32,
        pub year: Option<u32>,
    }

    pub fn tagged_wav(path: &Path, t: T) {
        write_wav(path, 0.5);
        let mut tag = Tag::new(TagType::Id3v2);
        tag.set_artist(t.artist.into());
        tag.set_album(t.album.into());
        tag.set_title(t.title.into());
        tag.set_track(t.track);
        if let Some(y) = t.year {
            tag.set_year(y);
        }
        if let Some(aa) = t.album_artist {
            tag.insert_text(ItemKey::AlbumArtist, aa.into());
        }
        tag.save_to_path(path, WriteOptions::default()).unwrap();
    }

    fn setup(dir: &Path) {
        let m = dir.join("Music");
        tagged_wav(
            &m.join("Radiohead/OK Computer/01 Airbag.wav"),
            T {
                artist: "Radiohead",
                album_artist: Some("Radiohead"),
                album: "OK Computer",
                title: "Airbag",
                track: 1,
                year: Some(1997),
            },
        );
        tagged_wav(
            &m.join("Radiohead/OK Computer/02 Paranoid Android.wav"),
            T {
                artist: "Radiohead",
                album_artist: Some("Radiohead"),
                album: "OK Computer",
                title: "Paranoid Android",
                track: 2,
                year: Some(1997),
            },
        );
        write_wav(&m.join("untagged/track01.wav"), 0.5);
        std::fs::write(m.join("Radiohead/OK Computer/notes.txt"), "x").unwrap();
        // Left out: a DAW project, a sample pack and a hidden folder.
        write_wav(&dir.join("Projects/Song/bounce.wav"), 0.2);
        std::fs::write(dir.join("Projects/Song/Song.als"), "x").unwrap();
        write_wav(&dir.join("Samples/kick.wav"), 0.2);
        write_wav(&dir.join(".cache/x.wav"), 0.2);
    }

    #[test]
    fn scans_rescans_and_forgets() {
        let tmp = tempfile::tempdir().unwrap();
        setup(tmp.path());
        let db = Db::open_in_memory().unwrap();
        let v = volumes::folder_volume(tmp.path());
        db.add_location(&v, "", 1).unwrap();
        let jobs = plan(&db, &[]).unwrap();
        assert_eq!(jobs.len(), 1);
        let ex = Excludes::new(vec![Rule {
            id: db.rules().unwrap()[0].id,
            kind: RuleKind::Name,
            pattern: "Samples".into(),
            note: String::new(),
            hits: 0,
        }]);
        let p = Progress::default();
        let s = run(&db, &jobs, &ex, &p, 10).unwrap();
        assert_eq!((s.found, s.read, s.errors), (3, 3, 0));
        assert_eq!(db.rules().unwrap()[0].hits, 1);

        let known = db.known(&v.id).unwrap();
        let mut paths: Vec<_> = known.keys().cloned().collect();
        paths.sort();
        assert_eq!(
            paths,
            [
                "Music/Radiohead/OK Computer/01 Airbag.wav",
                "Music/Radiohead/OK Computer/02 Paranoid Android.wav",
                "Music/untagged/track01.wav"
            ]
        );
        let (title, year, has_tags): (String, u32, bool) = db
            .conn
            .query_row(
                "SELECT title, year, has_tags FROM tracks WHERE rel_path LIKE '%Airbag%'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!((title.as_str(), year, has_tags), ("Airbag", 1997, true));

        // Nothing changed: nothing is read again.
        p.reset();
        let s = run(&db, &jobs, &ex, &p, 11).unwrap();
        assert_eq!((s.found, s.read, s.removed), (3, 0, 0));

        // A file that's gone is forgotten.
        std::fs::remove_file(tmp.path().join("Music/untagged/track01.wav")).unwrap();
        p.reset();
        let s = run(&db, &jobs, &ex, &p, 12).unwrap();
        assert_eq!((s.found, s.removed), (2, 1));
    }
}
