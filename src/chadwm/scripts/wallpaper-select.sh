#!/bin/sh
# The active desktop uses the supervised GPU ASCII Earth renderer. The static
# picker remains available for non-Earth sessions, but cannot replace this root.
set -eu

if [ -x "$HOME/.local/bin/gxwc-wallpaper" ] && "$HOME/.local/bin/gxwc-wallpaper" status >/dev/null 2>&1; then
    notify-send "Wallpaper: ASCII Earth is active" 2>/dev/null || true
    exit 0
fi

dir="$HOME/Pictures/wallpapers"
[ -d "$dir" ] || { notify-send "wallpaper: $dir missing"; exit 1; }

sel=$(find "$dir" -maxdepth 1 -type f \
	\( -iname '*.png' -o -iname '*.jpg' -o -iname '*.jpeg' -o -iname '*.webp' \) \
	-printf '%f\n' | sort | rofi -dmenu -i -p wallpaper)
[ -n "${sel:-}" ] || exit 0

path="$dir/$sel"
if command -v dwm-settings-wallpaper >/dev/null 2>&1 &&
	dwm-settings-wallpaper apply "$path" fill >/dev/null 2>&1; then
	:
else
	feh --no-fehbg --bg-fill "$path" >/dev/null 2>&1
fi
notify-send "Wallpaper: $sel"
