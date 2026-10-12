//! Small pieces for the list-heavy parts of the panel: single-line text that
//! fits its column, chips, status lights, wells and scrolling lists.

use super::theme::*;
use super::widgets::text;
use egui::text::{LayoutJob, TextFormat, TextWrapping};
use egui::{
    Align, Align2, Color32, CornerRadius, FontId, Id, Layout, Painter, Pos2, Rect, Response,
    ScrollArea, Sense, Stroke, StrokeKind, Ui, UiBuilder, Vec2,
};

/// One line of text, cut short with "…" if it's wider than `max_w`.
pub fn fit(
    p: &Painter,
    pos: Pos2,
    align: Align2,
    s: &str,
    font: FontId,
    color: Color32,
    max_w: f32,
) -> Rect {
    let mut job = LayoutJob {
        wrap: TextWrapping {
            max_width: max_w.max(1.0),
            max_rows: 1,
            break_anywhere: true,
            overflow_character: Some('…'),
        },
        ..Default::default()
    };
    job.append(
        s,
        0.0,
        TextFormat {
            font_id: font,
            color,
            ..Default::default()
        },
    );
    let g = p.layout_job(job);
    let rect = align.anchor_size(pos, g.size());
    p.galley(rect.min, g, color);
    rect
}

/// How wide `chip` draws a label.
pub fn chip_width(s: &str) -> f32 {
    // Barlow Semi Condensed Bold at 10 px averages about 5.6 px a capital,
    // plus the letter spacing and padding.
    s.chars().count() as f32 * 6.6 + 10.0
}

/// A small outlined label: a format, or something missing. Returns its width.
pub fn chip(p: &Painter, left_centre: Pos2, s: &str, color: Color32, filled: bool) -> f32 {
    let f = font(F_LEGEND_BOLD, 10.0);
    let g = p.layout_no_wrap(s.to_uppercase(), f.clone(), color);
    let w = g.size().x + 1.0 * s.chars().count() as f32 + 10.0;
    let r = Rect::from_min_size(
        Pos2::new(left_centre.x, left_centre.y - 8.0),
        Vec2::new(w, 16.0),
    );
    if filled {
        p.rect_filled(r, CornerRadius::same(2), color);
    } else {
        p.rect_stroke(
            r,
            CornerRadius::same(2),
            Stroke::new(1.0, color),
            StrokeKind::Inside,
        );
    }
    text(
        p,
        r.center(),
        Align2::CENTER_CENTER,
        &s.to_uppercase(),
        f,
        if filled {
            Color32::from_rgb(0x1b, 0x1a, 0x18)
        } else {
            color
        },
        1.0,
    );
    w
}

/// A status light: green, amber, red or off.
#[derive(Clone, Copy, PartialEq)]
pub enum Light {
    Ok,
    Warn,
    Bad,
    Off,
}

pub fn light(p: &Painter, c: Pos2, l: Light, th: &Theme) {
    let col = match l {
        Light::Ok => OK_GREEN,
        Light::Warn => AMBER,
        Light::Bad => LED,
        Light::Off => th.tick,
    };
    if l != Light::Off {
        p.circle_filled(c, 6.5, col.gamma_multiply(0.18));
    }
    p.circle_filled(c, 4.0, col);
}

/// The recessed background lists sit on.
pub fn well(p: &Painter, rect: Rect, th: &Theme) {
    p.rect_filled(rect, CornerRadius::same(2), th.well);
    p.rect_stroke(
        rect,
        CornerRadius::same(2),
        Stroke::new(1.0, Color32::from_black_alpha(90)),
        StrokeKind::Inside,
    );
}

/// Column titles along the top of a well.
pub fn header(p: &Painter, rect: Rect, cols: &[(f32, &str, Align2)], th: &Theme) {
    let y = rect.center().y;
    for (x, s, align) in cols {
        text(
            p,
            Pos2::new(*x, y),
            *align,
            &s.to_uppercase(),
            font(F_LEGEND_BOLD, 10.5),
            th.legend_dim,
            1.6,
        );
    }
    p.line_segment(
        [
            Pos2::new(rect.left() + 4.0, rect.bottom()),
            Pos2::new(rect.right() - 4.0, rect.bottom()),
        ],
        Stroke::new(1.0, th.frame),
    );
}

/// A scrolling list of fixed-height rows inside `rect`. Only the rows on
/// screen are drawn. `row` draws one row and gets its response.
pub fn rows(
    ui: &mut Ui,
    rect: Rect,
    id: &str,
    row_h: f32,
    n: usize,
    scroll_to: Option<usize>,
    mut row: impl FnMut(&mut Ui, usize, Rect, &Response),
) {
    let mut child = ui.new_child(
        UiBuilder::new()
            .max_rect(rect)
            .layout(Layout::top_down(Align::Min)),
    );
    child.set_clip_rect(rect);
    child.spacing_mut().item_spacing = Vec2::ZERO;
    let mut area = ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false]);
    if let Some(i) = scroll_to {
        area = area.vertical_scroll_offset((i as f32 * row_h - rect.height() * 0.4).max(0.0));
    }
    area.show_rows(&mut child, row_h, n, |ui, range| {
        for i in range {
            let (r, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), row_h), Sense::hover());
            let resp = ui.interact(r, Id::new((id, i)), Sense::click());
            row(ui, i, r, &resp);
        }
    });
}

/// The hover and selection shading of a list row.
pub fn row_bg(p: &Painter, r: Rect, resp: &Response, selected: bool, th: &Theme) {
    if selected {
        p.rect_filled(r, 0, th.row_sel);
        p.rect_filled(
            Rect::from_min_size(r.min, Vec2::new(2.0, r.height())),
            0,
            FOCUS,
        );
    } else if resp.hovered() {
        p.rect_filled(r, 0, th.row_hover);
    }
    p.line_segment(
        [r.left_bottom(), r.right_bottom()],
        Stroke::new(1.0, th.row_hover),
    );
}

/// 59,706
pub fn thousands(n: impl Into<u64>) -> String {
    let s = n.into().to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// "5 min ago", "3 h ago", "2 days ago".
pub fn ago(then: i64, now: i64) -> String {
    let d = (now - then).max(0);
    match d {
        0..=59 => "just now".into(),
        60..=3599 => format!("{} min ago", d / 60),
        3600..=86_399 => format!("{} h ago", d / 3600),
        86_400..=172_799 => "yesterday".into(),
        _ => format!("{} days ago", d / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(thousands(0u64), "0");
        assert_eq!(thousands(999u64), "999");
        assert_eq!(thousands(59_706u64), "59,706");
        assert_eq!(thousands(1_234_567u64), "1,234,567");
        assert_eq!(ago(0, 30), "just now");
        assert_eq!(ago(0, 7_200), "2 h ago");
        assert_eq!(ago(0, 3 * 86_400), "3 days ago");
    }
}
