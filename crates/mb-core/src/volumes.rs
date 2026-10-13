//! Finding drives. Each one gets an id that stays the same when its drive
//! letter or mount point changes: the filesystem UUID on Linux, the volume
//! serial number on Windows, and the share for network drives.

use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Fixed,
    Removable,
    Network,
    /// A folder added by hand that isn't on a drive the Library can see.
    Folder,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Fixed => "fixed",
            Kind::Removable => "removable",
            Kind::Network => "network",
            Kind::Folder => "folder",
        }
    }
    pub fn parse(s: &str) -> Kind {
        match s {
            "removable" => Kind::Removable,
            "network" => Kind::Network,
            "folder" => Kind::Folder,
            _ => Kind::Fixed,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Volume {
    pub id: String,
    pub label: String,
    pub kind: Kind,
    pub mount: PathBuf,
    /// The drive the operating system runs from.
    pub system: bool,
}

/// The drive a path is on, from a list of mounted volumes: the one with the
/// longest mount point that contains it.
pub fn containing<'a>(vols: &'a [Volume], path: &Path) -> Option<&'a Volume> {
    vols.iter()
        .filter(|v| path.starts_with(&v.mount))
        .max_by_key(|v| v.mount.as_os_str().len())
}

/// A folder's path relative to its volume, with '/' separators.
pub fn rel_path(mount: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(mount).ok()?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    Some(parts.join("/"))
}

/// A relative path back onto a mount point.
pub fn join(mount: &Path, rel: &str) -> PathBuf {
    let mut p = mount.to_path_buf();
    for part in rel.split('/').filter(|s| !s.is_empty()) {
        p.push(part);
    }
    p
}

/// A volume for a folder that isn't on any drive the Library can see, such as
/// a Windows network share that has no drive letter.
pub fn folder_volume(path: &Path) -> Volume {
    let s = path.to_string_lossy();
    let network = s.starts_with("\\\\") || s.starts_with("//");
    Volume {
        id: format!(
            "{}:{}",
            if network { "net" } else { "folder" },
            s.to_lowercase()
        ),
        label: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| s.to_string()),
        kind: if network { Kind::Network } else { Kind::Folder },
        mount: path.to_path_buf(),
        system: false,
    }
}

// ---------------------------------------------------------------- Linux

#[cfg(not(windows))]
pub fn list() -> Vec<Volume> {
    let text = std::fs::read_to_string("/proc/self/mountinfo").unwrap_or_default();
    parse_mountinfo(&text, &|dev| linux::uuid_for(dev), &|dev| {
        linux::label_for(dev)
    })
}

#[cfg(not(windows))]
mod linux {
    use std::path::Path;

    fn find_link(dir: &str, dev: &str) -> Option<String> {
        let target = std::fs::canonicalize(dev).ok()?;
        std::fs::read_dir(dir).ok()?.flatten().find_map(|e| {
            (std::fs::canonicalize(e.path()).ok()? == target)
                .then(|| unescape(&e.file_name().to_string_lossy()))
        })
    }

    /// udev escapes spaces and slashes in /dev/disk/by-label as \x20 etc.
    fn unescape(s: &str) -> String {
        let mut out = String::new();
        let mut rest = s;
        while let Some(i) = rest.find("\\x") {
            out.push_str(&rest[..i]);
            match u8::from_str_radix(rest.get(i + 2..i + 4).unwrap_or(""), 16) {
                Ok(b) => {
                    out.push(b as char);
                    rest = &rest[i + 4..];
                }
                Err(_) => {
                    out.push_str("\\x");
                    rest = &rest[i + 2..];
                }
            }
        }
        out.push_str(rest);
        out
    }

    pub fn uuid_for(dev: &str) -> Option<String> {
        if !Path::new(dev).is_absolute() {
            return None;
        }
        find_link("/dev/disk/by-uuid", dev)
    }

    pub fn label_for(dev: &str) -> Option<String> {
        if !Path::new(dev).is_absolute() {
            return None;
        }
        find_link("/dev/disk/by-label", dev)
    }
}

/// mountinfo escapes spaces and other characters as octal (\040).
fn unescape_octal(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\'
            && i + 3 < b.len()
            && b[i + 1..i + 4].iter().all(|c| (b'0'..=b'7').contains(c))
        {
            let v = (b[i + 1] - b'0') * 64 + (b[i + 2] - b'0') * 8 + (b[i + 3] - b'0');
            out.push(v);
            i += 4;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

const LOCAL_FS: &[&str] = &[
    "ext2", "ext3", "ext4", "btrfs", "xfs", "f2fs", "zfs", "jfs", "reiserfs", "ntfs", "ntfs3",
    "vfat", "exfat", "fuseblk", "hfsplus", "apfs", "bcachefs",
];
const NETWORK_FS: &[&str] = &["cifs", "smb3", "smbfs", "nfs", "nfs4", "fuse.sshfs", "9p"];
const SKIP_UNDER: &[&str] = &[
    "/proc",
    "/sys",
    "/dev",
    "/run",
    "/boot",
    "/snap",
    "/var/lib",
    "/var/snap",
    "/tmp",
    "/efi",
    "/opt",
    "/usr",
    "/nix",
];

/// Parses /proc/self/mountinfo into volumes. The lookups turn a device into
/// its filesystem UUID and label; they're passed in so tests can fake them.
pub fn parse_mountinfo(
    text: &str,
    uuid_for: &dyn Fn(&str) -> Option<String>,
    label_for: &dyn Fn(&str) -> Option<String>,
) -> Vec<Volume> {
    let mut out: Vec<Volume> = Vec::new();
    for line in text.lines() {
        let Some((left, right)) = line.split_once(" - ") else {
            continue;
        };
        let l: Vec<&str> = left.split(' ').collect();
        let r: Vec<&str> = right.split(' ').collect();
        if l.len() < 5 || r.len() < 2 {
            continue;
        }
        let (root, mount) = (unescape_octal(l[3]), unescape_octal(l[4]));
        let (fstype, source) = (r[0], unescape_octal(r[1]));
        let network = NETWORK_FS.contains(&fstype);
        if !network && !LOCAL_FS.contains(&fstype) {
            continue;
        }
        // Bind mounts show a sub-folder of a filesystem a second time.
        if root != "/" {
            continue;
        }
        let under_run_media = mount.starts_with("/run/media/");
        if !under_run_media
            && SKIP_UNDER
                .iter()
                .any(|s| mount == *s || mount.starts_with(&format!("{s}/")))
        {
            continue;
        }
        let id = if network {
            format!("net:{source}")
        } else if let Some(u) = uuid_for(&source) {
            format!("uuid:{u}")
        } else {
            format!("dev:{source}")
        };
        if out.iter().any(|v| v.id == id) {
            continue;
        }
        let system = mount == "/";
        let label = label_for(&source).unwrap_or_else(|| {
            if system {
                "System".to_string()
            } else if network {
                source.clone()
            } else {
                Path::new(&mount)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| mount.clone())
            }
        });
        let removable = mount.starts_with("/media/") || under_run_media || removable_dev(&source);
        out.push(Volume {
            id,
            label,
            kind: if network {
                Kind::Network
            } else if removable {
                Kind::Removable
            } else {
                Kind::Fixed
            },
            mount: PathBuf::from(mount),
            system,
        });
    }
    out
}

/// Whether the kernel marks a block device's disk as removable.
fn removable_dev(dev: &str) -> bool {
    let Some(name) = dev.strip_prefix("/dev/") else {
        return false;
    };
    // sdb1 -> sdb, nvme0n1p2 -> nvme0n1, mmcblk0p1 -> mmcblk0
    let disk = if name.starts_with("nvme") || name.starts_with("mmcblk") {
        name.rsplit_once('p').map_or(name, |(d, _)| d)
    } else {
        name.trim_end_matches(|c: char| c.is_ascii_digit())
    };
    std::fs::read_to_string(format!("/sys/block/{disk}/removable")).is_ok_and(|s| s.trim() == "1")
}

// ---------------------------------------------------------------- Windows

#[cfg(windows)]
pub fn list() -> Vec<Volume> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetDriveTypeW, GetLogicalDriveStringsW, GetVolumeInformationW,
    };
    const DRIVE_REMOVABLE: u32 = 2;
    const DRIVE_FIXED: u32 = 3;
    const DRIVE_REMOTE: u32 = 4;

    let mut buf = vec![0u16; 1024];
    let n = unsafe { GetLogicalDriveStringsW(buf.len() as u32, buf.as_mut_ptr()) } as usize;
    if n == 0 || n > buf.len() {
        return Vec::new();
    }
    let system_drive = std::env::var("SystemDrive")
        .unwrap_or_else(|_| "C:".into())
        .to_uppercase();
    let mut out = Vec::new();
    for root in buf[..n].split(|&c| c == 0).filter(|s| !s.is_empty()) {
        let mut root_z = root.to_vec();
        root_z.push(0);
        let kind = match unsafe { GetDriveTypeW(root_z.as_ptr()) } {
            DRIVE_FIXED => Kind::Fixed,
            DRIVE_REMOVABLE => Kind::Removable,
            DRIVE_REMOTE => Kind::Network,
            _ => continue, // CD drives, RAM disks, unknown
        };
        let mut label = vec![0u16; 261];
        let mut serial: u32 = 0;
        let ok = unsafe {
            GetVolumeInformationW(
                root_z.as_ptr(),
                label.as_mut_ptr(),
                label.len() as u32,
                &mut serial,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                0,
            )
        };
        if ok == 0 {
            continue; // an empty card reader, or a drive that isn't ready
        }
        let root_s = String::from_utf16_lossy(root);
        let letter = root_s.trim_end_matches('\\').to_uppercase();
        let label_s =
            String::from_utf16_lossy(&label[..label.iter().position(|&c| c == 0).unwrap_or(0)]);
        out.push(Volume {
            id: format!("win:{serial:08X}"),
            label: if label_s.is_empty() {
                match kind {
                    Kind::Removable => "Removable".into(),
                    Kind::Network => "Network".into(),
                    _ => "Local Disk".into(),
                }
            } else {
                label_s
            },
            kind,
            mount: PathBuf::from(&root_s),
            system: letter == system_drive,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const MOUNTINFO: &str = "\
22 1 259:2 / / rw,relatime shared:1 - ext4 /dev/nvme0n1p2 rw
23 22 0:5 / /proc rw shared:2 - proc proc rw
30 22 259:1 / /boot/efi rw shared:3 - vfat /dev/nvme0n1p1 rw
41 22 8:17 / /mnt/data rw shared:4 - ext4 /dev/sdb1 rw
42 22 8:17 /music /srv/music rw shared:5 - ext4 /dev/sdb1 rw
43 22 0:50 / /mnt/nas rw shared:6 - cifs //nas.local/music rw
44 22 8:33 / /run/media/me/OLD\\040IPOD rw shared:7 - vfat /dev/sdc1 rw
45 22 0:40 / /run/user/1000 rw shared:8 - tmpfs tmpfs rw
";

    #[test]
    fn mountinfo() {
        let uuid = |d: &str| match d {
            "/dev/nvme0n1p2" => Some("aaaa-1111".to_string()),
            "/dev/sdb1" => Some("bbbb-2222".to_string()),
            _ => None,
        };
        let label = |d: &str| (d == "/dev/sdb1").then(|| "Data".to_string());
        let v = parse_mountinfo(MOUNTINFO, &uuid, &label);
        let ids: Vec<&str> = v.iter().map(|v| v.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "uuid:aaaa-1111",
                "uuid:bbbb-2222",
                "net://nas.local/music",
                "dev:/dev/sdc1"
            ]
        );
        assert!(v[0].system);
        assert_eq!(v[0].label, "System");
        assert_eq!(v[1].label, "Data");
        assert_eq!(v[2].kind, Kind::Network);
        assert_eq!(v[3].kind, Kind::Removable);
        assert_eq!(v[3].label, "OLD IPOD");
        assert_eq!(v[3].mount, PathBuf::from("/run/media/me/OLD IPOD"));
    }

    #[test]
    fn paths() {
        let vols = parse_mountinfo(MOUNTINFO, &|_| None, &|_| None);
        let v = containing(&vols, Path::new("/mnt/data/Music/Radiohead")).unwrap();
        assert_eq!(v.mount, PathBuf::from("/mnt/data"));
        assert_eq!(
            rel_path(&v.mount, Path::new("/mnt/data/Music/Radiohead")).unwrap(),
            "Music/Radiohead"
        );
        assert_eq!(
            join(&v.mount, "Music/Radiohead"),
            PathBuf::from("/mnt/data/Music/Radiohead")
        );
        assert_eq!(rel_path(&v.mount, &v.mount).unwrap(), "");
        let home = containing(&vols, Path::new("/home/me")).unwrap();
        assert!(home.system);
    }
}
