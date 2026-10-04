#!/bin/bash
# External monitor layout. Every connected DP/HDMI output gets its native-aspect
# mode with the most pixels at the highest refresh the link allows, placed left
# of eDP-1 (eDP-1 stays primary); unplugged outputs are switched off.
# Usage: monitor.sh          apply once
#        monitor.sh watch    apply now and on every DRM hotplug (singleton)

internal=eDP-1

apply() {
	xrandr --query | awk -v internal="$internal" '
	function flush() {
		if (out == "") return
		if (best == "") { best = any; rate = anyrate }
		if (best != "") cmd = cmd " --output " out " --mode " best " --rate " rate " --left-of " left
		left = out; out = ""; best = ""
	}
	/^[^ ]/ { flush() }
	/^[^ ]+ disconnected/ && $1 != internal { cmd = cmd " --output " $1 " --off" }
	/^[^ ]+ connected/ && $1 != internal {
		out = $1; asp = 0; bestpx = 0; rate = 0; any = ""; anypx = 0; anyrate = 0
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
		for (i = 2; i <= NF; i++) {
			r = $i; gsub(/[*+]/, "", r)
			if (r !~ /^[0-9.]+$/) continue
			if (w * h > anypx || (w * h == anypx && r + 0 > anyrate + 0)) {
				anypx = w * h; any = $1; anyrate = r
			}
			if (native && (w * h > bestpx || (w * h == bestpx && r + 0 > rate + 0))) {
				bestpx = w * h; best = $1; rate = r
			}
		}
	}
	BEGIN { left = internal }
	END { flush(); if (cmd != "") print cmd }
	' | {
		read -r args || return 0
		# shellcheck disable=SC2086
		xrandr --output "$internal" --primary --auto $args
	}
}

if [ "$1" = watch ]; then
	exec 9>"${XDG_RUNTIME_DIR:-/tmp}/monitor-watch.lock"
	flock -n 9 || exit 0
	apply
	udevadm monitor --udev --subsystem-match=drm | while read -r line; do
		case $line in *change*) ;; *) continue ;; esac
		# collapse the burst of uevents one plug produces
		while read -t 1 -r _; do :; done 2>/dev/null
		sleep 1; apply
	done
else
	apply
fi
