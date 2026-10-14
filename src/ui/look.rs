//! The Library's drawing helpers in the TAW look (TAW-Panel's `hifi`
//! tokens): the list pieces the views share, so each view only lays out.

use egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Response, Stroke, Vec2};
use taw_panel::hifi::{self, Tokens};

pub use taw_panel::lists::{ago, fit, rows, thousands};

/// Worth fixing, not broken: amber.
pub const AMBER: Color32 = Color32::from_rgb(0xe0, 0xa0, 0x30);
/// Missing: a soft red.
pub const BAD: Color32 = Color32::from_rgb(0xd9, 0x57, 0x3b);

/// Plain text with a font and colour.
pub fn text(p: &Painter, pos: Pos2, align: Align2, s: &str, font: FontId, c: Color32) -> Rect {
    hifi::label(p, pos, align, s, font, c)
}

/// A small heading over a group: VIEW, DRIVES, ARTISTS.
pub fn heading(p: &Painter, pos: Pos2, s: &str, t: &Tokens) -> Rect {
    hifi::label(
        p,
        pos,
        Align2::LEFT_TOP,
        &s.to_uppercase(),
        hifi::semi(10.5),
        t.dim,
    )
}

/// A recessed area for a list.
pub fn well(p: &Painter, r: Rect, t: &Tokens) {
    hifi::well(p, r, t);
}

/// Column titles along the top of a list, with a line under them.
pub fn header(p: &Painter, r: Rect, cols: &[(f32, &str, Align2)], t: &Tokens) {
    for (x, s, align) in cols {
        hifi::label(
            p,
            Pos2::new(*x, r.center().y),
            *align,
            &s.to_uppercase(),
            hifi::semi(10.5),
            t.dim,
        );
    }
    p.line_segment(
        [
            Pos2::new(r.left() + 1.0, r.bottom()),
            Pos2::new(r.right() - 1.0, r.bottom()),
        ],
        Stroke::new(1.0, t.line),
    );
}

/// A list row's background: chosen, hovered, and a soft line under it.
pub fn row_bg(p: &Painter, r: Rect, resp: &Response, selected: bool, t: &Tokens) {
    if selected {
        p.rect_filled(r, 0, t.select);
    } else if resp.hovered() {
        p.rect_filled(r, 0, t.line_soft);
    }
    p.line_segment(
        [r.left_bottom(), r.right_bottom()],
        Stroke::new(1.0, t.line_soft),
    );
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Light {
    Ok,
    Warn,
    Bad,
    Off,
}

/// A status light: green, amber, red, or an empty ring when off.
pub fn light(p: &Painter, c: Pos2, l: Light, t: &Tokens) {
    match l {
        Light::Ok => p.circle_filled(c, 4.0, t.ok),
        Light::Warn => p.circle_filled(c, 4.0, AMBER),
        Light::Bad => p.circle_filled(c, 4.0, BAD),
        Light::Off => p.circle_stroke(c, 3.5, Stroke::new(1.5, t.dim)),
    };
}

/// A small rounded tag (a format, or something missing), outlined in its
/// colour or filled with it. Returns its width.
pub fn chip(p: &Painter, left_centre: Pos2, s: &str, c: Color32, filled: bool) -> f32 {
    let g = p.layout_no_wrap(s.to_string(), hifi::ui_font(11.5), c);
    let r = Rect::from_min_size(
        Pos2::new(left_centre.x, left_centre.y - 10.0),
        Vec2::new(g.size().x + 16.0, 20.0),
    );
    let text_c = if filled {
        p.rect_filled(r, CornerRadius::same(10), c);
        Color32::WHITE
    } else {
        p.rect_stroke(
            r,
            CornerRadius::same(10),
            Stroke::new(1.0, c.gamma_multiply(0.8)),
            egui::StrokeKind::Inside,
        );
        c
    };
    p.galley(
        Pos2::new(r.left() + 8.0, r.center().y - g.size().y / 2.0),
        g,
        text_c,
    );
    r.width()
}

/// How wide [`chip`] will be, roughly, before drawing it.
pub fn chip_width(s: &str) -> f32 {
    s.chars().count() as f32 * 6.4 + 16.0
}
