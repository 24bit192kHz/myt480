#!/bin/sh
# btmenu.sh [toggle] — rofi Bluetooth picker (BlueZ): power toggle, scan, devices.
# A connected device disconnects, a paired one connects, a new one pairs, trusts
# and connects. Rows are "display<TAB>address<TAB>state"; rofi shows column 1.
pkill -x rofi && exit 0
refresh() { kill -USR1 "$(cat "${XDG_RUNTIME_DIR:-/tmp}/chadwm-bar.pid")" 2>/dev/null; }
bt() { timeout 15 bluetoothctl "$@" 2>/dev/null; }
tab=$(printf '\t')
powered=$(bt show | awk '/Powered:/{print $2}')
if [ "$1" = toggle ]; then
	[ "$powered" = yes ] && bt power off >/dev/null || bt power on >/dev/null
	refresh; exit 0
fi
if [ "$powered" = yes ]; then power="󰂲  Turn Bluetooth off"; else power="󰂯  Turn Bluetooth on"; fi
# devices: connected first, then paired, then whatever a previous scan found
list=$( { bt devices Connected | sed 's/^Device /C /'; bt devices Paired | sed 's/^Device /P /'; bt devices | sed 's/^Device /N /'; } |
	awk '{addr=$2; st=$1; $1=""; $2=""; name=substr($0,3); if (seen[addr]++) next
	      icon = st=="C" ? "󰂱" : st=="P" ? "󰂯" : "󰂰"; tag = st=="C" ? "  connected" : st=="P" ? "  paired" : ""
	      printf "%s  %s%s\t%s\t%s\n", icon, name, tag, addr, st}')
choice=$(printf '%s\n%s\n%s\n' "$power${tab}power" "󰑐  Scan (8 s)${tab}scan" "$list" |
	rofi -dmenu -i -p bluetooth -no-custom -display-columns 1 -display-column-separator '\t' \
		-theme-str 'window {width: 576px;} listview {columns: 1; lines: 10;}')
[ -n "$choice" ] || exit 0
addr=$(printf '%s' "$choice" | cut -f2); st=$(printf '%s' "$choice" | cut -f3)
case $addr in
power) exec "$0" toggle ;;
scan) [ "$powered" = yes ] || bt power on >/dev/null; bt --timeout 8 scan on >/dev/null; exec "$0" ;;
esac
case $st in
C) out=$(bt disconnect "$addr" | tail -1) ;;
P) out=$(bt connect "$addr" | tail -1) ;;
*) out=$(bt pair "$addr" | tail -1); bt trust "$addr" >/dev/null; out="$out; $(bt connect "$addr" | tail -1)" ;;
esac
notify-send "Bluetooth" "$out"
refresh
