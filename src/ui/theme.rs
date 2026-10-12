//! Colours and type from the Drone Box panel, as EQ Box uses them.

use egui::{Color32, FontData, FontDefinitions, FontFamily, FontId};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum PanelKind {
    #[default]
    Black,
    Silver,
}

#[derive(Clone, Copy)]
pub struct Theme {
    pub page: Color32,
    pub panel: Color32,
    pub legend: Color32,
    pub legend_dim: Color32,
    pub frame: Color32,
    pub tick: Color32,
    /// The recessed background of lists.
    pub well: Color32,
    /// A list row under the pointer, and the selected row.
    pub row_hover: Color32,
    pub row_sel: Color32,
    pub silver: bool,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

impl Theme {
    pub fn new(panel: PanelKind) -> Self {
        match panel {
            PanelKind::Black => Theme {
                page: rgb(0x0a, 0x0a, 0x0a),
                panel: rgb(0x17, 0x17, 0x17),
                legend: rgb(0xec, 0xe8, 0xdd),
                legend_dim: rgb(0x8e, 0x8a, 0x81),
                frame: Color32::from_rgba_unmultiplied(0xec, 0xe8, 0xdd, 77),
                tick: rgb(0x7c, 0x78, 0x6f),
                well: rgb(0x10, 0x10, 0x10),
                row_hover: Color32::from_rgba_unmultiplied(0xec, 0xe8, 0xdd, 14),
                row_sel: Color32::from_rgba_unmultiplied(0xec, 0x7b, 0x24, 40),
                silver: false,
            },
            PanelKind::Silver => Theme {
                page: rgb(0x8f, 0x8c, 0x86),
                panel: rgb(0xd5, 0xd2, 0xca),
                legend: rgb(0x1b, 0x1a, 0x18),
                legend_dim: rgb(0x5a, 0x57, 0x50),
                frame: Color32::from_rgba_unmultiplied(0x1b, 0x1a, 0x18, 107),
                tick: rgb(0x4f, 0x4c, 0x46),
                well: rgb(0xc8, 0xc5, 0xbc),
                row_hover: Color32::from_rgba_unmultiplied(0x1b, 0x1a, 0x18, 18),
                row_sel: Color32::from_rgba_unmultiplied(0xec, 0x7b, 0x24, 56),
                silver: true,
            },
        }
    }
}

// Things that look the same on both panels, as on real hardware.
pub const LED: Color32 = rgb(0xff, 0x3a, 0x24);
pub const LED_OFF: Color32 = rgb(0x3a, 0x16, 0x11);
pub const LCD_BG: Color32 = rgb(0x11, 0x05, 0x04);
pub const LCD_INK: Color32 = rgb(0xff, 0x5b, 0x36);
pub const LCD_DIM: Color32 = rgb(0x4a, 0x17, 0x0e);
pub const LCD_GLOW: Color32 = Color32::from_rgba_premultiplied(140, 44, 22, 140);
pub const FOCUS: Color32 = rgb(0xec, 0x7b, 0x24);

/// Lit keys and "all present" lights.
pub const ON_GREEN: Color32 = rgb(0, 150, 100);
/// Lights for something that's there but could be better.
pub const AMBER: Color32 = rgb(0xf0, 0xb2, 0x32);
/// Lights for something present and correct.
pub const OK_GREEN: Color32 = rgb(0x2f, 0xd2, 0x7a);

/// The red, orange, yellow and cream stripe.
pub const STRIPE: [Color32; 4] = [
    rgb(0xd8, 0x43, 0x2a),
    rgb(0xec, 0x7b, 0x24),
    rgb(0xef, 0xc2, 0x3b),
    rgb(0xe7, 0xe0, 0xc9),
];

/// Coloured hardware keys: (face, highlight, lowlight, shadow).
#[derive(Clone, Copy)]
pub struct KeyColour {
    pub face: Color32,
    pub hi: Color32,
    pub lo: Color32,
    pub shadow: Color32,
}

pub const K_RED: KeyColour = KeyColour {
    face: rgb(0xd8, 0x43, 0x2a),
    hi: rgb(0xea, 0x6a, 0x50),
    lo: rgb(0xb5, 0x36, 0x1f),
    shadow: rgb(0x5e, 0x1a, 0x0f),
};
pub const K_ORANGE: KeyColour = KeyColour {
    face: rgb(0xec, 0x7b, 0x24),
    hi: rgb(0xf8, 0x98, 0x4d),
    lo: rgb(0xc9, 0x63, 0x1a),
    shadow: rgb(0x6b, 0x34, 0x0b),
};
pub const K_YELLOW: KeyColour = KeyColour {
    face: rgb(0xef, 0xc2, 0x3b),
    hi: rgb(0xfa, 0xd9, 0x68),
    lo: rgb(0xcd, 0xa1, 0x29),
    shadow: rgb(0x6d, 0x53, 0x0f),
};
pub const K_CREAM: KeyColour = KeyColour {
    face: rgb(0xe4, 0xdc, 0xc4),
    hi: rgb(0xf5, 0xf0, 0xe0),
    lo: rgb(0xc6, 0xbe, 0xa3),
    shadow: rgb(0x5f, 0x5a, 0x48),
};
pub const K_GREY: KeyColour = KeyColour {
    face: rgb(0x48, 0x46, 0x3f),
    hi: rgb(0x63, 0x60, 0x58),
    lo: rgb(0x34, 0x32, 0x2d),
    shadow: rgb(0x0e, 0x0e, 0x0d),
};

pub const F_LEGEND: &str = "legend";
pub const F_LEGEND_BOLD: &str = "legend-bold";
pub const F_LOGO: &str = "logo";
pub const F_MONO: &str = "mono";
pub const F_MAKER: &str = "maker";
pub const F_MAKER_BOLD: &str = "maker-bold";

pub fn font(name: &str, size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(name.into()))
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut defs = FontDefinitions::default();
    let faces: [(&str, &'static [u8]); 6] = [
        (
            F_LEGEND,
            include_bytes!("../../assets/fonts/BarlowSemiCondensed-SemiBold.ttf"),
        ),
        (
            F_LEGEND_BOLD,
            include_bytes!("../../assets/fonts/BarlowSemiCondensed-Bold.ttf"),
        ),
        (
            F_LOGO,
            include_bytes!("../../assets/fonts/Michroma-Regular.ttf"),
        ),
        (
            F_MONO,
            include_bytes!("../../assets/fonts/ShareTechMono-Regular.ttf"),
        ),
        (
            F_MAKER,
            include_bytes!("../../assets/fonts/SpaceGrotesk-Medium.ttf"),
        ),
        (
            F_MAKER_BOLD,
            include_bytes!("../../assets/fonts/SpaceGrotesk-Bold.ttf"),
        ),
    ];
    for (name, bytes) in faces {
        defs.font_data
            .insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
        let mut chain = vec![name.to_owned()];
        // Fall back to egui's fonts for symbols our faces lack.
        chain.extend(defs.families[&FontFamily::Proportional].iter().cloned());
        defs.families.insert(FontFamily::Name(name.into()), chain);
    }
    // Popup lists and other plain egui text use the legend face.
    defs.families
        .get_mut(&FontFamily::Proportional)
        .unwrap()
        .insert(0, F_LEGEND.to_owned());
    ctx.set_fonts(defs);
}

pub fn lerp_colour(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgba_premultiplied(
        l(a.r(), b.r()),
        l(a.g(), b.g()),
        l(a.b(), b.b()),
        l(a.a(), b.a()),
    )
}
