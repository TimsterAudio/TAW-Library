//! Stage 2: looking albums up on MusicBrainz, and their artwork on the
//! Cover Art Archive. Nothing here runs on its own: the panel only makes a
//! [`Client`] while Lookups is switched on. What comes back becomes
//! proposals in the database ([`crate::db::Db::save_lookup`]); nothing is
//! written to music files until stage 3, and then only from a plan the
//! user has reviewed.
//!
//! What goes online for an album: its artist, title and track count (the
//! search), then the chosen release's id (its tracks and artwork).

use serde::{Deserialize, Serialize};
use std::io::Read;
use std::time::{Duration, Instant};

/// Who's asking, as MusicBrainz asks every program to say.
pub const USER_AGENT: &str = concat!(
    "TAW-Library/",
    env!("CARGO_PKG_VERSION"),
    " ( https://github.com/TimsterAudio/TAW-Library )"
);
const MB: &str = "https://musicbrainz.org/ws/2";
const CAA: &str = "https://coverartarchive.org";
/// MusicBrainz allows one request a second.
const GAP: Duration = Duration::from_millis(1100);
/// The most a reply may be: 4 MB of JSON, 12 MB of artwork.
const MAX_JSON: u64 = 4 << 20;
const MAX_ART: u64 = 12 << 20;

// ---------------------------------------------------------------- queries

/// What's searched for: an album as the Library knows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Query {
    pub artist: String,
    pub album: String,
    pub tracks: usize,
    pub year: Option<i64>,
}

/// A release search: the album's title and artist as phrases, best 10.
/// "Various Artists" and "Unknown Artist" aren't searched for by name.
pub fn search_url(q: &Query) -> String {
    let mut lucene = format!("release:{}", phrase(&q.album));
    if !is_placeholder_artist(&q.artist) {
        lucene += &format!(" AND artist:{}", phrase(&q.artist));
    }
    format!(
        "{MB}/release/?query={}&fmt=json&limit=10",
        percent_encode(&lucene)
    )
}

/// One release with its tracks and their artists.
pub fn release_url(mbid: &str) -> String {
    format!(
        "{MB}/release/{}?inc=recordings+artist-credits&fmt=json",
        percent_encode(mbid)
    )
}

/// The release's front cover, 500 px on its longer side where there's one
/// that big (the Library's own threshold for artwork that's too small).
pub fn front_art_url(mbid: &str) -> String {
    format!("{CAA}/release/{}/front-500", percent_encode(mbid))
}

fn is_placeholder_artist(a: &str) -> bool {
    let a = a.trim().to_lowercase();
    a.is_empty() || a == "various artists" || a == "unknown artist"
}

/// A quoted Lucene phrase.
fn phrase(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.trim().chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out += &format!("%{b:02X}"),
        }
    }
    out
}

// ---------------------------------------------------------------- replies

#[derive(Deserialize)]
struct RawCredit {
    #[serde(default)]
    name: String,
    #[serde(default)]
    joinphrase: String,
}

fn credit(c: &[RawCredit]) -> String {
    c.iter()
        .map(|c| format!("{}{}", c.name, c.joinphrase))
        .collect()
}

fn year_of(date: Option<&str>) -> Option<i64> {
    date?.get(..4)?.parse().ok()
}

#[derive(Deserialize)]
struct RawSearch {
    #[serde(default)]
    releases: Vec<RawSearchRelease>,
}

#[derive(Deserialize)]
struct RawSearchRelease {
    id: String,
    #[serde(default)]
    score: i64,
    #[serde(default)]
    title: String,
    #[serde(rename = "artist-credit", default)]
    artist_credit: Vec<RawCredit>,
    date: Option<String>,
    country: Option<String>,
    #[serde(rename = "track-count", default)]
    track_count: usize,
    #[serde(default)]
    media: Vec<RawSearchMedium>,
}

#[derive(Deserialize)]
struct RawSearchMedium {
    format: Option<String>,
}

/// A release MusicBrainz offered for an album.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    pub mbid: String,
    pub title: String,
    pub artist: String,
    pub year: Option<i64>,
    pub country: Option<String>,
    /// "CD", "2×CD", "Digital Media".
    pub format: Option<String>,
    pub tracks: usize,
    /// MusicBrainz's own score for the search, 0 to 100.
    pub score: i64,
}

/// The releases in a search reply.
pub fn parse_search(json: &str) -> Result<Vec<Candidate>, String> {
    let raw: RawSearch =
        serde_json::from_str(json).map_err(|e| format!("MusicBrainz's reply didn't read: {e}"))?;
    Ok(raw
        .releases
        .into_iter()
        .map(|r| Candidate {
            year: year_of(r.date.as_deref()),
            artist: credit(&r.artist_credit),
            format: medium_format(&r.media),
            mbid: r.id,
            title: r.title,
            country: r.country,
            tracks: r.track_count,
            score: r.score,
        })
        .collect())
}

fn medium_format(media: &[RawSearchMedium]) -> Option<String> {
    let first = media.first()?.format.clone()?;
    if media.len() > 1 && media.iter().all(|m| m.format.as_ref() == Some(&first)) {
        Some(format!("{}×{first}", media.len()))
    } else if media.len() > 1 {
        Some(format!("{} discs", media.len()))
    } else {
        Some(first)
    }
}

/// How well a candidate fits the album: MusicBrainz's score, with the
/// track count (which must match to be sure) and the year counted in.
pub fn fit(c: &Candidate, q: &Query) -> i64 {
    let mut f = c.score;
    if c.tracks == q.tracks {
        f += 20;
    } else {
        f -= 30;
    }
    if q.year.is_some() && c.year == q.year {
        f += 5;
    }
    f
}

/// The candidates, best fit first (ties: the earlier release).
pub fn rank(mut c: Vec<Candidate>, q: &Query) -> Vec<Candidate> {
    c.sort_by(|a, b| {
        fit(b, q)
            .cmp(&fit(a, q))
            .then(a.year.unwrap_or(9999).cmp(&b.year.unwrap_or(9999)))
    });
    c
}

/// Sure enough to propose changes without asking: a strong search score
/// and the same number of tracks.
pub fn confident(c: &Candidate, q: &Query) -> bool {
    c.score >= 90 && c.tracks == q.tracks
}

#[derive(Deserialize)]
struct RawRelease {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(rename = "artist-credit", default)]
    artist_credit: Vec<RawCredit>,
    date: Option<String>,
    #[serde(default)]
    media: Vec<RawMedium>,
    #[serde(rename = "cover-art-archive")]
    cover_art: Option<RawCoverArt>,
}

#[derive(Deserialize)]
struct RawMedium {
    #[serde(default)]
    position: i64,
    #[serde(default)]
    tracks: Vec<RawTrack>,
}

#[derive(Deserialize)]
struct RawTrack {
    #[serde(default)]
    position: i64,
    #[serde(default)]
    title: String,
    length: Option<i64>,
    #[serde(rename = "artist-credit", default)]
    artist_credit: Vec<RawCredit>,
}

#[derive(Deserialize)]
struct RawCoverArt {
    #[serde(default)]
    front: bool,
}

/// A release's tracks, as MusicBrainz has them.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    pub mbid: String,
    pub title: String,
    pub artist: String,
    pub year: Option<i64>,
    pub discs: usize,
    /// In disc and track order.
    pub tracks: Vec<ReleaseTrack>,
    /// The Cover Art Archive has a front cover for it.
    pub front_art: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReleaseTrack {
    pub disc: i64,
    pub number: i64,
    pub title: String,
    pub artist: String,
    pub length_ms: Option<i64>,
}

/// A release reply (with `inc=recordings+artist-credits`).
pub fn parse_release(json: &str) -> Result<Release, String> {
    let r: RawRelease =
        serde_json::from_str(json).map_err(|e| format!("MusicBrainz's reply didn't read: {e}"))?;
    let artist = credit(&r.artist_credit);
    let mut tracks = Vec::new();
    for (i, m) in r.media.iter().enumerate() {
        let disc = if m.position > 0 {
            m.position
        } else {
            i as i64 + 1
        };
        for (j, t) in m.tracks.iter().enumerate() {
            tracks.push(ReleaseTrack {
                disc,
                number: if t.position > 0 {
                    t.position
                } else {
                    j as i64 + 1
                },
                title: t.title.clone(),
                artist: if t.artist_credit.is_empty() {
                    artist.clone()
                } else {
                    credit(&t.artist_credit)
                },
                length_ms: t.length,
            });
        }
    }
    Ok(Release {
        year: year_of(r.date.as_deref()),
        discs: r.media.len(),
        front_art: r.cover_art.is_some_and(|c| c.front),
        mbid: r.id,
        title: r.title,
        artist,
        tracks,
    })
}

// ---------------------------------------------------------------- proposals

/// A tag the Library proposes a value for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Field {
    Album,
    AlbumArtist,
    Year,
    Disc,
    Track,
    Title,
    Artist,
    /// The album's front cover (the proposal is a file the Library saved,
    /// in its own folder).
    Artwork,
}

impl Field {
    pub const ALL: [Field; 8] = [
        Field::Album,
        Field::AlbumArtist,
        Field::Year,
        Field::Disc,
        Field::Track,
        Field::Title,
        Field::Artist,
        Field::Artwork,
    ];
    /// As stored in the database.
    pub fn key(self) -> &'static str {
        match self {
            Field::Album => "album",
            Field::AlbumArtist => "album_artist",
            Field::Year => "year",
            Field::Disc => "disc",
            Field::Track => "track",
            Field::Title => "title",
            Field::Artist => "artist",
            Field::Artwork => "artwork",
        }
    }
    pub fn from_key(s: &str) -> Option<Field> {
        Field::ALL.into_iter().find(|f| f.key() == s)
    }
    /// For the panel.
    pub fn label(self) -> &'static str {
        match self {
            Field::Album => "Album",
            Field::AlbumArtist => "Album artist",
            Field::Year => "Year",
            Field::Disc => "Disc",
            Field::Track => "Track #",
            Field::Title => "Title",
            Field::Artist => "Artist",
            Field::Artwork => "Artwork",
        }
    }
    /// The same for every track of an album.
    pub fn album_wide(self) -> bool {
        matches!(
            self,
            Field::Album | Field::AlbumArtist | Field::Year | Field::Artwork
        )
    }
}

/// A proposed change: for one track (`rel_path`), or the whole album
/// (artwork).
#[derive(Clone, Debug, PartialEq)]
pub struct Proposal {
    pub rel_path: Option<String>,
    pub field: Field,
    pub current: Option<String>,
    pub proposed: String,
}

/// What a track has now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Have {
    pub rel_path: String,
    pub file_name: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<i64>,
    pub track_no: Option<i64>,
    pub disc_no: Option<i64>,
    pub duration_ms: i64,
}

/// For comparing titles: lower case, curly quotes straightened, spaces
/// squeezed.
fn norm(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '`' => '\'',
            '\u{201c}' | '\u{201d}' => '"',
            '\u{2010}'..='\u{2015}' => '-',
            c => c,
        })
        .collect::<String>()
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Pairs the album's tracks (`have`, in album order) with the release's.
/// The same number of tracks pair in order; otherwise by disc and track
/// number, then by title. Tracks that can't be paired are left out.
pub fn pair(have: &[Have], r: &Release) -> Vec<(usize, usize)> {
    if have.len() == r.tracks.len() {
        return (0..have.len()).map(|i| (i, i)).collect();
    }
    let mut used = vec![false; r.tracks.len()];
    let mut out = Vec::new();
    let mut left = Vec::new();
    for (i, h) in have.iter().enumerate() {
        let hit = h.track_no.and_then(|n| {
            let d = h.disc_no.unwrap_or(1);
            r.tracks
                .iter()
                .position(|t| t.number == n && t.disc == d)
                .filter(|&j| !used[j])
        });
        match hit {
            Some(j) => {
                used[j] = true;
                out.push((i, j));
            }
            None => left.push(i),
        }
    }
    for i in left {
        let Some(title) = have[i].title.as_deref().map(norm) else {
            continue;
        };
        if let Some(j) =
            (0..r.tracks.len()).find(|&j| !used[j] && norm(&r.tracks[j].title) == title)
        {
            used[j] = true;
            out.push((i, j));
        }
    }
    out.sort();
    out
}

/// The changes that would make the album's tags match the release: only
/// where they differ, and only for the tracks that pair up.
pub fn propose(have: &[Have], r: &Release) -> Vec<Proposal> {
    let mut out = Vec::new();
    for (i, j) in pair(have, r) {
        let h = &have[i];
        let t = &r.tracks[j];
        let mut add = |field: Field, current: Option<String>, proposed: String| {
            if proposed.trim().is_empty() || current.as_deref() == Some(proposed.as_str()) {
                return;
            }
            out.push(Proposal {
                rel_path: Some(h.rel_path.clone()),
                field,
                current,
                proposed,
            });
        };
        add(Field::Album, h.album.clone(), r.title.clone());
        add(Field::AlbumArtist, h.album_artist.clone(), r.artist.clone());
        if let Some(y) = r.year {
            add(Field::Year, h.year.map(|v| v.to_string()), y.to_string());
        }
        if r.discs > 1 || h.disc_no.is_some_and(|d| d != t.disc) {
            add(
                Field::Disc,
                h.disc_no.map(|v| v.to_string()),
                t.disc.to_string(),
            );
        }
        add(
            Field::Track,
            h.track_no.map(|v| v.to_string()),
            t.number.to_string(),
        );
        add(Field::Title, h.title.clone(), t.title.clone());
        add(Field::Artist, h.artist.clone(), t.artist.clone());
    }
    out
}

// ---------------------------------------------------------------- the client

/// Talks to MusicBrainz and the Cover Art Archive, one request a second.
/// Make one only while Lookups is switched on.
pub struct Client {
    agent: ureq::Agent,
    last: Option<Instant>,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    pub fn new() -> Client {
        Client {
            agent: ureq::AgentBuilder::new()
                .user_agent(USER_AGENT)
                .timeout_connect(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                .build(),
            last: None,
        }
    }

    /// A GET, after waiting for the one-second gap. A 503 (MusicBrainz is
    /// busy, or we were too quick) is tried once more after two seconds.
    /// `Ok(None)` for a 404.
    fn get(&mut self, url: &str, limit: u64, accept: &str) -> Result<Option<Vec<u8>>, String> {
        for attempt in 0..2 {
            if let Some(last) = self.last {
                let wait = GAP.saturating_sub(last.elapsed());
                std::thread::sleep(wait);
            }
            self.last = Some(Instant::now());
            match self.agent.get(url).set("Accept", accept).call() {
                Ok(resp) => {
                    let mut buf = Vec::new();
                    resp.into_reader()
                        .take(limit)
                        .read_to_end(&mut buf)
                        .map_err(|e| format!("The reply was cut short: {e}"))?;
                    return Ok(Some(buf));
                }
                Err(ureq::Error::Status(404, _)) => return Ok(None),
                Err(ureq::Error::Status(503, _)) if attempt == 0 => {
                    std::thread::sleep(Duration::from_secs(2));
                }
                Err(ureq::Error::Status(code, _)) => {
                    return Err(format!("The server answered {code}."))
                }
                Err(e) => return Err(format!("Couldn't reach the server: {e}")),
            }
        }
        Err("MusicBrainz is busy. Try again in a minute.".into())
    }

    fn json(&mut self, url: &str) -> Result<Option<String>, String> {
        self.get(url, MAX_JSON, "application/json")?
            .map(|b| String::from_utf8(b).map_err(|_| "The reply wasn't text.".to_string()))
            .transpose()
    }

    /// Releases that might be the album, best fit first.
    pub fn search(&mut self, q: &Query) -> Result<Vec<Candidate>, String> {
        match self.json(&search_url(q))? {
            Some(j) => Ok(rank(parse_search(&j)?, q)),
            None => Ok(Vec::new()),
        }
    }

    pub fn release(&mut self, mbid: &str) -> Result<Release, String> {
        match self.json(&release_url(mbid))? {
            Some(j) => parse_release(&j),
            None => Err("MusicBrainz doesn't have that release any more.".into()),
        }
    }

    /// The release's front cover, if the Cover Art Archive has one.
    pub fn front_art(&mut self, mbid: &str) -> Result<Option<Vec<u8>>, String> {
        self.get(&front_art_url(mbid), MAX_ART, "image/*")
    }
}

// ---------------------------------------------------------------- a lookup

/// One album to look up.
#[derive(Clone, Debug)]
pub struct Job {
    pub album: crate::db::AlbumRef,
    pub query: Query,
    /// Its tracks, in album order.
    pub have: Vec<Have>,
    /// Its artwork now ("600 × 600"), or `None` when it has none.
    pub art_now: Option<String>,
    /// Fetch the front cover: the album has no artwork, or it's small.
    pub want_art: bool,
    /// The release the user picked, with the candidates it was picked from
    /// (so the search isn't repeated).
    pub chosen: Option<(String, Vec<Candidate>)>,
}

/// Looks one album up: searches (unless the user chose a release), takes
/// the best release if it fits for certain, proposes the changes, and
/// fetches the front cover into `art_dir` (the Library's own folder, never
/// the album's) when it's wanted. Never fails: what went wrong is in the
/// row's note.
pub fn run(
    c: &mut Client,
    art_dir: &std::path::Path,
    job: &Job,
    now: i64,
) -> (crate::db::LookupRow, Vec<Proposal>) {
    use crate::db::{LookupRow, LookupState};
    let row = |state, mbid: Option<String>, candidates: Vec<Candidate>, note: String| LookupRow {
        state,
        mbid,
        candidates,
        note,
        looked: now,
    };
    let (mbid, candidates) = match &job.chosen {
        Some((m, cands)) => (Some(m.clone()), cands.clone()),
        None => match c.search(&job.query) {
            Ok(cands) => {
                let sure = cands
                    .first()
                    .filter(|b| confident(b, &job.query))
                    .map(|b| b.mbid.clone());
                (sure, cands)
            }
            Err(e) => return (row(LookupState::Failed, None, vec![], e), vec![]),
        },
    };
    if candidates.is_empty() && mbid.is_none() {
        return (
            row(LookupState::NotFound, None, vec![], String::new()),
            vec![],
        );
    }
    let Some(mbid) = mbid else {
        return (
            row(LookupState::Unsure, None, candidates, String::new()),
            vec![],
        );
    };
    let release = match c.release(&mbid) {
        Ok(r) => r,
        Err(e) => return (row(LookupState::Failed, Some(mbid), candidates, e), vec![]),
    };
    let mut proposals = propose(&job.have, &release);
    let mut note = String::new();
    if job.want_art && release.front_art {
        match c.front_art(&mbid) {
            Ok(Some(bytes)) => match save_art(art_dir, &mbid, &bytes) {
                Ok(path) => proposals.push(Proposal {
                    rel_path: None,
                    field: Field::Artwork,
                    current: job.art_now.clone(),
                    proposed: path.display().to_string(),
                }),
                Err(e) => note = format!("The cover couldn't be saved: {e}"),
            },
            Ok(None) => note = "The Cover Art Archive has no front cover for it.".into(),
            Err(e) => note = format!("The cover couldn't be fetched: {e}"),
        }
    }
    (
        row(LookupState::Matched, Some(mbid), candidates, note),
        proposals,
    )
}

/// Where a release's front cover is kept: `<art_dir>/<mbid>-front.jpg` (or
/// `.png`).
fn save_art(
    art_dir: &std::path::Path,
    mbid: &str,
    bytes: &[u8],
) -> std::io::Result<std::path::PathBuf> {
    let ext = if bytes.starts_with(b"\x89PNG") {
        "png"
    } else {
        "jpg"
    };
    let safe: String = mbid
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    std::fs::create_dir_all(art_dir)?;
    let path = art_dir.join(format!("{safe}-front.{ext}"));
    std::fs::write(&path, bytes)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEARCH: &str = r#"{
      "created": "2026-11-01T12:00:00.000Z", "count": 3, "offset": 0,
      "releases": [
        {"id": "aaaa", "score": 100, "title": "OK Computer", "status": "Official",
         "artist-credit": [{"name": "Radiohead", "artist": {"id": "a74b", "name": "Radiohead"}}],
         "release-group": {"id": "rg1", "primary-type": "Album"},
         "date": "2009-08-31", "country": "GB", "track-count": 24,
         "media": [{"format": "CD", "track-count": 12}, {"format": "CD", "track-count": 12}]},
        {"id": "bbbb", "score": 100, "title": "OK Computer",
         "artist-credit": [{"name": "Radiohead"}],
         "date": "1997-05-21", "country": "GB", "track-count": 12,
         "media": [{"format": "CD", "track-count": 12}]},
        {"id": "cccc", "score": 72, "title": "OK Computer OKNOTOK 1997 2017",
         "artist-credit": [{"name": "Radiohead"}],
         "date": "2017", "track-count": 23, "media": []}
      ]
    }"#;

    const RELEASE: &str = r#"{
      "id": "bbbb", "title": "OK Computer", "date": "1997-05-21", "country": "GB",
      "artist-credit": [{"name": "Radiohead", "joinphrase": ""}],
      "cover-art-archive": {"artwork": true, "front": true, "back": false, "count": 1},
      "media": [{"position": 1, "format": "CD", "track-count": 3, "tracks": [
        {"id": "t1", "number": "1", "position": 1, "title": "Airbag", "length": 284000,
         "artist-credit": [{"name": "Radiohead"}], "recording": {"id": "r1", "title": "Airbag"}},
        {"id": "t2", "number": "2", "position": 2, "title": "Paranoid Android", "length": 383000},
        {"id": "t3", "number": "3", "position": 3, "title": "Subterranean Homesick Alien",
         "length": null, "artist-credit": [{"name": "Radiohead", "joinphrase": " feat. "}, {"name": "Nobody"}]}
      ]}]
    }"#;

    fn q(tracks: usize) -> Query {
        Query {
            artist: "Radiohead".into(),
            album: "OK Computer".into(),
            tracks,
            year: Some(1997),
        }
    }

    #[test]
    fn search_urls() {
        let u = search_url(&q(12));
        assert!(u.starts_with("https://musicbrainz.org/ws/2/release/?query="));
        assert!(u.contains("release%3A%22OK%20Computer%22%20AND%20artist%3A%22Radiohead%22"));
        assert!(u.ends_with("&fmt=json&limit=10"));
        // Quotes inside a title are escaped; placeholder artists aren't sent.
        let v = search_url(&Query {
            artist: "Various Artists".into(),
            album: "Say \"Hi\"".into(),
            tracks: 3,
            year: None,
        });
        assert!(v.contains("release%3A%22Say%20%5C%22Hi%5C%22%22&"));
        assert!(!v.contains("artist"));
        assert_eq!(
            release_url("bbbb"),
            "https://musicbrainz.org/ws/2/release/bbbb?inc=recordings+artist-credits&fmt=json"
        );
        assert_eq!(
            front_art_url("bbbb"),
            "https://coverartarchive.org/release/bbbb/front-500"
        );
    }

    #[test]
    fn searches_read_and_rank() {
        let c = parse_search(SEARCH).unwrap();
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].artist, "Radiohead");
        assert_eq!(c[0].format.as_deref(), Some("2×CD"));
        assert_eq!(c[1].year, Some(1997));
        assert_eq!(c[2].format, None);
        let r = rank(c, &q(12));
        assert_eq!(r[0].mbid, "bbbb"); // the 12-track release from 1997
        assert!(confident(&r[0], &q(12)));
        assert!(!confident(&r[1], &q(12)));
        assert!(parse_search("{}").unwrap().is_empty());
        assert!(parse_search("not json").is_err());
    }

    #[test]
    fn releases_read() {
        let r = parse_release(RELEASE).unwrap();
        assert_eq!(r.title, "OK Computer");
        assert_eq!(r.year, Some(1997));
        assert_eq!((r.discs, r.tracks.len()), (1, 3));
        assert!(r.front_art);
        assert_eq!(r.tracks[1].artist, "Radiohead"); // no credit: the release's
        assert_eq!(r.tracks[2].artist, "Radiohead feat. Nobody");
        assert_eq!(r.tracks[2].length_ms, None);
    }

    fn have(n: Option<i64>, title: Option<&str>) -> Have {
        Have {
            rel_path: format!("Music/OKC/{}.mp3", title.unwrap_or("x")),
            title: title.map(str::to_string),
            artist: Some("Radiohead".into()),
            album: Some("OK Computer".into()),
            year: Some(1997),
            track_no: n,
            ..Default::default()
        }
    }

    #[test]
    fn proposals_only_where_tags_differ() {
        let r = parse_release(RELEASE).unwrap();
        let h = vec![
            have(Some(1), Some("Airbag")),
            have(Some(2), Some("paranoid android")),
            have(None, Some("Subterranean Homesick Alien")),
        ];
        let p = propose(&h, &r);
        let fields: Vec<(Field, &str)> = p.iter().map(|p| (p.field, p.proposed.as_str())).collect();
        assert_eq!(
            fields,
            vec![
                (Field::AlbumArtist, "Radiohead"),
                (Field::AlbumArtist, "Radiohead"),
                (Field::Title, "Paranoid Android"),
                (Field::AlbumArtist, "Radiohead"),
                (Field::Track, "3"),
                (Field::Artist, "Radiohead feat. Nobody"),
            ]
        );
        assert_eq!(p[2].current.as_deref(), Some("paranoid android"));
        assert_eq!(p[4].current, None);
    }

    #[test]
    fn tracks_pair_by_number_then_title() {
        let r = parse_release(RELEASE).unwrap();
        // Two of three, out of order: by number, then by title (curly quotes
        // and case don't matter).
        let h = vec![
            have(None, Some("SUBTERRANEAN  homesick alien")),
            have(Some(1), Some("Airbag")),
        ];
        assert_eq!(pair(&h, &r), vec![(0, 2), (1, 0)]);
        let h = vec![have(None, Some("Something else")), have(Some(9), None)];
        assert!(pair(&h, &r).is_empty());
        assert!(propose(&h, &r).is_empty());
    }

    #[test]
    fn covers_are_kept_in_the_librarys_folder() {
        let d = tempfile::tempdir().unwrap();
        let jpg = save_art(d.path(), "bb/bb", &[0xff, 0xd8, 0xff]).unwrap();
        assert_eq!(jpg, d.path().join("bbbb-front.jpg"));
        let png = save_art(d.path(), "cccc", b"\x89PNG\r\n").unwrap();
        assert_eq!(png.extension().unwrap(), "png");
    }

    #[test]
    fn fields_round_trip() {
        for f in Field::ALL {
            assert_eq!(Field::from_key(f.key()), Some(f));
        }
        assert_eq!(Field::from_key("genre"), None);
        assert!(USER_AGENT.starts_with("TAW-Library/"));
    }
}
