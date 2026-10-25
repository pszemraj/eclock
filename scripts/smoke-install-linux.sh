#!/usr/bin/env bash
# Manually exercise install, checkout launch, cargo clean, and uninstall.
# Uses isolated state outside target; cargo clean removes this checkout's builds.
set -euo pipefail
repository=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd -- "$repository"
mkdir -p .install-smoke
scratch=$(mktemp -d "$repository/.install-smoke/run.XXXXXX")
export CARGO_INSTALL_ROOT="$scratch/cargo root"
export XDG_DATA_HOME="$scratch/data home"
unset CARGO_TARGET_DIR

"$repository/scripts/install-linux.sh" > "$scratch/install.log" 2>&1
executable="$CARGO_INSTALL_ROOT/bin/eclock"
launcher="$XDG_DATA_HOME/applications/eclock.desktop"
icon="$XDG_DATA_HOME/eclock/icon.svg"
test -x "$executable"
desktop-file-validate "$launcher"
cmp assets/icon.svg "$icon"
cp -- "$launcher" "$scratch/launcher.before"
printf '%s\n' 'user preferences' > "$XDG_DATA_HOME/eclock/app.ron"
printf '%s\n' 'PASS: real Cargo installation, desktop entry, and copied icon'

# No display is available in this headless probe; startup must leave assets alone.
if DISPLAY= WAYLAND_DISPLAY= XDG_RUNTIME_DIR="$scratch/no-session" \
  cargo run --release --locked > "$scratch/checkout.log" 2>&1; then
  printf '%s\n' 'Expected the checkout launch to fail without a display.' >&2
  exit 1
fi
cmp -- "$launcher" "$scratch/launcher.before"
cmp assets/icon.svg "$icon"
printf '%s\n' 'PASS: checkout launch did not change installed desktop assets'

cargo clean > "$scratch/clean.log" 2>&1
test ! -d target
test -x "$executable"
cmp -- "$launcher" "$scratch/launcher.before"
cmp assets/icon.svg "$icon"
# This parses the real launcher after all compilation artifacts are gone.
gjs -c '
const {Gio, GLib} = imports.gi;
const entry = Gio.DesktopAppInfo.new_from_filename(ARGV[0]);
if (!entry) throw new Error("Launcher did not parse after cargo clean");
const [, argv] = GLib.shell_parse_argv(entry.get_commandline());
if (argv.length !== 1 || argv[0] !== ARGV[1]) throw new Error("Wrong installed executable");
if (!(entry.get_icon() instanceof Gio.FileIcon)) throw new Error("Expected an absolute icon path");
entry.get_icon().get_file().load_contents(null);
' "$launcher" "$executable"
printf '%s\n' 'PASS: installed binary, launcher, and icon survived cargo clean'

"$repository/scripts/uninstall-linux.sh" > "$scratch/uninstall.log" 2>&1
test ! -e "$executable"
test ! -e "$launcher"
test ! -e "$icon"
test "$(cat "$XDG_DATA_HOME/eclock/app.ron")" = 'user preferences'
printf '%s\n' 'PASS: uninstall removed application files and preserved preferences'
printf 'Installation smoke logs: %s\n' "$scratch"
