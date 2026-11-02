#!/bin/bash
# External monitor switching. When a DP/HDMI output is connected it takes over:
# it gets its preferred (EDID) mode at the highest refresh the link allows (or,
# if none is offered, the native-aspect mode with the most pixels), becomes
# primary, and eDP-1 is switched off (more externals go to its right). With
# nothing plugged eDP-1 is the only output again; audio follows (see audio()).
# Usage: monitor.sh          apply once
#        monitor.sh watch    apply now and whenever a connector changes (singleton)

internal=eDP-1
card=alsa_card.pci-0000_00_1f.3
# Per-display refresh overrides: "EDID monitor name<TAB>Hz" lines, e.g.
# "LG TV	30" (half the TMDS clock of 60 Hz: more margin on a weak cable).
rates_conf=~/.config/chadwm/monitors.conf

# xrandr name -> EDID monitor name (descriptor 0xFC) from the kernel's copy,
# which reading does not re-probe the link.
edid_name() {
	python3 -I -c '
import glob, sys
for f in glob.glob("/sys/class/drm/card*-" + sys.argv[1] + "/edid"):
    d = open(f, "rb").read()
    for o in range(54, 126, 18):
        if d[o:o + 3] == b"\0\0\0" and d[o + 3] == 0xFC:
            print(d[o + 5:o + 18].split(b"\n")[0].decode("ascii", "replace").strip())
            sys.exit()' "$(printf '%s' "$1" | sed 's/^HDMI-/HDMI-A-/')"
}

# "OUT=Hz ..." for connected outputs with an override in $rates_conf
wanted_rates() {
	[ -r "$rates_conf" ] || return
	xrandr --current | awk '$2 == "connected" { print $1 }' | while read -r o; do
		n=$(edid_name "$o")
		[ -n "$n" ] || continue
		r=$(awk -F '\t' -v n="$n" '$1 == n { print $2; exit }' "$rates_conf")
		[ -n "$r" ] && printf '%s=%s ' "$o" "$r"
	done
}

# Audio follows the screen: the HDMI profile while an external is on (its port
# only turns available once the TV's ELD arrives, so wait for it), analog when
# not. The default sink is moved only if it is on this card (a Bluetooth
# headset stays the default).
audio() {
	profile=output:analog-stereo+input:analog-stereo
	if [ -n "$1" ]; then
		for _ in 1 2 3 4 5 6 7 8 9 10; do
			port=$(pactl list cards | awk '$1 ~ /^hdmi-output-[0-9]+:$/ && !/not available/ { sub(":", "", $1); print $1; exit }')
			[ -n "$port" ] && break
			sleep 0.5
		done
		case $port in
		hdmi-output-0) profile=output:hdmi-stereo+input:analog-stereo ;;
		hdmi-output-[0-9]*) profile=output:hdmi-stereo-extra${port#hdmi-output-}+input:analog-stereo ;;
		esac
	fi
	pactl list cards | grep -q "Active Profile: $profile\$" ||
		pactl set-card-profile "$card" "$profile" || return
	sink=$(pactl list short sinks | awk -v c="${card#alsa_card.}" '$2 ~ "^alsa_output\\." c { print $2; exit }')
	default=$(pactl get-default-sink)
	case $default in alsa_output.${card#alsa_card.}*|'') ;; *) return ;; esac
	[ -n "$sink" ] && [ "$sink" != "$default" ] || return
	pactl set-default-sink "$sink"
	pactl list short sink-inputs | while read -r id _; do
		pactl move-sink-input "$id" "$sink"
	done
	kill -USR2 "$(cat "${XDG_RUNTIME_DIR:-/tmp}/chadwm-bar.pid")" 2>/dev/null
}

# Connector states as the kernel last saw them (reading these does not probe).
state() {
	for c in /sys/class/drm/card*-*/status; do printf '%s=%s ' "${c%/status}" "$(cat "$c")"; done
}

# Number of connected external outputs in a state() string.
n_ext() {
	printf '%s' "$1" | tr ' ' '\n' | grep '=connected' | grep -vc eDP
}

apply() {
	# Outputs the kernel calls disconnected never take over from eDP-1, even
	# if xrandr's own probe got a lucky EDID read on a flaky link.
	gone=
	for c in /sys/class/drm/card*-*/status; do
		[ "$(cat "$c")" = connected ] && continue
		c=${c%/status}; c=${c##*/card[0-9]-}; gone="$gone ${c/HDMI-A-/HDMI-}"
	done
	# X's cached view; a real probe re-reads DDC on unpinned outputs. Probe only
	# if the kernel reports an external that X has no modes for yet.
	query=--current
	[ "$1" = probe ] && query=--query
	want=$(wanted_rates)
	xrandr $query | awk -v internal="$internal" -v gone="$gone " -v want="$want" '
	function flush() {
		if (out == "") return
		# the EDID preferred mode wins; TVs report bogus mm sizes
		if (pref != "") { best = pref; rate = prefrate; rates = prefrates }
		else if (best == "") { best = any; rate = anyrate; rates = anyrates }
		# a configured refresh, if the chosen mode offers it
		if (match(" " want, " " out "=[0-9.]+")) {
			w = substr(" " want, RSTART + length(out) + 2, RLENGTH - length(out) - 2) + 0
			n = split(rates, rr, " ")
			d = 0.05
			for (i = 1; i <= n; i++) {
				e = rr[i] - w; if (e < 0) e = -e
				if (e < d) { d = e; rate = rr[i] }
			}
		}
		if (best != "") {
			on = on " --output " out " --mode " best " --rate " rate
			on = on (prev == "" ? " --pos 0x0 --primary" : " --right-of " prev)
			prev = out
		}
		out = ""; best = ""
	}
	/^[^ ]/ { flush() }
	/^[^ ]+ (dis)?connected/ && $1 != internal && ($2 == "disconnected" || index(gone, " " $1 " ")) {
		off = off " --output " $1 " --off"; next
	}
	/^[^ ]+ connected/ && $1 != internal {
		out = $1; asp = 0; bestpx = 0; rate = 0; any = ""; anypx = 0; anyrate = 0; pref = ""; prefrate = 0
		rates = ""; prefrates = ""; anyrates = ""
		if (match($0, /[0-9]+mm x [0-9]+mm/)) {
			split(substr($0, RSTART, RLENGTH), mm, /mm x |mm/)
			if (mm[2] > 0) asp = mm[1] / mm[2]
		}
		next
	}
	out != "" && /^   [0-9]+x[0-9]+ / {
		split($1, wh, "x"); w = wh[1] + 0; h = wh[2] + 0
		# modes off the physical panel aspect are scaled: use them only if
		# nothing matches (best* = native aspect, any* = fallback)
		native = asp == 0 || (w / h / asp < 1.03 && w / h / asp > 0.97)
		if (pref == "" && $0 ~ /\+/) pref = $1
		for (i = 2; i <= NF; i++) {
			r = $i; gsub(/[*+]/, "", r)
			if (r !~ /^[0-9.]+$/) continue
			if (pref == $1) {
				prefrates = prefrates " " r
				if (r + 0 > prefrate + 0) prefrate = r
			}
			if (w * h > anypx || (w * h == anypx && r + 0 > anyrate + 0)) {
				if (any != $1) anyrates = ""
				anypx = w * h; any = $1; anyrate = r
			}
			if (any == $1) anyrates = anyrates " " r
			if (native && (w * h > bestpx || (w * h == bestpx && r + 0 > rate + 0))) {
				bestpx = w * h; best = $1; rate = r
			}
		}
	}
	END { flush(); print on; print off }
	' | {
		read -r on; read -r off
		if [ -z "$on" ] && [ "$query" = --current ] && [ "$(n_ext "$(state)")" -gt 0 ]; then
			apply probe; return
		fi
		# HDMI 1.4 is marginal at 12 bpc deep colour (223 MHz at 1080p60): the
		# TV loses lock, shows red/pink garbage and drops HPD. Stay at 8 bpc.
		for o in $on; do
			case $o in HDMI-*) xrandr --output "$o" --set "max bpc" 8 --set audio auto ;; esac
		done
		# shellcheck disable=SC2086
		if [ -n "$on" ]; then
			xrandr $off $on --output "$internal" --off
		else
			xrandr $off --output "$internal" --auto --primary --pos 0x0
		fi
		audio "$on"
	}
}

if [ "$1" = watch ]; then
	# t480-watch (myt480 src/t480-watch) does the same watch without a fork per
	# tick and runs this script (apply) on a change; the loop below is the fallback.
	w=$(command -v t480-watch) && exec "$w" monitors "$0"
	exec 9>"${XDG_RUNTIME_DIR:-/tmp}/monitor-watch.lock"
	flock -n 9 || exit 0
	# Poll the kernel's connector states rather than trusting uevents (a
	# userspace probe can swallow a change event). HDMI states are stable
	# because edid-pind (/usr/local/sbin) takes presence from the HPD pin and
	# pins the EDID, so a new state only has to hold for 2 s.
	last=$(state); apply
	while sleep 2; do
		now=$(state)
		[ "$now" = "$last" ] && continue
		sleep 2
		[ "$(state)" = "$now" ] || continue
		last=$now; apply
	done
else
	apply
fi
