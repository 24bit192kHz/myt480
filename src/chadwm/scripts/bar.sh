#!/bin/dash
# chadwm status (xsetroot -name). Each module starts with ^kN^: chadwm
# records its position and a click/scroll runs scripts/barclick.sh N BUTTON.
#   1 updates  2 cpu  3 mem  4 battery  5 brightness  6 volume  7 wifi
#   8 clock  9 power  10 caffeine (cup filled = sleep inhibited, caffeine.sh)
#   11 bluetooth (name of the connected device; btmenu.sh)
# Refresh: every 5 s (aligned so the clock flips on the minute), at once on
# SIGUSR1 (`pkill -USR1 -x bar.sh` style: barclick.sh / volume keys send it).
# Cost per tick: builtins + one `date`; pactl only on start, SIGUSR1 and
# every 60 s; battery every 30 s (EC read); SSID when the link changes.

# ^c$var^ = fg color, ^b$var^ = bg color
. /home/btw/.config/chadwm/scripts/bar_themes/grayscale

echo $$ > "${XDG_RUNTIME_DIR:-/tmp}/chadwm-bar.pid"

for wlan_dev in /sys/class/net/wl*; do break; done
for bl_dev in /sys/class/backlight/*; do break; done

pkg_updates() {
	# arch: pacman -Qu lists what the last -Sy saw (no network access here)
	n=$(timeout 20 pacman -Qu 2>/dev/null | wc -l)
	if [ "$n" -eq 0 ]; then
		printf "^k1^^c$green^ 󰏗 "
	else
		printf "^k1^^c$white^ 󰏗 $n "
	fi
}

cpu() {
	read -r cpu_val _ < /proc/loadavg
	printf "^k2^^c$black^^b$green^ CPU ^c$white^^b$grey^ $cpu_val ^b$black^"
}

mem() {
	# MemTotal - MemAvailable with builtins only
	while read -r k v _; do
		case $k in
		MemTotal:) t=$v ;;
		MemAvailable:) a=$v; break ;;
		esac
	done < /proc/meminfo
	u=$(( (t - a) / 1024 ))
	printf "^k3^^c$red^^b$black^  $((u / 1024)).$(( (u % 1024) * 10 / 1024 ))G "
}

battery() {
	# combined charge of all packs (energy_* uWh or charge_* uAh)
	now=0 full=0 st=
	for b in /sys/class/power_supply/BAT*; do
		[ -r "$b/energy_now" ] && f=energy || f=charge
		read -r v < "$b/${f}_now" && read -r w < "$b/${f}_full" || continue
		now=$((now + v)) full=$((full + w))
		read -r s < "$b/status"; [ "$s" = Charging ] && st=+
	done
	[ $full -gt 0 ] || return
	printf "^k4^^c$black^^b$red^ BAT ^c$white^^b$grey^ $(( (now * 100 + full / 2) / full ))%%$st ^b$black^"
}

brightness() {
	read -r cur < "$bl_dev/actual_brightness" 2>/dev/null || return
	read -r max < "$bl_dev/max_brightness"
	printf "^k5^^c$red^^b$black^ 󰃠 $((cur * 100 / max))%% "
}

volume() {
	v=$(pactl get-sink-volume @DEFAULT_SINK@ 2>/dev/null)
	v=${v#*/}; v=${v%%%*}; v=${v# }; v=${v# }
	case $(pactl get-sink-mute @DEFAULT_SINK@ 2>/dev/null) in
	*yes) printf "^k6^^c$darkblue^^b$black^ 󰝟 mute " ;;
	*) printf "^k6^^c$red^^b$black^ 󰕾 ${v:-?}%% " ;;
	esac
}

wifi() {
	# sets $wifis in the main shell (a $(...) subshell would lose the
	# cached SSID and run nmcli every tick)
	state=down
	[ -r "$wlan_dev/operstate" ] && read -r state < "$wlan_dev/operstate"
	if [ "$state" != "$wstate" ] || [ $wakeup = 1 ]; then
		wstate=$state
		ssid=
		[ "$state" = up ] && ssid=$(nmcli -t -g GENERAL.CONNECTION dev show "${wlan_dev##*/}" 2>/dev/null)
	fi
	case $state in
	up) wifis="^k7^^c$black^^b$blue^ 󰤨 ^c$blue^^b$black^ ${ssid:-connected} " ;;
	*) wifis="^k7^^c$black^^b$blue^ 󰤭 ^c$blue^^b$black^ offline " ;;
	esac
}

bluetooth() {
	# bluetoothd state every 30 s and on SIGUSR1; a D-Bus call, not a fork storm
	p=$(timeout 3 bluetoothctl show 2>/dev/null | awk '/Powered:/{print $2}')
	case $p in
	yes)
		d=$(timeout 3 bluetoothctl devices Connected 2>/dev/null | head -1 | cut -d' ' -f3-)
		if [ -n "$d" ]; then printf "^k11^^c$black^^b$blue^ 󰂱 ^c$blue^^b$black^ ${d%% *} "
		else printf "^k11^^c$blue^^b$black^ 󰂯 "; fi ;;
	*) printf "^k11^^c$darkblue^^b$black^ 󰂲 " ;;
	esac
}

caffeine() {
	# on while caffeine.sh's elogind-inhibit process lives (builtins only)
	if read -r cpid 2>/dev/null < "${XDG_RUNTIME_DIR:-/tmp}/caffeine.pid" &&
		read -r ccomm 2>/dev/null < "/proc/$cpid/comm" &&
		[ "$ccomm" = elogind-inhibit ]; then
		printf "^k10^^c$black^^b$white^ 󰅶 ^b$black^ "
	else
		printf "^k10^^c$darkblue^^b$black^ 󰛊 "
	fi
}

clock() {
	printf "^k8^^c$black^^b$darkblue^ 󱑆 ^c$black^^b$blue^ $hm "
}

power() {
	printf "^k9^^c$white^^b$black^  ⏻  "
}

wakeup=0
trap 'wakeup=1; kill $sp 2>/dev/null' USR1

interval=0 last=
while :; do
	set -- $(date '+%s %-I:%M %p'); now=$1 hm="$2 $3"
	[ $interval = 0 ] || [ $((interval % 720)) = 0 ] && updates=$(pkg_updates)
	[ $((interval % 6)) = 0 ] && bat=$(battery)
	[ $wakeup = 1 ] || [ $((interval % 6)) = 0 ] && bts=$(bluetooth)
	if [ $wakeup = 1 ] || [ $((interval % 12)) = 0 ]; then
		vol=$(volume)
	fi
	wifi
	wakeup=0
	interval=$((interval + 1))

	line="$updates$(cpu) $(mem)$bat $(brightness)$vol$wifis$bts$(caffeine)$(clock)$(power)"
	[ "$line" = "$last" ] || { xsetroot -name "$line"; last=$line; }
	sleep $((5 - now % 5)) & sp=$!
	wait $sp 2>/dev/null
done
