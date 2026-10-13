// Hide the console window on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use taw_library::{icon, ui};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TAW Library")
            .with_app_id("taw-library")
            .with_inner_size(ui::app::DESIGN_MIN)
            .with_min_inner_size(ui::app::DESIGN_MIN * 0.5)
            .with_icon(icon::icon()),
        ..Default::default()
    };
    eframe::run_native(
        "TAW Library",
        options,
        Box::new(|cc| Ok(Box::new(ui::app::LibraryApp::new(cc)))),
    )
}
