#!/usr/bin/env bash
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
bin_dir="${XDG_BIN_HOME:-$HOME/.local/bin}"

cd "$here"
cargo build --release

install -Dm755 target/release/rustpad "$bin_dir/rustpad"
tmp_desktop="$(mktemp)"
trap 'rm -f "$tmp_desktop"' EXIT
sed "s|^Exec=rustpad |Exec=$bin_dir/rustpad |" assets/rustpad.desktop > "$tmp_desktop"
install -Dm644 "$tmp_desktop" "$data_home/applications/rustpad.desktop"
install -Dm644 assets/rustpad.svg "$data_home/icons/hicolor/scalable/apps/rustpad.svg"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$data_home/applications" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q "$data_home/icons/hicolor" >/dev/null 2>&1 || true
fi

printf 'Rustpad terpasang.\n'
printf '  binary : %s/rustpad\n' "$bin_dir"
printf '  menu   : %s/applications/rustpad.desktop\n' "$data_home"
printf '  ikon   : %s/icons/hicolor/scalable/apps/rustpad.svg\n' "$data_home"
