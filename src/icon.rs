//! The window icon, drawn in code: a black badge with the panel stripe and
//! a red record.

use egui::IconData;

pub fn icon() -> IconData {
    const S: usize = 64;
    let mut rgba = vec![0u8; S * S * 4];
    let stripe = [
        [0xd8, 0x43, 0x2a],
        [0xec, 0x7b, 0x24],
        [0xef, 0xc2, 0x3b],
        [0xe7, 0xe0, 0xc9],
    ];
    let ink = [0xff, 0x5b, 0x36];
    for y in 0..S {
        for x in 0..S {
            let i = (y * S + x) * 4;
            // Rounded square
            let (dx, dy) = (
                (x as f32 - 31.5).abs() - 24.0,
                (y as f32 - 31.5).abs() - 24.0,
            );
            let outside = dx.max(0.0).hypot(dy.max(0.0)) - 7.5;
            if outside > 0.5 {
                continue;
            }
            let mut c = [0x17, 0x17, 0x17];
            if (50..54).contains(&y) && (8..56).contains(&x) {
                c = stripe[((x - 8) / 12).min(3)];
            }
            // A record: an outer ring, grooves and the label.
            let r = (x as f32 - 32.0).hypot(y as f32 - 28.0);
            let ring = |at: f32, w: f32| (w - (r - at).abs()).clamp(0.0, 1.0);
            let a = ring(17.0, 1.6).max(ring(11.5, 0.7)).max(if r < 5.5 {
                (5.5 - r).min(1.0)
            } else {
                0.0
            });
            if a > 0.0 {
                for k in 0..3 {
                    c[k] = (c[k] as f32 + (ink[k] as f32 - c[k] as f32) * a) as u8;
                }
            }
            rgba[i..i + 3].copy_from_slice(&c);
            rgba[i + 3] = ((0.5 - outside).clamp(0.0, 1.0) * 255.0) as u8;
        }
    }
    IconData {
        rgba,
        width: S as u32,
        height: S as u32,
    }
}
