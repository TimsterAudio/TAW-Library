// Hide the console window on Windows release builds.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use taw_library::{icon, ui};

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("TAW Library")
            .with_app_id("taw-library")
            .with_inner_size(ui::app::DEFAULT_SIZE)
            .with_min_inner_size(egui::Vec2::new(640.0, 400.0))
            // The Library draws its own title strip and window buttons;
            // Settings can bring back the system's.
            .with_decorations(false)
            .with_icon(icon::icon()),
        ..Default::default()
    };
    eframe::run_native(
        "TAW Library",
        options,
        Box::new(|cc| Ok(Box::new(ui::app::LibraryApp::new(cc)))),
    )
}
