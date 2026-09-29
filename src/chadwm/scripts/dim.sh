#!/bin/sh
# HyDE hyprshaderd dimmer/brighter analogue for X11: software dimming via
# xrandr --brightness (clamped 0.30 … 1.00). Usage: dim.sh up|down|reset|status
set -eu

step=0.05
state=${XDG_RUNTIME_DIR:-/tmp}/chadwm-dim
mode=${1:-down}

command -v xrandr >/dev/null 2>&1 || { notify-send "dim: xrandr missing"; exit 1; }
out=$(xrandr --listmonitors 2>/dev/null | awk '/^[[:space:]]*[0-9]+:/{print $NF; exit}')
[ -n "$out" ] || { notify-send "dim: no monitor found"; exit 1; }

if [ "$mode" = status ]; then
	printf 'output=%s brightness=%s\n' "$out" "$(cat "$state" 2>/dev/null || echo 1.00)"
	exit 0
fi

cur=$(cat "$state" 2>/dev/null || echo 1.0)
case $mode in
up)    new=$(awk -v c="$cur" 'BEGIN{v=c+0.05; if(v>1.00)v=1.00; printf "%.2f", v}') ;;
down)  new=$(awk -v c="$cur" 'BEGIN{v=c-0.05; if(v<0.30)v=0.30; printf "%.2f", v}') ;;
reset) new=1.00 ;;
*)     echo "usage: dim.sh up|down|reset|status" >&2; exit 2 ;;
esac

if ! xrandr --output "$out" --brightness "$new" 2>/dev/null; then
	notify-send "dim" "xrandr cannot set brightness on $out"
	exit 1
fi
printf '%s' "$new" > "$state"
notify-send "Screen dim" "$new (software, $out)"
