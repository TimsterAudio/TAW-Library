//! The views chosen with the View keys: Library, What's Missing, and the
//! two that come in later stages.

use super::app::{AlbumKey, LibraryApp, View};
use super::look::*;
use crate::workers::ArtSource;
use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use mb_core::db::{LookupRow, LookupState};
use mb_core::library::{fmt_duration, Album, Art, Missing};
use mb_core::lookup::{self, Candidate, Field, Proposal};
use mb_core::volumes;
use taw_panel::hifi::{self, Tokens};
use taw_panel::table::{self, Col, Side};

/// The gap between panes, where a drag resizes them.
const GAP: f32 = 14.0;
const ARTISTS_W: f32 = 210.0;
const DETAIL_W: f32 = 390.0;
const DETAIL_MIN: f32 = 340.0;
/// The albums list keeps at least this much when the album pane grows.
/// (The columns' minimums, 58 + 60 + 140 + 110, and the well's edges.)
const ALBUMS_MIN: f32 = 372.0;
const TILES_W: f32 = 270.0;

/// The albums list: artwork, year, album (the flexible one), what it needs.
const ALBUM_COLS: [Col; 4] = [
    Col::new("art", "", 58.0).fixed(),
    Col::new("year", "Year", 62.0).right().min(60.0),
    Col::new("album", "Album", 260.0).flex().min(140.0),
    Col::new("needs", "Needs", 230.0).min(110.0),
];

/// Changes a lookup proposes.
const CHANGE_COLS: [Col; 4] = [
    Col::new("field", "Field", 104.0).min(70.0).always(),
    Col::new("which", "Track", 64.0).min(44.0),
    Col::new("now", "Now", 150.0).min(60.0),
    Col::new("new", "Proposed", 160.0).flex().min(90.0),
];

/// Releases a lookup offered.
const RELEASE_COLS: [Col; 5] = [
    Col::new("release", "Release", 200.0).flex().min(120.0),
    Col::new("year", "Year", 54.0).right().min(48.0),
    Col::new("tracks", "Tracks", 60.0).right().min(48.0),
    Col::new("format", "Format", 92.0).min(50.0),
    Col::new("score", "Score", 58.0).right().min(48.0),
];

/// What's missing: the albums with that gap.
const MISSING_COLS: [Col; 5] = [
    Col::new("artist", "Artist", 150.0).min(80.0),
    Col::new("album", "Album", 220.0).flex().min(140.0),
    Col::new("year", "Year", 62.0).right().min(60.0),
    Col::new("missing", "Missing", 190.0).min(90.0),
    Col::new("where", "Where it is", 200.0).min(100.0),
];

impl LibraryApp {
    // ------------------------------------------------------------ library

    pub fn library_view(&mut self, ui: &mut Ui, main: Rect, tk: &Tokens) {
        let (artists, rest) = table::pane(
            ui,
            main,
            "artists",
            Side::Left,
            self.s.layout.pane_mut("artists"),
            ARTISTS_W,
            // Room left for the albums list and the album pane at its narrowest.
            (
                160.0,
                360.0f32.min(main.width() - ALBUMS_MIN - DETAIL_MIN - 2.0 * GAP),
            ),
            GAP,
            tk,
        );
        let (detail, albums) = table::pane(
            ui,
            rest,
            "album",
            Side::Right,
            self.s.layout.pane_mut("album"),
            DETAIL_W,
            (DETAIL_MIN, rest.width() - ALBUMS_MIN - GAP),
            GAP,
            tk,
        );
        self.artists_list(ui, artists, tk);
        self.albums_list(ui, albums, tk);
        self.album_detail(ui, detail, tk);
    }

    fn artists_list(&mut self, ui: &mut Ui, r: Rect, tk: &Tokens) {
        heading(ui.painter(), r.min, "Artists", tk);
        let q = self.find.trim().to_lowercase();
        let list: Vec<usize> = (0..self.lib.artists.len())
            .filter(|&i| {
                let a = &self.lib.artists[i];
                q.is_empty()
                    || a.name.to_lowercase().contains(&q)
                    || a.albums
                        .iter()
                        .any(|&x| self.lib.albums[x].title.to_lowercase().contains(&q))
            })
            .collect();
        let inner = Rect::from_min_max(Pos2::new(r.left(), r.top() + 20.0), r.max);
        well(ui.painter(), inner, tk);
        if list.is_empty() {
            let msg = if self.lib.artists.is_empty() {
                "Nothing scanned yet"
            } else {
                "No artist matches"
            };
            text(
                ui.painter(),
                inner.left_top() + Vec2::new(10.0, 10.0),
                Align2::LEFT_TOP,
                msg,
                hifi::ui_font(12.5),
                tk.dim,
            );
        }
        let sel = self.s.artist.clone();
        let scroll = self
            .scroll_to_selection
            .then(|| {
                list.iter()
                    .position(|&i| Some(&self.lib.artists[i].name) == sel.as_ref())
            })
            .flatten();
        let mut clicked = None;
        rows(
            ui,
            inner,
            "artists",
            28.0,
            list.len(),
            scroll,
            |ui, i, row, resp| {
                let a = &self.lib.artists[list[i]];
                let p = ui.painter();
                let selected = Some(&a.name) == sel.as_ref();
                row_bg(p, row, resp, selected, tk);
                let mut right = row.right() - 12.0;
                light(
                    p,
                    Pos2::new(right - 2.0, row.center().y),
                    if a.incomplete == 0 {
                        Light::Ok
                    } else if a.incomplete == a.albums.len() {
                        Light::Bad
                    } else {
                        Light::Warn
                    },
                    tk,
                );
                right -= 14.0;
                let n = text(
                    p,
                    Pos2::new(right, row.center().y),
                    Align2::RIGHT_CENTER,
                    &a.albums.len().to_string(),
                    hifi::num(12.0),
                    tk.dim,
                );
                right = n.left() - 8.0;
                let mut name_w = right - row.left() - 12.0;
                let merge = a.merge_into.as_ref().filter(|_| !self.s.simple);
                if merge.is_some() {
                    name_w -= 56.0;
                }
                let nr = fit(
                    p,
                    Pos2::new(row.left() + 10.0, row.center().y),
                    Align2::LEFT_CENTER,
                    &a.name,
                    hifi::ui_font(13.5),
                    tk.text,
                    name_w,
                );
                if let Some(into) = merge {
                    chip(
                        p,
                        Pos2::new(nr.right() + 6.0, row.center().y),
                        "Merge?",
                        AMBER,
                        true,
                    );
                    if resp.hovered() {
                        resp.clone().on_hover_text(format!("Probably the same artist as \"{into}\". Merging comes with the organise plan."));
                    }
                }
                if resp.clicked() {
                    clicked = Some(list[i]);
                }
            },
        );
        if let Some(i) = clicked {
            let a = &self.lib.artists[i];
            self.s.artist = Some(a.name.clone());
            self.s.album = a.albums.first().map(|&x| AlbumKey::of(&self.lib.albums[x]));
            let msg = match &a.merge_into {
                Some(into) => (format!("Merge into {into}?"), "Later".to_string()),
                None => (a.name.clone(), format!("{} albums", a.albums.len())),
            };
            self.touch(msg.0, msg.1);
        }
    }

    fn albums_list(&mut self, ui: &mut Ui, r: Rect, tk: &Tokens) {
        let artist = self
            .s
            .artist
            .as_ref()
            .and_then(|n| self.lib.artists.iter().find(|a| &a.name == n))
            .cloned();
        let title = match &artist {
            Some(a) => format!("Albums · {}", a.name),
            None => "Albums".into(),
        };
        heading(ui.painter(), r.min, &title, tk);
        let inner = Rect::from_min_max(r.min + Vec2::new(0.0, 20.0), r.max);
        well(ui.painter(), inner, tk);
        let Some(artist) = artist else { return };
        let head = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 28.0));
        let cols = table::header(
            ui,
            head,
            "albums",
            &ALBUM_COLS,
            self.s.layout.list_mut("albums"),
            tk,
        );
        let list = Rect::from_min_max(Pos2::new(inner.left(), head.bottom() + 1.0), inner.max);
        let sel = self.s.album.clone();
        let scroll = self
            .scroll_to_selection
            .then(|| {
                artist
                    .albums
                    .iter()
                    .position(|&i| sel.as_ref().is_some_and(|k| k.matches(&self.lib.albums[i])))
            })
            .flatten();
        let mut clicked = None;
        let ctx = ui.ctx().clone();
        rows(
            ui,
            list,
            "albums",
            50.0,
            artist.albums.len(),
            scroll,
            |ui, i, row, resp| {
                let ai = artist.albums[i];
                let a = &self.lib.albums[ai].clone();
                let selected = sel.as_ref().is_some_and(|k| k.matches(a));
                row_bg(ui.painter(), row, resp, selected, tk);
                let y = row.center().y;
                if let Some(c) = cols.get("art") {
                    let thumb = Rect::from_min_size(Pos2::new(c.x(), y - 19.0), Vec2::splat(38.0));
                    let tex = art_texture(self, &ctx, a, 96);
                    draw_art(ui.painter(), thumb, tex.as_ref(), &a.art, 9.0, tk);
                }
                let p = ui.painter();
                match a.year {
                    None if a.missing.contains(&Missing::Year) => {
                        cols.text(ui, p, "year", y, "----", hifi::num(13.0), BAD)
                    }
                    Some(yr) => {
                        cols.text(ui, p, "year", y, &yr.to_string(), hifi::num(13.0), tk.text)
                    }
                    None => None,
                };
                if let Some(c) = cols.get("album") {
                    let title_c = if a.untitled { tk.dim } else { tk.text };
                    let tracks = format!(
                        "{} track{} · {} · {}",
                        a.tracks.len(),
                        if a.tracks.len() == 1 { "" } else { "s" },
                        fmt_duration(a.duration_ms),
                        a.formats
                            .iter()
                            .take(2)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(" + ")
                    );
                    fit(
                        p,
                        Pos2::new(c.x(), y - 1.0),
                        Align2::LEFT_BOTTOM,
                        &a.title,
                        hifi::ui_font(13.5),
                        title_c,
                        c.width(),
                    );
                    let natural = |s: &str, f| p.layout_no_wrap(s.to_string(), f, tk.text).size().x;
                    cols.note(
                        ui,
                        "album",
                        natural(&a.title, hifi::ui_font(13.5))
                            .max(natural(&tracks, hifi::ui_font(11.5))),
                    );
                    fit(
                        p,
                        Pos2::new(c.x(), y + 2.0),
                        Align2::LEFT_TOP,
                        &tracks,
                        hifi::ui_font(11.5),
                        tk.dim,
                        c.width(),
                    );
                }
                if let Some(c) = cols.get("needs") {
                    let p = &in_column(p, row, &c);
                    let x0 = c.x();
                    let end = c.right - table::PAD;
                    let chips: Vec<(&str, Color32)> = if a.missing.is_empty() {
                        vec![("Complete", tk.ok)]
                    } else if self.s.simple {
                        vec![("Needs work", AMBER)]
                    } else {
                        a.missing
                            .iter()
                            .map(|m| (m.chip(), if m.is_warning() { AMBER } else { BAD }))
                            .collect()
                    };
                    cols.note(
                        ui,
                        "needs",
                        chips.iter().map(|(s, _)| chip_width(s) + 4.0).sum::<f32>() - 4.0,
                    );
                    // Up to two lines of chips.
                    let two = chips.len() > 1;
                    let line_y = |l: i32| if two { y - 10.5 + l as f32 * 21.0 } else { y };
                    let (mut x, mut line) = (x0, 0);
                    for (s, colour) in &chips {
                        let w = chip_width(s);
                        if x + w > end {
                            if line == 1 || x == x0 {
                                text(
                                    p,
                                    Pos2::new(x, line_y(line)),
                                    Align2::LEFT_CENTER,
                                    "…",
                                    hifi::semi(13.0),
                                    BAD,
                                );
                                break;
                            }
                            line = 1;
                            x = x0;
                        }
                        x += chip(p, Pos2::new(x, line_y(line)), s, *colour, false) + 4.0;
                    }
                }
                if resp.clicked() {
                    clicked = Some(ai);
                }
            },
        );
        if let Some(ai) = clicked {
            let a = &self.lib.albums[ai];
            self.s.album = Some(AlbumKey::of(a));
            self.touch(a.artist.clone(), a.title.clone());
        }
    }

    fn selected_album(&self) -> Option<usize> {
        let k = self.s.album.as_ref()?;
        let artist = self
            .s
            .artist
            .as_ref()
            .and_then(|n| self.lib.artists.iter().find(|a| &a.name == n))?;
        artist
            .albums
            .iter()
            .copied()
            .find(|&i| k.matches(&self.lib.albums[i]))
    }

    fn album_detail(&mut self, ui: &mut Ui, r: Rect, tk: &Tokens) {
        heading(ui.painter(), r.min, "Album", tk);
        let Some(ai) = self.selected_album() else {
            text(
                ui.painter(),
                r.left_top() + Vec2::new(14.0, 24.0),
                Align2::LEFT_TOP,
                "Choose an album.",
                hifi::ui_font(13.0),
                tk.dim,
            );
            return;
        };
        let a = self.lib.albums[ai].clone();
        let ctx = ui.ctx().clone();
        let tex = art_texture(self, &ctx, &a, 320);
        let art_r = Rect::from_min_size(r.min + Vec2::new(0.0, 22.0), Vec2::splat(150.0));
        let p = ui.painter();
        draw_art(p, art_r, tex.as_ref(), &a.art, 13.0, tk);
        if let Some((w, h)) = a.art.size() {
            let s = format!("{w} × {h}");
            let g = p.layout_no_wrap(s.clone(), hifi::num(10.5), Color32::WHITE);
            let br = Rect::from_min_size(
                art_r.left_bottom() + Vec2::new(6.0, -6.0 - g.size().y - 2.0),
                g.size() + Vec2::new(8.0, 2.0),
            );
            p.rect_filled(br, CornerRadius::same(4), Color32::from_black_alpha(150));
            p.galley(br.min + Vec2::new(4.0, 1.0), g, Color32::WHITE);
        }

        // Who, what, and how complete it is.
        let ts: Vec<mb_core::library::Track> = a
            .tracks
            .iter()
            .map(|&i| self.lib.tracks[i].clone())
            .collect();
        let first = &ts[0];
        let lossless = first.lossless();
        let fmt = {
            let detail = if lossless {
                match (first.bit_depth, first.sample_rate) {
                    (Some(b), Some(sr)) => format!(" {b}/{}", sr as f32 / 1000.0),
                    _ => String::new(),
                }
            } else {
                first.bitrate.map_or(String::new(), |b| format!(" {b}k"))
            };
            if a.formats.len() > 1 {
                a.formats.iter().cloned().collect::<Vec<_>>().join(" + ")
            } else {
                format!("{}{detail}", first.format)
            }
        };
        let discs = ts.iter().filter_map(|t| t.disc).max();
        let simple = self.s.simple;
        let fx = art_r.right() + 18.0;
        let fw = r.right() - fx;
        let p = ui.painter();
        fit(
            p,
            Pos2::new(fx, art_r.top() + 2.0),
            Align2::LEFT_TOP,
            &format!("{} · album", a.artist),
            hifi::ui_font(12.0),
            tk.dim,
            fw,
        );
        fit(
            p,
            Pos2::new(fx, art_r.top() + 20.0),
            Align2::LEFT_TOP,
            &a.title,
            hifi::bold(22.0),
            if a.untitled { tk.dim } else { tk.text },
            fw,
        );
        let mut facts = Vec::new();
        if let Some(y) = a.year {
            facts.push(y.to_string());
        }
        facts.push(format!(
            "{} track{}",
            a.tracks.len(),
            if a.tracks.len() == 1 { "" } else { "s" }
        ));
        facts.push(fmt_duration(a.duration_ms));
        if let Some(d) = discs.filter(|&d| d > 1) {
            facts.push(format!("{d} discs"));
        }
        fit(
            p,
            Pos2::new(fx, art_r.top() + 52.0),
            Align2::LEFT_TOP,
            &facts.join(" · "),
            hifi::ui_font(13.5),
            tk.dim,
            fw,
        );
        // Tags: the format, then what's missing (or that nothing is).
        let mut cx = fx;
        let mut cy = art_r.top() + 88.0;
        let mut chips: Vec<(String, Color32)> = Vec::new();
        if simple {
            chips.push(if a.missing.is_empty() {
                ("Complete".into(), tk.ok)
            } else {
                ("Needs a little work".into(), AMBER)
            });
        } else {
            chips.push((fmt.clone(), if lossless { tk.ok } else { tk.dim }));
            if a.missing.is_empty() {
                chips.push(("Tags complete".into(), tk.ok));
            }
            for m in &a.missing {
                chips.push((
                    m.title().to_string(),
                    if m.is_warning() { AMBER } else { BAD },
                ));
            }
        }
        for (s, c) in &chips {
            let w = chip_width(s);
            if cx + w > r.right() && cx > fx {
                cx = fx;
                cy += 24.0;
            }
            if cy > art_r.bottom() - 46.0 {
                break;
            }
            cx += chip(p, Pos2::new(cx, cy), s, *c, false) + 6.0;
        }

        // Play, and what else can be done with it.
        let folder = self.full_path(&a.volume, &a.dir);
        let by = art_r.bottom() - 30.0;
        let mut bx = fx;
        // (id, label, a shorter label for a narrow pane, primary)
        let mut buttons = vec![
            ("play", "Play in Player", "Play", true),
            ("open-folder", "Show folder", "Folder", false),
        ];
        let key = AlbumKey::of(&a);
        let queued = self
            .lookups
            .as_ref()
            .is_some_and(|l| l.queued.contains(&key.as_ref()));
        if !simple {
            // Next to Play, so it's there in the narrowest album pane.
            buttons.insert(
                1,
                (
                    "lookup",
                    if queued { "Looking up…" } else { "Look up" },
                    if queued { "Looking…" } else { "Look up" },
                    false,
                ),
            );
            buttons.push(("edit", "Edit", "Edit", false));
        }
        // Long labels while Play, Look up and Show folder all fit; short
        // ones (Play, Folder) when they don't.
        let long_w: f32 = buttons
            .iter()
            .take(3)
            .map(|b| hifi::button_width(ui, b.1) + 8.0)
            .sum();
        let short = bx + long_w > r.right() + 9.0;
        for (id, long, short_label, primary) in buttons {
            let label = if short { short_label } else { long };
            let w = hifi::button_width(ui, label);
            if bx + w > r.right() + 1.0 {
                break;
            }
            let br = Rect::from_min_size(Pos2::new(bx, by), Vec2::new(w, 30.0));
            if hifi::button(ui, br, Id::new(id), tk, label, primary).clicked() {
                match id {
                    "play" => self.play(&a),
                    "open-folder" => match &folder {
                        Some(f) => {
                            open_path(f);
                            self.touch("Open folder", a.title.clone());
                        }
                        None => self.tell("That album's drive isn't connected."),
                    },
                    "lookup" if !queued => self.look_up(ai, None),
                    "lookup" => {}
                    _ => self.tell("Editing tags comes in stage 3, with an undo log."),
                }
            }
            bx = br.right() + 8.0;
        }
        let ky = art_r.bottom() - 30.0;

        // Where it is
        let p = ui.painter();
        let vol = self.volume(&a.volume);
        let where_ = match (vol, &folder) {
            (_, Some(f)) => f.display().to_string(),
            (Some(v), None) => format!("{} (offline) › {}", v.label, a.dir),
            (None, None) => a.dir.clone(),
        };
        let g = p.layout(where_, hifi::ui_font(11.5), tk.dim, r.width());
        let wy = ky + 46.0;
        let wh = g.size().y.min(32.0);
        p.with_clip_rect(Rect::from_min_size(
            Pos2::new(r.left(), wy),
            Vec2::new(r.width(), wh),
        ))
        .galley(Pos2::new(r.left(), wy), g, tk.dim);

        // Tracks, and (once it's been looked up) the proposed changes and
        // the releases MusicBrainz offered.
        let (lk, props) = self.album_lookup(&key);
        let mut top = wy + wh + 10.0;
        let tab = if !simple && (lk.is_some() || queued) {
            let n_rel = lk.as_ref().map_or(0, |l| l.candidates.len());
            let labels = [
                "Tracks".to_string(),
                format!("Changes · {}", props.len()),
                format!("Releases · {n_rel}"),
            ];
            let items: Vec<&str> = labels.iter().map(String::as_str).collect();
            let (seg, picked) = hifi::segmented(
                ui,
                Pos2::new(r.left(), top + 14.0),
                false,
                Id::new("album-tab"),
                tk,
                &items,
                self.album_tab.min(2),
                hifi::Seg::Plain,
                &[],
                &[],
            );
            if let Some(i) = picked {
                self.album_tab = i;
            }
            let status = lookup_status(lk.as_ref(), queued);
            fit(
                ui.painter(),
                Pos2::new(seg.right() + 12.0, seg.center().y),
                Align2::LEFT_CENTER,
                &status.0,
                hifi::ui_font(12.0),
                if status.1 { BAD } else { tk.dim },
                r.right() - seg.right() - 12.0,
            );
            top = seg.bottom() + 8.0;
            self.album_tab.min(2)
        } else {
            0
        };
        let inner = Rect::from_min_max(Pos2::new(r.left(), top), r.max);
        match tab {
            1 => return self.changes_list(ui, inner, &ts, &props, lk.as_ref(), tk),
            2 => return self.releases_list(ui, inner, ai, lk.as_ref(), tk),
            _ => {}
        }
        well(ui.painter(), inner, tk);
        let head = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 26.0));
        let mixed = a.formats.len() > 1;
        let mut track_cols = vec![
            Col::new("n", "#", 54.0).right().min(40.0).always(),
            Col::new("title", "Title", 200.0).flex().min(120.0),
        ];
        if mixed {
            track_cols.push(Col::new("format", "Format", 76.0).min(56.0));
        }
        track_cols.push(Col::new("time", "Time", 64.0).right().min(52.0));
        let cols = table::header(
            ui,
            head,
            "tracks",
            &track_cols,
            self.s.layout.list_mut("tracks"),
            tk,
        );
        let list = Rect::from_min_max(Pos2::new(inner.left(), head.bottom() + 1.0), inner.max);
        let multi_disc = discs.unwrap_or(1) > 1;
        rows(
            ui,
            list,
            "tracks",
            25.0,
            ts.len(),
            None,
            |ui, i, row, resp| {
                let t = &ts[i];
                let p = ui.painter();
                row_bg(p, row, resp, false, tk);
                let y = row.center().y;
                let num = match (t.track_no, multi_disc) {
                    (Some(n), true) => format!("{}.{n:02}", t.disc.unwrap_or(1)),
                    (Some(n), false) => format!("{n:02}"),
                    (None, _) => "--".into(),
                };
                let ok = |b: bool| if b { tk.text } else { BAD };
                cols.text(
                    ui,
                    p,
                    "n",
                    y,
                    &num,
                    hifi::num(12.5),
                    ok(t.track_no.is_some()),
                );
                let title = t.title.clone().unwrap_or_else(|| t.file_name().to_string());
                let title_c = if t.title.is_some() { tk.text } else { tk.dim };
                cols.text(ui, p, "title", y, &title, hifi::ui_font(13.0), title_c);
                let time = fmt_duration(t.duration_ms);
                cols.text(ui, p, "time", y, &time, hifi::num(12.5), tk.text);
                if let Some(c) = cols.get("format") {
                    cols.note(ui, "format", chip_width(&t.format));
                    let colour = if t.lossless() { tk.ok } else { tk.dim };
                    chip(p, Pos2::new(c.x(), y), &t.format, colour, false);
                }
                if let Some(e) = &t.error {
                    resp.clone()
                        .on_hover_text(format!("Couldn't read this file's tags: {e}"));
                }
            },
        );
    }

    /// Proposed changes: album-wide ones once with how many tracks they're
    /// for, then each track's.
    fn changes_list(
        &mut self,
        ui: &mut Ui,
        inner: Rect,
        ts: &[mb_core::library::Track],
        props: &[Proposal],
        lk: Option<&LookupRow>,
        tk: &Tokens,
    ) {
        well(ui.painter(), inner, tk);
        let note_h = 28.0;
        let head = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 26.0));
        let cols = table::header(
            ui,
            head,
            "changes",
            &CHANGE_COLS,
            self.s.layout.list_mut("changes"),
            tk,
        );
        let rows_r = Rect::from_min_max(
            Pos2::new(inner.left(), head.bottom() + 1.0),
            Pos2::new(inner.right(), inner.bottom() - note_h),
        );
        let track_no = |rel: &str| {
            ts.iter()
                .find(|t| t.rel_path == rel)
                .map_or("?".to_string(), |t| match t.track_no {
                    Some(n) => format!("{n:02}"),
                    None => t.file_name().to_string(),
                })
        };
        // (field, which tracks, now, proposed)
        let mut lines: Vec<(Field, String, Option<String>, String)> = Vec::new();
        let mut wide: Vec<(Field, Option<String>, String, usize)> = Vec::new();
        for p in props.iter().filter(|p| p.field.album_wide()) {
            match wide
                .iter_mut()
                .find(|w| w.0 == p.field && w.1 == p.current && w.2 == p.proposed)
            {
                Some(w) => w.3 += 1,
                None => wide.push((p.field, p.current.clone(), p.proposed.clone(), 1)),
            }
        }
        wide.sort_by_key(|w| w.0);
        for (f, now, new, n) in wide {
            let which = if f == Field::Artwork {
                "album".to_string()
            } else if n == ts.len() {
                "all".to_string()
            } else {
                format!("{n} tracks")
            };
            let new = if f == Field::Artwork {
                "Front cover from the Cover Art Archive".to_string()
            } else {
                new
            };
            lines.push((f, which, now, new));
        }
        for p in props.iter().filter(|p| !p.field.album_wide()) {
            let which = p.rel_path.as_deref().map_or("album".into(), track_no);
            lines.push((p.field, which, p.current.clone(), p.proposed.clone()));
        }
        if lines.is_empty() {
            let msg = match lk {
                Some(l) => match l.state {
                    LookupState::Matched => "Your tags already match this release.".to_string(),
                    LookupState::Unsure => {
                        "Not sure which release this is: pick one under Releases.".to_string()
                    }
                    LookupState::NotFound => "MusicBrainz has nothing like this album.".to_string(),
                    LookupState::Failed => format!("It couldn't be looked up. {}", l.note),
                },
                None => "Looking it up…".to_string(),
            };
            let p = ui.painter();
            let g = p.layout(msg, hifi::ui_font(13.0), tk.dim, rows_r.width() - 24.0);
            p.galley(rows_r.left_top() + Vec2::new(12.0, 12.0), g, tk.dim);
        }
        rows(
            ui,
            rows_r,
            "changes",
            25.0,
            lines.len(),
            None,
            |ui, i, row, resp| {
                let (f, which, now, new) = &lines[i];
                let p = ui.painter();
                row_bg(p, row, resp, false, tk);
                let y = row.center().y;
                cols.text(ui, p, "field", y, f.label(), hifi::semi(12.0), tk.text);
                cols.text(ui, p, "which", y, which, hifi::num(12.0), tk.dim);
                match now {
                    Some(v) => cols.text(ui, p, "now", y, v, hifi::ui_font(12.5), tk.dim),
                    None => cols.text(ui, p, "now", y, "(none)", hifi::ui_font(12.5), BAD),
                };
                cols.text(ui, p, "new", y, new, hifi::ui_font(12.5), tk.ok);
            },
        );
        let p = ui.painter();
        p.line_segment(
            [
                Pos2::new(inner.left() + 1.0, inner.bottom() - note_h),
                Pos2::new(inner.right() - 1.0, inner.bottom() - note_h),
            ],
            Stroke::new(1.0, tk.line_soft),
        );
        fit(
            p,
            Pos2::new(inner.left() + 12.0, inner.bottom() - note_h / 2.0),
            Align2::LEFT_CENTER,
            "Proposed only: nothing is written to your files. Applying comes in stage 3, with an undo log.",
            hifi::ui_font(11.5),
            tk.dim,
            inner.width() - 24.0,
        );
    }

    /// The releases a lookup offered; click one to use it.
    fn releases_list(
        &mut self,
        ui: &mut Ui,
        inner: Rect,
        ai: usize,
        lk: Option<&LookupRow>,
        tk: &Tokens,
    ) {
        well(ui.painter(), inner, tk);
        let head = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 26.0));
        let cols = table::header(
            ui,
            head,
            "releases",
            &RELEASE_COLS,
            self.s.layout.list_mut("releases"),
            tk,
        );
        let list = Rect::from_min_max(Pos2::new(inner.left(), head.bottom() + 1.0), inner.max);
        let cands: Vec<Candidate> = lk.map(|l| l.candidates.clone()).unwrap_or_default();
        let chosen = lk.and_then(|l| l.mbid.clone());
        if cands.is_empty() {
            text(
                ui.painter(),
                list.left_top() + Vec2::new(12.0, 12.0),
                Align2::LEFT_TOP,
                "No releases.",
                hifi::ui_font(13.0),
                tk.dim,
            );
        }
        let tracks = self.lib.albums[ai].tracks.len();
        let mut picked = None;
        rows(
            ui,
            list,
            "releases",
            25.0,
            cands.len(),
            None,
            |ui, i, row, resp| {
                let c = &cands[i];
                let p = ui.painter();
                row_bg(p, row, resp, chosen.as_ref() == Some(&c.mbid), tk);
                let y = row.center().y;
                let name = format!("{} · {}", c.title, c.artist);
                cols.text(ui, p, "release", y, &name, hifi::ui_font(12.5), tk.text);
                if let Some(yr) = c.year {
                    cols.text(ui, p, "year", y, &yr.to_string(), hifi::num(12.0), tk.text);
                }
                let tc = if c.tracks == tracks { tk.text } else { BAD };
                cols.text(
                    ui,
                    p,
                    "tracks",
                    y,
                    &c.tracks.to_string(),
                    hifi::num(12.0),
                    tc,
                );
                let fmt: Vec<&str> = [c.format.as_deref(), c.country.as_deref()]
                    .into_iter()
                    .flatten()
                    .collect();
                cols.text(
                    ui,
                    p,
                    "format",
                    y,
                    &fmt.join(" · "),
                    hifi::ui_font(12.0),
                    tk.dim,
                );
                cols.text(
                    ui,
                    p,
                    "score",
                    y,
                    &format!("{}%", c.score),
                    hifi::num(12.0),
                    tk.dim,
                );
                let resp = resp
                .clone()
                .on_hover_text("Use this release: its tracks are compared with yours and the differences proposed.");
                if resp.clicked() {
                    picked = Some(c.mbid.clone());
                }
            },
        );
        if let Some(m) = picked {
            if chosen.as_ref() != Some(&m) {
                self.look_up(ai, Some((m, cands)));
                self.album_tab = 1;
            }
        }
    }

    /// Queues an album for a lookup (Lookups must be on). `chosen` is a
    /// release the user picked, with the releases it was picked from.
    pub fn look_up(&mut self, ai: usize, chosen: Option<(String, Vec<Candidate>)>) {
        if !self.s.lookups || self.lookups.is_none() {
            self.tell("Turn Lookups on first (along the top, or in Settings). Until then nothing is sent online.");
            return;
        }
        let a = &self.lib.albums[ai];
        let have = a
            .tracks
            .iter()
            .map(|&i| {
                let t = &self.lib.tracks[i];
                lookup::Have {
                    rel_path: t.rel_path.clone(),
                    file_name: t.file_name().to_string(),
                    title: t.title.clone(),
                    artist: t.artist.clone(),
                    album_artist: t.album_artist.clone(),
                    album: t.album.clone(),
                    year: t.year,
                    track_no: t.track_no,
                    disc_no: t.disc_no,
                    duration_ms: t.duration_ms,
                }
            })
            .collect();
        let job = lookup::Job {
            album: AlbumKey::of(a).as_ref(),
            query: lookup::Query {
                artist: a.artist.clone(),
                album: a.title.clone(),
                tracks: a.tracks.len(),
                year: a.year,
            },
            have,
            art_now: a.art.size().map(|(w, h)| format!("{w} × {h}")),
            want_art: a.missing.contains(&Missing::Art) || a.missing.contains(&Missing::SmallArt),
            chosen,
        };
        self.touch("Look up", a.title.clone());
        if let Some(run) = &mut self.lookups {
            run.send(job);
        }
        self.shown_lookup = None;
    }

    /// Plays an album in the computer's default player, through a playlist
    /// file. When the Player can take a playlist from the Library, this goes there.
    fn play(&mut self, a: &Album) {
        let mut m3u = String::from("#EXTM3U\n");
        let mut n = 0;
        for &i in &a.tracks {
            let t = &self.lib.tracks[i];
            let Some(path) = self.full_path(&t.volume, &t.rel_path) else {
                continue;
            };
            let title = t.title.clone().unwrap_or_else(|| t.file_name().to_string());
            let artist = t.artist.clone().unwrap_or_default();
            m3u += &format!(
                "#EXTINF:{},{} - {}\n{}\n",
                t.duration_ms / 1000,
                artist,
                title,
                path.display()
            );
            n += 1;
        }
        if n == 0 {
            self.tell("That album's drive isn't connected.");
            return;
        }
        let dir = std::env::temp_dir().join("taw-library");
        let _ = std::fs::create_dir_all(&dir);
        let safe: String = format!("{} - {}", a.artist, a.title)
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || " -_.()".contains(c) {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        let file = dir.join(format!("{safe}.m3u8"));
        match std::fs::write(&file, m3u) {
            Ok(()) => {
                self.touch("Play", a.title.clone());
                let started = find_app("taw-player").map(|exe| {
                    crate::instance::start(
                        std::process::Command::new(&exe)
                            .arg("--play")
                            .arg(&file)
                            .stdin(std::process::Stdio::null())
                            .stdout(std::process::Stdio::null())
                            .stderr(std::process::Stdio::null()),
                    )
                    .is_ok()
                });
                match started {
                    Some(true) => self.tell("Playing in the TAW Player."),
                    Some(false) => {
                        open_path(&file);
                        self.tell("The TAW Player wouldn't start, so it's playing in your default music player.");
                    }
                    None => {
                        open_path(&file);
                        self.tell("Playing in your default music player (the TAW Player isn't next to the Library or on the PATH).");
                    }
                }
            }
            Err(e) => self.tell(format!("Couldn't make the playlist: {e}")),
        }
    }

    // ------------------------------------------------------------ missing

    pub fn missing_view(&mut self, ui: &mut Ui, main: Rect, tk: &Tokens) {
        let (tiles, lr) = table::pane(
            ui,
            main,
            "missing-tiles",
            Side::Left,
            self.s.layout.pane_mut("missing-tiles"),
            TILES_W,
            (230.0, 380.0),
            GAP,
            tk,
        );
        heading(ui.painter(), tiles.min, "What's missing", tk);
        let cur = Missing::ALL[self.s.missing.min(Missing::ALL.len() - 1)];
        let mut y = tiles.top() + 18.0;
        for (i, m) in Missing::ALL.iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(tiles.left() + 10.0, y),
                Vec2::new(tiles.width() - 20.0, 58.0),
            );
            let resp = ui.interact(r, Id::new(("tile", i)), Sense::click());
            let p = ui.painter();
            p.rect_filled(r.expand(1.0), CornerRadius::same(hifi::RADIUS + 1), tk.line);
            p.rect_filled(r, CornerRadius::same(hifi::RADIUS), tk.well);
            if *m == cur {
                p.rect_stroke(
                    r,
                    CornerRadius::same(hifi::RADIUS),
                    Stroke::new(1.5, tk.accent),
                    StrokeKind::Inside,
                );
            } else if resp.hovered() {
                p.rect_stroke(
                    r,
                    CornerRadius::same(hifi::RADIUS),
                    Stroke::new(1.0, tk.line),
                    StrokeKind::Inside,
                );
            }
            let albums = self.lib.missing_albums.get(m).map_or(0, Vec::len);
            text(
                p,
                r.left_top() + Vec2::new(10.0, 10.0),
                Align2::LEFT_TOP,
                &m.title().to_uppercase(),
                hifi::num(12.0),
                tk.text,
            );
            text(
                p,
                Pos2::new(r.right() - 10.0, r.top() + 8.0),
                Align2::RIGHT_TOP,
                &thousands(albums as u64),
                hifi::num(22.0),
                tk.accent,
            );
            let unit = match self.lib.missing_tracks.get(m) {
                Some(n) => format!("ALBUMS · {} TRACKS", thousands(*n as u64)),
                None => "ALBUMS".into(),
            };
            text(
                p,
                r.left_top() + Vec2::new(10.0, 34.0),
                Align2::LEFT_TOP,
                &unit,
                hifi::num(11.0),
                tk.dim,
            );
            if resp
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                self.s.missing = i;
                self.touch(m.title(), format!("{} albums", thousands(albums as u64)));
            }
            y += 66.0;
        }
        let p = ui.painter();
        let note = "The fields the Library checks: artist, album artist, album, year, disc and track number, title, time and artwork. Every other tag in your files is kept as it is.";
        let g = p.layout(
            note.into(),
            hifi::ui_font(11.5),
            tk.dim,
            tiles.width() - 28.0,
        );
        if y + g.size().y < tiles.bottom() - 10.0 {
            p.galley(Pos2::new(tiles.left() + 14.0, y + 6.0), g, tk.dim);
        }

        heading(
            ui.painter(),
            lr.min,
            &format!("Albums · {}", cur.title()),
            tk,
        );
        let inner = Rect::from_min_max(lr.min + Vec2::new(0.0, 20.0), lr.max);
        well(ui.painter(), inner, tk);
        let head = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 28.0));
        let cols = table::header(
            ui,
            head,
            "missing",
            &MISSING_COLS,
            self.s.layout.list_mut("missing"),
            tk,
        );
        let list = Rect::from_min_max(Pos2::new(inner.left(), head.bottom() + 1.0), inner.max);
        let items: Vec<usize> = self
            .lib
            .missing_albums
            .get(&cur)
            .cloned()
            .unwrap_or_default();
        // Look the whole list up (Detailed, and it goes online only with
        // Lookups on).
        if !self.s.simple && !items.is_empty() {
            let label = format!("Look up all {}", thousands(items.len() as u64));
            let w = hifi::button_width(ui, &label);
            let br = Rect::from_min_size(
                Pos2::new(lr.right() - w, lr.top() - 6.0),
                Vec2::new(w, 24.0),
            );
            if hifi::button(ui, br, Id::new("look-up-all"), tk, &label, false).clicked() {
                if self.s.lookups {
                    for &ai in &items {
                        self.look_up(ai, None);
                    }
                    self.tell(format!(
                        "Looking up {} albums, one request a second. Changes are proposed, not made.",
                        items.len()
                    ));
                } else {
                    self.look_up(items[0], None); // says to turn Lookups on
                }
            }
        }
        if items.is_empty() {
            let msg = if self.lib.tracks.is_empty() {
                "Nothing scanned yet. Press Scan."
            } else {
                "None. Nothing to do here."
            };
            text(
                ui.painter(),
                list.left_top() + Vec2::new(12.0, 12.0),
                Align2::LEFT_TOP,
                msg,
                hifi::ui_font(13.0),
                tk.dim,
            );
        }
        let mut clicked = None;
        rows(
            ui,
            list,
            "missing",
            30.0,
            items.len(),
            None,
            |ui, i, row, resp| {
                let a = &self.lib.albums[items[i]];
                let p = ui.painter();
                row_bg(p, row, resp, false, tk);
                let y = row.center().y;
                cols.text(ui, p, "artist", y, &a.artist, hifi::semi(13.0), tk.text);
                cols.text(ui, p, "album", y, &a.title, hifi::ui_font(13.0), tk.text);
                match a.year {
                    Some(yr) => {
                        cols.text(ui, p, "year", y, &yr.to_string(), hifi::num(12.5), tk.text)
                    }
                    None => cols.text(ui, p, "year", y, "----", hifi::num(12.5), BAD),
                };
                if let Some(c) = cols.get("missing") {
                    let p = &in_column(p, row, &c);
                    let mut x = c.x();
                    let end = c.right - table::PAD;
                    let widths: Vec<f32> = a.missing.iter().map(|m| chip_width(m.chip())).collect();
                    cols.note(
                        ui,
                        "missing",
                        widths.iter().map(|w| w + 4.0).sum::<f32>() - 4.0,
                    );
                    for (m, w) in a.missing.iter().zip(widths) {
                        if x + w > end {
                            text(
                                p,
                                Pos2::new(x, y),
                                Align2::LEFT_CENTER,
                                "…",
                                hifi::semi(13.0),
                                BAD,
                            );
                            break;
                        }
                        x += chip(
                            p,
                            Pos2::new(x, y),
                            m.chip(),
                            if m.is_warning() { AMBER } else { BAD },
                            false,
                        ) + 4.0;
                    }
                }
                let label = self
                    .volumes
                    .iter()
                    .find(|v| v.id == a.volume)
                    .map_or("?", |v| v.label.as_str());
                let where_ = format!("{label} › {}", a.dir);
                cols.text(ui, p, "where", y, &where_, hifi::ui_font(12.0), tk.dim);
                if resp.clicked() {
                    clicked = Some(items[i]);
                }
            },
        );
        if let Some(ai) = clicked {
            let a = &self.lib.albums[ai];
            let artist = self
                .lib
                .artists
                .iter()
                .find(|x| x.albums.contains(&ai))
                .map(|x| x.name.clone());
            self.s.artist = artist;
            self.s.album = Some(AlbumKey::of(a));
            self.s.view = View::Library;
            self.scroll_to_selection = true;
            self.touch(a.artist.clone(), a.title.clone());
        }
    }

    // ------------------------------------------------------------ later

    pub fn later_view(&mut self, ui: &mut Ui, main: Rect, tk: &Tokens, which: &str) {
        heading(ui.painter(), main.min, which, tk);
        let (title, body) = if which == "Dupes" {
            (
                "Duplicates come in stage 3",
                "The Library will find copies at three levels: identical files, the same audio with different tags, and the same recording in another format (by its audio fingerprint). It will suggest which copy to keep (lossless first, then the higher bitrate, then the more complete tags). Copies you don't keep go to a Review folder. Nothing is deleted.",
            )
        } else {
            (
                "The organise plan comes in stage 4",
                "The Library will show your folders now, the plan, and the new Music folder side by side: {Album Artist}\\{Year} - {Album}\\{Disc}{Track} {Title}. Copy is the default and leaves your originals where they are. Each copy is checked against its original, and every step is logged so it can be undone. Nothing changes until you press Apply.",
            )
        };
        let p = ui.painter();
        let c = main.center() - Vec2::new(0.0, 60.0);
        text(
            p,
            c,
            Align2::CENTER_BOTTOM,
            &title.to_uppercase(),
            hifi::num(18.0),
            tk.accent,
        );
        let g = p.layout(body.into(), hifi::ui_font(14.0), tk.dim, 620.0);
        p.galley(Pos2::new(c.x - g.size().x / 2.0, c.y + 18.0), g, tk.dim);
    }
}

/// The line beside the album's tabs: what the lookup found, and whether
/// it's bad news.
fn lookup_status(lk: Option<&LookupRow>, queued: bool) -> (String, bool) {
    if queued {
        return ("Looking it up on MusicBrainz…".into(), false);
    }
    let Some(l) = lk else {
        return (String::new(), false);
    };
    match l.state {
        LookupState::Matched => {
            let c = l
                .candidates
                .iter()
                .find(|c| Some(&c.mbid) == l.mbid.as_ref());
            let mut s = match c {
                Some(c) => {
                    let mut s = format!("MusicBrainz: {} · {}", c.title, c.artist);
                    if let Some(y) = c.year {
                        s += &format!(" · {y}");
                    }
                    if let Some(ct) = &c.country {
                        s += &format!(" · {ct}");
                    }
                    s
                }
                None => "MusicBrainz: the release you picked".into(),
            };
            if !l.note.is_empty() {
                s += &format!(". {}", l.note);
            }
            (s, false)
        }
        LookupState::Unsure => ("Not sure which release this is: pick one.".into(), false),
        LookupState::NotFound => ("MusicBrainz has nothing like it.".into(), true),
        LookupState::Failed => (format!("Couldn't look it up. {}", l.note), true),
    }
}

/// A painter that keeps what's drawn in a row inside one column.
fn in_column(p: &egui::Painter, row: Rect, c: &table::Span) -> egui::Painter {
    let cell = Rect::from_x_y_ranges(c.left..=c.right - table::PAD / 2.0, row.y_range());
    p.with_clip_rect(cell.intersect(p.clip_rect()))
}

/// The album's artwork as a texture, asking the loader for it if needed.
fn art_texture(
    app: &mut LibraryApp,
    ctx: &egui::Context,
    a: &Album,
    size: u32,
) -> Option<egui::TextureHandle> {
    if matches!(a.art, Art::None) {
        return None;
    }
    let key = AlbumKey::of(a).cache_key();
    let src = || match &a.art {
        Art::None => None,
        Art::Embedded { track, .. } => {
            let t = &app.lib.tracks[*track];
            app.full_path(&t.volume, &t.rel_path)
                .map(ArtSource::Embedded)
        }
        Art::Folder { file, .. } => app
            .full_path(&a.volume, &a.dir)
            .map(|d| ArtSource::File(volumes::join(&d, file))),
    };
    let src = src();
    app.art.get(ctx, &key, move || src, size).flatten()
}

/// Album artwork, or a red "no art" square.
fn draw_art(
    p: &egui::Painter,
    r: Rect,
    tex: Option<&egui::TextureHandle>,
    art: &Art,
    text_size: f32,
    tk: &Tokens,
) {
    match (tex, art) {
        (Some(t), _) => {
            p.image(
                t.id(),
                r,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        (None, Art::None) => {
            p.rect_filled(r, CornerRadius::same(2), tk.well);
            p.rect_stroke(
                r,
                CornerRadius::same(2),
                Stroke::new(1.0, tk.line),
                StrokeKind::Inside,
            );
            let big = r.width() > 80.0;
            let lines: &[&str] = if big { &["NO", "ARTWORK"] } else { &["NO ART"] };
            let line_h = text_size + 4.0;
            let top = r.center().y - line_h * lines.len() as f32 / 2.0;
            for (i, line) in lines.iter().enumerate() {
                text(
                    p,
                    Pos2::new(r.center().x, top + i as f32 * line_h),
                    Align2::CENTER_TOP,
                    line,
                    hifi::num(text_size),
                    if big { tk.accent } else { tk.dim },
                );
            }
        }
        (None, _) => {
            // There's art but it isn't loaded (or the drive is offline).
            p.rect_filled(
                r,
                CornerRadius::same(2),
                Color32::from_rgb(0x2a, 0x29, 0x26),
            );
        }
    }
}

/// Where to look for the TAW Player: next to the Library, in a folder beside
/// the Library's named after it (two unzipped downloads side by side), then
/// the PATH.
fn app_candidates(
    program: &str,
    here: Option<&std::path::Path>,
    path_var: Option<&std::ffi::OsStr>,
) -> Vec<std::path::PathBuf> {
    let exe = if cfg!(windows) {
        format!("{program}.exe")
    } else {
        program.to_string()
    };
    let mut v = Vec::new();
    if let Some(dir) = here {
        v.push(dir.join(&exe));
        if let Some(Ok(rd)) = dir.parent().map(std::fs::read_dir) {
            let mut sib: Vec<_> = rd
                .flatten()
                .filter(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .to_ascii_lowercase()
                        .starts_with(program)
                })
                .map(|e| e.path().join(&exe))
                .collect();
            // The newest version first.
            sib.sort();
            sib.reverse();
            v.extend(sib);
        }
    }
    if let Some(p) = path_var {
        v.extend(std::env::split_paths(p).map(|d| d.join(&exe)));
    }
    v
}

/// A family app (`taw-player`, `taw-equaliser`, `taw-analyser`), if it's
/// installed where the Library can find it.
pub(crate) fn find_app(program: &str) -> Option<std::path::PathBuf> {
    let here = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf));
    let path = std::env::var_os("PATH");
    app_candidates(program, here.as_deref(), path.as_deref())
        .into_iter()
        .find(|p| p.is_file())
}

/// Opens a file or folder with the computer's default program.
pub fn open_path(path: &std::path::Path) {
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(path).spawn();
    let _ = r;
}

#[cfg(test)]
mod player_tests {
    use super::app_candidates;

    #[test]
    fn looks_beside_then_siblings_then_path() {
        let d = std::env::temp_dir().join(format!("taw-library-player-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let here = d.join("taw-library-0.2.0");
        std::fs::create_dir_all(&here).unwrap();
        std::fs::create_dir_all(d.join("taw-player-0.1.0")).unwrap();
        let path = std::env::join_paths([d.join("bin")]).unwrap();
        let c = app_candidates("taw-player", Some(&here), Some(&path));
        let exe = if cfg!(windows) {
            "taw-player.exe"
        } else {
            "taw-player"
        };
        assert_eq!(c[0], here.join(exe));
        assert_eq!(c[1], d.join("taw-player-0.1.0").join(exe));
        assert_eq!(c[2], d.join("bin").join(exe));
        let _ = std::fs::remove_dir_all(&d);
    }
}
