//! What the scanner leaves out: system folders (always), DAW project
//! folders (always), and the rules you can change, such as sample packs.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleKind {
    /// One folder, by its full path.
    Path,
    /// Any folder with this name, anywhere.
    Name,
}

impl RuleKind {
    pub fn as_str(self) -> &'static str {
        match self {
            RuleKind::Path => "path",
            RuleKind::Name => "name",
        }
    }
    pub fn parse(s: &str) -> RuleKind {
        if s == "path" {
            RuleKind::Path
        } else {
            RuleKind::Name
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rule {
    pub id: i64,
    pub kind: RuleKind,
    pub pattern: String,
    pub note: String,
    /// Folders this rule skipped in the last scan.
    pub hits: i64,
}

/// The rules a new library starts with. They can be removed.
pub const DEFAULT_RULES: &[(RuleKind, &str, &str)] = &[
    (RuleKind::Name, "Samples", "Sample packs"),
    (RuleKind::Name, "Sample Packs", "Sample packs"),
    (RuleKind::Name, "Splice", "Splice downloads"),
    (RuleKind::Name, "Podcasts", "Podcasts"),
];

/// A file with one of these extensions marks its folder as a DAW project.
/// Its audio is stems and bounces, not your music library.
pub const DAW_MARKERS: &[&str] = &[
    "als",
    "flp",
    "ptx",
    "ptf",
    "cpr",
    "npr",
    "rpp",
    "bwproject",
    "song",
    "reason",
    "aup3",
    "aup",
    "logicx",
    "band",
    "dawproject",
    "ardour",
    "mmpz",
];

pub fn is_daw_marker(name: &str) -> bool {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| DAW_MARKERS.iter().any(|m| m.eq_ignore_ascii_case(e)))
}

/// Folders inside any drive that never hold a music library.
const SYSTEM_NAMES: &[&str] = &[
    "$recycle.bin",
    "system volume information",
    "appdata",
    "node_modules",
    "steamapps",
    "windowsapps",
    "lost+found",
];

/// Folders at the top of a drive that hold the operating system.
const SYSTEM_ROOT_NAMES: &[&str] = &[
    "windows",
    "program files",
    "program files (x86)",
    "programdata",
    "recovery",
    "usr",
    "bin",
    "sbin",
    "lib",
    "lib32",
    "lib64",
    "libx32",
    "etc",
    "var",
    "proc",
    "sys",
    "dev",
    "run",
    "boot",
    "snap",
    "opt",
    "tmp",
];

/// Why a folder was skipped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Skip {
    System,
    Hidden,
    /// Index into the rules list.
    Rule(usize),
}

#[derive(Clone, Debug, Default)]
pub struct Excludes {
    pub rules: Vec<Rule>,
    names: Vec<String>,
    paths: Vec<PathBuf>,
}

impl Excludes {
    pub fn new(rules: Vec<Rule>) -> Self {
        let names = rules.iter().map(|r| r.pattern.to_lowercase()).collect();
        let paths = rules.iter().map(|r| PathBuf::from(&r.pattern)).collect();
        Excludes {
            rules,
            names,
            paths,
        }
    }

    /// Whether to skip a folder. `at_root` is true for folders directly at
    /// the top of a drive.
    pub fn skip_dir(&self, path: &Path, name: &str, at_root: bool) -> Option<Skip> {
        let lower = name.to_lowercase();
        if name.starts_with('.') {
            return Some(Skip::Hidden);
        }
        if SYSTEM_NAMES.contains(&lower.as_str())
            || (at_root && SYSTEM_ROOT_NAMES.contains(&lower.as_str()))
        {
            return Some(Skip::System);
        }
        for (i, r) in self.rules.iter().enumerate() {
            let hit = match r.kind {
                RuleKind::Name => self.names[i] == lower,
                RuleKind::Path => same_path(path, &self.paths[i]),
            };
            if hit {
                return Some(Skip::Rule(i));
            }
        }
        None
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.to_string_lossy().to_lowercase().trim_end_matches('\\')
            == b.to_string_lossy().to_lowercase().trim_end_matches('\\')
    } else {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(kind: RuleKind, p: &str) -> Rule {
        Rule {
            id: 0,
            kind,
            pattern: p.into(),
            note: String::new(),
            hits: 0,
        }
    }

    #[test]
    fn skips() {
        let ex = Excludes::new(vec![
            rule(RuleKind::Name, "Samples"),
            rule(RuleKind::Path, "/mnt/data/Stems"),
        ]);
        let p = Path::new("/x");
        assert_eq!(ex.skip_dir(p, "samples", false), Some(Skip::Rule(0)));
        assert_eq!(
            ex.skip_dir(Path::new("/mnt/data/Stems"), "Stems", false),
            Some(Skip::Rule(1))
        );
        assert_eq!(ex.skip_dir(p, ".cache", false), Some(Skip::Hidden));
        assert_eq!(ex.skip_dir(p, "AppData", false), Some(Skip::System));
        assert_eq!(ex.skip_dir(p, "Windows", true), Some(Skip::System));
        assert_eq!(ex.skip_dir(p, "usr", true), Some(Skip::System));
        // Only at the top of a drive.
        assert_eq!(ex.skip_dir(p, "usr", false), None);
        assert_eq!(ex.skip_dir(p, "Radiohead", false), None);
        assert!(is_daw_marker("Song 3.als"));
        assert!(is_daw_marker("mix.RPP"));
        assert!(!is_daw_marker("01 Airbag.flac"));
    }
}
