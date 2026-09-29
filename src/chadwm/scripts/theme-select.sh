#!/bin/sh
# HyDE "select a theme" (Super+Shift+T) analogue: rofi picker over the box's
# dwm-titus themes.toml, applied through dwm-settings-theme (toolkit env: GTK/Qt/
# terminal colours). chadwm's own bar/window colours are switched separately by
# Super+Alt+Up/Down (scripts/bar-theme.sh).
set -eu

toml="$HOME/.config/dwm-titus/themes.toml"
[ -f "$toml" ] || { notify-send "theme: $toml missing"; exit 1; }

sel=$(grep -o '^\[theme\.[^]]*\]' "$toml" | sed 's/^\[theme\.//; s/\]$//' | sort -u |
	rofi -dmenu -i -p theme)
[ -n "${sel:-}" ] || exit 0

if command -v dwm-settings-theme >/dev/null 2>&1 && dwm-settings-theme apply "$sel"; then
	notify-send "Theme: $sel"
else
	notify-send "Theme: could not apply $sel"
fi
