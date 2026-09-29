#!/bin/sh
# chadwm session script (siduck/chadwm) — box-adapted for this T480, 2026-09-20.
# Replaces the dwm-titus session; box glue kept: cursor Xresources, toolkit
# theme-env, xsettingsd, polkit agent, screen-power policy, lock watcher.

xrdb -nocpp -merge ~/.Xresources
xrdb -nocpp -merge ~/.config/dwm-titus/cursor.Xresources 2>/dev/null
[ -f ~/.config/dwm-titus/theme-env.sh ] && . ~/.config/dwm-titus/theme-env.sh

# screen power policy (box default: blanking/DPMS off, GPU wake issues)
xset s off; xset s noblank; xset -dpms

/usr/local/bin/dwm-xsettings session-apply >/dev/null 2>&1 &
/usr/local/bin/dwm-polkit >/dev/null 2>&1 &
# dwm-lock-watch is singleton; this start remains safe across WM restarts.
/usr/local/bin/dwm-lock-watch >/dev/null 2>&1 &

~/.local/bin/bright restore &
# smooth Fn brightness ramping (polls the physical key state, immune to X repeat quirks)
# (C port of brightd.py: 2 devices, kernel event mask, no idle wakeups; src scripts/brightd.c)
pgrep -x brightd >/dev/null 2>&1 || setsid ~/.local/bin/brightd >/dev/null 2>&1 &
# Neutral root prevents the static image from flashing beneath the live Earth.
xsetroot -solid black >/dev/null 2>&1
picom &
# live ASCII Earth background: the supervisor is the only renderer owner.
# Started in the background so the WM never waits for it.
if [ -x "$HOME/.local/bin/gxwc-wallpaper" ]; then
    { "$HOME/.local/bin/gxwc-wallpaper" start || echo "wallpaper: gxwc supervisor failed to start" >&2; } &
else
    echo "wallpaper: missing $HOME/.local/bin/gxwc-wallpaper" >&2
fi
xset r rate 200 50 &

dash ~/.config/chadwm/scripts/bar.sh &
while type chadwm >/dev/null; do chadwm && continue || break; done
