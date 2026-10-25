#!/usr/bin/env bash
# Remove the installed executable and desktop assets, preserving clock settings.
# Use the same CARGO_INSTALL_ROOT/CARGO_HOME and XDG_DATA_HOME as installation.
set -euo pipefail
install_root=${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}
data_home=${XDG_DATA_HOME:-$HOME/.local/share}

cargo uninstall --root "$install_root" eclock
rm -f -- "$data_home/applications/eclock.desktop" "$data_home/eclock/icon.svg"
printf '%s\n' 'Uninstalled eclock; saved clock preferences are preserved.'
