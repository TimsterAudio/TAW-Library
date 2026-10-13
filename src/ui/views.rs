//! The views chosen with the View keys: Library, What's Missing, and the
//! two that come in later stages.

use super::app::{AlbumKey, LibraryApp, View};
use super::parts::*;
use super::theme::*;
use super::widgets::*;
use crate::workers::ArtSource;
use egui::{Align2, Color32, CornerRadius, Id, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use mb_core::library::{fmt_duration, is_lossless, Album, Art, Missing};
use mb_core::volumes;

const ARTISTS_W: f32 = 240.0;
const DETAIL_W: f32 = 410.0;

impl LibraryApp {
    // ------------------------------------------------------------ library

    pub fn library_view(&mut self, ui: &mut Ui, main: Rect, th: &Theme) {
        let artists = Rect::from_min_size(main.min, Vec2::new(ARTISTS_W, main.height()));
        let detail = Rect::from_min_max(Pos2::new(main.right() - DETAIL_W, main.top()), main.max);
        let albums = Rect::from_min_max(
            Pos2::new(artists.right() + 14.0, main.top()),
            Pos2::new(detail.left() - 14.0, main.bottom()),
        );
        self.artists_list(ui, artists, th);
        self.albums_list(ui, albums, th);
        self.album_detail(ui, detail, th);
    }

    fn artists_list(&mut self, ui: &mut Ui, r: Rect, th: &Theme) {
        section(ui.painter(), r, "Artists", th, false);
        let find = Rect::from_min_size(
            r.min + Vec2::new(10.0, 16.0),
            Vec2::new(r.width() - 20.0, 28.0),
        );
        ui.painter()
            .rect_filled(find.expand(1.0), CornerRadius::same(2), Color32::BLACK);
        ui.painter()
            .rect_filled(find, CornerRadius::same(2), LCD_BG);
        ui.put(
            find.shrink2(Vec2::new(8.0, 4.0)),
            egui::TextEdit::singleline(&mut self.find)
                .frame(egui::Frame::NONE)
                .font(font(F_MONO, 13.5))
                .text_color(LCD_INK)
                .hint_text(
                    egui::RichText::new("FIND ARTIST")
                        .color(LCD_DIM)
                        .font(font(F_MONO, 13.5)),
                ),
        );
        let q = self.find.trim().to_lowercase();
        let list: Vec<usize> = (0..self.lib.artists.len())
            .filter(|&i| q.is_empty() || self.lib.artists[i].name.to_lowercase().contains(&q))
            .collect();
        let inner = Rect::from_min_max(
            Pos2::new(r.left() + 10.0, find.bottom() + 8.0),
            r.max - Vec2::new(10.0, 10.0),
        );
        well(ui.painter(), inner, th);
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
                font(F_LEGEND, 12.5),
                th.legend_dim,
                0.0,
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
                row_bg(p, row, resp, selected, th);
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
                    th,
                );
                right -= 14.0;
                let n = text(
                    p,
                    Pos2::new(right, row.center().y),
                    Align2::RIGHT_CENTER,
                    &a.albums.len().to_string(),
                    font(F_MONO, 12.0),
                    th.legend_dim,
                    0.0,
                );
                right = n.left() - 8.0;
                let mut name_w = right - row.left() - 12.0;
                if a.merge_into.is_some() {
                    name_w -= 56.0;
                }
                let nr = fit(
                    p,
                    Pos2::new(row.left() + 10.0, row.center().y),
                    Align2::LEFT_CENTER,
                    &a.name,
                    font(F_LEGEND, 13.5),
                    th.legend,
                    name_w,
                );
                if let Some(into) = &a.merge_into {
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

    fn albums_list(&mut self, ui: &mut Ui, r: Rect, th: &Theme) {
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
        section(ui.painter(), r, &title, th, false);
        let inner =
            Rect::from_min_max(r.min + Vec2::new(10.0, 16.0), r.max - Vec2::new(10.0, 10.0));
        well(ui.painter(), inner, th);
        let Some(artist) = artist else { return };
        let head = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 28.0));
        let w = inner.width();
        let c_year = inner.left() + 96.0;
        let c_album = inner.left() + 108.0;
        let c_fmt = inner.left() + (w * 0.55).max(300.0);
        let c_needs = c_fmt + 92.0;
        header(
            ui.painter(),
            head,
            &[
                (c_year, "Year", Align2::RIGHT_CENTER),
                (c_album, "Album", Align2::LEFT_CENTER),
                (c_fmt, "Format", Align2::LEFT_CENTER),
                (c_needs, "Needs", Align2::LEFT_CENTER),
            ],
            th,
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
                row_bg(ui.painter(), row, resp, selected, th);
                let thumb = Rect::from_min_size(
                    Pos2::new(row.left() + 10.0, row.center().y - 19.0),
                    Vec2::splat(38.0),
                );
                let tex = art_texture(self, &ctx, a, 96);
                let p = ui.painter();
                draw_art(p, thumb, tex.as_ref(), &a.art, 9.0);
                let y = row.center().y;
                if a.missing.contains(&Missing::Year) && a.year.is_none() {
                    text(
                        p,
                        Pos2::new(c_year, y),
                        Align2::RIGHT_CENTER,
                        "----",
                        font(F_MONO, 13.0),
                        LED,
                        0.0,
                    );
                } else if let Some(yr) = a.year {
                    text(
                        p,
                        Pos2::new(c_year, y),
                        Align2::RIGHT_CENTER,
                        &yr.to_string(),
                        font(F_MONO, 13.0),
                        th.legend,
                        0.0,
                    );
                }
                fit(
                    p,
                    Pos2::new(c_album, y - 1.0),
                    Align2::LEFT_BOTTOM,
                    &a.title,
                    font(F_LEGEND, 13.5),
                    if a.untitled { th.legend_dim } else { th.legend },
                    c_fmt - c_album - 12.0,
                );
                let tracks = format!(
                    "{} track{} · {}",
                    a.tracks.len(),
                    if a.tracks.len() == 1 { "" } else { "s" },
                    fmt_duration(a.duration_ms)
                );
                text(
                    p,
                    Pos2::new(c_album, y + 2.0),
                    Align2::LEFT_TOP,
                    &tracks,
                    font(F_MONO, 11.5),
                    th.legend_dim,
                    0.0,
                );
                let mut x = c_fmt;
                for f in a.formats.iter().take(2) {
                    let lossless = is_lossless(f, None)
                        || a.tracks.iter().any(|&t| {
                            self.lib.tracks[t].format == *f && self.lib.tracks[t].lossless()
                        });
                    x += chip(
                        p,
                        Pos2::new(x, y),
                        f,
                        if lossless { ON_GREEN } else { th.legend_dim },
                        false,
                    ) + 4.0;
                }
                if a.missing.is_empty() {
                    chip(p, Pos2::new(c_needs, y), "Complete", ON_GREEN, false);
                }
                // Up to two lines of chips.
                let (mut x, mut line) = (c_needs, 0);
                let chips: Vec<Missing> = a.missing.iter().copied().collect();
                let line_y = |l: i32| {
                    if chips.len() > 1 {
                        y - 9.0 + l as f32 * 18.0
                    } else {
                        y
                    }
                };
                for m in &chips {
                    let w = chip_width(m.chip());
                    if x + w > row.right() - 8.0 && x > c_needs {
                        if line == 1 {
                            text(
                                p,
                                Pos2::new(x, line_y(1)),
                                Align2::LEFT_CENTER,
                                "…",
                                font(F_LEGEND_BOLD, 13.0),
                                LED,
                                0.0,
                            );
                            break;
                        }
                        line = 1;
                        x = c_needs;
                    }
                    x += chip(
                        p,
                        Pos2::new(x, line_y(line)),
                        m.chip(),
                        if m.is_warning() { AMBER } else { LED },
                        false,
                    ) + 4.0;
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

    fn album_detail(&mut self, ui: &mut Ui, r: Rect, th: &Theme) {
        section(ui.painter(), r, "Album", th, false);
        let Some(ai) = self.selected_album() else {
            text(
                ui.painter(),
                r.left_top() + Vec2::new(14.0, 24.0),
                Align2::LEFT_TOP,
                "Choose an album.",
                font(F_LEGEND, 13.0),
                th.legend_dim,
                0.0,
            );
            return;
        };
        let a = self.lib.albums[ai].clone();
        let ctx = ui.ctx().clone();
        let tex = art_texture(self, &ctx, &a, 320);
        let art_r = Rect::from_min_size(r.min + Vec2::new(12.0, 20.0), Vec2::splat(156.0));
        let p = ui.painter();
        draw_art(p, art_r, tex.as_ref(), &a.art, 13.0);
        if let Some((w, h)) = a.art.size() {
            let s = format!("{w} × {h}");
            let g = p.layout_no_wrap(s.clone(), font(F_MONO, 11.0), Color32::WHITE);
            let br = Rect::from_min_size(
                art_r.left_bottom() + Vec2::new(6.0, -6.0 - g.size().y - 2.0),
                g.size() + Vec2::new(8.0, 2.0),
            );
            p.rect_filled(br, CornerRadius::same(2), Color32::from_black_alpha(160));
            p.galley(br.min + Vec2::new(4.0, 1.0), g, Color32::WHITE);
        }

        // Fields, each with a light: green present, amber could be better,
        // red missing.
        let ts: Vec<mb_core::library::Track> = a
            .tracks
            .iter()
            .map(|&i| self.lib.tracks[i].clone())
            .collect();
        let first = &ts[0];
        let all = |f: &dyn Fn(&mb_core::library::Track) -> bool| ts.iter().all(f);
        let fmt = {
            let lossless = first.lossless();
            let detail = if lossless {
                match (first.bit_depth, first.sample_rate) {
                    (Some(b), Some(sr)) => format!(" · {b}/{}", sr as f32 / 1000.0),
                    _ => String::new(),
                }
            } else {
                first.bitrate.map_or(String::new(), |b| format!(" · {b}k"))
            };
            if a.formats.len() > 1 {
                a.formats.iter().cloned().collect::<Vec<_>>().join(" + ")
            } else {
                format!("{}{detail}", first.format)
            }
        };
        let discs = ts.iter().filter_map(|t| t.disc).max();
        let fields: Vec<(&str, String, Light)> = vec![
            (
                "Title",
                a.title.clone(),
                if a.untitled { Light::Bad } else { Light::Ok },
            ),
            (
                "Artist",
                first.artist.clone().unwrap_or_default(),
                if all(&|t| t.artist.is_some()) {
                    Light::Ok
                } else {
                    Light::Bad
                },
            ),
            (
                "Album artist",
                if a.missing.contains(&Missing::AlbumArtist) {
                    String::new()
                } else {
                    a.artist.clone()
                },
                if a.missing.contains(&Missing::AlbumArtist) {
                    Light::Bad
                } else {
                    Light::Ok
                },
            ),
            (
                "Year",
                a.year.map_or(String::new(), |y| y.to_string()),
                if a.missing.contains(&Missing::Year) {
                    Light::Bad
                } else {
                    Light::Ok
                },
            ),
            (
                "Disc",
                discs.map_or("1".into(), |d| {
                    format!("{d} disc{}", if d == 1 { "" } else { "s" })
                }),
                if a.missing.contains(&Missing::DiscNo) {
                    Light::Bad
                } else {
                    Light::Ok
                },
            ),
            (
                "Tracks",
                format!("{} · {}", a.tracks.len(), fmt_duration(a.duration_ms)),
                if a.missing.contains(&Missing::TrackNo) {
                    Light::Bad
                } else {
                    Light::Ok
                },
            ),
            (
                "Format",
                fmt,
                if a.formats.len() > 1 {
                    Light::Warn
                } else {
                    Light::Ok
                },
            ),
            (
                "Artwork",
                match &a.art {
                    Art::None => String::new(),
                    Art::Embedded { .. } => "Embedded".into(),
                    Art::Folder { file, .. } => file.clone(),
                },
                if a.missing.contains(&Missing::Art) {
                    Light::Bad
                } else if a.missing.contains(&Missing::SmallArt) {
                    Light::Warn
                } else {
                    Light::Ok
                },
            ),
        ];
        let fx = art_r.right() + 14.0;
        for (i, (k, v, l)) in fields.iter().enumerate() {
            let y = art_r.top() + 8.0 + i as f32 * 19.5;
            light(p, Pos2::new(fx + 4.0, y), *l, th);
            text(
                p,
                Pos2::new(fx + 14.0, y),
                Align2::LEFT_CENTER,
                &k.to_uppercase(),
                font(F_LEGEND, 10.5),
                th.legend_dim,
                1.3,
            );
            if v.is_empty() || (*l == Light::Bad && *k != "Title" && *k != "Disc" && *k != "Tracks")
            {
                glow_text(
                    p,
                    Pos2::new(fx + 104.0, y),
                    Align2::LEFT_CENTER,
                    "MISSING",
                    font(F_MONO, 12.0),
                    LED,
                    0.3,
                );
            } else {
                fit(
                    p,
                    Pos2::new(fx + 104.0, y),
                    Align2::LEFT_CENTER,
                    v,
                    font(F_LEGEND, 13.0),
                    th.legend,
                    r.right() - fx - 116.0,
                );
            }
        }

        // Keys
        let ky = art_r.bottom() + 26.0;
        let k = |x: f32| Rect::from_min_size(Pos2::new(r.left() + x, ky), Vec2::new(54.0, 24.0));
        let folder = self.full_path(&a.volume, &a.dir);
        if key(
            ui,
            k(14.0),
            Id::new("open-folder"),
            K_CREAM,
            None,
            "Folder",
            th,
        )
        .clicked()
        {
            match &folder {
                Some(f) => {
                    open_path(f);
                    self.touch("Open folder", a.title.clone());
                }
                None => self.tell("That album's drive isn't connected."),
            }
        }
        if key(ui, k(78.0), Id::new("lookup"), K_CREAM, None, "Look up", th).clicked() {
            self.tell("Looking albums up online comes in stage 2, when Lookups is turned on.");
        }
        if key(ui, k(142.0), Id::new("edit"), K_CREAM, None, "Edit", th).clicked() {
            self.tell("Editing tags comes in stage 3, with an undo log.");
        }
        let play_r = Rect::from_min_size(
            Pos2::new(r.right() - 14.0 - 76.0, ky),
            Vec2::new(76.0, 24.0),
        );
        if key(ui, play_r, Id::new("play"), K_RED, None, "Play", th).clicked() {
            self.play(&a);
        }

        // Where it is
        let p = ui.painter();
        let vol = self.volume(&a.volume);
        let where_ = match (vol, &folder) {
            (_, Some(f)) => f.display().to_string(),
            (Some(v), None) => format!("{} (offline) › {}", v.label, a.dir),
            (None, None) => a.dir.clone(),
        };
        let g = p.layout(
            where_,
            font(F_LEGEND, 11.5),
            th.legend_dim,
            r.width() - 28.0,
        );
        let wy = ky + 46.0;
        let wh = g.size().y.min(32.0);
        p.with_clip_rect(Rect::from_min_size(
            Pos2::new(r.left(), wy),
            Vec2::new(r.width(), wh),
        ))
        .galley(Pos2::new(r.left() + 14.0, wy), g, th.legend_dim);

        // Tracks
        let inner = Rect::from_min_max(
            Pos2::new(r.left() + 10.0, wy + wh + 10.0),
            r.max - Vec2::new(10.0, 10.0),
        );
        well(ui.painter(), inner, th);
        let head = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 26.0));
        let mixed = a.formats.len() > 1;
        header(
            ui.painter(),
            head,
            &[
                (inner.left() + 34.0, "#", Align2::RIGHT_CENTER),
                (inner.left() + 46.0, "Title", Align2::LEFT_CENTER),
                (inner.right() - 12.0, "Time", Align2::RIGHT_CENTER),
            ],
            th,
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
                row_bg(p, row, resp, false, th);
                let y = row.center().y;
                let num = match (t.track_no, multi_disc) {
                    (Some(n), true) => format!("{}.{n:02}", t.disc.unwrap_or(1)),
                    (Some(n), false) => format!("{n:02}"),
                    (None, _) => "--".into(),
                };
                text(
                    p,
                    Pos2::new(row.left() + 34.0, y),
                    Align2::RIGHT_CENTER,
                    &num,
                    font(F_MONO, 12.5),
                    if t.track_no.is_some() { th.legend } else { LED },
                    0.0,
                );
                let title = t.title.clone().unwrap_or_else(|| t.file_name().to_string());
                let mut tw = row.width() - 120.0;
                if mixed {
                    tw -= 50.0;
                }
                fit(
                    p,
                    Pos2::new(row.left() + 46.0, y),
                    Align2::LEFT_CENTER,
                    &title,
                    font(F_LEGEND, 13.0),
                    if t.title.is_some() {
                        th.legend
                    } else {
                        th.legend_dim
                    },
                    tw,
                );
                text(
                    p,
                    Pos2::new(row.right() - 12.0, y),
                    Align2::RIGHT_CENTER,
                    &fmt_duration(t.duration_ms),
                    font(F_MONO, 12.5),
                    th.legend,
                    0.0,
                );
                if mixed {
                    chip(
                        p,
                        Pos2::new(row.right() - 100.0, y),
                        &t.format,
                        if t.lossless() {
                            ON_GREEN
                        } else {
                            th.legend_dim
                        },
                        false,
                    );
                }
                if let Some(e) = &t.error {
                    resp.clone()
                        .on_hover_text(format!("Couldn't read this file's tags: {e}"));
                }
            },
        );
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
                open_path(&file);
                self.touch("Play", a.title.clone());
                self.tell("Playing in your default music player. The TAW Player will take this over when it's ready.");
            }
            Err(e) => self.tell(format!("Couldn't make the playlist: {e}")),
        }
    }

    // ------------------------------------------------------------ missing

    pub fn missing_view(&mut self, ui: &mut Ui, main: Rect, th: &Theme) {
        let tiles = Rect::from_min_size(main.min, Vec2::new(270.0, main.height()));
        section(ui.painter(), tiles, "What's missing", th, false);
        let cur = Missing::ALL[self.s.missing.min(Missing::ALL.len() - 1)];
        let mut y = tiles.top() + 18.0;
        for (i, m) in Missing::ALL.iter().enumerate() {
            let r = Rect::from_min_size(
                Pos2::new(tiles.left() + 10.0, y),
                Vec2::new(tiles.width() - 20.0, 58.0),
            );
            let resp = ui.interact(r, Id::new(("tile", i)), Sense::click());
            let p = ui.painter();
            p.rect_filled(r.expand(1.0), CornerRadius::same(3), Color32::BLACK);
            p.rect_filled(r, CornerRadius::same(3), LCD_BG);
            if *m == cur {
                p.rect_stroke(
                    r,
                    CornerRadius::same(3),
                    Stroke::new(1.5, FOCUS),
                    StrokeKind::Inside,
                );
            } else if resp.hovered() {
                p.rect_stroke(
                    r,
                    CornerRadius::same(3),
                    Stroke::new(1.0, LCD_DIM),
                    StrokeKind::Inside,
                );
            }
            let albums = self.lib.missing_albums.get(m).map_or(0, Vec::len);
            glow_text(
                p,
                r.left_top() + Vec2::new(10.0, 10.0),
                Align2::LEFT_TOP,
                &m.title().to_uppercase(),
                font(F_MONO, 12.0),
                LCD_INK.gamma_multiply(0.9),
                0.4,
            );
            glow_text(
                p,
                Pos2::new(r.right() - 10.0, r.top() + 8.0),
                Align2::RIGHT_TOP,
                &thousands(albums as u64),
                font(F_MONO, 22.0),
                LCD_INK,
                0.3,
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
                font(F_MONO, 11.0),
                LCD_INK.gamma_multiply(0.55),
                0.4,
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
            font(F_LEGEND, 11.5),
            th.legend_dim,
            tiles.width() - 28.0,
        );
        if y + g.size().y < tiles.bottom() - 10.0 {
            p.galley(Pos2::new(tiles.left() + 14.0, y + 6.0), g, th.legend_dim);
        }

        let lr = Rect::from_min_max(Pos2::new(tiles.right() + 14.0, main.top()), main.max);
        section(
            ui.painter(),
            lr,
            &format!("Albums · {}", cur.title()),
            th,
            false,
        );
        let inner = Rect::from_min_max(
            lr.min + Vec2::new(10.0, 16.0),
            lr.max - Vec2::new(10.0, 10.0),
        );
        well(ui.painter(), inner, th);
        let head = Rect::from_min_size(inner.min, Vec2::new(inner.width(), 28.0));
        let w = inner.width();
        let c_artist = inner.left() + 12.0;
        let c_album = inner.left() + w * 0.17;
        let c_year = inner.left() + w * 0.42;
        let c_miss = c_year + 14.0;
        let c_where = inner.left() + w * 0.7;
        header(
            ui.painter(),
            head,
            &[
                (c_artist, "Artist", Align2::LEFT_CENTER),
                (c_album, "Album", Align2::LEFT_CENTER),
                (c_year, "Year", Align2::RIGHT_CENTER),
                (c_miss, "Missing", Align2::LEFT_CENTER),
                (c_where, "Where it is", Align2::LEFT_CENTER),
            ],
            th,
        );
        let list = Rect::from_min_max(Pos2::new(inner.left(), head.bottom() + 1.0), inner.max);
        let items: Vec<usize> = self
            .lib
            .missing_albums
            .get(&cur)
            .cloned()
            .unwrap_or_default();
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
                font(F_LEGEND, 13.0),
                th.legend_dim,
                0.0,
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
                row_bg(p, row, resp, false, th);
                let y = row.center().y;
                fit(
                    p,
                    Pos2::new(c_artist, y),
                    Align2::LEFT_CENTER,
                    &a.artist,
                    font(F_LEGEND_BOLD, 13.0),
                    th.legend,
                    c_album - c_artist - 10.0,
                );
                fit(
                    p,
                    Pos2::new(c_album, y),
                    Align2::LEFT_CENTER,
                    &a.title,
                    font(F_LEGEND, 13.0),
                    th.legend,
                    c_year - c_album - 50.0,
                );
                match a.year {
                    Some(yr) => text(
                        p,
                        Pos2::new(c_year, y),
                        Align2::RIGHT_CENTER,
                        &yr.to_string(),
                        font(F_MONO, 12.5),
                        th.legend,
                        0.0,
                    ),
                    None => text(
                        p,
                        Pos2::new(c_year, y),
                        Align2::RIGHT_CENTER,
                        "----",
                        font(F_MONO, 12.5),
                        LED,
                        0.0,
                    ),
                };
                let mut x = c_miss;
                for m in &a.missing {
                    if x > c_where - 70.0 {
                        text(
                            p,
                            Pos2::new(x, y),
                            Align2::LEFT_CENTER,
                            "…",
                            font(F_LEGEND_BOLD, 13.0),
                            LED,
                            0.0,
                        );
                        break;
                    }
                    x += chip(
                        p,
                        Pos2::new(x, y),
                        m.chip(),
                        if m.is_warning() { AMBER } else { LED },
                        false,
                    ) + 4.0;
                }
                let label = self
                    .volumes
                    .iter()
                    .find(|v| v.id == a.volume)
                    .map_or("?", |v| v.label.as_str());
                fit(
                    p,
                    Pos2::new(c_where, y),
                    Align2::LEFT_CENTER,
                    &format!("{label} › {}", a.dir),
                    font(F_LEGEND, 12.0),
                    th.legend_dim,
                    row.right() - c_where - 10.0,
                );
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

    pub fn later_view(&mut self, ui: &mut Ui, main: Rect, th: &Theme, which: &str) {
        section(ui.painter(), main, which, th, false);
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
        glow_text(
            p,
            c,
            Align2::CENTER_BOTTOM,
            &title.to_uppercase(),
            font(F_MONO, 18.0),
            LCD_INK,
            0.6,
        );
        let g = p.layout(body.into(), font(F_LEGEND, 14.0), th.legend_dim, 620.0);
        p.galley(
            Pos2::new(c.x - g.size().x / 2.0, c.y + 18.0),
            g,
            th.legend_dim,
        );
    }
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
            p.rect_filled(r, CornerRadius::same(2), LCD_BG);
            p.rect_stroke(
                r,
                CornerRadius::same(2),
                Stroke::new(1.0, LCD_DIM),
                StrokeKind::Inside,
            );
            let big = r.width() > 80.0;
            let lines: &[&str] = if big { &["NO", "ARTWORK"] } else { &["NO ART"] };
            let line_h = text_size + 4.0;
            let top = r.center().y - line_h * lines.len() as f32 / 2.0;
            for (i, line) in lines.iter().enumerate() {
                glow_text(
                    p,
                    Pos2::new(r.center().x, top + i as f32 * line_h),
                    Align2::CENTER_TOP,
                    line,
                    font(F_MONO, text_size),
                    if big {
                        LCD_INK
                    } else {
                        LCD_DIM.gamma_multiply(2.0)
                    },
                    0.3,
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

/// Opens a file or folder with the computer's default program.
pub fn open_path(path: &std::path::Path) {
    #[cfg(windows)]
    let r = std::process::Command::new("explorer").arg(path).spawn();
    #[cfg(not(windows))]
    let r = std::process::Command::new("xdg-open").arg(path).spawn();
    let _ = r;
}
