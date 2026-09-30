#!/bin/sh
# powermenu.sh — HyDE logout menu (wlogout) for chadwm: rofi list,
# palette-themed via ~/.config/rofi/grayscale.rasi. Used by Ctrl+Alt+Delete
# and the bar power widget. Hibernate target = swap partition (resume= on
# the kernel cmdline); elogind performs every action.
# Usage: powermenu.sh [lock|logout|suspend|hibernate|reboot|shutdown]

pkill -x rofi && exit 0

choice=$1
if [ -z "$choice" ]; then
	choice=$(printf '%s\n' \
		"󰌾  Lock" "󰍃  Logout" "󰤄  Suspend" "󰒲  Hibernate" \
		"󰜉  Reboot" "󰐥  Shutdown" |
		rofi -dmenu -i -p power -no-custom -theme-str \
		'window {width: 312px;} listview {columns: 1; lines: 6;} inputbar {enabled: false;}' |
		awk '{print tolower($2)}')
fi

confirm() {
	[ "$(printf 'No\nYes\n' | rofi -dmenu -p "$1?" -no-custom -theme-str \
		'window {width: 264px;} listview {columns: 1; lines: 2;}')" = Yes ]
}

case $choice in
lock) exec /usr/local/bin/dwm-lock ;;
logout) confirm Logout && exec pkill -x chadwm ;;
suspend) exec loginctl suspend ;;
hibernate) confirm Hibernate && exec loginctl hibernate ;;
reboot) confirm Reboot && exec loginctl reboot ;;
shutdown) confirm Shutdown && exec loginctl poweroff ;;
esac
