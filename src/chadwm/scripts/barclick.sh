#!/bin/sh
# barclick.sh MODULE BUTTON — clicks on the chadwm status modules (bar.sh
# prefixes each with ^kN^). Buttons: 1 left, 2 middle, 3 right, 4/5 scroll.
# The trailing "# icon module: action" comments are the Super+? cheat sheet
# rows (keyhelp.sh reads them); keep one on every case line. Hovering a
# module shows its details instead (barhover.sh, chadwm bartip.c).
S=$HOME/.config/chadwm/scripts
refresh() { kill -USR1 "$(cat "${XDG_RUNTIME_DIR:-/tmp}/chadwm-bar.pid")" 2>/dev/null; }

case $1:$2 in
1:1) exec st -t updates -e sh -c 'sudo pacman -Syu; echo; read -p "done, Enter to close " _' ;;  # 󰏗 updates: system update
1:3) refresh ;;  # 󰏗 updates: recount
2:1|3:1) exec st -e btop ;;  # 󰻠 cpu / memory: btop
4:1) command -v battwatch-gui >/dev/null && exec battwatch-gui  # 󰁹 battery: battwatch window (click again closes)
     exec notify-send "Battery" "$(for b in /sys/class/power_supply/BAT*; do echo "${b##*/}: $(cat $b/capacity)% $(cat $b/status)"; done)" ;;
4:2) exec notify-send -a battwatch -i battery "Battery" "$(battwatch-daemon --status | sed -n 's/^\(pct\|status\|watts\|eta_min\|packs\|health\)=/\1: /p')" ;;  # 󰁹 battery: live summary
4:3) exec st -t battwatch -e battwatch-tui ;;  # 󰁹 battery: terminal viewer
5:1) exec bright up ;;  # 󰃠 brightness: up / down
5:3) exec bright down ;;  # 󰃠 brightness: up / down
5:4) exec bright up ;;  # 󰃠 brightness: up / down
5:5) exec bright down ;;  # 󰃠 brightness: up / down
6:1) exec "$S/vol.sh" mute ;;  # 󰕾 volume: mute
6:3) exec pavucontrol ;;  # 󰕾 volume: pavucontrol
6:4) exec "$S/vol.sh" up ;;  # 󰕾 volume: ±5%
6:5) exec "$S/vol.sh" down ;;  # 󰕾 volume: ±5%
7:1) command -v chadnet >/dev/null && exec chadnet  # 󰤨 wifi: network popup (Wi-Fi, VPN, wired)
     exec "$S/wifimenu.sh" ;;
7:3) exec nm-connection-editor ;;  # 󰤨 wifi: connections
8:1) command -v chadcal >/dev/null && exec chadcal  # 󰃭 clock: calendar window (click again closes)
     exec st -t calcurse -e calcurse ;;
8:2) exec "$S/arabic-font.sh" next ;;  # 󰃭 clock: next Arabic font
8:3) exec "$S/calendar.sh" ;;  # 󰃭 clock: Gregorian + Hijri months
9:*) command -v chadpower >/dev/null && exec chadpower --at-pointer  # ⏻ power: session menu (lock, suspend, reboot, shutdown, boot menu)
     exec "$S/powermenu.sh" ;;
10:1) exec "$S/caffeine.sh" toggle ;;  # 󰅶 caffeine: toggle
11:1) command -v chadbt >/dev/null && exec chadbt  # 󰂯 bluetooth: devices popup
      exec "$S/btmenu.sh" ;;
11:3) exec "$S/btmenu.sh" toggle ;;  # 󰂯 bluetooth: power on / off
12:1) exec "$S/vpn.sh" toggle ;;  # 󰕥 vpn: on / off
12:3) command -v chadnet >/dev/null && exec chadnet  # 󰕥 vpn: network popup
      exec "$S/vpn.sh" status ;;
13:1) exec st -t nvtop -e nvtop ;;  # 󰢮 gpu: nvtop
13:3) exec notify-send -t 6000 "MX150" "$(gpu-power status)" ;;  # 󰢮 gpu: MX150 power
esac
