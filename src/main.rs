#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod core;
mod gui;

use eframe::egui;
use gui::app::DuckDnsApp;

fn main() -> eframe::Result<()> {
    let config = core::config::AppConfig::load();
    let start_minimized = config.start_minimized;

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("DuckDNS Updater")
            .with_inner_size([520.0, 580.0])
            .with_resizable(false)
            .with_visible(!start_minimized)
            .with_icon(load_app_icon()),
        ..Default::default()
    };

    eframe::run_native(
        "DuckDNS Updater",
        options,
        Box::new(|cc| Ok(Box::new(DuckDnsApp::new(cc)))),
    )
}

fn load_app_icon() -> egui::IconData {
    let icon_bytes = include_bytes!("../assets/icon.ico");
    if let Ok(img) = image::load_from_memory(icon_bytes) {
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        }
    } else {
        let width = 32;
        let height = 32;
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);

        for y in 0..height {
            for x in 0..width {
                let dx = x as f32 - 15.5;
                let dy = y as f32 - 15.5;
                if dx * dx + dy * dy <= 14.0 * 14.0 {
                    rgba.extend_from_slice(&[20, 140, 220, 255]);
                } else {
                    rgba.extend_from_slice(&[0, 0, 0, 0]);
                }
            }
        }

        egui::IconData {
            rgba,
            width,
            height,
        }
    }
}
