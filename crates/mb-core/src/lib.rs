//! Music Box's library engine: drives, scanning, tags and the database.
//! Nothing here touches the UI, and nothing here writes to music files.

pub mod db;
pub mod excludes;
pub mod library;
pub mod scan;
pub mod tags;
pub mod volumes;

/// File extensions Music Box treats as music (lower case).
pub const AUDIO_EXTS: &[&str] = &[
    "mp3", "flac", "wav", "m4a", "aif", "aiff", "ogg", "opus", "ape", "wv", "mpc", "aac",
];

/// Folder artwork file names, without extension (lower case).
pub const FOLDER_ART_NAMES: &[&str] = &["cover", "folder", "front", "album", "albumart"];
pub const IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png"];

/// Artwork smaller than this on its longer side counts as too small.
pub const SMALL_ART_PX: u32 = 500;

pub fn is_audio(ext: &str) -> bool {
    AUDIO_EXTS.iter().any(|e| e.eq_ignore_ascii_case(ext))
}

/// Seconds since 1970.
pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
