#!/bin/sh
# wifimenu.sh — rofi Wi-Fi picker (NetworkManager): radio toggle, rescan,
# networks by signal; known networks connect directly, new secured ones ask
# for the password. Rows are "display<TAB>ssid<TAB>security"; rofi shows
# only column 1, so SSIDs are never parsed back out of the display text.
pkill -x rofi && exit 0
refresh() { kill -USR1 "$(cat "${XDG_RUNTIME_DIR:-/tmp}/chadwm-bar.pid")" 2>/dev/null; }
tab=$(printf '\t')

if [ "$(nmcli -t radio wifi)" = enabled ]; then radio="󰖪  Disable Wi-Fi"; else radio="󰖩  Enable Wi-Fi"; fi
# IN-USE:SIGNAL:SECURITY:SSID (SSID last: it may contain escaped colons)
list=$(nmcli -t -f IN-USE,SIGNAL,SECURITY,SSID dev wifi list --rescan auto 2>/dev/null |
	awk -F: '{ssid = $4; for (i = 5; i <= NF; i++) ssid = ssid ":" $i; gsub(/\\:/, ":", ssid)
		if (ssid == "" || seen[ssid]++) next
		s = $2 + 0; icon = s >= 75 ? "󰤨" : s >= 50 ? "󰤥" : s >= 25 ? "󰤢" : "󰤟"
		sec = ($3 != "" && $3 != "--") ? $3 : ""
		printf "%s %s %3d%%  %s%s\t%s\t%s\n", ($1 == "*" ? "" : " "), icon, s, ssid,
			(sec != "" ? "  " : ""), ssid, sec}')
choice=$(printf '%s\n%s\n%s\n' "$radio${tab}radio" "󰑐  Rescan${tab}rescan" "$list" |
	rofi -dmenu -i -p wifi -no-custom -display-columns 1 -display-column-separator '\t' \
		-theme-str 'window {width: 576px;} listview {columns: 1; lines: 10;}')
[ -n "$choice" ] || exit 0

ssid=$(printf '%s' "$choice" | cut -f2)
sec=$(printf '%s' "$choice" | cut -f3)
case $ssid in
radio)
	case $choice in
	*Disable*) nmcli radio wifi off ;;
	*) nmcli radio wifi on ;;
	esac
	refresh; exit 0 ;;
rescan) nmcli dev wifi rescan 2>/dev/null; exec "$0" ;;
esac

if nmcli -t -f NAME con show | grep -qxF "$ssid"; then
	out=$(nmcli con up id "$ssid" 2>&1)
elif [ -n "$sec" ]; then
	pass=$(rofi -dmenu -password -p "password for $ssid" \
		-theme-str 'window {width: 504px;} listview {lines: 0;}' < /dev/null)
	[ -n "$pass" ] || exit 0
	out=$(nmcli dev wifi connect "$ssid" password "$pass" 2>&1)
else
	out=$(nmcli dev wifi connect "$ssid" 2>&1)
fi
notify-send "Wi-Fi" "$out"
refresh
