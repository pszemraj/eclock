#![forbid(unsafe_code)]
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

//! A minimal, low-power desktop clock built on egui/eframe.
//!
//! Idle repaints follow the displayed second or minute; tray events wake the
//! event loop to restore the window without continuous polling.

mod app;
mod icon;
mod platform;
mod tray;

use app::ClockApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let tray_supported = platform::tray_supported();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("eclock")
        .with_app_id("eclock")
        .with_inner_size([900.0, 520.0])
        .with_min_inner_size(app::MIN_WINDOW_SIZE)
        .with_decorations(false)
        .with_resizable(true);

    if let Some(icon) = icon::icon() {
        viewport = viewport.with_icon(icon);
    }

    let mut options = eframe::NativeOptions {
        viewport,
        // glow (OpenGL) is lighter than the default wgpu backend and plenty for 2D text.
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    platform::configure(&mut options, tray_supported);

    eframe::run_native(
        "eclock",
        options,
        Box::new(move |cc| Ok(Box::new(ClockApp::new(cc, tray_supported)))),
    )
}
