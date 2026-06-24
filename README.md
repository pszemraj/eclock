# Clock

A minimal, low-power desktop clock built with [egui/eframe](https://github.com/emilk/egui).

It shows a large `HH·MM` readout with the date and your timezone, and is designed to stay out of the way: the window only repaints when a displayed digit actually changes — once a second when seconds are shown, once a minute otherwise. That keeps it at roughly 1 FPS instead of redrawing 60 times a second, so it can sit on screen all day without spinning a CPU core.

The timezone is read from the operating system at startup. DST transitions (EDT↔EST and the like) are resolved per-instant, so the offset stays correct without restarting.

## Build

You need a Rust toolchain (stable). Install via [rustup](https://rustup.rs/).

On **Linux**, eframe needs a few system libraries to build and link against X11/Wayland and OpenGL:

```sh
sudo apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
```

(Use the equivalent packages for your distribution. A working OpenGL driver is required at runtime, which desktop systems already have.) **macOS** and **Windows** build with no extra system packages. Everything else, including SVG rendering for the icon, is pure Rust and builds automatically.

Then:

```sh
cargo run --release
```

## Controls

| Key / input | Action |
| --- | --- |
| `S` | Toggle seconds on/off |
| `+` / `=` | Zoom in (larger digits) |
| `-` | Zoom out (smaller digits) |
| scroll wheel | Zoom in/out |
| `0` | Reset zoom to default |
| `A` | Toggle the hourly alert |
| `F` | Toggle fullscreen |
| `Esc` | Leave fullscreen |
| `H` | Show/hide the on-screen hint line |

Zoom changes only the size of the clock, not the window.

The **hourly alert**, when enabled, tints the window border as the top of the hour approaches: a muted amber from minute `:55` (five minutes out) and a muted red from `:58` (two minutes out). The colors are deliberately desaturated rather than neon.

## Settings

Seconds visibility, zoom level, alert on/off, and hint visibility persist between runs. eframe writes them to an `app.ron` file in the per-user data directory:

- **Linux:** `$XDG_DATA_HOME/clock/app.ron` (defaults to `~/.local/share/clock/app.ron`)
- **macOS:** `~/Library/Application Support/clock/app.ron`
- **Windows:** `%APPDATA%\clock\data\app.ron`

Delete that file to reset everything to defaults.

## Icon

The icon lives at `assets/icon.svg` and is the single source of truth. It is compiled into the binary and rasterized at startup into the window / taskbar / dock icon, so editing the SVG and rebuilding is all it takes to change it — there is no separate icon file to regenerate.

## Dock and system tray

This is an ordinary windowed application: it appears in the dock or taskbar with the icon above while its window is open. It does **not** live in the system tray / menu bar. eframe owns the windowing event loop and exposes no tray integration, and it cannot reliably hide and restore its own window, so a true tray-resident clock would mean replacing eframe with a hand-written `egui_glow`/`winit` event loop plus a tray-icon crate and per-OS code. That is a substantial rewrite and is intentionally out of scope here.

On macOS, if you want to keep it off the Dock while its window is open, you can mark the bundle as an agent (`LSUIElement`/accessory activation policy), but without a tray icon there would then be no way to reopen a closed window — so the Dock presence is kept by default.

## Customizing

The look and behavior are driven by constants at the top of `src/app.rs`:

- `BG` / `FG` — background and text colors.
- `WARN_RGB` / `SOON_RGB` — the two alert border colors.
- `WARN_FROM_MINUTE` / `SOON_FROM_MINUTE` — the minutes at which each alert color kicks in.
- `DEFAULT_SCALE`, `MIN_SCALE`, `MAX_SCALE`, `SCALE_STEP` — zoom bounds and defaults.

The digits use egui's bundled monospace font. To use a different typeface, register it with `egui::Context::set_fonts` in `ClockApp::new` and reference it in place of `FontId::monospace(..)` in `draw`.

The initial window size lives in `src/main.rs` (`with_inner_size`).
