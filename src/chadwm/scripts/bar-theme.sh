#!/bin/sh
# chadwm bar-theme cycle (HyDE "next/prev waybar layout" analogue).
# Switches BOTH the status-segment colours (bar.sh -> bar_themes/<name>) and the
# C-side scheme (config.def.h -> themes/<name>.h), rebuilds, restarts bar.sh and
# reloads the WM. Reverts the header edit if the build fails.
set -eu

dir=$(cd "$(dirname "$0")" && pwd)
root=$(dirname "$dir")
mode=${1:-next}

cur=$(sed -n 's|^\. .*/bar_themes/\([^ ]*\)$|\1|p' "$dir/bar.sh" | head -1)
[ -n "$cur" ] || { notify-send "bar-theme: cannot parse bar.sh theme line"; exit 1; }

# themes present in both places only (bar colour file + C header)
list=$(for t in $(ls "$dir/bar_themes" | sort); do
	[ -f "$root/chadwm/themes/$t.h" ] && echo "$t"
done)
[ -n "$list" ] || { notify-send "bar-theme: no paired themes found"; exit 1; }

next=$(printf '%s\n' "$list" | awk -v c="$cur" -v m="$mode" '
	{ a[NR] = $0; if ($0 == c) i = NR }
	END {
		if (i == 0) { print a[1]; exit }
		if (m == "prev") print a[i == 1 ? NR : i - 1]
		else             print a[i == NR ? 1 : i + 1]
	}')
[ -n "$next" ] || exit 1
[ "$next" = "$cur" ] && { notify-send "bar-theme: only one theme installed ($cur)"; exit 0; }

hdr="$root/chadwm/config.def.h"
oldinc=$(grep -n '^#include "themes/' "$hdr" | head -1)
sed -i "s|^\. .*/bar_themes/.*$|. $dir/bar_themes/$next|" "$dir/bar.sh"
sed -i "s|^#include \"themes/.*\.h\"\$|#include \"themes/$next.h\"|" "$hdr"

if ! make -C "$root/chadwm" clean >/dev/null 2>&1 || ! make -C "$root/chadwm" >/dev/null 2>&1; then
	sed -i "s|^#include \"themes/.*\.h\"\$|${oldinc#*:}|" "$hdr"
	make -C "$root/chadwm" >/dev/null 2>&1 || true
	notify-send "bar-theme: build failed, reverted to $cur"
	exit 1
fi

# restart only the bar.sh of THIS display (other sessions' feeders must survive)
for p in $(pgrep -f 'chadwm/scripts/bar.sh' 2>/dev/null); do
	if tr '\0' '\n' < "/proc/$p/environ" 2>/dev/null | grep -qx "DISPLAY=${DISPLAY:-}"; then
		kill "$p" 2>/dev/null || true
	fi
done
sleep 0.3
dash "$dir/bar.sh" >/dev/null 2>&1 &

# dwm 'restart' leaves the event loop (exit 0) and run.sh respawns the new binary
if command -v xdotool >/dev/null 2>&1; then
	xdotool key --clearmodifiers super+shift+r
else
	notify-send "bar-theme: $next applied (relog to reload WM)"
fi
notify-send "Bar theme: $next"
