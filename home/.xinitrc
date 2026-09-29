#!/bin/sh
# Session: chadwm (siduck/chadwm), switched 2026-09-20.
# Pre-chadwm (dwm-titus + quickshell) variant kept at
# ~/.xinitrc.bak-pre-chadwm-20260920; restore that file to go back.

# Boot lock: cover screen during session bring-up. Inherits DISPLAY/XAUTHORITY from startx.
export SLOCK_USER=btw
/usr/local/bin/slock-ly &
# dwm-titus boot env (theme/toolkit vars) — kept, toolkit apps still use it.
export NO_AT_BRIDGE=1
export GTK_A11Y=none
export QT_QPA_PLATFORMTHEME=gtk3
export XCURSOR_THEME=Capitaine-Cursors-White
export XCURSOR_SIZE=32
[ -f /home/btw/.config/dwm-titus/theme-env.sh ] && . /home/btw/.config/dwm-titus/theme-env.sh
# (static feh wallpaper removed: the live earth is the background)
xset r rate 250 50 2>/dev/null &
# Brightness memory: restore last saved percent (bright saves on every up/down).
PATH="$HOME/.local/bin:$PATH" bright restore >/dev/null 2>&1 &
# Session bus first so pulseaudio inherits DBUS_SESSION_BUS_ADDRESS (fixes zombie audio).
if [ -z "$DBUS_SESSION_BUS_ADDRESS" ]; then
  eval "$(dbus-launch --sh-syntax --exit-with-session)" >/dev/null 2>&1
  export DBUS_SESSION_BUS_ADDRESS
fi
pgrep -x pulseaudio >/dev/null || pulseaudio --start 2>/dev/null &
# Secret service (Bitwarden secure storage needs org.freedesktop.secrets).
# gnome-keyring-daemon --start is idempotent: exits if already running.
if command -v gnome-keyring-daemon >/dev/null 2>&1; then
  eval "$(gnome-keyring-daemon --start --components=secrets,ssh 2>/dev/null)"
  export SSH_AUTH_SOCK
fi
# User s6 scandir: supervises user services (battwatch-daemon battery logger).
# Guarded so repeated startx sessions never stack supervisors.
SCAN="$HOME/.local/share/s6/scan"
if [ -d "$SCAN/battwatch-daemon" ] || [ -L "$SCAN/battwatch-daemon" ]; then
	if ! pgrep -f "s6-svscan.*\.local/share/s6/scan" >/dev/null 2>&1; then
		( setsid s6-svscan "$SCAN" >/dev/null 2>&1 < /dev/null & )
	fi
fi
unset SCAN
# chadwm session: xrdb + cursor.Xresources, wallpaper, screen-power policy,
# xsettingsd/polkit/lock-watch, picom, bar.sh (xsetroot status2d feeder), WM loop.
exec /home/btw/.config/chadwm/scripts/run.sh
