#!/bin/sh
# HyDE "capture text with OCR" (Super+O, Super+Ctrl+S) for X11:
# select a region with maim, OCR it with tesseract (eng+ara), copy the text to
# the clipboard and show it in a notification.
# `ocr.sh --file IMAGE` runs the same pipeline on an existing image (for checks).
set -u

img=${TMPDIR:-/tmp}/ocr.$$.png
trap 'rm -f "$img"' EXIT

if [ "${1:-}" = "--file" ] && [ -n "${2:-}" ]; then
	img=$2
else
	command -v maim >/dev/null 2>&1 || { notify-send "OCR: maim missing"; exit 1; }
	maim -s "$img" || exit 0	# selection aborted
fi

text=$(tesseract "$img" stdout -l eng+ara 2>/dev/null | sed '/^[[:space:]]*$/d')
if [ -z "$text" ]; then
	notify-send "OCR" "no text found in the selection"
	exit 1
fi

printf '%s' "$text" | xclip -selection clipboard
notify-send "OCR → clipboard" "$(printf '%s' "$text" | head -c 240)"
