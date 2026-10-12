// Hide the console window on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use music_box::{icon, ui};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Music Box")
            .with_app_id("music-box")
            .with_inner_size(ui::app::DESIGN_MIN)
            .with_min_inner_size(ui::app::DESIGN_MIN * 0.5)
            .with_icon(icon::icon()),
        ..Default::default()
    };
    eframe::run_native(
        "Music Box",
        options,
        Box::new(|cc| Ok(Box::new(ui::app::MusicBox::new(cc)))),
    )
}
