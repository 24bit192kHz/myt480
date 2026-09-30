#!/bin/sh
# barclick.sh MODULE BUTTON — clicks on the chadwm status modules (bar.sh
# prefixes each with ^kN^). Buttons: 1 left, 2 middle, 3 right, 4/5 scroll.
S=$HOME/.config/chadwm/scripts
refresh() { kill -USR1 "$(cat "${XDG_RUNTIME_DIR:-/tmp}/chadwm-bar.pid")" 2>/dev/null; }

case $1:$2 in
1:1) exec st -t updates -e sh -c 'sudo pacman -Syu; echo; read -p "done, Enter to close " _' ;;
1:3) refresh ;;
2:1|3:1) exec st -e btop ;;
4:1) command -v battwatch-tui >/dev/null && exec st -e battwatch-tui
     exec notify-send "Battery" "$(for b in /sys/class/power_supply/BAT*; do echo "${b##*/}: $(cat $b/capacity)% $(cat $b/status)"; done)" ;;
5:1) bright up; refresh ;;
5:3) bright down; refresh ;;
5:4) bright up; refresh ;;
5:5) bright down; refresh ;;
6:1) exec "$S/vol.sh" mute ;;
6:3) exec pavucontrol ;;
6:4) exec "$S/vol.sh" up ;;
6:5) exec "$S/vol.sh" down ;;
7:1) exec "$S/wifimenu.sh" ;;
7:3) exec nm-connection-editor ;;
8:1) exec notify-send -t 8000 "$(date '+%A %d %B %Y')" "$(cal | sed 1d)" ;;
9:*) exec "$S/powermenu.sh" ;;
10:1) exec "$S/caffeine.sh" toggle ;;
esac
