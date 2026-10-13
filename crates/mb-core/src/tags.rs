//! Reading what's in a music file. Read-only: nothing here writes tags.

use lofty::config::ParseOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::PictureType;
use lofty::probe::Probe;
use lofty::tag::{Accessor, ItemKey, Tag};
use std::path::Path;

/// The fields the Library cares about, plus the file's audio properties.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TrackInfo {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub year: Option<u32>,
    pub track_no: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_no: Option<u32>,
    pub disc_total: Option<u32>,
    pub genre: Option<String>,
    /// True when the file has a tag with at least a title, artist or album.
    pub has_tags: bool,
    pub duration_ms: u64,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub bitrate_kbps: Option<u32>,
    pub channels: Option<u8>,
    /// Size of the embedded front cover (or first picture), if any.
    pub art: Option<(u32, u32)>,
}

fn clean(s: Option<std::borrow::Cow<'_, str>>) -> Option<String> {
    let s = s?.trim().trim_matches('\0').trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn get(tag: &Tag, key: &ItemKey) -> Option<String> {
    clean(tag.get_string(key).map(Into::into))
}

/// The first four-digit year in a date such as "1997", "1997-05-21" or
/// "21/05/1997".
pub fn parse_year(s: &str) -> Option<u32> {
    let b = s.as_bytes();
    (0..b.len().saturating_sub(3)).find_map(|i| {
        let w = &b[i..i + 4];
        let before = i == 0 || !b[i - 1].is_ascii_digit();
        let after = i + 4 == b.len() || !b[i + 4].is_ascii_digit();
        if before && after && w.iter().all(u8::is_ascii_digit) {
            let y: u32 = s[i..i + 4].parse().ok()?;
            (1000..=2999).contains(&y).then_some(y)
        } else {
            None
        }
    })
}

/// Reads a file's tags and properties. Pictures are measured, not kept.
pub fn read(path: &Path) -> Result<TrackInfo, String> {
    let file = Probe::open(path)
        .and_then(|p| p.options(ParseOptions::new().read_cover_art(true)).read())
        .map_err(|e| e.to_string())?;
    let props = file.properties();
    let mut info = TrackInfo {
        duration_ms: props.duration().as_millis() as u64,
        sample_rate: props.sample_rate(),
        bit_depth: props.bit_depth(),
        bitrate_kbps: props.audio_bitrate().or(props.overall_bitrate()),
        channels: props.channels(),
        ..Default::default()
    };
    // Prefer the format's main tag, then fill gaps from any other tag
    // (an MP3 can carry ID3v2 and ID3v1, a WAV both ID3 and RIFF INFO).
    let mut tags: Vec<&Tag> = file.primary_tag().into_iter().collect();
    for t in file.tags() {
        if !tags.iter().any(|x| std::ptr::eq(*x, t)) {
            tags.push(t);
        }
    }
    for t in tags {
        info.title = info.title.or_else(|| clean(t.title()));
        info.artist = info.artist.or_else(|| clean(t.artist()));
        info.album = info.album.or_else(|| clean(t.album()));
        info.album_artist = info.album_artist.or_else(|| get(t, &ItemKey::AlbumArtist));
        info.genre = info.genre.or_else(|| clean(t.genre()));
        info.year = info.year.or_else(|| {
            [
                ItemKey::Year,
                ItemKey::RecordingDate,
                ItemKey::OriginalReleaseDate,
                ItemKey::ReleaseDate,
            ]
            .iter()
            .find_map(|k| t.get_string(k).and_then(parse_year))
        });
        info.track_no = info.track_no.or(t.track()).filter(|&n| n > 0);
        info.track_total = info.track_total.or(t.track_total()).filter(|&n| n > 0);
        info.disc_no = info.disc_no.or(t.disk()).filter(|&n| n > 0);
        info.disc_total = info.disc_total.or(t.disk_total()).filter(|&n| n > 0);
        if info.art.is_none() {
            let pics = t.pictures();
            let pic = pics
                .iter()
                .find(|p| p.pic_type() == PictureType::CoverFront)
                .or(pics.first());
            if let Some(p) = pic {
                info.art = imagesize::blob_size(p.data())
                    .ok()
                    .map(|s| (s.width as u32, s.height as u32));
            }
        }
    }
    info.has_tags = info.title.is_some() || info.artist.is_some() || info.album.is_some();
    Ok(info)
}

/// The embedded front cover's bytes, for showing it.
pub fn read_art(path: &Path) -> Option<Vec<u8>> {
    let file = Probe::open(path)
        .and_then(|p| p.options(ParseOptions::new().read_cover_art(true)).read())
        .ok()?;
    let mut best: Option<Vec<u8>> = None;
    for t in file.tags() {
        let pics = t.pictures();
        if let Some(p) = pics
            .iter()
            .find(|p| p.pic_type() == PictureType::CoverFront)
            .or(pics.first())
        {
            if p.pic_type() == PictureType::CoverFront || best.is_none() {
                best = Some(p.data().to_vec());
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn years() {
        assert_eq!(parse_year("1997"), Some(1997));
        assert_eq!(parse_year("1997-05-21"), Some(1997));
        assert_eq!(parse_year("21/05/1997"), Some(1997));
        assert_eq!(parse_year("12345"), None);
        assert_eq!(parse_year(""), None);
        assert_eq!(parse_year("0000"), None);
    }
}
