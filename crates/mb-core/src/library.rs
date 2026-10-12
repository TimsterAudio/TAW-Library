//! The library as the panel shows it: tracks grouped into albums and
//! artists, with what each album is missing. Built from the database in one
//! pass; cheap enough to rebuild after every scan.

use crate::db::{Db, Result};
use crate::SMALL_ART_PX;
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Clone, Debug, Default)]
pub struct Track {
    pub id: i64,
    pub volume: String,
    pub rel_path: String,
    pub dir: String,
    pub size: i64,
    pub format: String,
    pub duration_ms: i64,
    pub sample_rate: Option<i64>,
    pub bit_depth: Option<i64>,
    pub bitrate: Option<i64>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<i64>,
    pub track_no: Option<i64>,
    pub disc_no: Option<i64>,
    pub disc_total: Option<i64>,
    /// The disc number from the tags, or else from a CD1/Disc 2 folder.
    pub disc: Option<i64>,
    pub has_tags: bool,
    pub art: Option<(i64, i64)>,
    pub error: Option<String>,
}

impl Track {
    pub fn file_name(&self) -> &str {
        self.rel_path.rsplit('/').next().unwrap_or(&self.rel_path)
    }
    pub fn lossless(&self) -> bool {
        is_lossless(&self.format, self.bit_depth)
    }
}

/// FLAC, WAV, AIFF, APE and WavPack always; M4A when it's ALAC (it has a bit
/// depth; AAC doesn't).
pub fn is_lossless(format: &str, bit_depth: Option<i64>) -> bool {
    matches!(format, "FLAC" | "WAV" | "AIFF" | "APE" | "WV")
        || (format == "M4A" && bit_depth.is_some())
}

/// What an album can be missing. The panel's What's Missing view has a
/// tile for each.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Missing {
    Art,
    SmallArt,
    Year,
    AlbumArtist,
    TrackNo,
    /// More than one track with the same number and no disc numbers.
    DiscNo,
    /// Files with no title, artist or album at all.
    Tags,
}

impl Missing {
    pub const ALL: [Missing; 7] = [
        Missing::Art,
        Missing::SmallArt,
        Missing::Year,
        Missing::AlbumArtist,
        Missing::TrackNo,
        Missing::DiscNo,
        Missing::Tags,
    ];
    /// Short label for chips on album rows.
    pub fn chip(self) -> &'static str {
        match self {
            Missing::Art => "Art",
            Missing::SmallArt => "Art small",
            Missing::Year => "Year",
            Missing::AlbumArtist => "Album artist",
            Missing::TrackNo => "Track #",
            Missing::DiscNo => "Disc #",
            Missing::Tags => "Tags",
        }
    }
    /// Title for the What's Missing tile.
    pub fn title(self) -> &'static str {
        match self {
            Missing::Art => "No artwork",
            Missing::SmallArt => "Artwork under 500 px",
            Missing::Year => "No year",
            Missing::AlbumArtist => "No album artist",
            Missing::TrackNo => "No track number",
            Missing::DiscNo => "Disc numbers needed",
            Missing::Tags => "No tags at all",
        }
    }
    /// A warning rather than something missing.
    pub fn is_warning(self) -> bool {
        self == Missing::SmallArt
    }
}

/// Where an album's artwork comes from.
#[derive(Clone, Debug, PartialEq)]
pub enum Art {
    None,
    /// Embedded in a track (the largest one found).
    Embedded {
        track: usize,
        w: i64,
        h: i64,
    },
    /// An image file in the album's folder.
    Folder {
        file: String,
        w: i64,
        h: i64,
    },
}

impl Art {
    pub fn size(&self) -> Option<(i64, i64)> {
        match self {
            Art::None => None,
            Art::Embedded { w, h, .. } | Art::Folder { w, h, .. } => Some((*w, *h)),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Album {
    /// The album artist, or the track artist when every track shares one,
    /// or "Various Artists".
    pub artist: String,
    pub title: String,
    pub year: Option<i64>,
    /// Indexes into `Library::tracks`, in disc and track order.
    pub tracks: Vec<usize>,
    pub formats: BTreeSet<String>,
    pub duration_ms: i64,
    pub volume: String,
    /// The album's folder inside its volume (the parent of CD1/CD2 folders).
    pub dir: String,
    pub art: Art,
    pub missing: BTreeSet<Missing>,
    /// The album has no album tag; it's named after its folder.
    pub untitled: bool,
}

#[derive(Clone, Debug)]
pub struct Artist {
    pub name: String,
    /// Indexes into `Library::albums`, by year.
    pub albums: Vec<usize>,
    pub tracks: usize,
    /// Albums with something missing.
    pub incomplete: usize,
    /// Another artist that's probably the same one ("Beatles" and
    /// "The Beatles"): the name with more tracks.
    pub merge_into: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Library {
    pub tracks: Vec<Track>,
    pub albums: Vec<Album>,
    pub artists: Vec<Artist>,
    /// Albums with each kind of problem.
    pub missing_albums: BTreeMap<Missing, Vec<usize>>,
    /// Tracks with each kind of problem.
    pub missing_tracks: BTreeMap<Missing, usize>,
    /// Tracks per format, most first.
    pub formats: Vec<(String, usize)>,
    /// Share of tracks with every core field (artist, album artist, album,
    /// year, track number, title).
    pub tags_complete: f32,
    /// Share of albums with artwork of at least 500 px.
    pub art_complete: f32,
    pub unreadable: usize,
}

const DISC_DIR_PREFIXES: &[&str] = &["cd", "disc", "disk"];

/// "CD1", "Disc 2", "disk_3" are the parts of one album.
fn is_disc_dir(name: &str) -> bool {
    let l = name.to_lowercase();
    DISC_DIR_PREFIXES.iter().any(|p| {
        l.strip_prefix(p).is_some_and(|rest| {
            let rest = rest.trim_start_matches([' ', '_', '-', '.']);
            !rest.is_empty() && rest.chars().take_while(|c| c.is_ascii_digit()).count() > 0
        })
    })
}

fn split_dir(dir: &str) -> (&str, &str) {
    dir.rsplit_once('/').unwrap_or(("", dir))
}

/// The disc number in a "CD2" or "Disc 2" folder name.
pub fn disc_from_dir(dir: &str) -> Option<i64> {
    let name = split_dir(dir).1;
    if !is_disc_dir(name) {
        return None;
    }
    let digits: String = name
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}

/// The folder an album lives in: the parent of CD1/CD2 folders.
pub fn album_dir(dir: &str) -> &str {
    let (parent, name) = split_dir(dir);
    if is_disc_dir(name) && !parent.is_empty() {
        parent
    } else {
        dir
    }
}

/// A loose form of an artist name for spotting the same artist written two
/// ways: lower case, no leading "the", letters and digits only.
pub fn artist_key(name: &str) -> String {
    let l = name.trim().to_lowercase();
    let l = l.strip_prefix("the ").unwrap_or(&l).to_string();
    let l = l.replace('&', "and");
    l.chars().filter(|c| c.is_alphanumeric()).collect()
}

impl Library {
    pub fn load(db: &Db) -> Result<Library> {
        let mut st = db.conn.prepare(
            "SELECT id, volume, rel_path, dir, size, format, duration_ms, sample_rate, bit_depth,
                    bitrate, title, artist, album_artist, album, year, track_no, disc_no,
                    disc_total, has_tags, art_w, art_h, error
             FROM tracks",
        )?;
        let tracks = st
            .query_map([], |r| {
                let art_w: Option<i64> = r.get(19)?;
                let art_h: Option<i64> = r.get(20)?;
                Ok(Track {
                    id: r.get(0)?,
                    volume: r.get(1)?,
                    rel_path: r.get(2)?,
                    dir: r.get(3)?,
                    size: r.get(4)?,
                    format: r.get(5)?,
                    duration_ms: r.get(6)?,
                    sample_rate: r.get(7)?,
                    bit_depth: r.get(8)?,
                    bitrate: r.get(9)?,
                    title: r.get(10)?,
                    artist: r.get(11)?,
                    album_artist: r.get(12)?,
                    album: r.get(13)?,
                    year: r.get(14)?,
                    track_no: r.get(15)?,
                    disc_no: r.get(16)?,
                    disc_total: r.get(17)?,
                    disc: None,
                    has_tags: r.get(18)?,
                    art: art_w.zip(art_h),
                    error: r.get(21)?,
                })
            })?
            .collect::<Result<Vec<_>>>()?;
        let mut st = db
            .conn
            .prepare("SELECT volume, dir, file, w, h FROM folder_art")?;
        let folder_art: HashMap<(String, String), (String, i64, i64)> = st
            .query_map([], |r| {
                Ok(((r.get(0)?, r.get(1)?), (r.get(2)?, r.get(3)?, r.get(4)?)))
            })?
            .collect::<Result<_>>()?;
        Ok(Self::build(tracks, &folder_art))
    }

    pub fn build(
        mut tracks: Vec<Track>,
        folder_art: &HashMap<(String, String), (String, i64, i64)>,
    ) -> Library {
        for t in &mut tracks {
            t.disc = t.disc_no.or_else(|| disc_from_dir(&t.dir));
        }
        // Group: same drive, same album folder, same album name. Copies of
        // an album on two drives stay two albums (the Dupes view's job).
        let mut groups: BTreeMap<(String, String, String), Vec<usize>> = BTreeMap::new();
        for (i, t) in tracks.iter().enumerate() {
            let album = t.album.as_deref().unwrap_or("").to_lowercase();
            groups
                .entry((t.volume.clone(), album_dir(&t.dir).to_string(), album))
                .or_default()
                .push(i);
        }

        let mut albums = Vec::with_capacity(groups.len());
        for ((volume, dir, _), mut idx) in groups {
            idx.sort_by(|&a, &b| {
                let (a, b) = (&tracks[a], &tracks[b]);
                (
                    a.disc.unwrap_or(1),
                    a.track_no.unwrap_or(i64::MAX),
                    &a.rel_path,
                )
                    .cmp(&(
                        b.disc.unwrap_or(1),
                        b.track_no.unwrap_or(i64::MAX),
                        &b.rel_path,
                    ))
            });
            albums.push(Self::album(&tracks, idx, volume, dir, folder_art));
        }

        // Artists, case-insensitive.
        let mut by_name: BTreeMap<String, Artist> = BTreeMap::new();
        for (ai, a) in albums.iter().enumerate() {
            let e = by_name
                .entry(a.artist.to_lowercase())
                .or_insert_with(|| Artist {
                    name: a.artist.clone(),
                    albums: Vec::new(),
                    tracks: 0,
                    incomplete: 0,
                    merge_into: None,
                });
            e.albums.push(ai);
            e.tracks += a.tracks.len();
            if !a.missing.is_empty() {
                e.incomplete += 1;
            }
        }
        let mut artists: Vec<Artist> = by_name.into_values().collect();
        // Same loose key, different spelling: suggest merging the smaller
        // into the larger.
        let mut by_key: HashMap<String, Vec<usize>> = HashMap::new();
        for (i, a) in artists.iter().enumerate() {
            let k = artist_key(&a.name);
            if !k.is_empty() {
                by_key.entry(k).or_default().push(i);
            }
        }
        for idx in by_key.values().filter(|v| v.len() > 1) {
            let main = *idx
                .iter()
                .max_by_key(|&&i| (artists[i].tracks, artists[i].name.starts_with("The ")))
                .unwrap();
            let name = artists[main].name.clone();
            for &i in idx {
                if i != main {
                    artists[i].merge_into = Some(name.clone());
                }
            }
        }
        for a in &mut artists {
            a.albums.sort_by_key(|&i| {
                (
                    albums[i].year.unwrap_or(i64::MAX),
                    albums[i].title.to_lowercase(),
                )
            });
        }
        artists.sort_by_key(|a| sort_name(&a.name));

        let mut missing_albums: BTreeMap<Missing, Vec<usize>> = BTreeMap::new();
        for (i, a) in albums.iter().enumerate() {
            for m in &a.missing {
                missing_albums.entry(*m).or_default().push(i);
            }
        }
        for v in missing_albums.values_mut() {
            v.sort_by(|&a, &b| {
                (
                    sort_name(&albums[a].artist),
                    albums[a].year,
                    albums[a].title.to_lowercase(),
                )
                    .cmp(&(
                        sort_name(&albums[b].artist),
                        albums[b].year,
                        albums[b].title.to_lowercase(),
                    ))
            });
        }

        let mut missing_tracks: BTreeMap<Missing, usize> = BTreeMap::new();
        let mut complete = 0usize;
        let mut formats: HashMap<String, usize> = HashMap::new();
        let mut unreadable = 0;
        for t in &tracks {
            *formats.entry(t.format.clone()).or_default() += 1;
            if t.error.is_some() {
                unreadable += 1;
            }
            let checks = [
                (Missing::Year, t.year.is_none()),
                (Missing::AlbumArtist, t.album_artist.is_none()),
                (Missing::TrackNo, t.track_no.is_none()),
                (Missing::Tags, !t.has_tags),
            ];
            let mut ok = t.title.is_some() && t.artist.is_some() && t.album.is_some();
            for (m, bad) in checks {
                if bad {
                    *missing_tracks.entry(m).or_default() += 1;
                    ok = false;
                }
            }
            if ok {
                complete += 1;
            }
        }
        let mut formats: Vec<(String, usize)> = formats.into_iter().collect();
        formats.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        let good_art = albums
            .iter()
            .filter(|a| {
                a.art
                    .size()
                    .is_some_and(|(w, h)| w.max(h) >= SMALL_ART_PX as i64)
            })
            .count();
        let share = |n: usize, of: usize| if of == 0 { 0.0 } else { n as f32 / of as f32 };
        Library {
            tags_complete: share(complete, tracks.len()),
            art_complete: share(good_art, albums.len()),
            tracks,
            albums,
            artists,
            missing_albums,
            missing_tracks,
            formats,
            unreadable,
        }
    }

    fn album(
        tracks: &[Track],
        idx: Vec<usize>,
        volume: String,
        dir: String,
        folder_art: &HashMap<(String, String), (String, i64, i64)>,
    ) -> Album {
        let ts: Vec<&Track> = idx.iter().map(|&i| &tracks[i]).collect();
        let most_common = |f: &dyn Fn(&Track) -> Option<&str>| -> Option<String> {
            let mut n: HashMap<&str, usize> = HashMap::new();
            for t in &ts {
                if let Some(v) = f(t) {
                    *n.entry(v).or_default() += 1;
                }
            }
            n.into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(a.0)))
                .map(|(s, _)| s.to_string())
        };
        let distinct_artists: BTreeSet<String> = ts
            .iter()
            .filter_map(|t| t.artist.as_ref().map(|a| a.to_lowercase()))
            .collect();
        let artist = most_common(&|t| t.album_artist.as_deref())
            .or_else(|| {
                if distinct_artists.len() > 1 {
                    Some("Various Artists".to_string())
                } else {
                    most_common(&|t| t.artist.as_deref())
                }
            })
            .unwrap_or_else(|| "Unknown Artist".to_string());
        let album_tag = most_common(&|t| t.album.as_deref());
        let untitled = album_tag.is_none();
        let title = album_tag.unwrap_or_else(|| {
            let name = split_dir(&dir).1;
            if name.is_empty() {
                "(top of drive)".to_string()
            } else {
                name.to_string()
            }
        });
        let year = {
            let mut n: HashMap<i64, usize> = HashMap::new();
            for y in ts.iter().filter_map(|t| t.year) {
                *n.entry(y).or_default() += 1;
            }
            n.into_iter()
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
                .map(|(y, _)| y)
        };

        let mut art = Art::None;
        for (k, &i) in idx.iter().enumerate() {
            if let Some((w, h)) = tracks[i].art {
                if art.size().is_none_or(|(bw, bh)| w * h > bw * bh) {
                    art = Art::Embedded {
                        track: idx[k],
                        w,
                        h,
                    };
                }
            }
        }
        // Folder art from the album folder or a disc folder.
        let dirs: BTreeSet<&str> = std::iter::once(dir.as_str())
            .chain(ts.iter().map(|t| t.dir.as_str()))
            .collect();
        for d in dirs {
            if let Some((file, w, h)) = folder_art.get(&(volume.clone(), d.to_string())) {
                if art.size().is_none_or(|(bw, bh)| w * h > bw * bh) {
                    let file = if d == dir {
                        file.clone()
                    } else {
                        format!("{}/{file}", d[dir.len()..].trim_start_matches('/'))
                    };
                    art = Art::Folder { file, w: *w, h: *h };
                }
            }
        }

        let mut missing = BTreeSet::new();
        match art.size() {
            None => {
                missing.insert(Missing::Art);
            }
            Some((w, h)) if w.max(h) < SMALL_ART_PX as i64 => {
                missing.insert(Missing::SmallArt);
            }
            _ => {}
        }
        if ts.iter().any(|t| t.year.is_none()) {
            missing.insert(Missing::Year);
        }
        if ts.iter().any(|t| t.album_artist.is_none()) {
            missing.insert(Missing::AlbumArtist);
        }
        if ts.iter().any(|t| t.track_no.is_none()) {
            missing.insert(Missing::TrackNo);
        }
        if ts.iter().any(|t| !t.has_tags) {
            missing.insert(Missing::Tags);
        }
        let mut seen = BTreeSet::new();
        let repeated = ts
            .iter()
            .filter_map(|t| t.track_no.map(|n| (t.disc_no, n)))
            .any(|k| !seen.insert(k));
        if repeated && ts.iter().any(|t| t.disc_no.is_none()) {
            missing.insert(Missing::DiscNo);
        }

        Album {
            formats: ts.iter().map(|t| t.format.clone()).collect(),
            duration_ms: ts.iter().map(|t| t.duration_ms).sum(),
            artist,
            title,
            year,
            tracks: idx,
            volume,
            dir,
            art,
            missing,
            untitled,
        }
    }
}

/// Sort "The Beatles" under B.
pub fn sort_name(name: &str) -> String {
    let l = name.to_lowercase();
    l.strip_prefix("the ").map(str::to_string).unwrap_or(l)
}

/// "53:21" or "1:13:30".
pub fn fmt_duration(ms: i64) -> String {
    let s = (ms + 500) / 1000;
    let (h, m, s) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(
        dir: &str,
        file: &str,
        artist: Option<&str>,
        album: Option<&str>,
        n: Option<i64>,
    ) -> Track {
        Track {
            volume: "v".into(),
            dir: dir.into(),
            rel_path: format!("{dir}/{file}"),
            format: "FLAC".into(),
            duration_ms: 60_000,
            artist: artist.map(Into::into),
            album: album.map(Into::into),
            title: Some(file.into()),
            track_no: n,
            has_tags: artist.is_some() || album.is_some(),
            ..Default::default()
        }
    }

    #[test]
    fn grouping_and_missing() {
        let mut a1 = t(
            "Radiohead/OK Computer",
            "01.flac",
            Some("Radiohead"),
            Some("OK Computer"),
            Some(1),
        );
        a1.album_artist = Some("Radiohead".into());
        a1.year = Some(1997);
        a1.art = Some((1400, 1400));
        let mut a2 = a1.clone();
        a2.rel_path = "Radiohead/OK Computer/02.flac".into();
        a2.track_no = Some(2);
        // Two discs in CD1/CD2 folders, both with a track 1 and no disc numbers.
        let d1 = t(
            "Beatles/White Album/CD1",
            "01.flac",
            Some("Beatles"),
            Some("The White Album"),
            Some(1),
        );
        let d2 = t(
            "Beatles/White Album/CD2",
            "01.flac",
            Some("Beatles"),
            Some("The White Album"),
            Some(1),
        );
        let b = t(
            "The Beatles/Abbey Road",
            "01.flac",
            Some("The Beatles"),
            Some("Abbey Road"),
            Some(1),
        );
        let b2 = t(
            "The Beatles/Abbey Road",
            "02.flac",
            Some("The Beatles"),
            Some("Abbey Road"),
            Some(2),
        );
        let comp1 = t("Comp", "01.flac", Some("Artist A"), Some("Mix"), Some(1));
        let comp2 = t("Comp", "02.flac", Some("Artist B"), Some("Mix"), Some(2));
        let raw = t("Downloads/stuff", "track01.wav", None, None, None);

        let mut fa = HashMap::new();
        fa.insert(
            ("v".to_string(), "The Beatles/Abbey Road".to_string()),
            ("cover.jpg".to_string(), 300, 300),
        );
        let lib = Library::build(vec![a1, a2, d1, d2, b, b2, comp1, comp2, raw], &fa);

        let find = |title: &str| lib.albums.iter().find(|a| a.title == title).unwrap();
        let ok = find("OK Computer");
        assert!(ok.missing.is_empty(), "{:?}", ok.missing);
        assert_eq!(ok.tracks.len(), 2);
        assert_eq!(ok.year, Some(1997));

        let white = find("The White Album");
        assert_eq!(white.tracks.len(), 2);
        // CD1 before CD2, from the folder names.
        assert_eq!(lib.tracks[white.tracks[0]].dir, "Beatles/White Album/CD1");
        assert_eq!(lib.tracks[white.tracks[1]].disc, Some(2));
        assert_eq!(white.dir, "Beatles/White Album");
        assert!(white.missing.contains(&Missing::DiscNo));
        assert!(white.missing.contains(&Missing::Art));

        let abbey = find("Abbey Road");
        assert!(abbey.missing.contains(&Missing::SmallArt));
        assert!(matches!(abbey.art, Art::Folder { .. }));

        assert_eq!(find("Mix").artist, "Various Artists");

        let raw = find("stuff");
        assert!(raw.untitled);
        assert!(raw.missing.contains(&Missing::Tags));

        // Same number of tracks: the name with "The" wins.
        let beatles = lib.artists.iter().find(|a| a.name == "Beatles").unwrap();
        assert_eq!(beatles.merge_into.as_deref(), Some("The Beatles"));
        let the = lib
            .artists
            .iter()
            .find(|a| a.name == "The Beatles")
            .unwrap();
        assert_eq!(the.merge_into, None);

        assert_eq!(lib.missing_tracks[&Missing::Tags], 1);
        assert_eq!(lib.art_complete, 1.0 / lib.albums.len() as f32);
    }

    #[test]
    fn small_things() {
        assert_eq!(artist_key("The Beatles"), artist_key("beatles"));
        assert_eq!(
            artist_key("Simon & Garfunkel"),
            artist_key("Simon and Garfunkel")
        );
        assert_eq!(album_dir("A/Album/CD 2"), "A/Album");
        assert_eq!(album_dir("A/Album/Disc1"), "A/Album");
        assert_eq!(album_dir("A/CDs"), "A/CDs");
        assert_eq!(disc_from_dir("A/Album/CD 2"), Some(2));
        assert_eq!(disc_from_dir("A/Album/Disc_10"), Some(10));
        assert_eq!(disc_from_dir("A/Album"), None);
        assert_eq!(fmt_duration(3_201_000), "53:21");
        assert_eq!(fmt_duration(4_410_000), "1:13:30");
        assert!(is_lossless("M4A", Some(16)));
        assert!(!is_lossless("M4A", None));
    }
}
