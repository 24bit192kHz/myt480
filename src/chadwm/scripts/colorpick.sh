#!/bin/sh
# HyDE "color picker" (Super+Shift+P) analogue for X11: pick a pixel with maim,
# read its colour with ImageMagick, copy "#RRGGBB" to the clipboard.
set -eu

tmp=$(mktemp /tmp/colorpick.XXXXXX.png)
trap 'rm -f "$tmp"' EXIT

command -v maim >/dev/null 2>&1 || { notify-send "colorpick: maim missing"; exit 1; }
maim -s "$tmp" || exit 0            # user aborted the selection

hex=$(magick "$tmp" -resize '1x1!' -format '%[hex:p{0,0}]' info:- 2>/dev/null)
[ -n "$hex" ] || { notify-send "colorpick: could not read pixel"; exit 1; }

printf '#%s' "$hex" | xclip -selection clipboard
notify-send "Colour #$hex copied"
