//! Native window geometry, fixed UI zoom, and previous-format compatibility.

use std::{collections::HashMap, fs::File, sync::Arc};

use eframe::egui::{self, Pos2, Vec2};
use egui_winit::{WindowGeometry, WindowSettings};
use winit::window::Window;

/// Creates the native UI context without restoring framework-wide zoom.
/// Clock text scale remains in the application's own persisted preferences.
pub(crate) fn egui_context() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.options_mut(|options| options.zoom_with_keyboard = false);
    ctx
}

/// Geometry written by egui-winit 0.34, before `normal_geometry` was introduced.
#[derive(Clone, Copy, Debug, Default, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct LegacyWindow {
    inner_position_pixels: Option<Pos2>,
    outer_position_pixels: Option<Pos2>,
    pub(crate) inner_size_points: Option<Vec2>,
    fullscreen: bool,
    maximized: bool,
}

/// Reads only legacy native geometry; eframe loads the current format directly.
/// Missing or unreadable state uses the framework's ordinary defaults.
pub(crate) fn load_legacy_window() -> Option<LegacyWindow> {
    let path = eframe::storage_dir("eclock")?.join("app.ron");
    let storage: HashMap<String, String> = ron::de::from_reader(File::open(path).ok()?).ok()?;
    ron::from_str(storage.get("window")?).ok()
}

/// Seeds Linux's position cache from saved geometry, including fullscreen startups.
/// Other platforms retain the framework's monitor-validated native placement.
#[cfg(target_os = "linux")]
pub(crate) fn normal_position(cc: &eframe::CreationContext<'_>) -> Option<Pos2> {
    #[derive(serde::Deserialize)]
    struct SavedWindow {
        normal_geometry: SavedGeometry,
    }
    #[derive(serde::Deserialize)]
    struct SavedGeometry {
        outer_position_pixels: Option<Pos2>,
    }
    let storage = cc.storage?;
    let position = eframe::get_value::<SavedWindow>(storage, "window")
        .and_then(|saved| saved.normal_geometry.outer_position_pixels)
        .or_else(|| {
            eframe::get_value::<LegacyWindow>(storage, "window")
                .and_then(|saved| saved.outer_position_pixels.or(saved.inner_position_pixels))
        })?;
    Some(position / cc.winit_window()?.scale_factor() as f32)
}

impl LegacyWindow {
    /// Requests legacy placement and restores maximized/fullscreen state.
    /// Seeds normal geometry independently of asynchronous native placement.
    pub(crate) fn restore(self, window: Arc<Window>, ctx: &egui::Context) -> WindowState {
        let initial_position = self
            .outer_position_pixels
            .or(self.inner_position_pixels)
            .map(|position| winit::dpi::PhysicalPosition::new(position.x, position.y));
        if let Some(position) = initial_position {
            window.set_outer_position(position);
        }
        #[derive(serde::Serialize)]
        struct Geometry {
            outer_position_pixels: Option<Pos2>,
            scale_factor: f32,
            inner_size_points: Vec2,
        }
        // WindowGeometry's fields are private; deserialize the native geometry shape.
        let geometry = Geometry {
            outer_position_pixels: self.outer_position_pixels.or(self.inner_position_pixels),
            scale_factor: window.scale_factor() as f32,
            inner_size_points: self.inner_size_points.unwrap_or_else(|| {
                let size = window.inner_size().to_logical::<f32>(window.scale_factor());
                Vec2::new(size.width, size.height)
            }),
        };
        let serialized = ron::to_string(&geometry).expect("native geometry serializes to RON");
        let restoration = WindowState {
            normal_geometry: ron::from_str(&serialized).expect("egui-winit native geometry format"),
            window,
            initial_position,
        };
        if self.maximized {
            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(true));
        }
        if self.fullscreen {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        }
        restoration
    }
}

/// Tracks normal native geometry and saves it in eframe's window-state format.
pub(crate) struct WindowState {
    window: Arc<Window>,
    normal_geometry: WindowGeometry,
    initial_position: Option<winit::dpi::PhysicalPosition<f32>>,
}

impl WindowState {
    /// Restores the native normal geometry without inheriting framework UI zoom.
    pub(crate) fn new(window: Arc<Window>, storage: Option<&dyn eframe::Storage>) -> Self {
        Self {
            normal_geometry: storage
                .and_then(|storage| eframe::get_value::<WindowSettings>(storage, "window"))
                .map(|settings| settings.normal_geometry())
                .unwrap_or_else(|| WindowGeometry::from_window(1.0, &window)),
            window,
            initial_position: None,
        }
    }

    /// Reports whether legacy placement is awaiting a visible, windowed frame.
    #[cfg(target_os = "linux")]
    pub(crate) fn is_position_pending(&self) -> bool {
        self.initial_position.is_some()
    }

    /// Records visible windowed geometry and applies pending legacy placement.
    pub(crate) fn observe(&mut self) {
        if self.window.is_visible() != Some(false)
            && !self.window.is_maximized()
            && self.window.fullscreen().is_none()
        {
            // Window managers can place the window themselves when it first maps.
            if let Some(position) = self.initial_position.take() {
                self.window.set_outer_position(position);
            }
            self.normal_geometry = WindowGeometry::from_window(1.0, &self.window);
        }
    }

    /// Saves the current native format with the latest normal geometry.
    pub(crate) fn save(&mut self, storage: &mut dyn eframe::Storage) {
        self.observe();
        let settings = WindowSettings::from_window(1.0, &self.window, Some(self.normal_geometry));
        eframe::set_value(storage, "window", &settings);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_geometry_retains_size_position_and_window_state() {
        let saved = r#"(
            inner_position_pixels: Some((x: 2270.0, y: 1057.0)),
            outer_position_pixels: Some((x: 2260.0, y: 1025.0)),
            inner_size_points: Some((x: 900.0, y: 520.0)),
            fullscreen: true,
            maximized: true,
        )"#;
        let window: LegacyWindow = ron::from_str(saved).unwrap();
        assert_eq!(window.inner_size_points, Some(egui::vec2(900.0, 520.0)));
        assert_eq!(
            window.outer_position_pixels,
            Some(egui::pos2(2260.0, 1025.0))
        );
        assert_eq!(
            window.inner_position_pixels,
            Some(egui::pos2(2270.0, 1057.0))
        );
        assert!(window.fullscreen && window.maximized);
    }

    #[test]
    fn current_native_geometry_is_not_misread_as_legacy_defaults() {
        let saved = r#"(
            normal_geometry: (
                outer_position_pixels: Some((x: 2270.0, y: 1057.0)),
                scale_factor: 1.0,
                inner_size_points: (x: 900.0, y: 520.0),
            ),
            fullscreen: false,
            maximized: false,
        )"#;
        assert!(ron::from_str::<WindowSettings>(saved).is_ok());
        assert!(ron::from_str::<LegacyWindow>(saved).is_err());
    }
}
