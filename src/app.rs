//! Application state and rendering for the clock.

use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Datelike, Timelike, Utc};
use chrono_tz::Tz;
use eframe::egui;
use egui::{Color32, CornerRadius, FontId, Galley, Painter, Pos2, Rect, Stroke, StrokeKind};

// ---- Look ----------------------------------------------------------------
// Near-black background and a soft off-white, matching the source web clock.
const BG: Color32 = Color32::from_rgb(0x0a, 0x0b, 0x0d);
const FG: Color32 = Color32::from_rgb(0xf2, 0xf2, 0xf3);

// ---- Hour alert ----------------------------------------------------------
// Eye-friendly, desaturated tones (not neon). Amber as the hour approaches,
// red in the final stretch. Tune the two thresholds to taste.
const WARN_RGB: (u8, u8, u8) = (0xCB, 0x9A, 0x3E); // muted amber
const SOON_RGB: (u8, u8, u8) = (0xC2, 0x55, 0x50); // muted red
const WARN_FROM_MINUTE: u32 = 55; // amber starts at :55  (5 min out)
const SOON_FROM_MINUTE: u32 = 58; // red   starts at :58  (2 min out)

// ---- Zoom ----------------------------------------------------------------
const DEFAULT_SCALE: f32 = 1.8;
const MIN_SCALE: f32 = 0.4;
const MAX_SCALE: f32 = 3.0;
const SCALE_STEP: f32 = 0.1;

/// Persisted between runs via eframe's storage (the `persistence` feature).
#[derive(Clone, serde::Serialize, serde::Deserialize)]
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

/// The running clock application.
///
/// Holds the persisted user [`Settings`], the display time zone resolved from
/// the operating system at startup, and whether the window is currently
/// fullscreen.
pub struct ClockApp {
    settings: Settings,
    /// Resolved once at startup from the operating system.
    tz: Tz,
    fullscreen: bool,
}

impl ClockApp {
    /// Creates the clock, restoring saved [`Settings`] from eframe's storage and
    /// resolving the time zone from the operating system.
    ///
    /// A persisted zoom level is clamped back into the supported range, so a
    /// hand-edited or stale settings file cannot produce an unusable text size.
    ///
    /// # Returns
    ///
    /// A ready-to-run [`ClockApp`].
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut settings: Settings = cc
            .storage
            .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
            .unwrap_or_default();
        settings.scale = settings.scale.clamp(MIN_SCALE, MAX_SCALE);

        Self {
            settings,
            tz: system_timezone(),
            fullscreen: false,
        }
    }

    /// Which alert tone (if any) applies at the given minute-of-hour.
    fn alert_rgb(&self, minute: u32) -> Option<(u8, u8, u8)> {
        if !self.settings.alert_enabled {
            return None;
        }
        if minute >= SOON_FROM_MINUTE {
            Some(SOON_RGB)
        } else if minute >= WARN_FROM_MINUTE {
            Some(WARN_RGB)
        } else {
            None
        }
    }

    /// Applies one frame of keyboard and scroll input: toggles (seconds, alert,
    /// hint, fullscreen), zoom changes, and leaving fullscreen on Escape.
    fn handle_input(&mut self, ctx: &egui::Context) {
        let mut zoom_in = false;
        let mut zoom_out = false;
        let mut reset = false;
        let mut toggle_seconds = false;
        let mut toggle_alert = false;
        let mut toggle_hint = false;
        let mut toggle_fullscreen = false;
        let mut escape = false;
        let mut scroll_y = 0.0;

        ctx.input(|i| {
            use egui::Key;
            toggle_seconds = i.key_pressed(Key::S);
            toggle_alert = i.key_pressed(Key::A);
            toggle_hint = i.key_pressed(Key::H);
            toggle_fullscreen = i.key_pressed(Key::F);
            escape = i.key_pressed(Key::Escape);
            zoom_in = i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals);
            zoom_out = i.key_pressed(Key::Minus);
            reset = i.key_pressed(Key::Num0);
            scroll_y = i.smooth_scroll_delta.y;
        });

        if toggle_seconds {
            self.settings.show_seconds = !self.settings.show_seconds;
        }
        if toggle_alert {
            self.settings.alert_enabled = !self.settings.alert_enabled;
        }
        if toggle_hint {
            self.settings.show_hint = !self.settings.show_hint;
        }
        if reset {
            self.settings.scale = DEFAULT_SCALE;
        }
        if zoom_in {
            self.settings.scale = (self.settings.scale + SCALE_STEP).min(MAX_SCALE);
        }
        if zoom_out {
            self.settings.scale = (self.settings.scale - SCALE_STEP).max(MIN_SCALE);
        }
        if scroll_y != 0.0 {
            self.settings.scale =
                (self.settings.scale * (1.0 + scroll_y * 0.0015)).clamp(MIN_SCALE, MAX_SCALE);
        }

        if toggle_fullscreen {
            self.fullscreen = !self.fullscreen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.fullscreen));
        } else if escape && self.fullscreen {
            self.fullscreen = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
        }
    }

    /// Paints the full clock face for the given local time: the `HH·MM` digits
    /// (with an optional seconds subscript), the date above, the time-zone
    /// abbreviation below, and the alert border and hint line when enabled.
    fn draw(&self, ui: &egui::Ui, now: DateTime<Tz>) {
        let painter = ui.painter();
        let rect = ui.max_rect();
        let center = rect.center();

        // Font size is responsive to the window, then multiplied by the zoom.
        let base = (rect.height() * 0.30).min(rect.width() * 0.16);
        let fs = (base * self.settings.scale).clamp(20.0, 2000.0);

        let big = FontId::monospace(fs);
        let g_hh = painter.layout_no_wrap(format!("{:02}", now.hour()), big.clone(), FG);
        let g_mm = painter.layout_no_wrap(format!("{:02}", now.minute()), big, FG);
        let digit_w = g_hh.size().x;
        let digit_h = g_hh.size().y;

        let dot_d = fs * 0.13;
        let gap = fs * 0.16;
        let sec_margin = fs * 0.05;

        let sec_galley = self.settings.show_seconds.then(|| {
            painter.layout_no_wrap(
                format!(":{:02}", now.second()),
                FontId::monospace(fs * 0.34),
                FG.gamma_multiply(0.55),
            )
        });
        let sec_w = sec_galley.as_ref().map_or(0.0, |g| sec_margin + g.size().x);

        let total_w = digit_w + gap + dot_d + gap + digit_w + sec_w;
        let mut x = center.x - total_w / 2.0;
        let cy = center.y;

        painter.galley(Pos2::new(x, cy - digit_h / 2.0), g_hh, FG);
        x += digit_w + gap;

        painter.circle_filled(
            Pos2::new(x + dot_d / 2.0, cy),
            dot_d / 2.0,
            FG.gamma_multiply(0.42),
        );
        x += dot_d + gap;

        painter.galley(Pos2::new(x, cy - digit_h / 2.0), g_mm, FG);
        x += digit_w;

        if let Some(g) = sec_galley {
            x += sec_margin;
            let sh = g.size().y;
            // Sit the seconds slightly low, like a subscript.
            painter.galley(Pos2::new(x, cy - sh / 2.0 + digit_h * 0.12), g, FG);
        }

        // Date, above the time.
        let date_fs = (fs * 0.105).clamp(11.0, 40.0);
        let date = format!(
            "{}, {} {}, {}",
            now.format("%A"),
            now.format("%B"),
            now.day(),
            now.year()
        )
        .to_uppercase();
        let date_g = tracked(
            painter,
            &date,
            FontId::monospace(date_fs),
            FG.gamma_multiply(0.5),
            date_fs * 0.28,
        );
        let date_y = (cy - digit_h / 2.0) - digit_h * 0.18 - date_g.size().y;
        painter.galley(
            Pos2::new(center.x - date_g.size().x / 2.0, date_y),
            date_g,
            FG,
        );

        // Timezone abbreviation, below the time.
        let tz_fs = (fs * 0.085).clamp(10.0, 32.0);
        let tz = now.format("%Z").to_string();
        let tz_g = tracked(
            painter,
            &tz,
            FontId::monospace(tz_fs),
            FG.gamma_multiply(0.32),
            tz_fs * 0.22,
        );
        let tz_y = (cy + digit_h / 2.0) + digit_h * 0.10;
        painter.galley(Pos2::new(center.x - tz_g.size().x / 2.0, tz_y), tz_g, FG);

        if let Some(rgb) = self.alert_rgb(now.minute()) {
            draw_alert_border(painter, rect, rgb);
        }
        if self.settings.show_hint {
            draw_hint(painter, rect);
        }
    }
}

impl eframe::App for ClockApp {
    /// Background color used to clear the framebuffer before painting.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        BG.to_normalized_gamma_f32()
    }

    /// Persists the current [`Settings`] so they are restored on the next launch.
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.settings);
    }

    /// Handles input, paints one frame, and schedules the next repaint for the
    /// moment the displayed value next changes (the next second or minute).
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.handle_input(&ctx);

        let now_utc = Utc::now();
        let now = now_utc.with_timezone(&self.tz);

        // Paint our own background, then the clock on top. `clear_color` covers
        // the framebuffer, but the wrapping panel draws its own fill over that,
        // so we repaint the full rect here to guarantee the exact backdrop.
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, CornerRadius::ZERO, BG);
        self.draw(ui, now);

        // Sleep until the next boundary that changes the display.
        let wait = ms_to_next_tick(&now_utc, self.settings.show_seconds);
        ctx.request_repaint_after(Duration::from_millis(wait));
    }
}

/// Read the system IANA zone (e.g. `America/New_York`); fall back to UTC.
/// chrono-tz applies the correct DST offset for any instant, so EDT/EST style
/// transitions are handled automatically without re-reading the OS.
fn system_timezone() -> Tz {
    iana_time_zone::get_timezone()
        .ok()
        .and_then(|name| name.parse().ok())
        .unwrap_or(chrono_tz::UTC)
}

/// Milliseconds until the next second (or minute) boundary, plus a small buffer
/// so we wake just after the tick rather than a hair before it.
fn ms_to_next_tick(now: &DateTime<Utc>, show_seconds: bool) -> u64 {
    // Time-zone offsets are whole minutes everywhere, so UTC seconds and
    // sub-seconds line up exactly with the local digits we render.
    let sub_ms = (now.timestamp_subsec_millis() as i64).min(999);
    let ms = if show_seconds {
        1000 - sub_ms
    } else {
        let sec = (now.second() as i64).min(59);
        (60 - sec) * 1000 - sub_ms
    };
    const BUFFER_MS: i64 = 25;
    (ms + BUFFER_MS).clamp(30, 60_000) as u64
}

/// Lay out a single run of text with extra letter spacing (Swiss-style tracking
/// for the date / timezone / hint lines).
fn tracked(
    painter: &Painter,
    text: &str,
    font_id: FontId,
    color: Color32,
    spacing: f32,
) -> Arc<Galley> {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        text,
        0.0,
        egui::TextFormat {
            font_id,
            color,
            extra_letter_spacing: spacing,
            ..Default::default()
        },
    );
    painter.layout_job(job)
}

/// A soft, layered border in the alert color: a wide faint halo under a crisp edge.
fn draw_alert_border(painter: &Painter, rect: Rect, (r, g, b): (u8, u8, u8)) {
    let area = rect.shrink(7.0);
    let radius = CornerRadius::same(14);
    // (stroke width, alpha) from widest/faintest to thinnest/strongest.
    for (width, alpha) in [(16.0_f32, 26_u8), (7.0, 55), (2.5, 230)] {
        let color = Color32::from_rgba_unmultiplied(r, g, b, alpha);
        painter.rect_stroke(area, radius, Stroke::new(width, color), StrokeKind::Inside);
    }
}

fn draw_hint(painter: &Painter, rect: Rect) {
    let text = "S seconds   \u{00B7}   +/- zoom   \u{00B7}   0 reset   \u{00B7}   A alert   \u{00B7}   F fullscreen   \u{00B7}   H hide";
    let g = tracked(
        painter,
        text,
        FontId::monospace(11.0),
        FG.gamma_multiply(0.16),
        1.0,
    );
    let x = rect.center().x - g.size().x / 2.0;
    let y = rect.bottom() - g.size().y - 16.0;
    painter.galley(Pos2::new(x, y), g, FG);
}
