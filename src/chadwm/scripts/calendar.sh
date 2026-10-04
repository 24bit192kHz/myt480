#!/bin/sh
# calendar.sh — bar clock right-click: the Gregorian month with the Umm al-Qura
# Hijri day under every date (hijri cal, ~/.local/bin/hijri from hijri.c),
# Sunday first. Previous / Today / Next step the month; Esc or a click
# outside closes it. Opens under the bar on the right, next to the clock.
pkill -x rofi && exit 0
command -v hijri >/dev/null || exec notify-send -u critical "calendar" "hijri helper missing: build scripts/hijri.c"

# Arabic lines in the bar's Arabic font (chadwm fonts[2], arabic-font.sh)
HIJRI_FONT=$(sed -n 's|^static const char \*fonts\[\].*, "\([^":]*\)[^"]*" };$|\1|p' \
	"$HOME/.config/chadwm/chadwm/config.def.h")
export HIJRI_FONT

off=0 row=1
while :; do
	out=$(hijri cal "$off") || exit 1
	title=${out%%
*}
	grid=${out#*
}
	# the message ends in a blank line: rofi sizes it one line short otherwise
	choice=$(printf '󰁍  Prev\n󰃭  Today\nNext  󰁔\n' |
		rofi -dmenu -i -no-custom -selected-row "$row" \
			-me-select-entry '' -me-accept-entry MousePrimary \
			-mesg "<span size=\"large\" weight=\"bold\">$title</span>
$grid
" \
			-theme-str 'window {location: northeast; anchor: northeast; x-offset: -12px; y-offset: 40px; width: 400px;}
				mainbox {children: [message, listview];}
				listview {columns: 3; lines: 1;}
				element {horizontal-align: 0.5;} element-text {horizontal-align: 0.5;}
				element-icon {enabled: false;}
				message {background-color: transparent; border: 0px; padding: 0px 0px 6px;}
				textbox {background-color: transparent; text-color: @fg-col;}')
	case $choice in
	*Prev) off=$((off - 1)) row=0 ;;
	*Today) off=0 row=1 ;;
	*Next*) off=$((off + 1)) row=2 ;;
	*) exit 0 ;;
	esac
done
