#![forbid(unsafe_code)]

//! A minimal, low-power desktop clock built on egui/eframe.
//!
//! The window only repaints when the displayed value actually changes (once a
//! second with seconds shown, once a minute otherwise), so it sits at ~1 FPS
//! instead of burning a core at 60 FPS.

mod app;
mod icon;

use app::ClockApp;
use eframe::egui;

fn main() -> eframe::Result<()> {
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Clock")
        .with_app_id("clock")
        .with_inner_size([900.0, 520.0])
        .with_min_inner_size([320.0, 200.0]);

    // Use the bundled SVG as the window/taskbar/dock icon when it renders.
    if let Some(icon) = icon::icon() {
        viewport = viewport.with_icon(icon);
    }

    let options = eframe::NativeOptions {
        viewport,
        // glow (OpenGL) is lighter than the default wgpu backend and plenty for 2D text.
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "Clock",
        options,
        Box::new(|cc| Ok(Box::new(ClockApp::new(cc)))),
    )
}
