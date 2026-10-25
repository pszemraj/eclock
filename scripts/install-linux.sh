#!/usr/bin/env bash
# Install the Cargo executable and its user-local launcher/icon.
# CARGO_INSTALL_ROOT/CARGO_HOME select the binary root; XDG_DATA_HOME selects data.
set -euo pipefail
repository=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
install_root=${CARGO_INSTALL_ROOT:-${CARGO_HOME:-$HOME/.cargo}}
data_home=${XDG_DATA_HOME:-$HOME/.local/share}

cargo install --locked --path "$repository" --root "$install_root"
install_root=$(cd -- "$install_root" && pwd)
mkdir -p "$data_home/applications" "$data_home/eclock"
data_home=$(cd -- "$data_home" && pwd)
install -m644 "$repository/assets/icon.svg" "$data_home/eclock/icon.svg"

# Desktop entries unescape strings before parsing Exec; this is not shell quoting.
desktop_exec="$install_root/bin/eclock"
desktop_exec=${desktop_exec//\\/\\\\\\\\}
desktop_exec=${desktop_exec//\"/\\\\\"}
desktop_exec=${desktop_exec//\$/\\\\\$}
desktop_exec=${desktop_exec//\`/\\\\\`}
desktop_exec=${desktop_exec//%/%%}
desktop_icon="$data_home/eclock/icon.svg"
desktop_icon=${desktop_icon//\\/\\\\}
desktop-file-install --dir="$data_home/applications" \
  --set-key=Exec --set-value="\"$desktop_exec\"" \
  --set-icon="$desktop_icon" "$repository/assets/eclock.desktop"

printf 'Installed eclock: %s/bin/eclock\n' "$install_root"
printf '%s\n' 'Open eclock from your applications menu or run the installed command.'
