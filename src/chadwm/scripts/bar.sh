#!/bin/dash
# chadwm status (xsetroot -name), Nerd Font icons in four pills + power:
#   [ updates cpu mem gpu ] [ wifi vpn bluetooth ]
#   [ volume brightness battery caffeine ] [ date hijri clock ]  power
# Each module starts with ^kN^: chadwm records its position and a click/scroll
# runs scripts/barclick.sh N BUTTON; hovering it runs scripts/barhover.sh
# (details popup, chadwm bartip.c).
#   1 updates  2 cpu (usage + package temp)  3 mem
#   13 gpu: MX150 load + temp, only while its driver is loaded (gpu-power)
#   7 wifi (signal icon)  12 stealth VPN laptop2 (vpn.sh)  11 bluetooth
#   6 volume  5 brightness  4 battery  10 caffeine (caffeine.sh)
#   8 date, Umm al-Qura Hijri date in Arabic (hijri, ~/.local/bin from hijri.c) and
#     clock; hover = this month in both calendars, click = calcurse,
#     right-click = both calendars month by month (calendar.sh)
#   9 power
# Refresh: every 5 s (aligned so the clock flips on the minute), at once on
# SIGUSR1 (everything; barclick.sh sends it) and SIGUSR2 (volume and
# brightness only: vol.sh and brightness clicks/scrolls send it, so fast
# scrolling does not rerun bluetoothctl, date, nvidia-smi... per notch).
# Cost per tick: builtins + one `date`; nvidia-smi (~20 ms) only while the
# MX150 is on; pactl only on start, SIGUSR1 and every 60 s; battery every
# tick from battwatch's state file (builtins; sysfs only as fallback);
# SSID when the link changes; hijri once a day.

# ^c$var^ = fg color, ^b$var^ = bg color
. /home/btw/.config/chadwm/scripts/bar_themes/grayscale

echo $$ > "${XDG_RUNTIME_DIR:-/tmp}/chadwm-bar.pid"

for wlan_dev in /sys/class/net/wl*; do break; done
for bl_dev in /sys/class/backlight/*; do break; done
for h in /sys/class/hwmon/hwmon*; do
	read -r n < "$h/name" && [ "$n" = coretemp ] && cpu_temp=$h/temp1_input && break
done
hijri_bin=$HOME/.local/bin/hijri
bw_now=${XDG_RUNTIME_DIR:-/tmp}/battwatch/now

pkg_updates() {
	# arch: pacman -Qu lists what the last -Sy saw (no network access here)
	n=$(timeout 20 pacman -Qu 2>/dev/null | wc -l)
	if [ "$n" -eq 0 ]; then
		printf "^k1^^c$darkblue^󰏗"
	else
		printf "^k1^^c$green^󰏗 ^c$white^$n"
	fi
}

cpu() {
	# sets $cpus in the main shell: usage since the last tick (/proc/stat)
	# and the package temperature (coretemp), builtins only
	read -r _ u n s i w q sq st _ < /proc/stat
	t=$((u + n + s + i + w + q + sq + st)) idle=$((i + w))
	dt=$((t - ${ct:-0})) di=$((idle - ${ci:-0}))
	ct=$t ci=$idle
	[ $dt -gt 0 ] && cpup=$(( (100 * (dt - di) + dt / 2) / dt ))
	tc=0; [ -n "$cpu_temp" ] && read -r tc < "$cpu_temp" 2>/dev/null
	cpus="^k2^^c$green^󰻠 ^c$white^${cpup:-0}% $((tc / 1000))°"
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
	printf "^k3^^c$green^  ^c$white^$((u / 1024)).$(( (u % 1024) * 10 / 1024 ))G"
}

gpu_users() {
	# prime-run holds a shared flock on /run/gpu-power.users while a program
	# runs; /proc/locks lines end in "pid maj:min:inode start end"
	[ -n "$gpu_ino" ] || gpu_ino=$(stat -c %i /run/gpu-power.users 2>/dev/null) || return 1
	while read -r _ _ _ _ _ id _; do
		[ "${id##*:}" = "$gpu_ino" ] && return 0
	done < /proc/locks
	return 1
}

gpu() {
	# sets $gpus: the MX150 is shown only while gpu-power has its driver
	# loaded (AC policy, `gpu-power on`, or a prime-run program). Never call
	# nvidia-smi without the driver: it would load it and power the GPU up.
	gpus=
	[ -d /sys/module/nvidia ] || return 0
	# Runtime D3 (gpu-power rtd3): the driver stays loaded and the GPU powers
	# itself off when idle. nvidia-smi would wake it every tick, so a sleeping
	# GPU is shown as such (reading runtime_status does not wake it).
	read -r rs < /sys/bus/pci/devices/0000:01:00.0/power/runtime_status 2>/dev/null
	if [ "$rs" = suspended ]; then
		gpus="  ^k13^^c$green^󰢮 ^c$white^zz"
	else
		r=$(timeout 3 nvidia-smi --query-gpu=utilization.gpu,temperature.gpu \
			--format=csv,noheader,nounits 2>/dev/null)
		oifs=$IFS; IFS=', '; set -- $r; IFS=$oifs
		[ $# -ge 2 ] && gpus="  ^k13^^c$green^󰢮 ^c$white^$1% $2°"
	fi
	# A prime-run program that ends while nvidia-smi has the GPU open makes
	# its `gpu-power release` fail (modprobe -r: in use), leaving the GPU on.
	# On battery with no program holding it, re-apply the policy (it keeps
	# the GPU on when it was turned on by hand).
	read -r ac < /sys/class/power_supply/AC/online 2>/dev/null
	[ "$ac" = 0 ] && ! gpu_users && gpu-power apply >/dev/null 2>&1
	return 0
}

battery() {
	# battwatch-daemon writes $XDG_RUNTIME_DIR/battwatch/now every tick
	# (combined %, status, smoothed watts, ETA): read it with builtins only.
	# Stale (>5 min) or missing: fall back to summing the packs from sysfs.
	p= st= eta=-1 ts=0
	if [ -r "$bw_now" ]; then
		while IFS== read -r k v; do
			case $k in
			ts) ts=$v ;; pct) p=$v ;; status) st=$v ;; eta_min) eta=$v ;;
			esac
		done < "$bw_now"
		[ $((now - ts)) -gt 300 ] && p=
	fi
	if [ -z "$p" ]; then
		now_e=0 full=0 st=
		for b in /sys/class/power_supply/BAT*; do
			[ -r "$b/energy_now" ] && f=energy || f=charge
			read -r v < "$b/${f}_now" && read -r w < "$b/${f}_full" || continue
			now_e=$((now_e + v)) full=$((full + w))
			read -r s < "$b/status"; [ "$s" = Charging ] && st=charging
		done
		[ $full -gt 0 ] || return
		p=$(( (now_e * 100 + full / 2) / full )) eta=-1
	fi
	if [ "$st" = charging ]; then i=󰂄
	elif [ $p -ge 95 ]; then i=󰁹
	elif [ $p -ge 75 ]; then i=󰂁
	elif [ $p -ge 50 ]; then i=󰁿
	elif [ $p -ge 25 ]; then i=󰁽
	elif [ $p -ge 10 ]; then i=󰁻
	else i=󰂃; fi
	c=$red; [ $p -lt 15 ] && [ "$st" != charging ] && c=$white
	# time left while draining (h:mm), from battwatch's smoothed draw
	t=
	if [ "$st" = discharging ] && [ "$eta" -ge 0 ]; then
		m=$((eta % 60)); [ $m -lt 10 ] && m=0$m
		t=" $((eta / 60)):$m"
	fi
	# live power from the packs (µW; only one pack works at a time): the
	# laptop's draw on battery, the charge rate (+) on AC
	uw=0
	for b in /sys/class/power_supply/BAT*; do
		read -r v < "$b/power_now" 2>/dev/null && uw=$((uw + v))
	done
	w=$(( (uw + 50000) / 100000 )) pw=
	if [ $w -gt 0 ]; then
		[ "$st" = charging ] && pw=" +" || pw=" "
		pw="$pw$((w / 10)).$((w % 10))W"
	fi
	printf "  ^k4^^c$c^$i ^c$white^$p%%$t$pw"
}

brightness() {
	read -r cur < "$bl_dev/actual_brightness" 2>/dev/null || return
	read -r max < "$bl_dev/max_brightness"
	p=$((cur * 100 / max))
	if [ $p -ge 67 ]; then i=󰃠; elif [ $p -ge 34 ]; then i=󰃟; else i=󰃞; fi
	printf "  ^k5^^c$red^$i ^c$white^$p%%"
}

volume() {
	v=$(pactl get-sink-volume @DEFAULT_SINK@ 2>/dev/null)
	v=${v#*/}; v=${v%%%*}; v=${v##* }
	case $(pactl get-sink-mute @DEFAULT_SINK@ 2>/dev/null) in
	*yes) printf "^k6^^c$darkblue^󰝟 mute" ;;
	*)
		if [ "${v:-0}" -ge 60 ]; then i=󰕾; elif [ "${v:-0}" -ge 25 ]; then i=󰖀; else i=󰕿; fi
		printf "^k6^^c$red^$i ^c$white^${v:-?}%%" ;;
	esac
}

wifi() {
	# sets $wifis in the main shell (a $(...) subshell would lose the
	# cached SSID and run nmcli every tick); signal from /proc/net/wireless
	state=down
	[ -r "$wlan_dev/operstate" ] && read -r state < "$wlan_dev/operstate"
	if [ "$state" != "$wstate" ] || [ $wakeup = 1 ]; then
		wstate=$state
		ssid=
		[ "$state" = up ] && ssid=$(nmcli -t -g GENERAL.CONNECTION dev show "${wlan_dev##*/}" 2>/dev/null)
	fi
	case $state in
	up)
		q=0
		while read -r ifc _ lq _; do
			[ "$ifc" = "${wlan_dev##*/}:" ] && q=${lq%.}
		done < /proc/net/wireless
		if [ "$q" -ge 52 ]; then i=󰤨; elif [ "$q" -ge 35 ]; then i=󰤥
		elif [ "$q" -ge 17 ]; then i=󰤢; else i=󰤟; fi
		wifis="^k7^^c$blue^$i ^c$white^${ssid:-connected}" ;;
	*) wifis="^k7^^c$darkblue^󰤭 offline" ;;
	esac
}

vpn() {
	# sets $vpns in the main shell (keeps $vrx/$vrt across ticks); builtins
	# only. Healthy = rx_bytes moved in the last 75 s: stealth-vpn pings
	# 10.0.0.1 through laptop2 every 30 s, and a fresh interface starts at 0.
	if read -r rx 2>/dev/null < /sys/class/net/laptop2/statistics/rx_bytes; then
		[ "$rx" = "$vrx" ] || { [ "$rx" -gt 0 ] && vrt=$now; vrx=$rx; }
		# icon only, right after the Wi-Fi name: the on/off switch is in chadnet
		if [ "$rx" -gt 0 ] && [ $((now - ${vrt:-0})) -lt 75 ]; then
			vpns=" ^k12^^c$blue^󰕥"
		else
			vpns=" ^k12^^c$darkblue^󰕥"
		fi
	else
		vrx=
		read -r vst 2>/dev/null < /run/stealth-vpn.state
		# off at home or by choice: nothing; off when it should be up: warn
		if [ -e /run/stealth-vpn.user-off ]; then
			vpns=
		else
			case $vst in
			home*) vpns= ;;
			*) vpns=" ^k12^^c$white^󰦞" ;;
			esac
		fi
	fi
}

bluetooth() {
	# bluetoothd state every 30 s and on SIGUSR1; a D-Bus call, not a fork storm
	p=$(timeout 3 bluetoothctl show 2>/dev/null | awk '/Powered:/{print $2}')
	case $p in
	yes)
		d=$(timeout 3 bluetoothctl devices Connected 2>/dev/null | head -1 | cut -d' ' -f3-)
		if [ -n "$d" ]; then printf "  ^k11^^c$blue^󰂱 ^c$white^${d%% *}"
		else printf "  ^k11^^c$blue^󰂯"; fi ;;
	*) printf "  ^k11^^c$darkblue^󰂲" ;;
	esac
}

caffeine() {
	# on while caffeine.sh's elogind-inhibit process lives (builtins only)
	if read -r cpid 2>/dev/null < "${XDG_RUNTIME_DIR:-/tmp}/caffeine.pid" &&
		read -r ccomm 2>/dev/null < "/proc/$cpid/comm" &&
		[ "$ccomm" = elogind-inhibit ]; then
		printf "  ^k10^^c$white^󰅶"
	else
		printf "  ^k10^^c$darkblue^󰛊"
	fi
}

clock() {
	# Gregorian + Umm al-Qura date in Arabic (hijri once a day, at the first
	# tick of a new date: the civil Hijri day turns at midnight too), then the
	# time. chadwm runs fribidi per ^…^ chunk, so the space after the Arabic
	# chunk sits in its own chunk (it would move to the RTL start otherwise).
	# The Arabic glyphs come from fonts[2] in config.h (Droid Arabic Kufi).
	# The moon icon follows the real phase: age since the new moon of
	# 2000-01-06 18:14 UTC modulo the mean synodic month (2551443 s), in 8
	# steps (new, waxing crescent, first quarter, waxing gibbous, full,
	# waning gibbous, last quarter, waning crescent). The mean month is
	# within about half a day of the true one, well inside a step.
	age=$(( (now - 947182440) % 2551443 ))
	case $(( (age * 8 + 1275721) / 2551443 % 8 )) in
	0) moon=󰽤 ;; 1) moon=󰽧 ;; 2) moon=󰽡 ;; 3) moon=󰽨 ;;
	4) moon=󰽢 ;; 5) moon=󰽦 ;; 6) moon=󰽣 ;; *) moon=󰽥 ;;
	esac
	printf "^k8^^c$blue^󰃭 ^c$white^$gdate  ^c$blue^$moon ^c$white^$hdate^c$white^   ^c$blue^󱑆 ^c$white^$hm "
}

power() {
	printf "^k9^^c$white^^b$black^ ⏻ "
}

wakeup=0 quick=0
trap 'wakeup=1; kill $sp 2>/dev/null' USR1
trap 'quick=1; kill $sp 2>/dev/null' USR2

interval=0 last=
while :; do
	if [ $quick = 1 ] && [ $wakeup = 0 ]; then
		quick=0
		vol=$(volume) bri=$(brightness)
	else
	quick=0
	set -- $(date '+%s %-I:%M %p %a %-d %b %F')
	now=$1 hm="$2 $3" gdate="$4 $5 $6"
	[ "$7" = "$hday" ] || { hday=$7; hdate=$("$hijri_bin" short 2>/dev/null); }
	[ $interval = 0 ] || [ $((interval % 720)) = 0 ] && updates=$(pkg_updates)
	bat=$(battery)
	[ $wakeup = 1 ] || [ $((interval % 6)) = 0 ] && bts=$(bluetooth)
	if [ $wakeup = 1 ] || [ $((interval % 12)) = 0 ]; then
		vol=$(volume)
	fi
	cpu
	gpu
	wifi
	vpn
	mems=$(mem) bri=$(brightness) caf=$(caffeine) clk=$(clock) pw=$(power)
	wakeup=0
	interval=$((interval + 1))
	fi

	line="^b$grey^ $updates  $cpus  $mems$gpus ^b$black^ ^b$grey^ $wifis$vpns$bts ^b$black^ ^b$grey^ $vol$bri$bat$caf ^b$black^ ^b$grey^ $clk^b$black^ $pw"
	# no X any more (a restarted bar outlives its session): stop
	[ "$line" = "$last" ] || { xsetroot -name "$line" || exit 0; last=$line; }
	# after a quick refresh $now is stale by a few seconds at most; the
	# next full tick realigns to the 5 s grid
	sleep $((5 - now % 5)) & sp=$!
	wait $sp 2>/dev/null
done
