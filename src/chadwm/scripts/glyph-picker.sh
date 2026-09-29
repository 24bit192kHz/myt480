#!/bin/sh
# HyDE "glyph picker" (Super+Period) analogue: rofimoji over symbol blocks.
# rofimoji ships per-Unicode-block CSVs; there is no Nerd Font data set here, so
# this picks symbols/dingbats/shapes (nerd-font glyphs are not insertable via it).
set -eu

data=$(python3 -c 'import pathlib, picker; print(pathlib.Path(picker.__file__).parent / "data")') || {
	notify-send "glyph-picker: rofimoji (python picker) not importable"
	exit 1
}

exec rofimoji \
	--files "$data/miscellaneous_symbols.csv" "$data/dingbats.csv" "$data/geometric_shapes.csv" \
	--action type --selector rofi --clipboarder xclip --typer xdotool
