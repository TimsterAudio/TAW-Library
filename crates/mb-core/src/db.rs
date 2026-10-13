//! The library database (SQLite). It only describes your files; it never
//! changes them. The scanner and the panel each open their own connection.

use crate::excludes::{Rule, RuleKind, DEFAULT_RULES};
use crate::tags::TrackInfo;
use crate::volumes::{Kind, Volume};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub type Result<T> = rusqlite::Result<T>;

const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT);
CREATE TABLE IF NOT EXISTS volumes (
    id TEXT PRIMARY KEY,
    label TEXT NOT NULL,
    kind TEXT NOT NULL,
    mount TEXT NOT NULL,
    system INTEGER NOT NULL DEFAULT 0,
    last_seen INTEGER,
    last_scan INTEGER,
    scan INTEGER NOT NULL DEFAULT 1
);
CREATE TABLE IF NOT EXISTS locations (
    volume TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
    rel TEXT NOT NULL,
    PRIMARY KEY (volume, rel)
);
CREATE TABLE IF NOT EXISTS excludes (
    id INTEGER PRIMARY KEY,
    kind TEXT NOT NULL,
    pattern TEXT NOT NULL,
    note TEXT NOT NULL DEFAULT '',
    hits INTEGER NOT NULL DEFAULT 0,
    UNIQUE (kind, pattern)
);
CREATE TABLE IF NOT EXISTS tracks (
    id INTEGER PRIMARY KEY,
    volume TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
    rel_path TEXT NOT NULL,
    dir TEXT NOT NULL,
    size INTEGER NOT NULL,
    mtime INTEGER NOT NULL,
    format TEXT NOT NULL,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    sample_rate INTEGER,
    bit_depth INTEGER,
    bitrate INTEGER,
    channels INTEGER,
    title TEXT,
    artist TEXT,
    album_artist TEXT,
    album TEXT,
    year INTEGER,
    track_no INTEGER,
    track_total INTEGER,
    disc_no INTEGER,
    disc_total INTEGER,
    genre TEXT,
    has_tags INTEGER NOT NULL DEFAULT 0,
    art_w INTEGER,
    art_h INTEGER,
    error TEXT,
    seen INTEGER NOT NULL,
    UNIQUE (volume, rel_path)
);
CREATE INDEX IF NOT EXISTS tracks_dir ON tracks (volume, dir);
CREATE TABLE IF NOT EXISTS folder_art (
    volume TEXT NOT NULL REFERENCES volumes(id) ON DELETE CASCADE,
    dir TEXT NOT NULL,
    file TEXT NOT NULL,
    w INTEGER NOT NULL,
    h INTEGER NOT NULL,
    seen INTEGER NOT NULL,
    PRIMARY KEY (volume, dir)
);
";

#[derive(Clone, Debug, PartialEq)]
pub struct VolumeRow {
    pub id: String,
    pub label: String,
    pub kind: Kind,
    pub mount: PathBuf,
    pub system: bool,
    pub last_seen: Option<i64>,
    pub last_scan: Option<i64>,
    /// Scan the whole drive. When false, only its locations are scanned.
    pub scan: bool,
    pub locations: Vec<String>,
    pub tracks: i64,
}

/// What the scanner already knows about a file.
#[derive(Clone, Copy, Debug)]
pub struct Known {
    pub id: i64,
    pub size: i64,
    pub mtime: i64,
}

pub struct Db {
    pub conn: Connection,
}

impl Db {
    /// Where the library lives, in the family's folder:
    /// `%APPDATA%\Timster's Audio Works\Library` on Windows,
    /// `~/.local/share/timsters-audio-works/library` on Linux.
    pub fn default_path() -> PathBuf {
        let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        Self::path_under(&base)
    }

    fn path_under(base: &Path) -> PathBuf {
        if cfg!(windows) {
            base.join("Timster's Audio Works")
                .join("Library")
                .join("library.db")
        } else {
            base.join("timsters-audio-works")
                .join("library")
                .join("library.db")
        }
    }

    /// Where Music Box 0.1 kept the library.
    fn old_path_under(base: &Path) -> PathBuf {
        if cfg!(windows) {
            base.join("Music Box").join("library.db")
        } else {
            base.join("music-box").join("library.db")
        }
    }

    /// Moves a library from where Music Box 0.1 kept it, if there's one and
    /// nothing at the new place yet. Returns true if it moved one. Only the
    /// database moves; music files are never touched.
    pub fn move_old_library() -> std::io::Result<bool> {
        let base = dirs::data_dir().unwrap_or_else(|| PathBuf::from("."));
        Self::move_library(&Self::old_path_under(&base), &Self::path_under(&base))
    }

    fn move_library(old: &Path, new: &Path) -> std::io::Result<bool> {
        if new.exists() || !old.exists() {
            return Ok(false);
        }
        if let Some(dir) = new.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // The database and SQLite's journal files beside it.
        for suffix in ["", "-wal", "-shm"] {
            let from = PathBuf::from(format!("{}{suffix}", old.display()));
            let to = PathBuf::from(format!("{}{suffix}", new.display()));
            if from.exists() && std::fs::rename(&from, &to).is_err() {
                std::fs::copy(&from, &to)?;
                std::fs::remove_file(&from)?;
            }
        }
        if let Some(dir) = old.parent() {
            let _ = std::fs::remove_dir(dir); // only if it's now empty
        }
        Ok(true)
    }

    pub fn open(path: &Path) -> Result<Db> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(10))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        Self::setup(conn)
    }

    pub fn open_in_memory() -> Result<Db> {
        Self::setup(Connection::open_in_memory()?)
    }

    fn setup(conn: Connection) -> Result<Db> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(SCHEMA)?;
        let version: Option<i64> = conn
            .query_row("SELECT value FROM meta WHERE key = 'schema'", [], |r| {
                r.get::<_, String>(0)
            })
            .optional()?
            .and_then(|s| s.parse().ok());
        if version.is_none() {
            conn.execute(
                "INSERT INTO meta (key, value) VALUES ('schema', ?1)",
                [SCHEMA_VERSION.to_string()],
            )?;
            for (kind, pattern, note) in DEFAULT_RULES {
                conn.execute(
                    "INSERT OR IGNORE INTO excludes (kind, pattern, note) VALUES (?1, ?2, ?3)",
                    params![kind.as_str(), pattern, note],
                )?;
            }
        }
        Ok(Db { conn })
    }

    // ------------------------------------------------------------ volumes

    /// Records the drives that are mounted now. New drives are set to be
    /// scanned whole; drives that have moved get their new mount point.
    pub fn sync_volumes(&self, mounted: &[Volume], now: i64) -> Result<()> {
        for v in mounted {
            self.conn.execute(
                "INSERT INTO volumes (id, label, kind, mount, system, last_seen, scan)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1)
                 ON CONFLICT (id) DO UPDATE SET
                    label = excluded.label, kind = excluded.kind, mount = excluded.mount,
                    system = excluded.system, last_seen = excluded.last_seen",
                params![
                    v.id,
                    v.label,
                    v.kind.as_str(),
                    v.mount.to_string_lossy(),
                    v.system,
                    now
                ],
            )?;
        }
        Ok(())
    }

    /// Adds a folder to scan on a drive that isn't scanned whole. A folder
    /// that isn't on any known drive becomes a volume of its own.
    pub fn add_location(&self, v: &Volume, rel: &str, now: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO volumes (id, label, kind, mount, system, last_seen, scan)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)
             ON CONFLICT (id) DO UPDATE SET last_seen = excluded.last_seen",
            params![
                v.id,
                v.label,
                v.kind.as_str(),
                v.mount.to_string_lossy(),
                v.system,
                now
            ],
        )?;
        self.conn.execute(
            "INSERT OR IGNORE INTO locations (volume, rel) VALUES (?1, ?2)",
            params![v.id, rel],
        )?;
        Ok(())
    }

    pub fn remove_location(&self, volume: &str, rel: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM locations WHERE volume = ?1 AND rel = ?2",
            params![volume, rel],
        )?;
        Ok(())
    }

    pub fn set_scan(&self, volume: &str, scan: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE volumes SET scan = ?2 WHERE id = ?1",
            params![volume, scan],
        )?;
        Ok(())
    }

    pub fn set_scanned(&self, volume: &str, now: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE volumes SET last_scan = ?2 WHERE id = ?1",
            params![volume, now],
        )?;
        Ok(())
    }

    /// Forgets a drive and everything found on it.
    pub fn remove_volume(&self, volume: &str) -> Result<()> {
        self.conn
            .execute("DELETE FROM volumes WHERE id = ?1", [volume])?;
        Ok(())
    }

    pub fn volumes(&self) -> Result<Vec<VolumeRow>> {
        let mut st = self.conn.prepare(
            "SELECT v.id, v.label, v.kind, v.mount, v.system, v.last_seen, v.last_scan, v.scan,
                    (SELECT COUNT(*) FROM tracks t WHERE t.volume = v.id)
             FROM volumes v ORDER BY v.system DESC, v.mount",
        )?;
        let mut rows: Vec<VolumeRow> = st
            .query_map([], |r| {
                Ok(VolumeRow {
                    id: r.get(0)?,
                    label: r.get(1)?,
                    kind: Kind::parse(&r.get::<_, String>(2)?),
                    mount: PathBuf::from(r.get::<_, String>(3)?),
                    system: r.get(4)?,
                    last_seen: r.get(5)?,
                    last_scan: r.get(6)?,
                    scan: r.get(7)?,
                    locations: Vec::new(),
                    tracks: r.get(8)?,
                })
            })?
            .collect::<Result<_>>()?;
        let mut st = self
            .conn
            .prepare("SELECT volume, rel FROM locations ORDER BY rel")?;
        let locs = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
        for l in locs {
            let (vol, rel) = l?;
            if let Some(v) = rows.iter_mut().find(|v| v.id == vol) {
                v.locations.push(rel);
            }
        }
        Ok(rows)
    }

    // ------------------------------------------------------------ excludes

    pub fn rules(&self) -> Result<Vec<Rule>> {
        let mut st = self
            .conn
            .prepare("SELECT id, kind, pattern, note, hits FROM excludes ORDER BY id")?;
        let rows = st.query_map([], |r| {
            Ok(Rule {
                id: r.get(0)?,
                kind: RuleKind::parse(&r.get::<_, String>(1)?),
                pattern: r.get(2)?,
                note: r.get(3)?,
                hits: r.get(4)?,
            })
        })?;
        rows.collect()
    }

    pub fn add_rule(&self, kind: RuleKind, pattern: &str, note: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO excludes (kind, pattern, note) VALUES (?1, ?2, ?3)",
            params![kind.as_str(), pattern, note],
        )?;
        Ok(())
    }

    pub fn remove_rule(&self, id: i64) -> Result<()> {
        self.conn
            .execute("DELETE FROM excludes WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn set_rule_hits(&self, id: i64, hits: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE excludes SET hits = ?2 WHERE id = ?1",
            params![id, hits],
        )?;
        Ok(())
    }

    // ------------------------------------------------------------ tracks

    pub fn known(&self, volume: &str) -> Result<HashMap<String, Known>> {
        let mut st = self
            .conn
            .prepare("SELECT rel_path, id, size, mtime FROM tracks WHERE volume = ?1")?;
        let rows = st.query_map([volume], |r| {
            Ok((
                r.get::<_, String>(0)?,
                Known {
                    id: r.get(1)?,
                    size: r.get(2)?,
                    mtime: r.get(3)?,
                },
            ))
        })?;
        rows.collect()
    }

    pub fn mark_seen(&self, id: i64, gen: i64) -> Result<()> {
        self.conn
            .prepare_cached("UPDATE tracks SET seen = ?2 WHERE id = ?1")?
            .execute(params![id, gen])?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_track(
        &self,
        volume: &str,
        rel_path: &str,
        dir: &str,
        size: i64,
        mtime: i64,
        format: &str,
        info: std::result::Result<&TrackInfo, &str>,
        gen: i64,
    ) -> Result<()> {
        let empty = TrackInfo::default();
        let (t, err) = match info {
            Ok(t) => (t, None),
            Err(e) => (&empty, Some(e)),
        };
        self.conn
            .prepare_cached(
                "INSERT INTO tracks (volume, rel_path, dir, size, mtime, format, duration_ms,
                    sample_rate, bit_depth, bitrate, channels, title, artist, album_artist, album,
                    year, track_no, track_total, disc_no, disc_total, genre, has_tags, art_w, art_h,
                    error, seen)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                    ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26)
                 ON CONFLICT (volume, rel_path) DO UPDATE SET
                    dir = excluded.dir, size = excluded.size, mtime = excluded.mtime,
                    format = excluded.format, duration_ms = excluded.duration_ms,
                    sample_rate = excluded.sample_rate, bit_depth = excluded.bit_depth,
                    bitrate = excluded.bitrate, channels = excluded.channels,
                    title = excluded.title, artist = excluded.artist,
                    album_artist = excluded.album_artist, album = excluded.album,
                    year = excluded.year, track_no = excluded.track_no,
                    track_total = excluded.track_total, disc_no = excluded.disc_no,
                    disc_total = excluded.disc_total, genre = excluded.genre,
                    has_tags = excluded.has_tags, art_w = excluded.art_w, art_h = excluded.art_h,
                    error = excluded.error, seen = excluded.seen",
            )?
            .execute(params![
                volume,
                rel_path,
                dir,
                size,
                mtime,
                format,
                t.duration_ms as i64,
                t.sample_rate,
                t.bit_depth,
                t.bitrate_kbps,
                t.channels,
                t.title,
                t.artist,
                t.album_artist,
                t.album,
                t.year,
                t.track_no,
                t.track_total,
                t.disc_no,
                t.disc_total,
                t.genre,
                t.has_tags,
                t.art.map(|a| a.0),
                t.art.map(|a| a.1),
                err,
                gen
            ])?;
        Ok(())
    }

    pub fn set_folder_art(
        &self,
        volume: &str,
        dir: &str,
        file: &str,
        size: (u32, u32),
        gen: i64,
    ) -> Result<()> {
        self.conn
            .prepare_cached(
                "INSERT INTO folder_art (volume, dir, file, w, h, seen) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (volume, dir) DO UPDATE SET
                    file = excluded.file, w = excluded.w, h = excluded.h, seen = excluded.seen",
            )?
            .execute(params![volume, dir, file, size.0, size.1, gen])?;
        Ok(())
    }

    /// After a complete scan of `root` (a folder inside the volume, "" for
    /// all of it), forgets files and folder art that weren't seen.
    /// Returns how many tracks were removed.
    pub fn remove_unseen(&self, volume: &str, root: &str, gen: i64) -> Result<usize> {
        let under = "(?3 = '' OR rel_path = ?3 OR substr(rel_path, 1, length(?3) + 1) = ?3 || '/')";
        let n = self.conn.execute(
            &format!("DELETE FROM tracks WHERE volume = ?1 AND seen <> ?2 AND {under}"),
            params![volume, gen, root],
        )?;
        let under_dir = under.replace("rel_path", "dir");
        self.conn.execute(
            &format!("DELETE FROM folder_art WHERE volume = ?1 AND seen <> ?2 AND {under_dir}"),
            params![volume, gen, root],
        )?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vol(id: &str, mount: &str) -> Volume {
        Volume {
            id: id.into(),
            label: id.into(),
            kind: Kind::Fixed,
            mount: mount.into(),
            system: false,
        }
    }

    #[test]
    fn old_library_moves_once() {
        let tmp = tempfile::tempdir().unwrap();
        let old = Db::old_path_under(tmp.path());
        let new = Db::path_under(tmp.path());
        // Nothing to move.
        assert!(!Db::move_library(&old, &new).unwrap());
        {
            let db = Db::open(&old).unwrap();
            db.add_rule(RuleKind::Name, "Stems", "test").unwrap();
        }
        assert!(Db::move_library(&old, &new).unwrap());
        assert!(!old.exists() && new.exists());
        let db = Db::open(&new).unwrap();
        assert!(db.rules().unwrap().iter().any(|r| r.pattern == "Stems"));
        // A library already at the new place is never overwritten.
        Db::open(&old).unwrap();
        assert!(!Db::move_library(&old, &new).unwrap());
    }

    #[test]
    fn volumes_and_rules() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.rules().unwrap().len(), DEFAULT_RULES.len());
        db.sync_volumes(&[vol("uuid:a", "/mnt/a")], 1).unwrap();
        // The same drive at a new mount point is the same volume.
        db.sync_volumes(&[vol("uuid:a", "/media/a")], 2).unwrap();
        let v = db.volumes().unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].mount, PathBuf::from("/media/a"));
        assert!(v[0].scan);
        db.set_scan("uuid:a", false).unwrap();
        db.add_location(&vol("uuid:a", "/media/a"), "Music", 3)
            .unwrap();
        let v = db.volumes().unwrap();
        assert_eq!(v[0].locations, ["Music"]);
    }

    #[test]
    fn unseen_tracks_are_forgotten_under_the_scanned_root_only() {
        let db = Db::open_in_memory().unwrap();
        db.sync_volumes(&[vol("v", "/m")], 1).unwrap();
        let info = TrackInfo::default();
        for p in ["A/1.mp3", "A/2.mp3", "AB/3.mp3", "B/4.mp3"] {
            db.upsert_track("v", p, "", 1, 1, "MP3", Ok(&info), 1)
                .unwrap();
        }
        // A rescan of A sees only 1.mp3.
        db.upsert_track("v", "A/1.mp3", "A", 1, 1, "MP3", Ok(&info), 2)
            .unwrap();
        assert_eq!(db.remove_unseen("v", "A", 2).unwrap(), 1);
        let known = db.known("v").unwrap();
        let mut left: Vec<_> = known.keys().cloned().collect();
        left.sort();
        assert_eq!(left, ["A/1.mp3", "AB/3.mp3", "B/4.mp3"]);
    }
}
