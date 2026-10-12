//! Work that runs off the panel's thread: the scan, the drive watcher and
//! the artwork loader. The panel polls them each frame.

use egui::{ColorImage, Context};
use mb_core::db::Db;
use mb_core::excludes::Excludes;
use mb_core::scan::{self, Progress, Summary};
use mb_core::volumes::{self, Volume};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::Ordering::Relaxed;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

// ---------------------------------------------------------------- scan

pub struct ScanRun {
    pub progress: Arc<Progress>,
    handle: Option<JoinHandle<Result<Summary, String>>>,
}

impl ScanRun {
    /// Scans every drive that's there now, on a thread with its own
    /// database connection.
    pub fn start(db_path: PathBuf, mounted: Vec<Volume>, ctx: Context) -> ScanRun {
        let progress = Arc::new(Progress::default());
        progress.running.store(true, Relaxed);
        let p = progress.clone();
        let handle = std::thread::Builder::new()
            .name("scan".into())
            .spawn(move || {
                let result = (|| {
                    let db = Db::open(&db_path).map_err(|e| e.to_string())?;
                    db.sync_volumes(&mounted, mb_core::now_secs())
                        .map_err(|e| e.to_string())?;
                    let ex = Excludes::new(db.rules().map_err(|e| e.to_string())?);
                    let jobs = scan::plan(&db, &mounted).map_err(|e| e.to_string())?;
                    scan::run(&db, &jobs, &ex, &p, mb_core::now_secs()).map_err(|e| e.to_string())
                })();
                p.running.store(false, Relaxed);
                ctx.request_repaint();
                result
            })
            .ok();
        ScanRun { progress, handle }
    }

    pub fn running(&self) -> bool {
        self.progress.running.load(Relaxed)
    }

    pub fn stop(&self) {
        self.progress.cancel.store(true, Relaxed);
    }

    /// The result, once the scan has finished.
    pub fn finish(&mut self) -> Option<Result<Summary, String>> {
        if self.running() {
            return None;
        }
        let h = self.handle.take()?;
        Some(
            h.join()
                .unwrap_or_else(|_| Err("The scan stopped unexpectedly.".into())),
        )
    }
}

// ---------------------------------------------------------------- drives

/// Lists the mounted drives every few seconds, so a drive that's plugged
/// in or removed shows up without a restart. Network drives can be slow to
/// answer, which is why this isn't done on the panel's thread.
pub struct DriveWatch {
    latest: Arc<Mutex<Option<Vec<Volume>>>>,
}

impl DriveWatch {
    pub fn start(ctx: Context) -> DriveWatch {
        let latest = Arc::new(Mutex::new(None));
        let l = latest.clone();
        let _ = std::thread::Builder::new()
            .name("drives".into())
            .spawn(move || {
                let mut last: Option<Vec<Volume>> = None;
                loop {
                    let now = volumes::list();
                    if last.as_ref() != Some(&now) {
                        *l.lock().unwrap() = Some(now.clone());
                        last = Some(now);
                        ctx.request_repaint();
                    }
                    std::thread::sleep(Duration::from_secs(4));
                }
            });
        DriveWatch { latest }
    }

    /// The drive list, if it has changed since the last call.
    pub fn changed(&self) -> Option<Vec<Volume>> {
        self.latest.lock().unwrap().take()
    }
}

// ---------------------------------------------------------------- artwork

#[derive(Clone, Debug)]
pub enum ArtSource {
    /// A music file with the picture embedded.
    Embedded(PathBuf),
    /// An image file.
    File(PathBuf),
}

type Done = (String, Option<ColorImage>);

/// Loads and shrinks album artwork on a thread, and keeps the results as
/// textures.
pub struct ArtLoader {
    tx: Sender<(String, ArtSource, u32)>,
    rx: Receiver<Done>,
    pending: HashSet<String>,
    cache: HashMap<String, Option<egui::TextureHandle>>,
}

impl ArtLoader {
    pub fn start(ctx: Context) -> ArtLoader {
        let (tx, jobs) = channel::<(String, ArtSource, u32)>();
        let (done, rx) = channel::<Done>();
        let _ = std::thread::Builder::new()
            .name("artwork".into())
            .spawn(move || {
                while let Ok((key, src, size)) = jobs.recv() {
                    let img = load(&src, size);
                    if done.send((key, img)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            });
        ArtLoader {
            tx,
            rx,
            pending: HashSet::new(),
            cache: HashMap::new(),
        }
    }

    /// The texture for `key`, asking for it if it hasn't been loaded.
    /// `None` while loading; `Some(None)` when there's no usable picture.
    pub fn get(
        &mut self,
        ctx: &Context,
        key: &str,
        src: impl FnOnce() -> Option<ArtSource>,
        size: u32,
    ) -> Option<Option<egui::TextureHandle>> {
        while let Ok((k, img)) = self.rx.try_recv() {
            self.pending.remove(&k);
            let tex = img.map(|i| ctx.load_texture(&k, i, egui::TextureOptions::LINEAR));
            self.cache.insert(k, tex);
        }
        let key = format!("{key}@{size}");
        if let Some(t) = self.cache.get(&key) {
            return Some(t.clone());
        }
        if !self.pending.contains(&key) {
            match src() {
                Some(s) => {
                    self.pending.insert(key.clone());
                    let _ = self.tx.send((key, s, size));
                }
                None => {
                    self.cache.insert(key, None);
                    return Some(None);
                }
            }
        }
        None
    }

    /// Forgets everything, after a scan may have changed the pictures.
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

fn load(src: &ArtSource, size: u32) -> Option<ColorImage> {
    let bytes = match src {
        ArtSource::Embedded(p) => mb_core::tags::read_art(p)?,
        ArtSource::File(p) => std::fs::read(p).ok()?,
    };
    let img = image::load_from_memory(&bytes).ok()?;
    let img = img.thumbnail(size, size).to_rgba8();
    Some(ColorImage::from_rgba_unmultiplied(
        [img.width() as usize, img.height() as usize],
        img.as_raw(),
    ))
}
