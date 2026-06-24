# eclock

A small desktop clock for keeping a large, low-power time readout on screen.

eclock shows the current time, date, and time zone in an ordinary desktop
window. It reads your system time zone at startup, handles DST offsets through
the time library, and only repaints when the displayed time changes.

## Run

You need a stable Rust toolchain. Install one with [rustup](https://rustup.rs/).

```sh
cargo run --release
```

To build once and test the local binary directly:

```sh
cargo build --release --locked
./target/release/eclock
```

To install it as a local command:

```sh
cargo install --path . --locked
eclock
```

On Linux, eframe needs X11/Wayland and OpenGL development libraries:

```sh
sudo apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
```

Use the equivalent packages for your distribution. macOS and Windows do not
need extra system packages.

## Controls

| Key / input | Action |
| --- | --- |
| `S` | Toggle seconds |
| `+` / `=` | Zoom in |
| `-` | Zoom out |
| scroll wheel | Zoom in/out |
| `0` | Reset zoom |
| `A` | Toggle hourly alert |
| `F` | Toggle fullscreen |
| `Esc` | Leave fullscreen |
| `H` | Show/hide the hint line |

The hourly alert, when enabled, tints the border near the top of the hour:
amber from `:55`, then red from `:58`.

## Settings

eclock remembers seconds visibility, zoom, alert state, and hint visibility in
an `app.ron` file:

- **Linux:** `$XDG_DATA_HOME/eclock/app.ron` (usually `~/.local/share/eclock/app.ron`)
- **macOS:** `~/Library/Application Support/eclock/app.ron`
- **Windows:** `%APPDATA%\eclock\data\app.ron`

Delete that file to reset the app to defaults.

## Notes

- The app appears as a normal window in your dock or taskbar. It does not use a
  system tray or menu bar icon.
- The icon source is `assets/icon.svg`; rebuild after editing it.
- Basic colors, alert timing, zoom limits, and initial window size are set in
  `src/app.rs` and `src/main.rs`.
