#!/bin/sh
# vol.sh up|down|mute — default sink volume (5%, capped at 100%) and an
# immediate cheap bar refresh (bar.sh re-reads volume/brightness on SIGUSR2).
case $1 in
up) pactl set-sink-mute @DEFAULT_SINK@ 0
    v=$(pactl get-sink-volume @DEFAULT_SINK@); v=${v#*/}; v=${v%%%*}; v=${v# }; v=${v# }
    if [ "${v:-0}" -ge 95 ]; then pactl set-sink-volume @DEFAULT_SINK@ 100%
    else pactl set-sink-volume @DEFAULT_SINK@ +5%; fi ;;
down) pactl set-sink-volume @DEFAULT_SINK@ -5% ;;
mute) pactl set-sink-mute @DEFAULT_SINK@ toggle ;;
esac
kill -USR2 "$(cat "${XDG_RUNTIME_DIR:-/tmp}/chadwm-bar.pid")" 2>/dev/null
