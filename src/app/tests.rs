//! Headless layout, window-lifecycle, persistence, and manual smoke scenarios.

use std::collections::BTreeMap;

use chrono::TimeZone;
use eframe::{App, Storage};

use super::*;

const VIEWPORT_SIZES: [[f32; 2]; 7] = [
    [320.0, 200.0],
    [480.0, 320.0],
    [900.0, 520.0],
    [1440.0, 760.0],
    [480.0, 2496.0],
    [2560.0, 400.0],
    [1400.0, 1332.0],
];

#[derive(Default)]
struct MemoryStorage(BTreeMap<String, String>);

impl eframe::Storage for MemoryStorage {
    fn get_string(&self, key: &str) -> Option<String> {
        self.0.get(key).cloned()
    }

    fn set_string(&mut self, key: &str, value: String) {
        self.0.insert(key.to_owned(), value);
    }

    fn remove_string(&mut self, key: &str) {
        self.0.remove(key);
    }

    fn flush(&mut self) {}
}

/// Constructs a clock without native resources for headless scenarios.
fn clock() -> ClockApp {
    ClockApp {
        settings: Settings::default(),
        tz: chrono_tz::America::New_York,
        tray: None,
        quitting: false,
        #[cfg(target_os = "windows")]
        taskbar_hidden: None,
        window_state: None,
        windowed_position: None,
        shortcuts_open: false,
        #[cfg(target_os = "macos")]
        resizing: None,
    }
}

/// Uses a long date and wide time digits to exercise caption wrapping and fitting.
fn example_time() -> DateTime<Tz> {
    chrono_tz::America::New_York
        .with_ymd_and_hms(2026, 9, 30, 23, 58, 59)
        .unwrap()
}

/// Runs a UI frame without a renderer and discards its texture uploads explicitly.
fn run_headless(
    ctx: &egui::Context,
    input: egui::RawInput,
    ui: impl FnMut(&mut egui::Ui),
) -> egui::FullOutput {
    let mut output = ctx.run_ui(input, ui);
    output.textures_delta.clear();
    output
}

/// Builds a single logical key press with explicit modifiers.
fn key_input(key: egui::Key, modifiers: egui::Modifiers) -> egui::RawInput {
    egui::RawInput {
        events: vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }],
        ..Default::default()
    }
}

/// Checks that every text rectangle fits and that adjacent rows remain separate.
fn check_layout(layout: &ClockLayout, viewport: Rect) {
    let mut rows = vec![layout.date_rect, layout.time_rect, layout.zone_rect];
    if let Some((_, rect)) = &layout.hint {
        rows.push(*rect);
    }
    for row in &rows {
        assert!(viewport.contains_rect(*row), "{row:?} outside {viewport:?}");
    }
    for pair in rows.windows(2) {
        assert!(
            pair[0].bottom() <= pair[1].top(),
            "overlapping rows: {pair:?}"
        );
    }
}

#[test]
fn full_clock_fits_small_tiles_and_large_windows_at_all_zoom_limits() {
    let ctx = egui::Context::default();
    for [width, height] in VIEWPORT_SIZES {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height));
        for scale in [MIN_SCALE, DEFAULT_SCALE, MAX_SCALE] {
            for show_seconds in [false, true] {
                for show_hint in [false, true] {
                    let settings = Settings {
                        scale,
                        show_seconds,
                        show_hint,
                        ..Settings::default()
                    };
                    let _ = run_headless(
                        &ctx,
                        egui::RawInput {
                            screen_rect: Some(rect),
                            ..Default::default()
                        },
                        |ui| {
                            let layout =
                                ClockLayout::new(ui.painter(), rect, example_time(), &settings);
                            check_layout(&layout, rect);
                        },
                    );
                }
            }
        }
    }
}

#[test]
fn large_window_captions_use_large_fonts() {
    let ctx = egui::Context::default();
    let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(1400.0, 1332.0));
    let _ = run_headless(
        &ctx,
        egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        },
        |ui| {
            let layout = ClockLayout::new(ui.painter(), rect, example_time(), &Settings::default());
            for (text, minimum_size) in [
                (&layout.date, 40.0),
                (&layout.zone, 32.0),
                (&layout.hint.as_ref().unwrap().0, 24.0),
            ] {
                assert!(text.job.sections[0].format.font_id.size >= minimum_size);
                check_layout(&layout, rect);
            }
        },
    );
}

#[test]
fn existing_settings_restore_and_survive_save_reload() {
    let mut storage = MemoryStorage::default();
    storage.set_string(
        eframe::APP_KEY,
        "(show_seconds:true,scale:2.3,alert_enabled:true,show_hint:false)".into(),
    );
    let mut app = clock();
    app.settings = Settings::load(Some(&storage));
    assert_eq!(
        app.settings,
        Settings {
            show_seconds: true,
            scale: 2.3,
            alert_enabled: true,
            show_hint: false
        }
    );
    app.save(&mut storage);
    assert_eq!(Settings::load(Some(&storage)), app.settings);
    assert_eq!(Settings::load(None), Settings::default());
}

#[test]
fn clock_zoom_does_not_change_global_zoom_or_native_dimensions() {
    let ctx = crate::persistence::egui_context();
    let mut app = clock();
    for key in [egui::Key::Minus, egui::Key::Equals, egui::Key::Num0] {
        for modifiers in [egui::Modifiers::NONE, egui::Modifiers::COMMAND] {
            let output = run_headless(&ctx, key_input(key, modifiers), |_| {
                app.handle_input(&ctx);
            });
            assert_eq!(ctx.zoom_factor(), 1.0);
            assert!(
                !output.viewport_output[&egui::ViewportId::ROOT]
                    .commands
                    .iter()
                    .any(|command| matches!(
                        command,
                        egui::ViewportCommand::InnerSize(_)
                            | egui::ViewportCommand::MinInnerSize(_)
                            | egui::ViewportCommand::OuterPosition(_)
                    ))
            );
            let _ = run_headless(&ctx, Default::default(), |_| {});
        }
    }
    let _ = run_headless(
        &ctx,
        key_input(egui::Key::Minus, egui::Modifiers::NONE),
        |_| {
            app.handle_input(&ctx);
        },
    );
    assert!(app.settings.scale < DEFAULT_SCALE);
}

#[test]
fn fixed_ui_zoom_preserves_native_display_scaling() {
    for native_scale in [1.0, 1.25, 2.0] {
        let ctx = crate::persistence::egui_context();
        let mut input = egui::RawInput::default();
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .native_pixels_per_point = Some(native_scale);
        let _ = run_headless(&ctx, input, |_| {});
        assert_eq!(ctx.zoom_factor(), 1.0);
        assert_eq!(ctx.pixels_per_point(), native_scale);
    }
}

#[test]
fn close_hides_but_explicit_quit_is_never_cancelled() {
    let ctx = egui::Context::default();
    let mut app = clock();
    let output = run_headless(&ctx, Default::default(), |_| app.close_window(&ctx, true));
    let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Visible(false)))
    );
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::CancelClose))
    );

    let output = run_headless(&ctx, Default::default(), |_| {
        app.window_action(&ctx, Action::Show);
        app.window_action(&ctx, Action::Hide);
        app.window_action(&ctx, Action::Quit);
        app.close_window(&ctx, true);
    });
    assert!(app.quitting);
    let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Visible(true)))
    );
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Close))
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::CancelClose))
    );
}

#[test]
fn hiding_fullscreen_preserves_the_native_restore_request() {
    let ctx = egui::Context::default();
    let mut app = clock();
    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .fullscreen = Some(true);
    let output = run_headless(&ctx, input, |_| app.window_action(&ctx, Action::Hide));
    let commands = &output.viewport_output[&egui::ViewportId::ROOT].commands;
    #[cfg(target_os = "linux")]
    assert!(
        matches!(
            commands.as_slice(),
            [
                egui::ViewportCommand::Fullscreen(false),
                egui::ViewportCommand::Visible(false),
                egui::ViewportCommand::Fullscreen(true),
                ..
            ]
        ),
        "{commands:?}"
    );
    #[cfg(not(target_os = "linux"))]
    assert!(matches!(
        commands.as_slice(),
        [egui::ViewportCommand::Visible(false), ..]
    ));
}

#[test]
fn close_without_a_tray_uses_normal_exit() {
    let ctx = egui::Context::default();
    let mut app = clock();
    let output = run_headless(&ctx, Default::default(), |_| app.close_window(&ctx, false));
    assert!(
        !output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .any(|command| matches!(
                command,
                egui::ViewportCommand::CancelClose | egui::ViewportCommand::Visible(false)
            ))
    );
}

#[test]
fn tray_restore_and_fullscreen_exit_retain_the_windowed_position() {
    let ctx = egui::Context::default();
    let mut app = clock();
    let position = Pos2::new(1900.0, 880.0);
    let rect = Rect::from_min_size(position, Vec2::new(980.0, 580.0));
    let mut input = egui::RawInput::default();
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .outer_rect = Some(rect);
    let _ = run_headless(&ctx, input, |_| app.window_action(&ctx, Action::Hide));
    assert_eq!(app.windowed_position, Some(position));

    let output = run_headless(&ctx, Default::default(), |_| {
        app.window_action(&ctx, Action::Show);
    });
    assert!(output.viewport_output[&egui::ViewportId::ROOT]
        .commands
        .iter()
        .any(|command| matches!(command, egui::ViewportCommand::OuterPosition(p) if *p == position)));

    let mut input = key_input(egui::Key::Escape, egui::Modifiers::NONE);
    let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
    viewport.fullscreen = Some(true);
    viewport.outer_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(2560.0, 1440.0)));
    let output = run_headless(&ctx, input, |_| app.handle_input(&ctx));
    assert!(output.viewport_output[&egui::ViewportId::ROOT]
        .commands
        .iter()
        .any(|command| matches!(command, egui::ViewportCommand::OuterPosition(p) if *p == position)));
    assert_eq!(app.windowed_position, Some(position));
}

#[test]
fn fullscreen_keys_follow_native_state_and_release_the_input_lock() {
    for (fullscreen, key, expected) in [
        (false, egui::Key::F, Some(true)),
        (true, egui::Key::F, Some(false)),
        (true, egui::Key::Escape, Some(false)),
        (false, egui::Key::Escape, None),
    ] {
        let ctx = egui::Context::default();
        let mut app = clock();
        let mut input = key_input(key, egui::Modifiers::NONE);
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .fullscreen = Some(fullscreen);
        let output = run_headless(&ctx, input, |_| app.handle_input(&ctx));
        let fullscreen = output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .find_map(|command| match command {
                egui::ViewportCommand::Fullscreen(value) => Some(*value),
                _ => None,
            });
        assert_eq!(fullscreen, expected);
    }
}

#[test]
fn shortcut_reference_requires_ctrl_slash_and_toggles_closed() {
    for (modifiers, opens) in [
        (egui::Modifiers::NONE, false),
        (egui::Modifiers::CTRL, true),
        (
            egui::Modifiers {
                mac_cmd: true,
                command: true,
                ..egui::Modifiers::NONE
            },
            false,
        ),
    ] {
        let ctx = egui::Context::default();
        let mut app = clock();
        let _ = run_headless(&ctx, key_input(egui::Key::Slash, modifiers), |_| {
            app.handle_input(&ctx);
        });
        assert_eq!(app.shortcuts_open, opens);
        if opens {
            let _ = run_headless(&ctx, key_input(egui::Key::Slash, modifiers), |_| {
                app.handle_input(&ctx);
            });
            assert!(!app.shortcuts_open);
        }
    }
}

#[test]
fn shortcut_reference_fits_extreme_viewports_and_scales_up_with_the_clock() {
    let mut menu_sizes = Vec::new();
    let ctx = egui::Context::default();
    let mut app = clock();
    app.shortcuts_open = true;
    for [width, height] in
        VIEWPORT_SIZES
            .into_iter()
            .chain([[320.0, 1200.0], [2000.0, 220.0], [3840.0, 2160.0]])
    {
        let viewport = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height));
        for _ in 0..3 {
            let _ = run_headless(
                &ctx,
                egui::RawInput {
                    screen_rect: Some(viewport),
                    ..Default::default()
                },
                |_| app.shortcuts_ui(&ctx),
            );
        }
        let menu = ctx
            .memory(|memory| memory.area_rect(egui::Id::unique("keyboard-shortcuts")))
            .unwrap();
        assert!(
            viewport.contains_rect(menu),
            "{menu:?} outside {viewport:?}"
        );
        menu_sizes.push(menu.size());
    }
    assert!(menu_sizes[2].x > menu_sizes[0].x);
    assert!(menu_sizes[3].x > menu_sizes[2].x);
    assert!(menu_sizes.last().unwrap().x > menu_sizes[3].x);
    assert!(menu_sizes.last().unwrap().y > menu_sizes[2].y);
}

#[test]
fn shortcut_reference_blocks_clock_actions_and_escape_only_closes_the_menu() {
    let ctx = egui::Context::default();
    let mut app = clock();
    app.shortcuts_open = true;
    app.settings.scale = 2.3;
    let rect = Rect::from_min_size(Pos2::ZERO, MIN_WINDOW_SIZE);
    let _ = run_headless(
        &ctx,
        egui::RawInput {
            screen_rect: Some(rect),
            ..Default::default()
        },
        |_| app.shortcuts_ui(&ctx),
    );
    let settings = app.settings.clone();
    let mut input = key_input(egui::Key::S, egui::Modifiers::NONE);
    input.screen_rect = Some(rect);
    input.events.extend(
        [
            egui::Key::A,
            egui::Key::H,
            egui::Key::F,
            egui::Key::Plus,
            egui::Key::Num0,
        ]
        .into_iter()
        .flat_map(|key| key_input(key, egui::Modifiers::NONE).events),
    );
    input.events.extend([
        egui::Event::PointerMoved(Pos2::new(1.0, 1.0)),
        egui::Event::PointerButton {
            pos: Pos2::new(1.0, 1.0),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        },
    ]);
    let output = run_headless(&ctx, input, |ui| {
        app.handle_input(&ctx);
        app.handle_pointer(ui);
        app.shortcuts_ui(&ctx);
    });
    assert_eq!(app.settings, settings);
    assert!(
        !output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .any(|command| matches!(
                command,
                egui::ViewportCommand::StartDrag
                    | egui::ViewportCommand::BeginResize(_)
                    | egui::ViewportCommand::Fullscreen(_)
            ))
    );

    let mut input = key_input(egui::Key::Escape, egui::Modifiers::NONE);
    input.screen_rect = Some(rect);
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .unwrap()
        .fullscreen = Some(true);
    let output = run_headless(&ctx, input, |_| {
        app.handle_input(&ctx);
        app.shortcuts_ui(&ctx);
    });
    assert!(!app.shortcuts_open);
    assert!(
        !output.viewport_output[&egui::ViewportId::ROOT]
            .commands
            .iter()
            .any(|command| matches!(command, egui::ViewportCommand::Fullscreen(_)))
    );
}

#[test]
fn native_fullscreen_disables_background_dragging_and_edge_resizing() {
    for fullscreen in [false, true] {
        for position in [Pos2::new(100.0, 100.0), Pos2::new(1.0, 1.0)] {
            let ctx = egui::Context::default();
            let mut app = clock();
            let mut input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, MIN_WINDOW_SIZE)),
                events: vec![
                    egui::Event::PointerMoved(position),
                    egui::Event::PointerButton {
                        pos: position,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                ],
                ..Default::default()
            };
            let viewport = input.viewports.get_mut(&egui::ViewportId::ROOT).unwrap();
            viewport.fullscreen = Some(fullscreen);
            viewport.outer_rect = input.screen_rect;
            let output = run_headless(&ctx, input, |ui| app.handle_pointer(ui));
            let manipulation_started = output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .iter()
                .any(|command| {
                    matches!(
                        command,
                        egui::ViewportCommand::StartDrag | egui::ViewportCommand::BeginResize(_)
                    )
                });
            #[cfg(target_os = "macos")]
            let manipulation_started = manipulation_started || app.resizing.is_some();
            assert_eq!(manipulation_started, !fullscreen);
        }
    }
}

#[test]
fn tick_deadlines_and_alert_thresholds_follow_the_display() {
    let time = Utc.with_ymd_and_hms(2026, 10, 24, 23, 59, 59).unwrap()
        + chrono::Duration::milliseconds(990);
    assert_eq!(ms_to_next_tick(&time, true), 35);
    assert_eq!(ms_to_next_tick(&time, false), 35);
    let time = Utc.with_ymd_and_hms(2026, 10, 24, 23, 0, 12).unwrap();
    assert_eq!(ms_to_next_tick(&time, false), 48_025);
    let mut app = clock();
    assert_eq!(app.alert_rgb(59), None);
    app.settings.alert_enabled = true;
    assert_eq!(app.alert_rgb(54), None);
    assert_eq!(app.alert_rgb(55), Some(WARN_RGB));
    assert_eq!(app.alert_rgb(58), Some(SOON_RGB));
}

#[test]
fn macos_resize_anchors_opposite_edges_at_the_minimum_size() {
    let original = Rect::from_min_size(Pos2::new(100.0, 200.0), Vec2::new(900.0, 520.0));
    let smaller = resized_rect(
        original,
        Vec2::new(800.0, 500.0),
        egui::ResizeDirection::NorthWest,
    );
    assert_eq!(smaller.max, original.max);
    assert_eq!(smaller.size(), Vec2::new(320.0, 200.0));
    let larger = resized_rect(
        original,
        Vec2::new(80.0, 50.0),
        egui::ResizeDirection::SouthEast,
    );
    assert_eq!(larger.min, original.min);
    assert_eq!(larger.size(), Vec2::new(980.0, 570.0));
}

#[test]
#[ignore = "manual headless workflow; prints rendered bounds and restored preferences"]
fn smoke_clock() {
    let ctx = crate::persistence::egui_context();
    let mut app = clock();
    let _ = run_headless(
        &ctx,
        key_input(egui::Key::Minus, egui::Modifiers::COMMAND),
        |_| app.handle_input(&ctx),
    );
    assert_eq!(ctx.zoom_factor(), 1.0);
    println!(
        "Clock zoom={}, global UI zoom={}, native minimum={:?}",
        app.settings.scale,
        ctx.zoom_factor(),
        MIN_WINDOW_SIZE
    );
    for [width, height] in VIEWPORT_SIZES {
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height));
        let input = egui::RawInput {
            screen_rect: Some(rect),
            ..key_input(egui::Key::S, egui::Modifiers::NONE)
        };
        let _ = run_headless(&ctx, input, |ui| {
            app.handle_input(&ctx);
            app.draw(ui, example_time());
            let layout = ClockLayout::new(ui.painter(), rect, example_time(), &app.settings);
            check_layout(&layout, rect);
            println!(
                "{width}x{height}: seconds={}, time={:?}, date={:?}, zone={:?}, hint={:?}",
                app.settings.show_seconds,
                layout.time_rect,
                layout.date_rect,
                layout.zone_rect,
                layout.hint.as_ref().map(|(_, rect)| rect)
            );
        });
        let _ = run_headless(&ctx, Default::default(), |_| {});
    }
    let _ = run_headless(&ctx, Default::default(), |_| {
        app.close_window(&ctx, true);
        app.window_action(&ctx, Action::Show);
        app.window_action(&ctx, Action::Hide);
        app.window_action(&ctx, Action::Quit);
        assert!(app.quitting);
    });
    let mut storage = MemoryStorage::default();
    app.save(&mut storage);
    assert_eq!(Settings::load(Some(&storage)), app.settings);
    println!(
        "Hide -> Show -> Hide -> Quit; saved/reloaded preferences: {:?}",
        app.settings
    );
}
