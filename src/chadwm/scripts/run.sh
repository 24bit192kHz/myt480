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
# Bitwarden desktop: waits for the boot lock to go away, starts in the tray
# and unlocks with it (bw-screen boot; bitwarden-slock, 2026-10-01).
[ -x /usr/local/bin/bw-screen ] && /usr/local/bin/bw-screen boot >/dev/null 2>&1 &

# Fn brightness keys are handled in the kernel (blkeys); the level is restored
# at boot and saved on every key change by udev (/usr/local/sbin/backlight-state).
# brightd is only a fallback for a kernel without blkeys.
[ -d /sys/module/blkeys ] ||
	pgrep -x brightd >/dev/null 2>&1 || setsid ~/.local/bin/brightd >/dev/null 2>&1 &
# External monitor takes over as the only display (preferred mode, audio
# follows, per-display Hz in ~/.config/chadwm/monitors.conf); re-applied
# on every connector change (monitor.sh watch is a singleton).
~/.config/chadwm/scripts/monitor.sh watch >/dev/null 2>&1 &
# KDE Connect: the indicator (tray icon) starts kdeconnectd over D-Bus; chadwm
# does not run /etc/xdg/autostart. Firewall: TCP/UDP 1714-1764 from the home LAN.
pgrep -x kdeconnect-indi >/dev/null 2>&1 || kdeconnect-indicator >/dev/null 2>&1 &
# Bluetooth: reconnect the buds/gamepad on login, resume and power-on (event driven).
pgrep -f 'bin/bt-autoconnect$' >/dev/null 2>&1 || setsid ~/.local/bin/bt-autoconnect >/dev/null 2>&1 &
# Mic: DeepFilterNet noise suppression, only running while something records (event driven).
pgrep -f 'bin/dfn-mic$' >/dev/null 2>&1 || setsid ~/.local/bin/dfn-mic >"${XDG_RUNTIME_DIR:-/tmp}/dfn-mic.log" 2>&1 &
# Lock-time power save: panel off 2 min into a slock session, Wi-Fi power save while locked on battery (lock-powersave).
pgrep -f "bin/lock[-]powersave$" >/dev/null 2>&1 || setsid ~/.local/bin/lock-powersave >/dev/null 2>&1 &
# Neutral root prevents the static image from flashing beneath the live Earth.
xsetroot -solid black >/dev/null 2>&1
picom &
# live ASCII planet background: the supervisor is the only renderer owner.
# A random planet each session (never the last one; Super+Shift+W picks one).
# Started in the background so the WM never waits for it.
if [ -x "$HOME/.local/bin/gxwc-wallpaper" ]; then
    { "$HOME/.local/bin/gxwc-wallpaper" planet random || echo "wallpaper: gxwc supervisor failed to start" >&2; } &
else
    echo "wallpaper: missing $HOME/.local/bin/gxwc-wallpaper" >&2
fi
xset r rate 200 50 &
# clipboard history for Super+V (singleton via flock)
rofi-clip.sh daemon >/dev/null 2>&1 &

dash ~/.config/chadwm/scripts/bar.sh &
while type chadwm >/dev/null; do chadwm && continue || break; done
