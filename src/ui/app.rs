//! The Library panel: the top strip, the drives column, the footer, and
//! the view that's chosen with the View keys.

use super::parts::*;
use super::theme::*;
use super::widgets::*;
use crate::workers::{ArtLoader, DriveWatch, ScanRun};
use egui::text::{LayoutJob, TextFormat};
use egui::{Align2, CornerRadius, Id, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};
use mb_core::db::{Db, VolumeRow};
use mb_core::excludes::{Rule, RuleKind};
use mb_core::library::{Library, Missing};
use mb_core::volumes::{self, Kind, Volume};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::Ordering::Relaxed;
use taw_panel::frame::draw_panel_bg;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const STORAGE_KEY: &str = "taw-library";

/// Below this size the whole panel scales down instead of squeezing.
pub const DESIGN_MIN: Vec2 = Vec2::new(1560.0, 880.0);

/// The left column (drives, formats, excluded).
pub const SIDE_W: f32 = 290.0;
pub const TOP_ROW: f32 = 136.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum View {
    #[default]
    Library,
    Missing,
    Dupes,
    Organise,
}

/// An album, by something that survives a rescan: drive, folder and name.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AlbumKey {
    pub volume: String,
    pub dir: String,
    pub title: String,
}

impl AlbumKey {
    pub fn of(a: &mb_core::library::Album) -> AlbumKey {
        AlbumKey {
            volume: a.volume.clone(),
            dir: a.dir.clone(),
            title: a.title.clone(),
        }
    }
    pub fn matches(&self, a: &mb_core::library::Album) -> bool {
        self.volume == a.volume && self.dir == a.dir && self.title == a.title
    }
    pub fn cache_key(&self) -> String {
        format!("{}|{}|{}", self.volume, self.dir, self.title)
    }
}

/// Remembered between runs. New fields need defaults; never rename one.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Saved {
    pub panel: PanelKind,
    pub view: View,
    pub artist: Option<String>,
    pub album: Option<AlbumKey>,
    /// Index into `Missing::ALL`.
    pub missing: usize,
}

pub struct LibraryApp {
    pub s: Saved,
    pub db: Option<Db>,
    pub db_path: PathBuf,
    pub db_error: Option<String>,
    pub lib: Library,
    pub volumes: Vec<VolumeRow>,
    pub rules: Vec<Rule>,
    pub mounted: Vec<Volume>,
    drives: DriveWatch,
    pub scan: Option<ScanRun>,
    seen_saves: u64,
    last_reload: f64,
    pub art: ArtLoader,
    pub find: String,
    touched: Option<(String, String, f64)>,
    pub notice: Option<(String, f64)>,
    /// Scroll the artist or album list to the selection on the next frame.
    pub scroll_to_selection: bool,
}

pub fn now() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

impl LibraryApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        let s: Saved = cc
            .storage
            .and_then(|st| eframe::get_value(st, STORAGE_KEY))
            .unwrap_or_default();
        let custom = std::env::var_os("TAW_LIBRARY_DB").map(PathBuf::from);
        // A Music Box 0.1 library moves into the family's folder once.
        let moved = custom.is_none() && Db::move_old_library().unwrap_or(false);
        let db_path = custom.unwrap_or_else(Db::default_path);
        let (db, db_error) = match Db::open(&db_path) {
            Ok(db) => (Some(db), None),
            Err(e) => (
                None,
                Some(format!(
                    "Can't open the library at {}: {e}",
                    db_path.display()
                )),
            ),
        };
        let mut app = LibraryApp {
            s,
            db,
            db_path,
            db_error,
            lib: Library::default(),
            volumes: Vec::new(),
            rules: Vec::new(),
            mounted: volumes::list(),
            drives: DriveWatch::start(cc.egui_ctx.clone()),
            scan: None,
            seen_saves: 0,
            last_reload: 0.0,
            art: ArtLoader::start(cc.egui_ctx.clone()),
            find: String::new(),
            touched: None,
            notice: None,
            scroll_to_selection: true,
        };
        if let Some(db) = &app.db {
            let _ = db.sync_volumes(&app.mounted, mb_core::now_secs());
        }
        // `--view missing` opens on a view (used for screenshots).
        let args: Vec<String> = std::env::args().collect();
        if let Some(v) = args
            .iter()
            .position(|a| a == "--view")
            .and_then(|i| args.get(i + 1))
        {
            app.s.view = match v.as_str() {
                "missing" => View::Missing,
                "dupes" => View::Dupes,
                "organise" => View::Organise,
                _ => View::Library,
            };
        }
        app.reload();
        if moved {
            app.tell(
                "Your Music Box library moved to the Library's new folder. Nothing was rescanned.",
            );
        }
        app
    }

    /// Reads the library, drives and rules again from the database.
    pub fn reload(&mut self) {
        let Some(db) = &self.db else { return };
        match Library::load(db) {
            Ok(l) => self.lib = l,
            Err(e) => self.db_error = Some(format!("Can't read the library: {e}")),
        }
        self.volumes = db.volumes().unwrap_or_default();
        self.rules = db.rules().unwrap_or_default();
        self.last_reload = now();
        self.fix_selection();
    }

    /// Keeps the chosen artist and album pointing at something that exists.
    pub fn fix_selection(&mut self) {
        let lib = &self.lib;
        let artist_ok = self
            .s
            .artist
            .as_ref()
            .is_some_and(|a| lib.artists.iter().any(|x| &x.name == a));
        if !artist_ok {
            self.s.artist = lib.artists.first().map(|a| a.name.clone());
        }
        let Some(artist) = self
            .s
            .artist
            .as_ref()
            .and_then(|n| lib.artists.iter().find(|a| &a.name == n))
        else {
            self.s.album = None;
            return;
        };
        let album_ok = self
            .s
            .album
            .as_ref()
            .is_some_and(|k| artist.albums.iter().any(|&i| k.matches(&lib.albums[i])));
        if !album_ok {
            self.s.album = artist.albums.first().map(|&i| AlbumKey::of(&lib.albums[i]));
        }
    }

    pub fn touch(&mut self, name: impl Into<String>, value: impl Into<String>) {
        self.touched = Some((name.into(), value.into(), now()));
    }

    pub fn tell(&mut self, msg: impl Into<String>) {
        self.notice = Some((msg.into(), now()));
    }

    pub fn volume(&self, id: &str) -> Option<&VolumeRow> {
        self.volumes.iter().find(|v| v.id == id)
    }

    pub fn online(&self, v: &VolumeRow) -> bool {
        mb_core::scan::is_online(v, &self.mounted)
    }

    /// A file's full path, if its drive is there now.
    pub fn full_path(&self, volume: &str, rel: &str) -> Option<PathBuf> {
        let v = self.volume(volume)?;
        self.online(v).then(|| volumes::join(&v.mount, rel))
    }

    fn scanning(&self) -> bool {
        self.scan.as_ref().is_some_and(|s| s.running())
    }

    fn start_scan(&mut self, ctx: &egui::Context) {
        if self.db.is_none() {
            return;
        }
        self.scan = Some(ScanRun::start(
            self.db_path.clone(),
            self.mounted.clone(),
            ctx.clone(),
        ));
        self.seen_saves = 0;
        self.touch("Scan", "Started");
    }

    /// Watches the drives and the scan, and reloads when they change.
    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(m) = self.drives.changed() {
            let before: Vec<String> = self.mounted.iter().map(|v| v.id.clone()).collect();
            let fresh: Vec<&Volume> = m.iter().filter(|v| !before.contains(&v.id)).collect();
            if let Some(v) = fresh.first() {
                self.tell(format!(
                    "{} is connected. Press Scan to add what's on it.",
                    v.label
                ));
            }
            self.mounted = m;
            if let Some(db) = &self.db {
                let _ = db.sync_volumes(&self.mounted, mb_core::now_secs());
            }
            self.volumes = self
                .db
                .as_ref()
                .and_then(|d| d.volumes().ok())
                .unwrap_or_default();
        }
        let Some(run) = &mut self.scan else { return };
        let saves = run.progress.saved.load(Relaxed);
        let running = run.running();
        if running {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
            if saves != self.seen_saves && now() - self.last_reload > 3.0 {
                self.seen_saves = saves;
                self.reload();
            }
            return;
        }
        if let Some(result) = run.finish() {
            self.scan = None;
            self.art.clear();
            self.reload();
            match result {
                Ok(s) if s.cancelled => self.tell(format!(
                    "Scan stopped. {} files read so far are kept.",
                    thousands(s.read)
                )),
                Ok(s) => {
                    let mut msg = format!(
                        "Scan finished: {} music files, {} new or changed",
                        thousands(s.found),
                        thousands(s.read)
                    );
                    if s.removed > 0 {
                        msg += &format!(", {} gone", thousands(s.removed));
                    }
                    if s.errors > 0 {
                        msg += &format!(", {} couldn't be read", thousands(s.errors));
                    }
                    self.tell(msg + ".");
                    self.touch("Scan finished", format!("{} files", thousands(s.found)));
                }
                Err(e) => self.tell(format!("The scan failed: {e}")),
            }
        }
    }

    // ------------------------------------------------------------ frame

    fn top_strip(&mut self, ui: &mut Ui, panel: Rect, th: &Theme) {
        let o = panel.min.to_vec2();
        let at = |x: f32, y: f32| Pos2::new(x, y) + o;
        let right = panel.right();

        // Maker wordmark, logo and model line.
        {
            let p = ui.painter();
            let mut job = LayoutJob::default();
            job.append(
                "Timster's Audio Works - ",
                0.0,
                TextFormat {
                    font_id: font(F_MAKER, 13.0),
                    color: th.legend_dim,
                    ..Default::default()
                },
            );
            job.append(
                "Library",
                0.0,
                TextFormat {
                    font_id: font(F_MAKER_BOLD, 13.0),
                    color: th.legend,
                    ..Default::default()
                },
            );
            let g = p.layout_job(job);
            let r = Rect::from_min_size(at(22.0, 14.0), g.size() + Vec2::new(20.0, 10.0));
            p.rect_stroke(
                r,
                CornerRadius::same(3),
                Stroke::new(1.0, th.frame),
                StrokeKind::Inside,
            );
            p.galley(r.min + Vec2::new(10.0, 5.0), g, th.legend);
        }
        slanted_text(
            ui,
            at(24.0, 50.0),
            "LIBRARY",
            font(F_LOGO, 30.0),
            th.legend,
            3.0,
            12.0,
        );
        text(
            ui.painter(),
            at(22.0, 92.0),
            Align2::LEFT_TOP,
            &format!("LB-001 · MUSIC LIBRARY MANAGER · V{VERSION}"),
            font(F_LEGEND, 10.0),
            th.legend_dim,
            2.4,
        );

        // LED display
        let t = now();
        let line1 = (
            format!("{} tracks", thousands(self.lib.tracks.len() as u64)),
            "Read only".to_string(),
        );
        let line2 = if let Some(run) = self.scan.as_ref().filter(|r| r.running()) {
            (
                format!("Scanning {}", run.progress.current.lock().unwrap()),
                format!("{} files", thousands(run.progress.found.load(Relaxed))),
            )
        } else {
            match &self.touched {
                Some((n, v, when)) if t - when < 3.0 => (n.clone(), v.clone()),
                _ => {
                    let offline = self.volumes.iter().filter(|v| !self.online(v)).count();
                    let n = self.volumes.len();
                    let drives = if offline > 0 {
                        format!("{n} drives · {offline} offline")
                    } else {
                        format!("{n} drives")
                    };
                    (
                        if self.lib.tracks.is_empty() {
                            "Press Scan to start".to_string()
                        } else {
                            drives
                        },
                        "Lookups off".to_string(),
                    )
                }
            }
        };
        lcd(
            ui.painter(),
            Rect::from_min_size(at(292.0, 26.0), Vec2::new(318.0, 52.0)),
            &[line1, line2],
        );

        // Library keys
        legend(
            ui.painter(),
            at(700.0, 16.0),
            Align2::CENTER_TOP,
            "Library",
            th,
        );
        let k = |x: f32| Rect::from_min_size(at(x, 44.0), Vec2::new(54.0, 24.0));
        if key(ui, k(640.0), Id::new("add"), K_CREAM, None, "Add", th).clicked() {
            self.add_folder();
        }
        if key(
            ui,
            k(704.0),
            Id::new("exclude"),
            K_GREY,
            None,
            "Exclude",
            th,
        )
        .clicked()
        {
            self.add_exclude();
        }

        // Completeness meters
        let p = ui.painter();
        let mx = 790.0;
        for (i, (name, share, col)) in [
            ("Tags", self.lib.tags_complete, OK_GREEN),
            ("Art", self.lib.art_complete, AMBER),
        ]
        .into_iter()
        .enumerate()
        {
            let y = 34.0 + i as f32 * 24.0;
            legend(p, at(mx - 8.0, y + 7.0), Align2::RIGHT_CENTER, name, th);
            ladder(
                p,
                Rect::from_min_size(at(mx, y), Vec2::new(116.0, 14.0)),
                share,
                col,
            );
            glow_text(
                p,
                at(mx + 124.0, y + 7.0),
                Align2::LEFT_CENTER,
                &format!("{:.0}%", share * 100.0),
                font(F_MONO, 13.0),
                LCD_INK,
                0.5,
            );
        }
        text(
            p,
            at(mx + 58.0, 84.0),
            Align2::CENTER_TOP,
            "COMPLETE",
            font(F_LEGEND, 10.0),
            th.legend_dim,
            2.0,
        );

        // From the right: Scan / Stop, Lookups, Panel, View.
        let scanning = self.scanning();
        let scan_r = Rect::from_min_size(
            Pos2::new(right - 22.0 - 96.0, panel.top() + 42.0),
            Vec2::new(96.0, 30.0),
        );
        if key(
            ui,
            scan_r,
            Id::new("scan"),
            K_RED,
            Some(scanning),
            "Scan / Stop",
            th,
        )
        .clicked()
        {
            if scanning {
                if let Some(s) = &self.scan {
                    s.stop();
                }
                self.touch("Scan", "Stopping");
            } else {
                self.start_scan(ui.ctx());
            }
        }

        let small =
            |x: f32| Rect::from_min_size(Pos2::new(x, panel.top() + 44.0), Vec2::new(50.0, 24.0));
        let mut x = scan_r.left() - 26.0 - 50.0;
        legend(
            ui.painter(),
            Pos2::new(x + 25.0, panel.top() + 16.0),
            Align2::CENTER_TOP,
            "Online",
            th,
        );
        if key(
            ui,
            small(x),
            Id::new("lookups"),
            K_YELLOW,
            Some(false),
            "Lookups",
            th,
        )
        .clicked()
        {
            self.touch("Lookups", "Stage 2");
            self.tell("Online lookups (MusicBrainz and cover art) come in stage 2. Nothing is sent online.");
        }

        x -= 26.0 + 50.0 * 2.0 + 8.0;
        legend(
            ui.painter(),
            Pos2::new(x + 54.0, panel.top() + 16.0),
            Align2::CENTER_TOP,
            "Panel",
            th,
        );
        for (i, (kind, label)) in [(PanelKind::Black, "Black"), (PanelKind::Silver, "Silver")]
            .into_iter()
            .enumerate()
        {
            let r = small(x + i as f32 * 58.0);
            if key(
                ui,
                r,
                Id::new(("panel", i)),
                K_GREY,
                Some(self.s.panel == kind),
                label,
                th,
            )
            .clicked()
            {
                self.s.panel = kind;
            }
        }

        let views = [
            (View::Library, "Library"),
            (View::Missing, "Missing"),
            (View::Dupes, "Dupes"),
            (View::Organise, "Organise"),
        ];
        x -= 26.0 + 58.0 * views.len() as f32;
        legend(
            ui.painter(),
            Pos2::new(x + 58.0 * 2.0 - 4.0, panel.top() + 16.0),
            Align2::CENTER_TOP,
            "View",
            th,
        );
        for (i, (v, label)) in views.into_iter().enumerate() {
            let r = small(x + i as f32 * 58.0);
            if key(
                ui,
                r,
                Id::new(("view", i)),
                K_GREY,
                Some(self.s.view == v),
                label,
                th,
            )
            .clicked()
            {
                self.s.view = v;
                self.scroll_to_selection = true;
                let value = match v {
                    View::Library => format!("{} albums", thousands(self.lib.albums.len() as u64)),
                    View::Missing => {
                        let n = self
                            .lib
                            .missing_albums
                            .get(&Missing::Art)
                            .map_or(0, Vec::len);
                        format!("{} need art", thousands(n as u64))
                    }
                    View::Dupes | View::Organise => "Coming later".into(),
                };
                self.touch(label, value);
            }
        }
    }

    fn add_folder(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Add a folder to scan")
            .pick_folder()
        else {
            return;
        };
        let Some(db) = &self.db else { return };
        let vols: Vec<Volume> = self.mounted.clone();
        let (vol, rel) = match volumes::containing(&vols, &path) {
            Some(v) => (
                v.clone(),
                volumes::rel_path(&v.mount, &path).unwrap_or_default(),
            ),
            None => (volumes::folder_volume(&path), String::new()),
        };
        if let Some(row) = self.volumes.iter().find(|r| r.id == vol.id && r.scan) {
            let label = row.label.clone();
            self.tell(format!(
                "{} is already scanned as a whole drive, so this folder is included.",
                label
            ));
            return;
        }
        match db.add_location(&vol, &rel, mb_core::now_secs()) {
            Ok(()) => {
                self.tell(format!("Added {}. Press Scan to read it.", path.display()));
                self.touch("Added", vol.label);
            }
            Err(e) => self.tell(format!("Couldn't add that folder: {e}")),
        }
        self.reload();
    }

    fn add_exclude(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Choose a folder to leave out of the library")
            .pick_folder()
        else {
            return;
        };
        if let Some(db) = &self.db {
            let _ = db.add_rule(RuleKind::Path, &path.to_string_lossy(), "Left out by you");
        }
        self.tell(format!(
            "{} will be left out from the next scan, and what's in it will be forgotten.",
            path.display()
        ));
        self.reload();
    }

    // ------------------------------------------------------------ side column

    fn side(&mut self, ui: &mut Ui, col: Rect, th: &Theme) {
        let now_s = mb_core::now_secs();

        // Drives
        // A drive is a hand-added folder when the Library can't see it as a
        // mounted drive (a network share with no drive letter, for one).
        let added = |v: &VolumeRow, mounted: &[Volume]| {
            matches!(v.kind, Kind::Folder | Kind::Network) && !mounted.iter().any(|m| m.id == v.id)
        };
        // One row per drive, then one per folder on drives not scanned whole.
        let mut items: Vec<(usize, Option<String>)> = Vec::new();
        for (i, v) in self.volumes.iter().enumerate() {
            items.push((i, None));
            if !v.scan && !added(v, &self.mounted) {
                for l in &v.locations {
                    items.push((i, Some(l.clone())));
                }
            }
        }
        let height: f32 = items
            .iter()
            .map(|(_, l)| if l.is_some() { 26.0 } else { 44.0 })
            .sum();
        let drives_h = (height.max(44.0) + 34.0).min(col.height() * 0.48);
        let r = Rect::from_min_size(col.min, Vec2::new(col.width(), drives_h));
        section(ui.painter(), r, "Drives", th, false);
        let inner =
            Rect::from_min_max(r.min + Vec2::new(10.0, 16.0), r.max - Vec2::new(10.0, 10.0));
        well(ui.painter(), inner, th);
        if self.volumes.is_empty() {
            fit(
                ui.painter(),
                inner.left_top() + Vec2::new(10.0, 10.0),
                Align2::LEFT_TOP,
                "No drives found. Use Add to choose a folder.",
                font(F_LEGEND, 12.5),
                th.legend_dim,
                inner.width() - 20.0,
            );
        }
        enum Act {
            Whole(usize, bool),
            RemoveFolder(usize, String),
            Forget(usize),
        }
        let mut action: Option<Act> = None;
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(inner));
        child.set_clip_rect(inner);
        egui::ScrollArea::vertical()
            .id_salt("drives")
            .auto_shrink([false, false])
            .show(&mut child, |ui| {
                ui.spacing_mut().item_spacing = Vec2::ZERO;
                for (i, (vi, loc)) in items.iter().enumerate() {
                    let v = &self.volumes[*vi];
                    let h = if loc.is_some() { 26.0 } else { 44.0 };
                    let (r, _) = ui.allocate_exact_size(
                        Vec2::new(ui.available_width(), h),
                        egui::Sense::hover(),
                    );
                    let resp = ui.interact(r, Id::new(("drive", i)), egui::Sense::hover());
                    let online = mb_core::scan::is_online(v, &self.mounted);
                    let p = ui.painter();
                    row_bg(p, r, &resp, false, th);
                    let x = r.left() + 10.0;
                    let cross = |ui: &mut Ui, c: Pos2, id: (&str, usize), tip: &str| {
                        let xr = Rect::from_center_size(c, Vec2::splat(16.0));
                        let xresp = ui
                            .interact(xr, Id::new(id), egui::Sense::click())
                            .on_hover_text(tip);
                        text(
                            ui.painter(),
                            xr.center(),
                            Align2::CENTER_CENTER,
                            "×",
                            font(F_LEGEND_BOLD, 15.0),
                            if xresp.hovered() {
                                FOCUS
                            } else {
                                th.legend_dim
                            },
                            0.0,
                        );
                        xresp.clicked()
                    };
                    if let Some(loc) = loc {
                        let label = if loc.is_empty() {
                            "The whole drive".to_string()
                        } else {
                            loc.clone()
                        };
                        fit(
                            p,
                            Pos2::new(x + 14.0, r.center().y),
                            Align2::LEFT_CENTER,
                            &format!("› {label}"),
                            font(F_LEGEND, 12.5),
                            th.legend,
                            r.width() - 60.0,
                        );
                        if cross(
                            ui,
                            Pos2::new(r.right() - 18.0, r.center().y),
                            ("rm-loc", i),
                            "Stop scanning this folder",
                        ) {
                            action = Some(Act::RemoveFolder(*vi, loc.clone()));
                        }
                        continue;
                    }
                    light(
                        p,
                        Pos2::new(x + 3.0, r.top() + 14.0),
                        if online { Light::Ok } else { Light::Off },
                        th,
                    );
                    let is_added = added(v, &self.mounted);
                    let name = if is_added {
                        v.label.clone()
                    } else {
                        format!(
                            "{}  {}",
                            v.mount.to_string_lossy().trim_end_matches('\\'),
                            v.label
                        )
                    };
                    fit(
                        p,
                        Pos2::new(x + 14.0, r.top() + 14.0),
                        Align2::LEFT_CENTER,
                        &name,
                        font(F_LEGEND_BOLD, 13.5),
                        th.legend,
                        r.width() - 110.0,
                    );
                    glow_text(
                        p,
                        Pos2::new(r.right() - 44.0, r.top() + 14.0),
                        Align2::RIGHT_CENTER,
                        &thousands(v.tracks as u64),
                        font(F_MONO, 12.5),
                        LCD_INK,
                        0.3,
                    );
                    let what = if is_added {
                        v.mount.to_string_lossy().into_owned()
                    } else if v.scan {
                        "whole drive".to_string()
                    } else if v.locations.is_empty() {
                        "not scanned".to_string()
                    } else {
                        format!(
                            "{} folder{}",
                            v.locations.len(),
                            if v.locations.len() == 1 { "" } else { "s" }
                        )
                    };
                    let state = if !online {
                        format!(
                            "offline · last seen {} · entries kept",
                            v.last_seen.map_or("never".into(), |s| ago(s, now_s))
                        )
                    } else {
                        match v.last_scan {
                            Some(s) => format!("scanned {}", ago(s, now_s)),
                            None => "not scanned yet".into(),
                        }
                    };
                    let sub = if is_added {
                        format!("{state} · {what}")
                    } else {
                        format!("{} · {what} · {state}", v.kind.as_str())
                    };
                    fit(
                        p,
                        Pos2::new(x + 14.0, r.top() + 31.0),
                        Align2::LEFT_CENTER,
                        &sub,
                        font(F_LEGEND, 11.5),
                        th.legend_dim,
                        r.width() - 50.0,
                    );
                    if is_added {
                        if cross(
                            ui,
                            Pos2::new(r.right() - 18.0, r.top() + 14.0),
                            ("rm-added", i),
                            "Remove this folder and forget what was found in it",
                        ) {
                            action = Some(Act::Forget(*vi));
                        }
                        continue;
                    }
                    // Whole-drive switch: a lit key, green when the whole drive is scanned.
                    let kr = Rect::from_min_size(
                        Pos2::new(r.right() - 34.0, r.top() + 8.0),
                        Vec2::new(24.0, 12.0),
                    );
                    if lit_key(ui, kr, Id::new(("whole", i)), ON_GREEN, v.scan, "", th)
                        .on_hover_text(if v.scan {
                            "Scanning the whole drive. Click to scan only the folders you add."
                        } else {
                            "Click to scan the whole drive"
                        })
                        .clicked()
                    {
                        action = Some(Act::Whole(*vi, !v.scan));
                    }
                    if !online
                        && cross(
                            ui,
                            Pos2::new(r.right() - 22.0, r.top() + 31.0),
                            ("forget", i),
                            "Forget this drive and what was found on it",
                        )
                    {
                        action = Some(Act::Forget(*vi));
                    }
                }
            });
        if let (Some(act), Some(db)) = (action, &self.db) {
            match act {
                Act::Forget(vi) => {
                    let v = self.volumes[vi].clone();
                    let _ = db.remove_volume(&v.id);
                    self.tell(format!(
                        "Forgot {} and the {} tracks found on it.",
                        v.label,
                        thousands(v.tracks as u64)
                    ));
                }
                Act::RemoveFolder(vi, l) => {
                    let _ = db.remove_location(&self.volumes[vi].id, &l);
                    self.tell("Removed. Its tracks stay in the library until that drive is scanned again.");
                }
                Act::Whole(vi, on) => {
                    let v = self.volumes[vi].clone();
                    let _ = db.set_scan(&v.id, on);
                    self.touch(
                        v.label.clone(),
                        if on { "Whole drive" } else { "Folders only" },
                    );
                }
            }
            self.reload();
        }

        // Formats
        let n_fmt = self.lib.formats.len().clamp(1, 8);
        let fr = Rect::from_min_size(
            Pos2::new(col.left(), r.bottom() + 18.0),
            Vec2::new(col.width(), 30.0 + n_fmt as f32 * 20.0),
        );
        section(ui.painter(), fr, "Formats", th, false);
        let p = ui.painter();
        let max = self.lib.formats.first().map_or(1, |f| f.1).max(1);
        if self.lib.formats.is_empty() {
            text(
                p,
                fr.left_top() + Vec2::new(14.0, 22.0),
                Align2::LEFT_TOP,
                "Nothing scanned yet",
                font(F_LEGEND, 12.0),
                th.legend_dim,
                0.0,
            );
        }
        for (i, (f, n)) in self.lib.formats.iter().take(8).enumerate() {
            let y = fr.top() + 26.0 + i as f32 * 20.0;
            text(
                p,
                Pos2::new(fr.left() + 14.0, y),
                Align2::LEFT_CENTER,
                f,
                font(F_LEGEND_BOLD, 11.5),
                th.legend,
                1.4,
            );
            let bar = Rect::from_min_size(
                Pos2::new(fr.left() + 64.0, y - 4.0),
                Vec2::new(fr.width() - 150.0, 8.0),
            );
            p.rect_filled(bar, 1, th.well);
            p.rect_filled(
                Rect::from_min_size(
                    bar.min,
                    Vec2::new(bar.width() * *n as f32 / max as f32, 8.0),
                ),
                1,
                th.legend_dim,
            );
            glow_text(
                p,
                Pos2::new(fr.right() - 14.0, y),
                Align2::RIGHT_CENTER,
                &thousands(*n as u64),
                font(F_MONO, 12.0),
                LCD_INK,
                0.3,
            );
        }

        // Excluded
        let er = Rect::from_min_max(Pos2::new(col.left(), fr.bottom() + 18.0), col.max);
        section(ui.painter(), er, "Excluded", th, false);
        let mut remove = None;
        let mut y = er.top() + 20.0;
        for (i, rule) in self.rules.iter().enumerate() {
            if y > er.bottom() - 90.0 {
                break;
            }
            let p = ui.painter();
            let pat = match rule.kind {
                RuleKind::Name => format!("Any folder named {}", rule.pattern),
                RuleKind::Path => rule.pattern.clone(),
            };
            fit(
                p,
                Pos2::new(er.left() + 14.0, y),
                Align2::LEFT_TOP,
                &pat,
                font(F_LEGEND_BOLD, 12.5),
                th.legend,
                er.width() - 50.0,
            );
            let hits = if rule.hits > 0 {
                format!(
                    "{} · skipped {} folder{} last scan",
                    rule.note,
                    rule.hits,
                    if rule.hits == 1 { "" } else { "s" }
                )
            } else {
                rule.note.clone()
            };
            fit(
                p,
                Pos2::new(er.left() + 14.0, y + 16.0),
                Align2::LEFT_TOP,
                &hits,
                font(F_LEGEND, 11.5),
                th.legend_dim,
                er.width() - 40.0,
            );
            let xr =
                Rect::from_center_size(Pos2::new(er.right() - 18.0, y + 8.0), Vec2::splat(16.0));
            let xresp = ui
                .interact(xr, Id::new(("rule", i)), egui::Sense::click())
                .on_hover_text("Stop leaving this out");
            text(
                ui.painter(),
                xr.center(),
                Align2::CENTER_CENTER,
                "×",
                font(F_LEGEND_BOLD, 15.0),
                if xresp.hovered() {
                    FOCUS
                } else {
                    th.legend_dim
                },
                0.0,
            );
            if xresp.clicked() {
                remove = Some(rule.id);
            }
            y += 38.0;
        }
        let p = ui.painter();
        let note = "Always left out: system folders, hidden folders and DAW project folders (their audio is stems and bounces). Exclude adds a folder.";
        let g = p.layout(
            note.to_string(),
            font(F_LEGEND, 11.5),
            th.legend_dim,
            er.width() - 28.0,
        );
        p.galley(
            Pos2::new(
                er.left() + 14.0,
                (y + 4.0)
                    .max(er.bottom() - 70.0)
                    .min(er.bottom() - g.size().y - 10.0),
            ),
            g,
            th.legend_dim,
        );
        if let (Some(id), Some(db)) = (remove, &self.db) {
            let _ = db.remove_rule(id);
            self.tell("That folder will be scanned again from the next scan.");
            self.reload();
        }
    }

    fn footer(&mut self, ui: &mut Ui, panel: Rect, th: &Theme) {
        let p = ui.painter();
        let y = panel.bottom() - 18.0;
        text(
            p,
            Pos2::new(panel.left() + 22.0, y),
            Align2::LEFT_CENTER,
            "Click an artist or album to see it · Scan reads only what changed since last time · the Library doesn't change your files",
            font(F_LEGEND, 12.0),
            th.legend_dim,
            0.0,
        );
        let msg = match (&self.db_error, &self.notice) {
            (Some(e), _) => Some((e.clone(), LED)),
            (None, Some((m, t))) if now() - t < 12.0 => Some((m.clone(), STRIPE[1])),
            _ => None,
        };
        if let Some((m, col)) = msg {
            fit(
                p,
                Pos2::new(panel.right() - 22.0, y),
                Align2::RIGHT_CENTER,
                &m,
                font(F_LEGEND_BOLD, 12.5),
                col,
                panel.width() * 0.5,
            );
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(1));
        }
    }
}

pub(crate) fn set_visuals(ctx: &egui::Context, th: &Theme) {
    let mut v = if th.silver {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };
    v.panel_fill = th.panel;
    v.extreme_bg_color = LCD_BG;
    v.selection.bg_fill = LCD_DIM;
    v.selection.stroke = Stroke::new(1.0, LCD_INK);
    v.text_cursor.stroke = Stroke::new(2.0, LCD_INK);
    if ctx.global_style().visuals != v {
        ctx.set_visuals(v);
    }
}

impl eframe::App for LibraryApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        // Never larger than designed: on a big window the lists get the room.
        taw_panel::frame::fit_to_window(ui.ctx(), DESIGN_MIN, 1.0);
        let th = Theme::panel(self.s.panel);
        set_visuals(ui.ctx(), &th);
        self.poll(&ui.ctx().clone());

        let panel = ui.max_rect();
        draw_panel_bg(ui.painter(), panel, &th);
        self.top_strip(ui, panel, &th);
        stripe(
            ui.painter(),
            Rect::from_min_size(
                panel.min + Vec2::new(22.0, 110.0),
                Vec2::new(panel.width() - 44.0, 6.0),
            ),
        );
        let body = Rect::from_min_max(
            panel.min + Vec2::new(22.0, TOP_ROW),
            panel.max - Vec2::new(22.0, 40.0),
        );
        let side = Rect::from_min_size(body.min, Vec2::new(SIDE_W, body.height()));
        self.side(ui, side, &th);
        let main = Rect::from_min_max(Pos2::new(side.right() + 16.0, body.top()), body.max);
        match self.s.view {
            View::Library => self.library_view(ui, main, &th),
            View::Missing => self.missing_view(ui, main, &th),
            View::Dupes => self.later_view(ui, main, &th, "Dupes"),
            View::Organise => self.later_view(ui, main, &th, "Organise"),
        }
        self.footer(ui, panel, &th);
        self.scroll_to_selection = false;
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.s);
    }
}
