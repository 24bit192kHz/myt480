#!/bin/sh
# caffeine.sh toggle|on|off|status — HyDE idle-inhibitor analogue (bar ☕).
# On: an elogind-inhibit block lock on sleep, idle, the lid switch and the
# suspend/hibernate keys, so neither closing the lid nor anything else can
# suspend or hibernate. Off: the lock's process is killed. The lock only lives
# as long as that process, so logout or reboot always returns to normal.
PIDF=${XDG_RUNTIME_DIR:-/tmp}/caffeine.pid
refresh() { kill -USR1 "$(cat "${XDG_RUNTIME_DIR:-/tmp}/chadwm-bar.pid")" 2>/dev/null; }

running() {
	read -r pid 2>/dev/null < "$PIDF" || return 1
	read -r comm 2>/dev/null < "/proc/$pid/comm" || return 1
	[ "$comm" = elogind-inhibit ]
}

on() {
	running && return 0
	setsid -f sh -c 'echo $$ > "$1"; exec elogind-inhibit \
		--what=sleep:idle:handle-lid-switch:handle-suspend-key:handle-hibernate-key \
		--who=caffeine --why="coffee cup in the bar" --mode=block sleep infinity' \
		sh "$PIDF" </dev/null >/dev/null 2>&1
	sleep 0.2
	if running; then
		notify-send -a caffeine -t 2500 "󰅶  Caffeine on" "No sleep or hibernation, lid closed or not"
	else
		rm -f "$PIDF"
		notify-send -a caffeine -u critical "Caffeine failed" "elogind refused the inhibitor lock"
	fi
}

off() {
	running && kill "$pid"
	rm -f "$PIDF"
	notify-send -a caffeine -t 2500 "󰛊  Caffeine off" "Sleep and lid behave normally again"
}

case ${1:-toggle} in
toggle) if running; then off; else on; fi ;;
on) on ;;
off) off ;;
status) running && echo on || echo off; exit 0 ;;
*) echo "usage: caffeine.sh toggle|on|off|status" >&2; exit 2 ;;
esac
refresh
