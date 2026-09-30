# 2026-09-30 — key repeat reset by udev re-trigger

- Symptom: key repeat fell back to X defaults (660 ms / 25 Hz) mid-session.
- Cause: `pacman -Su` upgraded libudev → `35-udev-reload.hook` re-triggered all input
  devices at 08:50:05; Xorg removed/re-added every keyboard (Xorg.0.log t≈1058) with
  default repeat. `xset r rate` in chadwm run.sh only runs once at login. Suspend/resume
  or a new keyboard can do the same.
- Fix (syswork kbd-repeat, committed in /etc): /etc/X11/xorg.conf.d/00-keyboard.conf
  "system-keyboard" InputClass gains `Option "AutoRepeat" "200 20"` (delay ms, interval ms
  = 50 Hz), matching run.sh `xset r rate 200 50`. libinput_drv reads it per device, so
  re-added keyboards keep it. Effective at next X start (InputClass is read at startup).
- Loose end: ~/.xinitrc:17 still says `xset r rate 250 50` (overridden by run.sh).
