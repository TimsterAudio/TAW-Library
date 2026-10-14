//! The Library panel, in the TAW look: the nameplate and family switcher,
//! the search and library buttons, the sidebar (views and drives), the
//! chosen view, the footer and the settings drawer.

use super::look::*;
use crate::workers::{ArtLoader, DriveWatch, ScanRun};
use egui::{
    Align2, CornerRadius, CursorIcon, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2,
    ViewportCommand,
};
use mb_core::db::{Db, VolumeRow};
use mb_core::excludes::{Rule, RuleKind};
use mb_core::library::Library;
use mb_core::volumes::{self, Kind, Volume};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::Ordering::Relaxed;
use taw_panel::hifi::{self, Tokens};
use taw_panel::theme::PanelKind;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const STORAGE_KEY: &str = "taw-library";

pub const DEFAULT_SIZE: Vec2 = Vec2::new(1360.0, 820.0);
/// Below this size the whole panel scales down instead of squeezing.
const MIN_DESIGN: Vec2 = Vec2::new(1240.0, 720.0);
const PLATE_H: f32 = 96.0;
const FOOT_H: f32 = 36.0;
/// The sidebar (views and drives).
pub const SIDE_W: f32 = 230.0;

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
    /// The Jupiter panel before the TAW look. Kept so older settings load.
    #[allow(dead_code)]
    pub panel: PanelKind,
    pub view: View,
    pub artist: Option<String>,
    pub album: Option<AlbumKey>,
    /// Index into `Missing::ALL`.
    pub missing: usize,
    /// Finish, light or dark, and accent.
    pub finish: hifi::Look,
    /// Simple view: hides Duplicates, Organise, Exclude and Lookups until
    /// they're wanted.
    pub simple: bool,
    /// Use the system's title bar instead of the Library's own.
    pub system_title_bar: bool,
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
    settings_open: bool,
    /// Which family apps were found: Library, Player, Equaliser, Analyser.
    installed: [bool; 4],
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
        hifi::install_fonts(&cc.egui_ctx);
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
            settings_open: false,
            installed: [true, false, false, false],
        };
        if app.s.system_title_bar {
            cc.egui_ctx
                .send_viewport_cmd(ViewportCommand::Decorations(true));
        }
        app.refresh_family();
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

    fn refresh_family(&mut self) {
        for (i, prog) in [(1, "taw-player"), (2, "taw-equaliser"), (3, "taw-analyser")] {
            self.installed[i] = super::views::find_app(prog).is_some();
        }
    }

    fn start_family(&mut self, i: usize) {
        let (prog, name) = match i {
            1 => ("taw-player", "Player"),
            2 => ("taw-equaliser", "Equaliser"),
            _ => ("taw-analyser", "Analyser"),
        };
        match super::views::find_app(prog) {
            Some(exe) => match std::process::Command::new(&exe)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                Ok(_) => self.tell(format!("Starting the {name}…")),
                Err(e) => self.tell(format!("The {name} wouldn't start: {e}")),
            },
            None => self.tell(format!(
                "The {name} isn't installed here (looked next to the Library and on the PATH)."
            )),
        }
        self.refresh_family();
    }

    // ------------------------------------------------------------ top

    fn plate(&mut self, ui: &mut Ui, screen: Rect, t: &Tokens) {
        let plate = Rect::from_min_size(screen.min, Vec2::new(screen.width(), PLATE_H));
        if !self.s.system_title_bar {
            hifi::drag_to_move(ui, plate);
        }
        hifi::nameplate(
            ui,
            screen.min + Vec2::new(24.0, 14.0),
            t,
            "Library",
            &format!("LB-001 · Music library manager · V{VERSION}"),
        );
        if !self.s.system_title_bar {
            hifi::window_buttons(ui, Pos2::new(screen.right() - 10.0, screen.top() + 10.0), t);
        }
        let right = screen.right() - 24.0;
        let row = screen.top() + 66.0;
        let gear = Rect::from_min_size(Pos2::new(right - 30.0, row - 15.0), Vec2::splat(30.0));
        if hifi::gear_button(ui, gear, Id::new("settings"), t, self.settings_open).clicked() {
            self.settings_open = !self.settings_open;
        }
        let (seg, picked) = hifi::segmented(
            ui,
            Pos2::new(gear.left() - 12.0, row),
            true,
            Id::new("simple"),
            t,
            &["Simple", "Detailed"],
            if self.s.simple { 0 } else { 1 },
            hifi::Seg::Plain,
            &[],
            &[],
        );
        if let Some(i) = picked {
            self.s.simple = i == 0;
            if self.s.simple && matches!(self.s.view, View::Dupes | View::Organise) {
                self.s.view = View::Library;
            }
        }
        let (_, picked) =
            hifi::family_switch(ui, Pos2::new(seg.left() - 14.0, row), t, 0, self.installed);
        if let Some(i) = picked {
            self.start_family(i);
        }
    }

    /// Search, then Scan and (in Detailed) Add folder, Exclude and Lookups.
    fn toolbar(&mut self, ui: &mut Ui, r: Rect, t: &Tokens) {
        let y = r.center().y;
        let mut x = r.right();
        let scanning = self.scanning();
        let mut buttons: Vec<(&str, &str, bool)> = vec![(
            "scan",
            if scanning { "Stop scan" } else { "Scan drives" },
            true,
        )];
        if !self.s.simple {
            buttons.push(("add", "Add folder", false));
            buttons.push(("exclude", "Exclude…", false));
            buttons.push(("lookups", "Lookups: off", false));
        }
        for (id, label, primary) in buttons {
            let w = hifi::button_width(ui, label).max(96.0);
            let br = Rect::from_min_size(Pos2::new(x - w, y - 16.0), Vec2::new(w, 32.0));
            if hifi::button(ui, br, Id::new(id), t, label, primary).clicked() {
                match id {
                    "scan" if scanning => {
                        if let Some(s) = &self.scan {
                            s.stop();
                        }
                        self.touch("Scan", "Stopping");
                    }
                    "scan" => self.start_scan(ui.ctx()),
                    "add" => self.add_folder(),
                    "exclude" => self.add_exclude(),
                    _ => self.tell(
                        "Online lookups (MusicBrainz and cover art) come in stage 2. Nothing is sent online.",
                    ),
                }
            }
            x = br.left() - 10.0;
        }

        // The search field.
        let field = Rect::from_min_max(Pos2::new(r.left(), y - 18.0), Pos2::new(x - 4.0, y + 18.0));
        let p = ui.painter();
        p.rect_filled(field, CornerRadius::same(8), t.well);
        p.rect_stroke(
            field,
            CornerRadius::same(8),
            Stroke::new(1.0, t.line),
            StrokeKind::Inside,
        );
        let c = Pos2::new(field.left() + 18.0, y - 1.0);
        p.circle_stroke(c, 5.0, Stroke::new(1.6, t.dim));
        p.line_segment(
            [c + Vec2::new(3.6, 3.6), c + Vec2::new(7.5, 7.5)],
            Stroke::new(1.8, t.dim),
        );
        ui.put(
            Rect::from_min_max(
                Pos2::new(field.left() + 34.0, field.top() + 8.0),
                Pos2::new(field.right() - 10.0, field.bottom() - 6.0),
            ),
            egui::TextEdit::singleline(&mut self.find)
                .frame(egui::Frame::NONE)
                .font(hifi::ui_font(14.0))
                .text_color(t.text)
                .hint_text(
                    egui::RichText::new("Search artists and albums")
                        .color(t.dim)
                        .font(hifi::ui_font(14.0)),
                ),
        );
    }

    // ------------------------------------------------------------ sidebar

    fn side(&mut self, ui: &mut Ui, col: Rect, t: &Tokens) {
        let now_s = mb_core::now_secs();
        let simple = self.s.simple;

        // Views.
        heading(ui.painter(), col.min, "View", t);
        let mut y = col.top() + 22.0;
        let incomplete = self
            .lib
            .albums
            .iter()
            .filter(|a| !a.missing.is_empty())
            .count();
        let mut views = vec![
            (
                View::Library,
                "Library",
                thousands(self.lib.tracks.len() as u64),
            ),
            (
                View::Missing,
                "What's missing",
                thousands(incomplete as u64),
            ),
        ];
        if !simple {
            views.push((View::Dupes, "Duplicates", "soon".into()));
            views.push((View::Organise, "Organise", "soon".into()));
        }
        for (v, label, n) in views {
            let r = Rect::from_min_size(Pos2::new(col.left(), y), Vec2::new(col.width(), 32.0));
            let resp = ui
                .interact(r, Id::new(("view", label)), Sense::click())
                .on_hover_cursor(CursorIcon::PointingHand);
            let on = self.s.view == v;
            let p = ui.painter();
            if on {
                p.rect_filled(r, CornerRadius::same(8), t.select);
            } else if resp.hovered() {
                p.rect_filled(r, CornerRadius::same(8), t.line_soft);
            }
            text(
                p,
                Pos2::new(r.left() + 10.0, r.center().y),
                Align2::LEFT_CENTER,
                label,
                if on {
                    hifi::semi(13.5)
                } else {
                    hifi::ui_font(13.5)
                },
                if on { t.accent } else { t.text },
            );
            text(
                p,
                Pos2::new(r.right() - 10.0, r.center().y),
                Align2::RIGHT_CENTER,
                &n,
                hifi::num(11.5),
                t.dim,
            );
            if resp.clicked() {
                self.s.view = v;
                self.scroll_to_selection = true;
            }
            y += 34.0;
        }

        // Drives (and, in Detailed, the folders scanned on each).
        y += 14.0;
        heading(ui.painter(), Pos2::new(col.left(), y), "Drives", t);
        y += 22.0;
        let excluded_h = if simple {
            0.0
        } else {
            (col.bottom() - y) * 0.36
        };
        let inner = Rect::from_min_max(
            Pos2::new(col.left(), y),
            Pos2::new(col.right(), col.bottom() - excluded_h),
        );
        // A drive is a hand-added folder when the Library can't see it as a
        // mounted drive (a network share with no drive letter, for one).
        let added = |v: &VolumeRow, mounted: &[Volume]| {
            matches!(v.kind, Kind::Folder | Kind::Network) && !mounted.iter().any(|m| m.id == v.id)
        };
        let mut items: Vec<(usize, Option<String>)> = Vec::new();
        for (i, v) in self.volumes.iter().enumerate() {
            items.push((i, None));
            if !simple && !v.scan && !added(v, &self.mounted) {
                for l in &v.locations {
                    items.push((i, Some(l.clone())));
                }
            }
        }
        if self.volumes.is_empty() {
            fit(
                ui.painter(),
                inner.left_top() + Vec2::new(10.0, 4.0),
                Align2::LEFT_TOP,
                "No drives found. Use Add folder to choose one.",
                hifi::ui_font(12.5),
                t.dim,
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
                    let h = if loc.is_some() {
                        24.0
                    } else if simple {
                        30.0
                    } else {
                        44.0
                    };
                    let (r, _) =
                        ui.allocate_exact_size(Vec2::new(ui.available_width(), h), Sense::hover());
                    let online = mb_core::scan::is_online(v, &self.mounted);
                    let x = r.left() + 10.0;
                    let cross = |ui: &mut Ui, c: Pos2, id: (&str, usize), tip: &str| {
                        let xr = Rect::from_center_size(c, Vec2::splat(16.0));
                        let xresp = ui
                            .interact(xr, Id::new(id), Sense::click())
                            .on_hover_text(tip);
                        text(
                            ui.painter(),
                            xr.center(),
                            Align2::CENTER_CENTER,
                            "×",
                            hifi::semi(15.0),
                            if xresp.hovered() { t.accent } else { t.dim },
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
                            ui.painter(),
                            Pos2::new(x + 14.0, r.center().y),
                            Align2::LEFT_CENTER,
                            &format!("› {label}"),
                            hifi::ui_font(12.0),
                            t.dim,
                            r.width() - 60.0,
                        );
                        if cross(
                            ui,
                            Pos2::new(r.right() - 14.0, r.center().y),
                            ("rm-loc", i),
                            "Stop scanning this folder",
                        ) {
                            action = Some(Act::RemoveFolder(*vi, loc.clone()));
                        }
                        continue;
                    }
                    let top = r.top() + 15.0;
                    let p = ui.painter();
                    light(
                        p,
                        Pos2::new(x + 2.0, top),
                        if online { Light::Ok } else { Light::Off },
                        t,
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
                    let count_right = if simple {
                        r.right() - 8.0
                    } else {
                        r.right() - 40.0
                    };
                    let n = text(
                        p,
                        Pos2::new(count_right, top),
                        Align2::RIGHT_CENTER,
                        &if online || v.tracks > 0 {
                            thousands(v.tracks as u64)
                        } else {
                            "offline".into()
                        },
                        hifi::num(11.5),
                        t.dim,
                    );
                    fit(
                        p,
                        Pos2::new(x + 14.0, top),
                        Align2::LEFT_CENTER,
                        &name,
                        hifi::ui_font(13.0),
                        t.text,
                        n.left() - x - 22.0,
                    );
                    if simple {
                        continue;
                    }
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
                            "offline · seen {}",
                            v.last_seen.map_or("never".into(), |s| ago(s, now_s))
                        )
                    } else {
                        match v.last_scan {
                            Some(s) => format!("scanned {}", ago(s, now_s)),
                            None => "not scanned yet".into(),
                        }
                    };
                    fit(
                        p,
                        Pos2::new(x + 14.0, r.top() + 32.0),
                        Align2::LEFT_CENTER,
                        &format!("{what} · {state}"),
                        hifi::ui_font(11.0),
                        t.dim,
                        r.width() - 40.0,
                    );
                    if is_added || !online {
                        let (id, tip) = if is_added {
                            (
                                "rm-added",
                                "Remove this folder and forget what was found in it",
                            )
                        } else {
                            ("forget", "Forget this drive and what was found on it")
                        };
                        if cross(ui, Pos2::new(r.right() - 16.0, top), (id, i), tip) {
                            action = Some(Act::Forget(*vi));
                        }
                        continue;
                    }
                    // Whole drive, or only the folders you add.
                    let sw = Rect::from_min_size(
                        Pos2::new(r.right() - 32.0, top - 7.0),
                        Vec2::new(26.0, 14.0),
                    );
                    if hifi::switch(ui, sw, Id::new(("whole", i)), t, v.scan)
                        .on_hover_text(if v.scan {
                            "Scanning the whole drive. Click to scan only the folders you add."
                        } else {
                            "Click to scan the whole drive"
                        })
                        .clicked()
                    {
                        action = Some(Act::Whole(*vi, !v.scan));
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

        if simple {
            return;
        }

        // Left out.
        let er = Rect::from_min_max(
            Pos2::new(col.left(), col.bottom() - excluded_h + 14.0),
            col.max,
        );
        heading(ui.painter(), er.min, "Left out", t);
        let mut remove = None;
        let mut y = er.top() + 22.0;
        for (i, rule) in self.rules.iter().enumerate() {
            if y > er.bottom() - 70.0 {
                break;
            }
            let p = ui.painter();
            let pat = match rule.kind {
                RuleKind::Name => format!("Any folder named {}", rule.pattern),
                RuleKind::Path => rule.pattern.clone(),
            };
            fit(
                p,
                Pos2::new(er.left() + 10.0, y),
                Align2::LEFT_TOP,
                &pat,
                hifi::ui_font(12.5),
                t.text,
                er.width() - 40.0,
            );
            let hits = if rule.hits > 0 {
                format!(
                    "{} · skipped {} folder{}",
                    rule.note,
                    rule.hits,
                    if rule.hits == 1 { "" } else { "s" }
                )
            } else {
                rule.note.clone()
            };
            fit(
                p,
                Pos2::new(er.left() + 10.0, y + 16.0),
                Align2::LEFT_TOP,
                &hits,
                hifi::ui_font(11.0),
                t.dim,
                er.width() - 30.0,
            );
            let xr =
                Rect::from_center_size(Pos2::new(er.right() - 12.0, y + 8.0), Vec2::splat(16.0));
            let xresp = ui
                .interact(xr, Id::new(("rule", i)), Sense::click())
                .on_hover_text("Stop leaving this out");
            text(
                ui.painter(),
                xr.center(),
                Align2::CENTER_CENTER,
                "×",
                hifi::semi(15.0),
                if xresp.hovered() { t.accent } else { t.dim },
            );
            if xresp.clicked() {
                remove = Some(rule.id);
            }
            y += 38.0;
        }
        let p = ui.painter();
        let note = "Always left out: system and hidden folders, and DAW project folders (their audio is stems and bounces).";
        let g = p.layout(
            note.to_string(),
            hifi::ui_font(11.0),
            t.dim,
            er.width() - 14.0,
        );
        let gy = (y + 4.0).max(er.bottom() - g.size().y - 4.0);
        if gy + g.size().y <= er.bottom() + 1.0 {
            p.galley(Pos2::new(er.left() + 10.0, gy), g, t.dim);
        }
        if let (Some(id), Some(db)) = (remove, &self.db) {
            let _ = db.remove_rule(id);
            self.tell("That folder will be scanned again from the next scan.");
            self.reload();
        }
    }

    // ------------------------------------------------------------ footer

    fn footer(&mut self, ui: &mut Ui, r: Rect, t: &Tokens) {
        hifi::bar(ui.painter(), r, t);
        let y = r.center().y;
        let left = if let Some(run) = self.scan.as_ref().filter(|r| r.running()) {
            format!(
                "Scanning {} · {} files",
                run.progress.current.lock().unwrap(),
                thousands(run.progress.found.load(Relaxed))
            )
        } else {
            let mut s = format!(
                "{} tracks · {} albums",
                thousands(self.lib.tracks.len() as u64),
                thousands(self.lib.albums.len() as u64)
            );
            if let Some(last) = self.volumes.iter().filter_map(|v| v.last_scan).max() {
                s += &format!(" · last scan {}", ago(last, mb_core::now_secs()));
            }
            if !self.s.simple && !self.lib.tracks.is_empty() {
                s += &format!(
                    " · tags {:.0}% and artwork {:.0}% complete",
                    self.lib.tags_complete * 100.0,
                    self.lib.art_complete * 100.0
                );
                let fmts: Vec<String> = self
                    .lib
                    .formats
                    .iter()
                    .take(3)
                    .map(|(f, n)| format!("{f} {}", thousands(*n as u64)))
                    .collect();
                if !fmts.is_empty() {
                    s += &format!(" · {}", fmts.join(", "));
                }
            }
            s
        };
        let p = ui.painter();
        let msg = match (&self.db_error, &self.notice, &self.touched) {
            (Some(e), _, _) => Some((e.clone(), BAD)),
            (None, Some((m, at)), _) if now() - at < 12.0 => Some((m.clone(), t.text)),
            (None, _, Some((n, v, at))) if now() - at < 3.0 => Some((format!("{n}: {v}"), t.dim)),
            _ => None,
        };
        let (right, right_c) =
            msg.unwrap_or(("Read only · your files are never changed".into(), t.dim));
        let rr = fit(
            p,
            Pos2::new(r.right() - 24.0, y),
            Align2::RIGHT_CENTER,
            &right,
            hifi::ui_font(12.5),
            right_c,
            r.width() * 0.55,
        );
        fit(
            p,
            Pos2::new(r.left() + 24.0, y),
            Align2::LEFT_CENTER,
            &left,
            hifi::ui_font(12.5),
            t.dim,
            rr.left() - r.left() - 48.0,
        );
        if self.notice.is_some() || self.touched.is_some() {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(1));
        }
    }

    // ------------------------------------------------------------ settings

    fn settings(&mut self, ctx: &egui::Context, t: &Tokens) {
        let mut open = self.settings_open;
        let mut look = self.s.finish;
        let mut system_bar = self.s.system_title_bar;
        let mut act = None;
        let db = self.db_path.display().to_string();
        hifi::drawer(ctx, t, "Settings", &mut open, |ui| {
            hifi::look_settings(ui, t, &mut look);

            hifi::drawer_heading(ui, t, "Library");
            for (id, label, what) in [
                ("s-add", "Add folder", "Scan a folder"),
                ("s-exclude", "Exclude…", "Leave a folder out"),
            ] {
                let r = hifi::drawer_row(ui, t, what, 110.0);
                if hifi::button(ui, r, Id::new(id), t, label, false).clicked() {
                    act = Some(id);
                }
            }
            ui.label(
                egui::RichText::new(format!("The library is kept in {db}"))
                    .font(hifi::ui_font(12.0))
                    .color(t.dim),
            );

            hifi::drawer_heading(ui, t, "Lookups");
            ui.label(
                egui::RichText::new(
                    "Off. Looking albums up online (MusicBrainz and cover art) comes in stage 2, and only when you turn it on. Until then nothing is sent online.",
                )
                .font(hifi::ui_font(12.5))
                .color(t.dim),
            );

            hifi::drawer_heading(ui, t, "Window");
            let r = hifi::drawer_row(ui, t, "Use the system title bar", 32.0);
            let r = Rect::from_min_size(r.min + Vec2::new(0.0, 4.0), Vec2::new(32.0, 18.0));
            if hifi::switch(ui, r, Id::new("system-bar"), t, system_bar).clicked() {
                system_bar = !system_bar;
            }
        });
        self.settings_open = open;
        self.s.finish = look;
        match act {
            Some("s-add") => self.add_folder(),
            Some("s-exclude") => self.add_exclude(),
            _ => {}
        }
        if system_bar != self.s.system_title_bar {
            self.s.system_title_bar = system_bar;
            ctx.send_viewport_cmd(ViewportCommand::Decorations(system_bar));
        }
    }

    fn fit_to_window(&self, ctx: &egui::Context) {
        let zoom = ctx.zoom_factor();
        let screen = ctx.content_rect().size() * zoom;
        let want = (screen.x / MIN_DESIGN.x)
            .min(screen.y / MIN_DESIGN.y)
            .clamp(0.5, 1.0);
        if (want - zoom).abs() > 0.002 {
            ctx.set_zoom_factor(want);
            ctx.request_repaint();
        }
    }
}

impl eframe::App for LibraryApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.fit_to_window(&ctx);
        let t = Tokens::from_ctx(&ctx, &self.s.finish);
        hifi::apply_visuals(&ctx, &t);
        self.poll(&ctx);

        let screen = ui.max_rect();
        hifi::faceplate(ui.painter(), screen, &t);
        self.plate(ui, screen, &t);
        let tools = Rect::from_min_size(
            Pos2::new(screen.left() + 24.0, screen.top() + PLATE_H + 4.0),
            Vec2::new(screen.width() - 48.0, 40.0),
        );
        self.toolbar(ui, tools, &t);
        let foot = Rect::from_min_max(
            Pos2::new(screen.left(), screen.bottom() - FOOT_H),
            screen.max,
        );
        self.footer(ui, foot, &t);
        ui.painter()
            .rect_stroke(screen, 0, Stroke::new(1.0, t.edge), StrokeKind::Inside);

        let body = Rect::from_min_max(
            Pos2::new(screen.left() + 24.0, tools.bottom() + 18.0),
            Pos2::new(screen.right() - 24.0, foot.top() - 16.0),
        );
        let side = Rect::from_min_size(body.min, Vec2::new(SIDE_W, body.height()));
        self.side(ui, side, &t);
        let main = Rect::from_min_max(Pos2::new(side.right() + 20.0, body.top()), body.max);
        match self.s.view {
            View::Library => self.library_view(ui, main, &t),
            View::Missing => self.missing_view(ui, main, &t),
            View::Dupes => self.later_view(ui, main, &t, "Dupes"),
            View::Organise => self.later_view(ui, main, &t, "Organise"),
        }
        self.scroll_to_selection = false;

        self.settings(&ctx, &t);
        if !self.s.system_title_bar {
            hifi::resize_edges(ui, screen);
        }
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, STORAGE_KEY, &self.s);
    }

    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        Tokens::new(&self.s.finish, false)
            .face_bottom
            .to_normalized_gamma_f32()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_from_the_jupiter_panel_still_load() {
        let s: Saved =
            serde_json::from_str(r#"{"panel": "Silver", "view": "Missing", "missing": 2}"#)
                .unwrap();
        assert_eq!(s.view, View::Missing);
        assert_eq!(s.missing, 2);
        assert!(!s.simple);
        assert_eq!(s.finish, hifi::Look::default());
    }
}
