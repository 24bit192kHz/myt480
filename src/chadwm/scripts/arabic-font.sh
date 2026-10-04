#!/bin/sh
# arabic-font.sh [next|prev|list] — cycle the bar's Arabic font (Hijri date,
# Arabic window titles): fonts[2] in chadwm config.def.h and config.h, then
# rebuild. Super+Shift+R loads it (injected keys did not reach the live
# chadwm on 2026-10-01, neither xdotool nor XTEST by keycode, so it is left
# to you). calendar.sh uses the same family. Only fonts checked in chadwm's renderer: it shapes Arabic with
# fribidi presentation forms, where Noto Kufi Arabic and Noto Sans Arabic (UI)
# draw لآ without the madda, and Arabic Newspaper / KO Methlama break joins.
set -eu

dir=$(cd "$(dirname "$0")" && pwd)
src=$(dirname "$dir")/chadwm
mode=${1:-next}
fonts='Droid Arabic Kufi:style=Bold:size=10
Noto Naskh Arabic:style=Bold:size=12
DejaVu Sans:style=Bold:size=10
Noto Naskh Arabic UI:style=SemiBold:size=12
Droid Naskh Shift Alt:size=12
Droid Arabic Kufi:size=11'

cur=$(sed -n 's|^static const char \*fonts\[\].*, "\([^"]*\)" };$|\1|p' "$src/config.def.h")
[ -n "$cur" ] || { notify-send "arabic-font: cannot parse fonts[] in config.def.h"; exit 1; }
if [ "$mode" = list ]; then
	printf '%s\n' "$fonts" | sed "s|^$cur\$|& *|"
	exit 0
fi

next=$(printf '%s\n' "$fonts" | awk -v c="$cur" -v m="$mode" '
	{ a[NR] = $0; if ($0 == c) i = NR }
	END {
		if (i == 0) { print a[1]; exit }
		if (m == "prev") print a[i == 1 ? NR : i - 1]
		else             print a[i == NR ? 1 : i + 1]
	}')

for f in config.def.h config.h; do
	sed -i "s|, \"$cur\" };\$|, \"$next\" };|" "$src/$f"
done
if ! make -C "$src" >/dev/null 2>&1; then
	for f in config.def.h config.h; do
		sed -i "s|, \"$next\" };\$|, \"$cur\" };|" "$src/$f"
	done
	make -C "$src" >/dev/null 2>&1 || true
	notify-send "arabic-font: build failed, kept ${cur%%:*}"
	exit 1
fi

n=$(printf '%s\n' "$fonts" | grep -nx "$next" | cut -d: -f1)
notify-send -t 6000 "Arabic font $n/6: ${next%%:*}" \
	"<span font_family='${next%%:*}' size='large'>$(hijri 2>/dev/null)</span>
Super+Shift+R puts it in the bar"
