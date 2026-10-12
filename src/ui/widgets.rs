//! Panel hardware drawn with egui's painter: LED keys, red readouts, the LED
//! display and the outlined sections. Taken from EQ Box (keep them in step
//! until the family shares one panel crate).

use super::theme::*;
use egui::epaint::{Mesh, TextShape};
use egui::text::{LayoutJob, TextFormat};
use egui::{
    Align2, Color32, CornerRadius, FontId, Id, Painter, Pos2, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2,
};

// ---------------------------------------------------------------- text

pub fn text(
    p: &Painter,
    pos: Pos2,
    align: Align2,
    s: &str,
    font: FontId,
    color: Color32,
    spacing: f32,
) -> Rect {
    let mut job = LayoutJob::default();
    job.append(
        s,
        0.0,
        TextFormat {
            font_id: font,
            color,
            extra_letter_spacing: spacing,
            ..Default::default()
        },
    );
    let galley = p.layout_job(job);
    // Letter spacing also follows the last glyph; don't let it skew centring.
    let size = galley.size() - Vec2::new(spacing, 0.0);
    let rect = align.anchor_size(pos, size);
    p.galley(rect.min, galley, color);
    rect
}

/// Red display text with a soft glow.
pub fn glow_text(
    p: &Painter,
    pos: Pos2,
    align: Align2,
    s: &str,
    font: FontId,
    color: Color32,
    spacing: f32,
) -> Rect {
    for d in [
        Vec2::new(-1.0, 0.0),
        Vec2::new(1.0, 0.0),
        Vec2::new(0.0, -1.0),
        Vec2::new(0.0, 1.0),
    ] {
        text(
            p,
            pos + d,
            align,
            s,
            font.clone(),
            LCD_GLOW.gamma_multiply(0.45),
            spacing,
        );
    }
    text(p, pos, align, s, font, color, spacing)
}

/// Panel legend: uppercase, letter-spaced Barlow Semi Condensed.
pub fn legend(p: &Painter, pos: Pos2, align: Align2, s: &str, th: &Theme) -> Rect {
    text(
        p,
        pos,
        align,
        &s.to_uppercase(),
        font(F_LEGEND, 11.0),
        th.legend,
        1.3,
    )
}

/// Text slanted like the Jupiter badge.
pub fn slanted_text(
    ui: &Ui,
    pos: Pos2,
    s: &str,
    font_id: FontId,
    color: Color32,
    spacing: f32,
    degrees: f32,
) {
    let p = ui.painter();
    let mut job = LayoutJob::default();
    job.append(
        s,
        0.0,
        TextFormat {
            font_id,
            color,
            extra_letter_spacing: spacing,
            ..Default::default()
        },
    );
    let galley = p.layout_job(job);
    let ctx = ui.ctx();
    let tex_size = ctx.fonts(|f| f.font_image_size());
    let mut tess = egui::epaint::Tessellator::new(
        ctx.pixels_per_point(),
        ctx.tessellation_options(|o| *o),
        tex_size,
        Vec::new(),
    );
    let baseline = pos.y + galley.size().y * 0.8;
    let mut mesh = Mesh::default();
    tess.tessellate_text(&TextShape::new(pos, galley, color), &mut mesh);
    let k = degrees.to_radians().tan();
    for v in &mut mesh.vertices {
        v.pos.x += (baseline - v.pos.y) * k;
    }
    p.add(Shape::mesh(mesh));
}

// ---------------------------------------------------------------- frames

/// An outlined section with its title set into the top edge.
pub fn section(p: &Painter, rect: Rect, title: &str, th: &Theme, highlight: bool) {
    let stroke = if highlight {
        Stroke::new(1.5, FOCUS)
    } else {
        Stroke::new(1.0, th.frame)
    };
    p.rect_stroke(rect, CornerRadius::same(3), stroke, StrokeKind::Middle);
    if !title.is_empty() {
        let mut job = LayoutJob::default();
        job.append(
            &title.to_uppercase(),
            0.0,
            TextFormat {
                font_id: font(F_LEGEND_BOLD, 12.5),
                color: th.legend,
                extra_letter_spacing: 2.6,
                ..Default::default()
            },
        );
        let g = p.layout_job(job);
        let pos = Pos2::new(rect.left() + 14.0, rect.top() - g.size().y / 2.0);
        let bg = Rect::from_min_size(pos - Vec2::new(6.0, 0.0), g.size() + Vec2::new(8.0, 0.0));
        p.rect_filled(bg, 0, th.panel);
        p.galley(pos, g, th.legend);
    }
}

/// The main LED display: two lines, each with a left and right part.
pub fn lcd(p: &Painter, rect: Rect, lines: &[(String, String); 2]) {
    p.rect_filled(
        rect.expand(3.0),
        CornerRadius::same(4),
        Color32::from_white_alpha(18),
    );
    p.rect_filled(rect.expand(2.0), CornerRadius::same(4), Color32::BLACK);
    p.rect_filled(rect, CornerRadius::same(3), LCD_BG);
    let f = font(F_MONO, 15.0);
    for (i, (l, r)) in lines.iter().enumerate() {
        let y = rect.top() + 9.0 + i as f32 * 20.0;
        let col = if i == 0 {
            LCD_INK
        } else {
            LCD_INK.gamma_multiply(0.85)
        };
        let rr = glow_text(
            p,
            Pos2::new(rect.right() - 12.0, y),
            Align2::RIGHT_TOP,
            &r.to_uppercase(),
            f.clone(),
            col,
            0.9,
        );
        // Keep the left part clear of the right part.
        let clip = Rect::from_min_max(rect.min, Pos2::new(rr.left() - 10.0, rect.bottom()));
        glow_text(
            &p.with_clip_rect(clip),
            Pos2::new(rect.left() + 12.0, y),
            Align2::LEFT_TOP,
            &l.to_uppercase(),
            f.clone(),
            col,
            0.9,
        );
    }
}

/// A small red value readout, centred under a control.
pub fn readout(p: &Painter, top_centre: Pos2, s: &str, min_w: f32) {
    let f = font(F_MONO, 12.0);
    let w = p
        .layout_no_wrap(s.to_uppercase(), f.clone(), LCD_INK)
        .size()
        .x
        + 12.0;
    let rect = Rect::from_center_size(
        top_centre + Vec2::new(0.0, 8.0),
        Vec2::new(w.max(min_w), 16.0),
    );
    p.rect_filled(rect.expand(1.0), CornerRadius::same(2), Color32::BLACK);
    p.rect_filled(rect, CornerRadius::same(2), LCD_BG);
    glow_text(
        p,
        rect.center(),
        Align2::CENTER_CENTER,
        &s.to_uppercase(),
        f,
        LCD_INK,
        0.5,
    );
}

/// A one-row LED ladder showing a share from 0 to 1 in one colour.
pub fn ladder(p: &Painter, rect: Rect, share: f32, colour: Color32) {
    p.rect_filled(rect.expand(2.0), CornerRadius::same(2), Color32::BLACK);
    p.rect_filled(
        rect,
        CornerRadius::same(2),
        Color32::from_rgb(0x0b, 0x0b, 0x0b),
    );
    const SEGS: usize = 24;
    let gap = 2.0;
    let seg_w = (rect.width() - 6.0 - gap * (SEGS - 1) as f32) / SEGS as f32;
    let lit = (share.clamp(0.0, 1.0) * SEGS as f32).round() as usize;
    for s in 0..SEGS {
        let x = rect.left() + 3.0 + s as f32 * (seg_w + gap);
        let col = if s < lit {
            colour
        } else {
            colour.gamma_multiply(0.13)
        };
        p.rect_filled(
            Rect::from_min_size(
                Pos2::new(x, rect.top() + 3.0),
                Vec2::new(seg_w, rect.height() - 6.0),
            ),
            0,
            col,
        );
    }
}

/// The red, orange, yellow and cream stripe.
pub fn stripe(p: &Painter, rect: Rect) {
    let w = rect.width() / 4.0;
    for (i, c) in STRIPE.iter().enumerate() {
        let r = Rect::from_min_size(
            Pos2::new(rect.left() + w * i as f32, rect.top()),
            Vec2::new(w + 0.5, rect.height()),
        );
        p.rect_filled(r, 0, *c);
    }
}

fn vgradient(p: &Painter, rect: Rect, stops: &[(f32, Color32)]) {
    let mut mesh = Mesh::default();
    for w in stops.windows(2) {
        let (t0, c0) = w[0];
        let (t1, c1) = w[1];
        let y0 = rect.top() + rect.height() * t0;
        let y1 = rect.top() + rect.height() * t1;
        let i = mesh.vertices.len() as u32;
        mesh.colored_vertex(Pos2::new(rect.left(), y0), c0);
        mesh.colored_vertex(Pos2::new(rect.right(), y0), c0);
        mesh.colored_vertex(Pos2::new(rect.right(), y1), c1);
        mesh.colored_vertex(Pos2::new(rect.left(), y1), c1);
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i, i + 2, i + 3);
    }
    p.add(Shape::mesh(mesh));
}

/// An LED: lit red with a glow, or dark.
pub fn led(p: &Painter, c: Pos2, on: bool) {
    if on {
        p.circle_filled(c, 7.0, LED.gamma_multiply(0.12));
        p.circle_filled(c, 4.5, LED.gamma_multiply(0.3));
        p.circle_filled(c, 3.0, LED);
        p.circle_filled(
            c + Vec2::new(-0.8, -0.8),
            1.0,
            Color32::from_rgb(0xff, 0xc0, 0xa8),
        );
    } else {
        p.circle_filled(c, 3.0, LED_OFF);
        p.circle_stroke(c, 3.0, Stroke::new(0.6, Color32::from_black_alpha(160)));
    }
}

// ---------------------------------------------------------------- controls

/// A coloured rectangular hardware key with an optional LED above it and its
/// legend below.
pub fn key(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    colour: KeyColour,
    led_on: Option<bool>,
    label: &str,
    th: &Theme,
) -> Response {
    let hit = rect.expand2(Vec2::new(2.0, 10.0));
    let resp = ui.interact(hit, id, Sense::click());
    let p = ui.painter();
    let pressed = resp.is_pointer_button_down_on();
    let r = if pressed {
        rect.translate(Vec2::new(0.0, 1.0))
    } else {
        rect
    };
    p.rect_filled(
        rect.translate(Vec2::new(0.0, 3.0)),
        CornerRadius::same(2),
        Color32::from_black_alpha(150),
    );
    p.rect_filled(
        r.translate(Vec2::new(0.0, 1.5)),
        CornerRadius::same(2),
        colour.shadow,
    );
    p.rect_filled(r.expand(1.0), CornerRadius::same(2), Color32::BLACK);
    let face = if resp.hovered() {
        lerp_colour(colour.face, Color32::WHITE, 0.08)
    } else {
        colour.face
    };
    vgradient(
        p,
        r.shrink(0.5),
        &[
            (0.0, colour.hi),
            (0.18, face),
            (0.8, face),
            (1.0, colour.lo),
        ],
    );
    if resp.has_focus() {
        p.rect_stroke(
            r.expand(3.0),
            CornerRadius::same(3),
            Stroke::new(2.0, FOCUS),
            StrokeKind::Middle,
        );
    }
    if let Some(on) = led_on {
        led(p, Pos2::new(rect.center().x, rect.top() - 8.0), on);
    }
    if !label.is_empty() {
        legend(
            p,
            Pos2::new(rect.center().x, rect.bottom() + 5.0),
            Align2::CENTER_TOP,
            label,
            th,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A tab on a section frame's top edge: an optional LED, a legend and an
/// optional value in display red. Returns true when it's clicked.
pub fn frame_tab(
    ui: &mut Ui,
    tab: Rect,
    id: &str,
    led_on: Option<bool>,
    label: &str,
    value: Option<&str>,
    th: &Theme,
) -> bool {
    let resp = ui.interact(tab, Id::new(id), Sense::click());
    let p = ui.painter();
    p.rect_filled(tab, CornerRadius::same(3), th.panel);
    p.rect_stroke(
        tab,
        CornerRadius::same(3),
        Stroke::new(1.0, if resp.hovered() { FOCUS } else { th.frame }),
        StrokeKind::Inside,
    );
    let mut x = tab.left() + 9.0;
    if let Some(on) = led_on {
        led(p, Pos2::new(tab.left() + 11.0, tab.center().y), on);
        x = tab.left() + 22.0;
    }
    text(
        p,
        Pos2::new(x, tab.center().y),
        Align2::LEFT_CENTER,
        label,
        font(F_LEGEND_BOLD, 11.0),
        th.legend,
        1.8,
    );
    if let Some(v) = value {
        glow_text(
            p,
            Pos2::new(tab.right() - 8.0, tab.center().y),
            Align2::RIGHT_CENTER,
            v,
            font(F_MONO, 11.5),
            LCD_INK,
            0.3,
        );
    }
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// A key with no LED that lights up itself, in `light`, when it's on. Off,
/// it's a dark lens with a hint of its colour.
pub fn lit_key(
    ui: &mut Ui,
    rect: Rect,
    id: Id,
    light: Color32,
    on: bool,
    label: &str,
    th: &Theme,
) -> Response {
    let colour = if on {
        KeyColour {
            face: lerp_colour(light, Color32::WHITE, 0.22),
            hi: lerp_colour(light, Color32::WHITE, 0.6),
            lo: lerp_colour(light, Color32::BLACK, 0.08),
            shadow: lerp_colour(light, Color32::BLACK, 0.6),
        }
    } else {
        let base = lerp_colour(Color32::from_rgb(0x26, 0x25, 0x22), light, 0.14);
        KeyColour {
            face: base,
            hi: lerp_colour(base, Color32::WHITE, 0.1),
            lo: lerp_colour(base, Color32::BLACK, 0.3),
            shadow: Color32::from_rgb(0x0e, 0x0e, 0x0d),
        }
    };
    if on {
        // The light spilling round the key.
        let p = ui.painter();
        for (grow, alpha) in [(10.0, 0.04), (6.0, 0.08), (3.0, 0.16)] {
            p.rect_filled(
                rect.expand(grow),
                CornerRadius::same((grow + 2.0) as u8),
                light.gamma_multiply(alpha),
            );
        }
    }
    key(ui, rect, id, colour, None, label, th)
}
