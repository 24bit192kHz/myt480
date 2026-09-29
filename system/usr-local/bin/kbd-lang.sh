#!/bin/sh
# dwm-titus keyboard language: us,ara + caps_toggle, Titus-managed state.
# Usage: kbd-lang.sh [apply|toggle|status]
STATE="${XDG_CONFIG_HOME:-$HOME/.config}/dwm-titus/kbd-lang"
CUR=$(cat "$STATE" 2>/dev/null || echo "us,ara")
case "${1:-apply}" in
  apply)
    mkdir -p "$(dirname "$STATE")"
    echo "$CUR" > "$STATE"
    setxkbmap -layout "us,ara" -variant ",basic" -option grp:caps_toggle 2>/dev/null
    notify-send "Keyboard" "$CUR (caps toggles)" 2>/dev/null
    ;;
  toggle)
    if [ "$CUR" = "us,ara" ]; then CUR="ara,us"; else CUR="us,ara"; fi
    echo "$CUR" > "$STATE"
    setxkbmap -layout "$CUR" -variant ",basic" -option grp:caps_toggle 2>/dev/null
    notify-send "Keyboard" "$CUR (caps toggles)" 2>/dev/null
    ;;
  status)
    setxkbmap -query 2>/dev/null | grep -E "layout|options"
    ;;
esac
