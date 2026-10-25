//! Clock preferences, native window actions, and responsive clock-face rendering.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Timelike, Utc};
use chrono_tz::Tz;
use eframe::egui;
use egui::{Color32, CornerRadius, FontId, Galley, Painter, Pos2, Rect, Stroke, StrokeKind, Vec2};

use crate::tray::{Action, Tray};

const BG: Color32 = Color32::from_rgb(0x0a, 0x0b, 0x0d);
const FG: Color32 = Color32::from_rgb(0xf2, 0xf2, 0xf3);
const WARN_RGB: (u8, u8, u8) = (0xCB, 0x9A, 0x3E);
const SOON_RGB: (u8, u8, u8) = (0xC2, 0x55, 0x50);
const WARN_FROM_MINUTE: u32 = 55;
const SOON_FROM_MINUTE: u32 = 58;
const DEFAULT_SCALE: f32 = 1.8;
const MIN_SCALE: f32 = 0.4;
const MAX_SCALE: f32 = 3.0;
const SCALE_STEP: f32 = 0.1;
const HINT: &str = "S\u{a0}seconds  ·  +/-\u{a0}zoom  ·  0\u{a0}reset  ·  A\u{a0}alert  ·  F\u{a0}fullscreen  ·  H\u{a0}hints";

/// Preferences persisted in eframe's app.ron file.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
struct Settings {
    show_seconds: bool,
    scale: f32,
    alert_enabled: bool,
    show_hint: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            show_seconds: false,
            scale: DEFAULT_SCALE,
            alert_enabled: false,
            show_hint: true,
        }
    }
}

impl Settings {
    /// Restores saved preferences and clamps zoom to its supported range.
    fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        let mut settings: Self = storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_default();
        settings.scale = settings.scale.clamp(MIN_SCALE, MAX_SCALE);
        settings
    }
}

/// Clock preferences and the lifetime of its native window and tray.
pub struct ClockApp {
    settings: Settings,
    tz: Tz,
    fullscreen: bool,
    tray: Option<Tray>,
    quitting: bool,
    taskbar_hidden: Option<bool>,
    #[cfg(target_os = "macos")]
    resizing: Option<(Rect, Pos2, egui::ResizeDirection)>,
}

impl ClockApp {
    /// Initializes the clock inside the running native event loop.
    ///
    /// # Arguments
    /// - `cc` - Native window, egui context, and persisted application storage.
    /// - `tray_supported` - Whether the window backend supports tray hide/restore.
    ///
    /// # Returns
    /// A clock with restored preferences, the system time zone, and a tray when
    /// native tray creation succeeds.
    pub fn new(cc: &eframe::CreationContext<'_>, tray_supported: bool) -> Self {
        let tray = if tray_supported {
            match Tray::new(&cc.egui_ctx) {
                Ok(tray) => Some(tray),
                Err(error) => {
                    eprintln!("Tray unavailable: {error}");
                    None
                }
            }
        } else {
            None
        };
        let mut app = Self {
            settings: Settings::load(cc.storage),
            tz: system_timezone(),
            fullscreen: false,
            tray,
            quitting: false,
            taskbar_hidden: None,
            #[cfg(target_os = "macos")]
            resizing: None,
        };
        app.sync_taskbar(cc.winit_window().map(AsRef::as_ref));
        app
    }

    /// Selects the hourly alert color, if alerts are enabled.
    fn alert_rgb(&self, minute: u32) -> Option<(u8, u8, u8)> {
        if !self.settings.alert_enabled {
            None
        } else if minute >= SOON_FROM_MINUTE {
            Some(SOON_RGB)
        } else if minute >= WARN_FROM_MINUTE {
            Some(WARN_RGB)
        } else {
            None
        }
    }

    /// Applies a tray action; explicit Quit bypasses close-to-tray handling.
    fn window_action(&mut self, ctx: &egui::Context, action: Action) {
        use egui::ViewportCommand;
        match action {
            Action::Show => {
                self.taskbar_hidden = None;
                ctx.send_viewport_cmd(ViewportCommand::Visible(true));
                ctx.send_viewport_cmd(ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(ViewportCommand::Focus);
            }
            Action::Hide => {
                ctx.send_viewport_cmd(ViewportCommand::Visible(false));
            }
            Action::Quit => {
                self.quitting = true;
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }

    /// Cancels native closing only when it can hide to a usable tray.
    fn close_window(&mut self, ctx: &egui::Context, tray_available: bool) {
        if tray_available && !self.quitting {
            self.window_action(ctx, Action::Hide);
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
        }
    }

    /// Applies taskbar presence initially and before revealing an unmapped window.
    fn sync_taskbar(&mut self, window: Option<&winit::window::Window>) {
        let hidden = self.tray.is_some();
        if self.taskbar_hidden == Some(hidden) {
            return;
        }
        if let Some(window) = window {
            if let Err(error) = crate::platform::set_taskbar_hidden(window, hidden) {
                eprintln!("Could not update taskbar presence: {error}");
            }
            self.taskbar_hidden = Some(hidden);
        }
    }

    /// Handles keyboard toggles, fullscreen controls, and zoom.
    fn handle_input(&mut self, ctx: &egui::Context) {
        let was_fullscreen = self.fullscreen;
        ctx.input(|input| {
            use egui::Key;
            if input.key_pressed(Key::S) {
                self.settings.show_seconds = !self.settings.show_seconds;
            }
            if input.key_pressed(Key::A) {
                self.settings.alert_enabled = !self.settings.alert_enabled;
            }
            if input.key_pressed(Key::H) {
                self.settings.show_hint = !self.settings.show_hint;
            }
            if input.key_pressed(Key::Num0) {
                self.settings.scale = DEFAULT_SCALE;
            }
            if input.key_pressed(Key::Plus) || input.key_pressed(Key::Equals) {
                self.settings.scale = (self.settings.scale + SCALE_STEP).min(MAX_SCALE);
            }
            if input.key_pressed(Key::Minus) {
                self.settings.scale = (self.settings.scale - SCALE_STEP).max(MIN_SCALE);
            }
            if input.smooth_scroll_delta.y != 0.0 {
                self.settings.scale = (self.settings.scale
                    * (1.0 + input.smooth_scroll_delta.y * 0.0015))
                    .clamp(MIN_SCALE, MAX_SCALE);
            }
            if input.key_pressed(Key::F) {
                self.fullscreen = !self.fullscreen;
            } else if input.key_pressed(Key::Escape) && self.fullscreen {
                self.fullscreen = false;
            }
        });
        if self.fullscreen != was_fullscreen {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
        }
    }

    /// Handles background dragging and resizing through invisible window edges.
    fn handle_pointer(&mut self, ui: &egui::Ui) {
        if self.fullscreen {
            return;
        }
        let ctx = ui.ctx();
        let (position, pressed) = ctx.input(|input| {
            (
                input.pointer.interact_pos(),
                input.pointer.primary_pressed(),
            )
        });
        let Some(position) = position else {
            return;
        };
        #[cfg(target_os = "macos")]
        if self.resizing.is_some() {
            self.resize_macos(ctx, position);
            return;
        }
        if !ui.max_rect().contains(position) {
            return;
        }
        let direction = resize_direction(ui.max_rect(), position);
        if let Some(direction) = direction {
            ctx.set_cursor_icon(match direction {
                egui::ResizeDirection::North | egui::ResizeDirection::South => {
                    egui::CursorIcon::ResizeVertical
                }
                egui::ResizeDirection::East | egui::ResizeDirection::West => {
                    egui::CursorIcon::ResizeHorizontal
                }
                egui::ResizeDirection::NorthEast | egui::ResizeDirection::SouthWest => {
                    egui::CursorIcon::ResizeNeSw
                }
                egui::ResizeDirection::NorthWest | egui::ResizeDirection::SouthEast => {
                    egui::CursorIcon::ResizeNwSe
                }
            });
        }
        if pressed {
            #[cfg(target_os = "macos")]
            if let Some(direction) = direction {
                if let Some(rect) = ctx.input(|input| input.viewport().outer_rect) {
                    self.resizing = Some((rect, rect.min + position.to_vec2(), direction));
                }
                return;
            }
            ctx.send_viewport_cmd(direction.map_or(
                egui::ViewportCommand::StartDrag,
                egui::ViewportCommand::BeginResize,
            ));
        }
    }

    /// Resizes macOS windows directly because winit's BeginResize is unsupported there.
    #[cfg(target_os = "macos")]
    fn resize_macos(&mut self, ctx: &egui::Context, position: Pos2) {
        let (down, current) =
            ctx.input(|input| (input.pointer.primary_down(), input.viewport().outer_rect));
        if !down {
            self.resizing = None;
            return;
        }
        if let (Some((start, pointer, direction)), Some(current)) = (self.resizing, current) {
            let delta = current.min + position.to_vec2() - pointer;
            let rect = resized_rect(start, delta, direction);
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(rect.min));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(rect.size()));
        }
    }

    /// Paints a measured clock face and exposes its readouts to accessibility/inspection.
    fn draw(&self, ui: &egui::Ui, now: DateTime<Tz>) {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, CornerRadius::ZERO, BG);
        let layout = ClockLayout::new(ui.painter(), rect, now, &self.settings);
        layout.paint(ui, now);
        if let Some(rgb) = self.alert_rgb(now.minute()) {
            draw_alert_border(ui.painter(), rect, rgb);
        }
    }
}

impl eframe::App for ClockApp {
    /// Clears the native framebuffer to the clock background.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        BG.to_normalized_gamma_f32()
    }

    /// Persists preferences on eframe's normal autosave and shutdown paths.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.settings);
    }

    /// Processes native events even when hidden, without a periodic tray poll.
    fn logic(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        while let Some(action) = self.tray.as_ref().and_then(Tray::next_action) {
            self.window_action(ctx, action);
        }
        if ctx.input(|input| input.viewport().close_requested()) {
            let available = self.tray.is_some() && crate::platform::tray_supported();
            self.close_window(ctx, available);
        }
        self.sync_taskbar(frame.winit_window().map(AsRef::as_ref));
    }

    /// Renders the current local time and schedules its next second/minute boundary.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.handle_input(ui.ctx());
        self.handle_pointer(ui);
        let now = Utc::now();
        self.draw(ui, now.with_timezone(&self.tz));
        ui.ctx()
            .request_repaint_after(Duration::from_millis(ms_to_next_tick(
                &now,
                self.settings.show_seconds,
            )));
    }
}

/// Measured hours, minutes, and optional subscript seconds at one font size.
struct TimeText {
    hours: Arc<Galley>,
    minutes: Arc<Galley>,
    seconds: Option<Arc<Galley>>,
    font_size: f32,
}

impl TimeText {
    /// Shapes all time digits so fitting accounts for the optional seconds.
    fn new(painter: &Painter, now: DateTime<Tz>, seconds: bool, font_size: f32) -> Self {
        let font = FontId::monospace(font_size);
        Self {
            hours: painter.layout_no_wrap(format!("{:02}", now.hour()), font.clone(), FG),
            minutes: painter.layout_no_wrap(format!("{:02}", now.minute()), font, FG),
            seconds: seconds.then(|| {
                painter.layout_no_wrap(
                    format!(":{:02}", now.second()),
                    FontId::monospace(font_size * 0.34),
                    FG.gamma_multiply(0.65),
                )
            }),
            font_size,
        }
    }

    /// Returns the complete time row's dimensions, including dot, gaps, and seconds.
    fn size(&self) -> Vec2 {
        Vec2::new(
            self.hours.size().x
                + self.minutes.size().x
                + self.font_size * 0.45
                + self
                    .seconds
                    .as_ref()
                    .map_or(0.0, |text| self.font_size * 0.05 + text.size().x),
            self.hours.size().y.max(self.minutes.size().y),
        )
    }
}

/// Clock text and non-overlapping rectangles within the padded native viewport.
struct ClockLayout {
    time: TimeText,
    time_rect: Rect,
    date: Arc<Galley>,
    date_rect: Rect,
    zone: Arc<Galley>,
    zone_rect: Rect,
    hint: Option<Arc<Galley>>,
    hint_rect: Option<Rect>,
}

impl ClockLayout {
    /// Fits measured time digits after reserving independent caption and hint space.
    fn new(painter: &Painter, rect: Rect, now: DateTime<Tz>, settings: &Settings) -> Self {
        let short_side = rect.width().min(rect.height());
        let padding = (short_side * 0.04).clamp(10.0, 24.0);
        let gap = (short_side * 0.035).clamp(8.0, 24.0);
        let mut body = rect.shrink(padding);
        let width = body.width();
        let date = caption(
            painter,
            &now.format("%A, %B %-d, %Y").to_string().to_uppercase(),
            (short_side * 0.045).clamp(14.0, 24.0),
            FG.gamma_multiply(0.65),
            0.8,
            width,
        );
        let zone = caption(
            painter,
            &now.format("%Z").to_string(),
            (short_side * 0.035).clamp(12.0, 18.0),
            FG.gamma_multiply(0.5),
            0.8,
            width,
        );
        let hint = settings
            .show_hint
            .then(|| caption(painter, HINT, 12.0, FG.gamma_multiply(0.4), 0.0, width));
        let hint_rect = hint.as_ref().map(|text| {
            let rect = Rect::from_min_size(
                Pos2::new(
                    body.center().x - text.size().x / 2.0,
                    body.bottom() - text.size().y,
                ),
                text.size(),
            );
            body.max.y = rect.top() - gap;
            rect
        });
        let available = Vec2::new(
            width,
            (body.height() - date.size().y - zone.size().y - gap * 2.0).max(1.0),
        );
        let requested = (rect.height() * 0.30).min(rect.width() * 0.16) * settings.scale;
        let mut time = TimeText::new(painter, now, settings.show_seconds, requested);
        loop {
            let size = time.size();
            let factor = (available.x / size.x).min(available.y / size.y).min(1.0);
            let fitted = (time.font_size * factor).floor().max(1.0);
            if factor >= 1.0 || fitted >= time.font_size {
                break;
            }
            time = TimeText::new(painter, now, settings.show_seconds, fitted);
        }
        let height = date.size().y + time.size().y + zone.size().y + gap * 2.0;
        let mut y = body.center().y - height / 2.0;
        let date_rect = centered_row(body.center().x, y, date.size());
        y = date_rect.bottom() + gap;
        let time_rect = centered_row(body.center().x, y, time.size());
        y = time_rect.bottom() + gap;
        let zone_rect = centered_row(body.center().x, y, zone.size());
        Self {
            time,
            time_rect,
            date,
            date_rect,
            zone,
            zone_rect,
            hint,
            hint_rect,
        }
    }

    /// Paints text at its measured bounds and registers matching semantic labels.
    fn paint(&self, ui: &egui::Ui, now: DateTime<Tz>) {
        let painter = ui.painter();
        let fs = self.time.font_size;
        let mut position = self.time_rect.min;
        paint_text(painter, position, &self.time.hours);
        position.x += self.time.hours.size().x + fs * 0.16;
        painter.circle_filled(
            Pos2::new(position.x + fs * 0.065, self.time_rect.center().y),
            fs * 0.065,
            FG.gamma_multiply(0.42),
        );
        position.x += fs * 0.29;
        paint_text(painter, position, &self.time.minutes);
        if let Some(seconds) = &self.time.seconds {
            position.x += self.time.minutes.size().x + fs * 0.05;
            position.y =
                self.time_rect.center().y - seconds.size().y / 2.0 + self.time_rect.height() * 0.12;
            paint_text(painter, position, seconds);
        }
        semantic_label(
            ui,
            "time",
            self.time_rect,
            format!(
                "Time: {}",
                now.format(if self.time.seconds.is_some() {
                    "%H:%M:%S"
                } else {
                    "%H:%M"
                })
            ),
        );
        paint_text(painter, self.date_rect.min, &self.date);
        semantic_label(
            ui,
            "date",
            self.date_rect,
            format!("Date: {}", self.date.text()),
        );
        paint_text(painter, self.zone_rect.min, &self.zone);
        semantic_label(
            ui,
            "zone",
            self.zone_rect,
            format!("Time zone: {}", self.zone.text()),
        );
        if let (Some(hint), Some(rect)) = (&self.hint, self.hint_rect) {
            paint_text(painter, rect.min, hint);
            semantic_label(ui, "hints", rect, HINT.to_owned());
        }
    }
}

/// Centers a measured text row horizontally at the given top coordinate.
fn centered_row(center_x: f32, top: f32, size: Vec2) -> Rect {
    Rect::from_min_size(Pos2::new(center_x - size.x / 2.0, top), size)
}

/// Paints a galley with its full layout bounds aligned to a top-left position.
fn paint_text(painter: &Painter, position: Pos2, text: &Arc<Galley>) {
    painter.galley(position - text.rect.min.to_vec2(), text.clone(), FG);
}

/// Registers readable content and logical bounds for directly painted text.
fn semantic_label(ui: &egui::Ui, id: &str, rect: Rect, text: String) {
    ui.interact(rect, ui.scope_id().with(id), egui::Sense::hover())
        .widget_info(|| egui::WidgetInfo::labeled(egui::Role::Label, true, &text));
}

/// Shapes a centered, wrapping caption with explicit font size and tracking.
fn caption(
    painter: &Painter,
    text: &str,
    size: f32,
    color: Color32,
    spacing: f32,
    width: f32,
) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id: FontId::monospace(size),
            color,
            extra_letter_spacing: spacing,
            ..Default::default()
        },
    );
    job.halign = egui::Align::Center;
    job.wrap.max_width = width;
    painter.layout_job(job)
}

/// Maps a six-point invisible edge or corner to its native resize direction.
fn resize_direction(rect: Rect, position: Pos2) -> Option<egui::ResizeDirection> {
    use egui::ResizeDirection::*;
    let left = position.x < rect.left() + 6.0;
    let right = position.x > rect.right() - 6.0;
    let top = position.y < rect.top() + 6.0;
    let bottom = position.y > rect.bottom() - 6.0;
    match (left, right, top, bottom) {
        (true, _, true, _) => Some(NorthWest),
        (_, true, true, _) => Some(NorthEast),
        (true, _, _, true) => Some(SouthWest),
        (_, true, _, true) => Some(SouthEast),
        (true, _, _, _) => Some(West),
        (_, true, _, _) => Some(East),
        (_, _, true, _) => Some(North),
        (_, _, _, true) => Some(South),
        _ => None,
    }
}

/// Resizes from a chosen edge while anchoring the opposite edge at the minimum size.
#[cfg(any(target_os = "macos", test))]
fn resized_rect(mut rect: Rect, delta: Vec2, direction: egui::ResizeDirection) -> Rect {
    use egui::ResizeDirection::*;
    if matches!(direction, West | NorthWest | SouthWest) {
        rect.min.x = (rect.min.x + delta.x).min(rect.max.x - 320.0);
    }
    if matches!(direction, East | NorthEast | SouthEast) {
        rect.max.x = (rect.max.x + delta.x).max(rect.min.x + 320.0);
    }
    if matches!(direction, North | NorthWest | NorthEast) {
        rect.min.y = (rect.min.y + delta.y).min(rect.max.y - 200.0);
    }
    if matches!(direction, South | SouthWest | SouthEast) {
        rect.max.y = (rect.max.y + delta.y).max(rect.min.y + 200.0);
    }
    rect
}

/// Resolves the system IANA zone once; chrono-tz supplies DST transitions.
fn system_timezone() -> Tz {
    iana_time_zone::get_timezone()
        .ok()
        .and_then(|name| name.parse().ok())
        .unwrap_or(chrono_tz::UTC)
}

/// Returns milliseconds through the next displayed tick, with a 25 ms buffer.
fn ms_to_next_tick(now: &DateTime<Utc>, show_seconds: bool) -> u64 {
    let sub_ms = i64::from(now.timestamp_subsec_millis()).min(999);
    let ms = if show_seconds {
        1000 - sub_ms
    } else {
        (60 - i64::from(now.second()).min(59)) * 1000 - sub_ms
    };
    (ms + 25).clamp(30, 60_000) as u64
}

/// Paints the optional hourly alert as a faint halo beneath a crisp colored edge.
fn draw_alert_border(painter: &Painter, rect: Rect, (r, g, b): (u8, u8, u8)) {
    let area = rect.shrink(7.0);
    for (width, alpha) in [(16.0_f32, 26_u8), (7.0, 55), (2.5, 230)] {
        painter.rect_stroke(
            area,
            CornerRadius::same(14),
            Stroke::new(width, Color32::from_rgba_unmultiplied(r, g, b, alpha)),
            StrokeKind::Inside,
        );
    }
}

#[cfg(test)]
mod tests;
