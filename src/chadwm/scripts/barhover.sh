#!/bin/dash
# barhover.sh SEQ MODULE — the hover popup for a bar module (bartip.c).
# chadwm runs it when the pointer settles on ^kMODULE^ and writes
# "SEQ MODULE" to $XDG_RUNTIME_DIR/chadwm-hover; the popup text goes back in
# the root property _CHADWM_TIP as "SEQ\ntext". Live modules refresh every
# 2 s until the pointer leaves (the file changes). Clicks stay in barclick.sh.
# Text codes: ^c#rrggbb^ colour, ^b#rrggbb^ background, ^d^ default,
# ^>N^ right-align the next text at column N.
. "$HOME/.config/chadwm/scripts/bar_themes/grayscale"
seq=$1 id=$2
state=${XDG_RUNTIME_DIR:-/tmp}/chadwm-hover

alive() { read -r s _ < "$state" 2>/dev/null && [ "$s" = "$seq" ]; }

T() { printf '^c%s^%s^d^\n' "$blue" "$*"; }        # title line
R() { printf '^c%s^%-10s^d^%s\n' "$darkblue" "$1" "$2"; }   # label  value

updates() {
	n=$(timeout 10 pacman -Qu 2>/dev/null | wc -l)
	T "󰏗  $n updates"
	timeout 10 pacman -Qu 2>/dev/null | head -15 | awk '{printf "%s  ^c'"$darkblue"'^%s → %s^d^\n", $1, $2, $4}'
	[ "$n" -gt 15 ] && echo "^c$darkblue^… $((n - 15)) more^d^"
	echo; echo "^c$darkblue^click: update   right: recount^d^"
}

cpu() {
	# usage over 0.5 s, per-core MHz, temperatures, fan, top processes
	read -r _ u n s i w q sq st _ < /proc/stat
	t1=$((u + n + s + i + w + q + sq + st)) i1=$((i + w))
	top=$(top -b -n 2 -d 0.5 -w 120 -o %CPU 2>/dev/null | awk '/^top -/{f++} f==2 && /^ *PID/{p=1; next} p && n<5 {printf "%5.1f%%  %s\n", $9, $12; n++}')
	read -r _ u n s i w q sq st _ < /proc/stat
	t2=$((u + n + s + i + w + q + sq + st)) i2=$((i + w))
	dt=$((t2 - t1)); [ $dt -gt 0 ] || dt=1
	T "󰻠  CPU $(( (100 * (dt - (i2 - i1)) + dt / 2) / dt ))%"
	R model "$(sed -n 's/^model name.*: //p;T;q' /proc/cpuinfo | sed 's/(R)//g; s/(TM)//g; s/ CPU//; s/ @.*//')"
	f=; for c in /sys/devices/system/cpu/cpu[0-9]*/cpufreq/scaling_cur_freq; do
		read -r v < "$c"; f="$f $((v / 1000))"
	done
	R MHz "${f# }"
	read -r gov < /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null
	read -r epp < /sys/devices/system/cpu/cpu0/cpufreq/energy_performance_preference 2>/dev/null
	R governor "$gov${epp:+ / $epp}"
	R load "$(cut -d' ' -f1-3 /proc/loadavg)"
	tl=
	for h in /sys/class/hwmon/hwmon*; do
		read -r nm < "$h/name"
		case $nm in
		coretemp) read -r v < "$h/temp1_input"; tl="$tl pkg $((v / 1000))°" ;;
		thinkpad)
			read -r fan < "$h/fan1_input" 2>/dev/null
			read -r v < "$h/temp2_input" 2>/dev/null && [ "$v" -gt 0 ] && tl="$tl gpu $((v / 1000))°" ;;
		nvme) read -r v < "$h/temp1_input"; tl="$tl nvme $((v / 1000))°" ;;
		esac
	done
	R temps "${tl# }"
	[ -n "$fan" ] && R fan "$fan rpm"
	echo; echo "^c$darkblue^top processes^d^"
	echo "$top"
	echo; echo "^c$darkblue^click: btop^d^"
}

mem() {
	while read -r k v _; do
		case $k in
		MemTotal:) mt=$v ;; MemAvailable:) ma=$v ;; Cached:) mc=$v ;;
		SwapTotal:) stt=$v ;; SwapFree:) sf=$v ;; Zswapped:) zs=$v ;;
		esac
	done < /proc/meminfo
	g() { awk "BEGIN{printf \"%.1fG\", $1/1048576}"; }
	T "  memory $(g $((mt - ma))) / $(g $mt)"
	R available "$(g $ma)"
	R cached "$(g $mc)"
	R swap "$(g $((stt - sf))) / $(g $stt)${zs:+  (zswap $(g $zs))}"
	echo; echo "^c$darkblue^top processes^d^"
	ps -eo rss=,comm= --sort=-rss | head -5 | awk '{printf "%6.0fM  %s\n", $1/1024, $2}'
	echo; echo "^c$darkblue^click: btop^d^"
}

gpu() {
	read -r rs < /sys/bus/pci/devices/0000:01:00.0/power/runtime_status 2>/dev/null
	if [ ! -d /sys/module/nvidia ]; then
		T "󰢮  MX150 off"; echo "driver not loaded"
	elif [ "$rs" = suspended ]; then
		T "󰢮  MX150 asleep"; echo "runtime-suspended (not woken for this)"
	else
		T "󰢮  MX150"
		timeout 3 nvidia-smi --query-gpu=utilization.gpu,temperature.gpu,power.draw,clocks.gr,clocks.mem,memory.used,memory.total,pstate \
			--format=csv,noheader 2>/dev/null | awk -F', ' -v c="$darkblue" '{
			printf "^c%s^%-10s^d^%s\n^c%s^%-10s^d^%s\n^c%s^%-10s^d^%s\n^c%s^%-10s^d^%s / %s\n^c%s^%-10s^d^%s / %s\n^c%s^%-10s^d^%s\n",
			c,"load",$1, c,"temp",$2"°", c,"power",$3, c,"clocks",$4,$5, c,"vram",$6,$7, c,"pstate",$8}'
	fi
	echo; gpu-power status 2>/dev/null | head -3 | sed "s/^/^c$darkblue^/"
	echo; echo "^c$darkblue^click: nvtop   right: power status^d^"
}

wifi() {
	dev=wlan0
	if [ "$(cat /sys/class/net/$dev/operstate 2>/dev/null)" != up ]; then
		T "󰤭  offline"
	else
		info=$(iw dev $dev link 2>/dev/null)
		# profile name, not the SSID (the home profile is renamed "Home" for privacy)
		T "󰤨  $(nmcli -t -g GENERAL.CONNECTION dev show $dev 2>/dev/null)"
		R signal "$(echo "$info" | sed -n 's/^\tsignal: //p')"
		R band "$(echo "$info" | sed -n 's/^\tfreq: \([0-9]*\).*/\1 MHz/p')"
		R rate "$(echo "$info" | sed -n 's/^\trx bitrate: \([0-9.]* MBit\/s\).*/\1 down/p') $(echo "$info" | sed -n 's/^\ttx bitrate: \([0-9.]* MBit\/s\).*/\1 up/p')"
		R ip "$(ip -4 -o addr show $dev | awk '{print $4}' | head -1)"
		R gateway "$(ip route show default dev $dev 2>/dev/null | awk '{print $3; exit}')"
	fi
	ip -4 -o addr show eth0 2>/dev/null | awk '{print $4}' | grep -q . && R ethernet "$(ip -4 -o addr show eth0 | awk '{print $4}')"
	echo; echo "^c$darkblue^click: networks   right: connections^d^"
}

vpn() {
	if [ -d /sys/class/net/laptop2 ]; then
		T "󰕥  VPN up (laptop2)"
		read -r rx < /sys/class/net/laptop2/statistics/rx_bytes
		read -r tx < /sys/class/net/laptop2/statistics/tx_bytes
		R traffic "$((rx / 1048576)) MiB down, $((tx / 1048576)) MiB up"
		R ip "$(ip -4 -o addr show laptop2 | awk '{print $4}')"
	else
		T "󰦞  VPN down"
	fi
	[ -r /run/stealth-vpn.state ] && R state "$(cat /run/stealth-vpn.state)"
	[ -e /run/stealth-vpn.user-off ] && R "" "turned off by you"
	echo; echo "^c$darkblue^click: on / off   right: status^d^"
}

bluetooth() {
	if [ "$(timeout 3 bluetoothctl show 2>/dev/null | awk '/Powered:/{print $2}')" = yes ]; then
		T "󰂯  Bluetooth on"
		c=$(timeout 3 bluetoothctl devices Connected 2>/dev/null | cut -d' ' -f3-)
		if [ -n "$c" ]; then echo "$c" | sed 's/^/󰂱 /'; else echo "^c$darkblue^no device connected^d^"; fi
	else
		T "󰂲  Bluetooth off"
	fi
	echo; echo "^c$darkblue^click: devices   right: power^d^"
}

volume() {
	sink=$(pactl get-default-sink 2>/dev/null)
	T "󰕾  $(pactl get-sink-volume @DEFAULT_SINK@ 2>/dev/null | sed -n 's|.* \([0-9]*%\).*|\1|p;q')$(pactl get-sink-mute @DEFAULT_SINK@ 2>/dev/null | grep -q yes && echo '  muted')"
	R output "$(pactl list sinks 2>/dev/null | awk -v s="$sink" '/^\tName:/{n=($2==s)} n && /^\tDescription:/{sub(/^\tDescription: /,""); print; exit}')"
	R input "$(pactl get-source-volume @DEFAULT_SOURCE@ 2>/dev/null | sed -n 's|.* \([0-9]*%\).*|\1|p;q')$(pactl get-source-mute @DEFAULT_SOURCE@ 2>/dev/null | grep -q yes && echo ' muted')"
	echo; echo "^c$darkblue^click: mute   right: mixer   scroll: ±5%^d^"
}

brightness() {
	for b in /sys/class/backlight/*; do break; done
	read -r cur < "$b/actual_brightness"; read -r max < "$b/max_brightness"
	T "󰃠  $((cur * 100 / max))%"
	R level "$cur / $max"
	echo; echo "^c$darkblue^click / scroll: up / down^d^"
}

battery() {
	f=${XDG_RUNTIME_DIR:-/tmp}/battwatch/now
	pct= status= watts= eta_min=-1 packs= health= now_wh= full_wh=
	[ -r "$f" ] && while IFS== read -r k v; do
		case $k in
		pct) pct=$v ;; status) status=$v ;; watts) watts=$v ;; eta_min) eta_min=$v ;;
		packs) packs=$v ;; health) health=$v ;; now_wh) now_wh=$v ;; full_wh) full_wh=$v ;;
		esac
	done < "$f"
	e=; [ "${eta_min:--1}" -ge 0 ] && e=$(printf '%d:%02d' $((eta_min / 60)) $((eta_min % 60)))
	T "󰁹  $pct%  $status"
	case $status in
	charging) R power "${watts#-} W in"; [ -n "$e" ] && R full "in $e" ;;
	discharging) R power "$watts W"; [ -n "$e" ] && R left "$e" ;;
	*) R power "${watts:-?} W" ;;
	esac
	R energy "$now_wh / $full_wh Wh"
	read -r plan < /var/lib/thermald-t480/plan 2>/dev/null
	R plan "${plan:-auto}"
	if [ -d /sys/module/nvidia ]; then
		read -r rs < /sys/bus/pci/devices/0000:01:00.0/power/runtime_status 2>/dev/null
		[ "$rs" = suspended ] && g="driver loaded, asleep" || g=on
	else g=off; fi
	[ -r /run/gpu-power.manual ] && read -r m < /run/gpu-power.manual && g="$g (by hand: $m)"
	R MX150 "$g"
	for p in $packs; do
		n=${p%%:*} r=${p#*:}; h=
		for x in $health; do [ "${x%%:*}" = "$n" ] && h=${x#*:}; done
		s=${r#*:}; [ "$s" = Notcharging ] && s="not charging"
		R "$n" "${r%%:*}%  $s  (health ${h:-?}%)"
	done
	echo; echo "^c$darkblue^click: battwatch   middle: summary   right: TUI^d^"
}

caffeine() {
	if read -r p 2>/dev/null < "${XDG_RUNTIME_DIR:-/tmp}/caffeine.pid" && [ -d "/proc/$p" ]; then
		T "󰅶  caffeine on"; echo "screen blanking and sleep inhibited"
	else
		T "󰛊  caffeine off"
	fi
	echo; echo "^c$darkblue^click: toggle^d^"
}

clock() {
	TIP_ACC=$blue TIP_DIM=$darkblue TIP_BG=$black hijri tip 2>/dev/null || cal
	# prayer times (Umm al-Qura, local) and today's holidays / events
	command -v chadcal >/dev/null && { echo; TIP_ACC=$blue TIP_DIM=$darkblue chadcal --tip; }
	echo; echo "^c$darkblue^نقرة: التقويم · يمين: الأشهر الهجرية · وسط: الخط العربي^d^"
}

power() {
	T "⏻  $(uptime -p)"
	R since "$(uptime -s)"
	echo; echo "^c$darkblue^click: power menu^d^"
}

sleep 0.3
while alive; do
	case $id in
	1) text=$(updates) ;; 2) text=$(cpu) ;; 3) text=$(mem) ;; 4) text=$(battery) ;;
	5) text=$(brightness) ;; 6) text=$(volume) ;; 7) text=$(wifi) ;; 8) text=$(clock) ;;
	9) text=$(power) ;; 10) text=$(caffeine) ;; 11) text=$(bluetooth) ;; 12) text=$(vpn) ;;
	13) text=$(gpu) ;; *) exit 0 ;;
	esac
	alive || exit 0
	xprop -root -f _CHADWM_TIP 8u -set _CHADWM_TIP "$seq
$text"
	case $id in 2|3|4|5|6|7|12|13) ;; *) exit 0 ;; esac
	sleep 2
done
