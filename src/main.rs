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
mod persistence;
mod platform;
mod tray;

use app::ClockApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let tray_supported = platform::tray_supported();
    let legacy_window = persistence::load_legacy_window();
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("eclock")
        .with_app_id("eclock")
        .with_inner_size(
            legacy_window
                .and_then(|window| window.inner_size_points)
                .unwrap_or(egui::vec2(900.0, 520.0)),
        )
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
        // The app tracks normal geometry before fullscreen/maximize transitions.
        persist_window: false,
        ..Default::default()
    };
    platform::configure(&mut options, tray_supported);

    eframe::run_native_ext(
        "eclock",
        options,
        Some(persistence::egui_context()),
        Box::new(move |cc| Ok(Box::new(ClockApp::new(cc, tray_supported, legacy_window)))),
    )
}
