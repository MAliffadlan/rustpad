mod app;
mod config;
mod dialogs;
mod document;
mod editor;
mod file_dialog;
mod find;
mod lang;
mod text_index;
mod theme;

use std::path::PathBuf;
use std::sync::Arc;

const WINDOW_ICON_RGBA: &[u8] = include_bytes!("../assets/rustpad-window.rgba");

fn main() -> eframe::Result<()> {
    let config_path = config::config_path();
    let mut config = config_path
        .as_deref()
        .map(config::load_from)
        .unwrap_or_default();
    config.prune_recent();

    let cli_paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();

    debug_assert_eq!(WINDOW_ICON_RGBA.len(), 64 * 64 * 4);
    let window_icon = egui::IconData {
        rgba: WINDOW_ICON_RGBA.to_vec(),
        width: 64,
        height: 64,
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Rustpad")
            .with_app_id("rustpad")
            .with_inner_size([1080.0, 720.0])
            .with_min_inner_size([520.0, 360.0])
            .with_icon(Arc::new(window_icon)),
        persist_window: true,
        ..Default::default()
    };

    eframe::run_native(
        "Rustpad",
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::RustpadApp::new(
                cc,
                cli_paths,
                config,
                config_path,
            )))
        }),
    )
}
