# 2026-09-29 — brightness keys in the kernel, persistence, 12-hour clocks

## Brightness keys: blkeys (kernel input handler)
- Source: `~/t480-build/kernel/blkeys/blkeys.c` (out-of-tree build:
  `make -C /usr/src/linux-t480 M=$PWD LLVM=1 modules`). Currently loaded by hand
  with insmod (live test); NOT yet in linux-t480, so it is gone after a reboot.
- Binds to every input device with KEY_BRIGHTNESSUP/DOWN, drives the preferred
  backlight (raw > platform > firmware). Cubic perceptual curve, tap = 5 %
  (`tap_step` 500/10000) with a 120 ms fade; holds accelerate per EC repeat
  pulse (1,1,2,4,6,8 steps, each glided over `pulse_ms` 280). Params in
  /sys/module/blkeys/parameters (0644, live-tunable).
- Measured EC typematic (Video Bus event3): first repeat ~0.52 s, then every
  ~0.26 s, press+release pulses only (no release-state info).
- Writes go through `backlight_update_status()` directly: `backlight_device_set_brightness()`
  raises a SOURCE=sysfs uevent per frame on 7.2, and `backlight_force_update()`
  re-reads `_BQC`, which coreboot's ACPI rounds (wrote 2, read 1) -> feedback
  loop (tap 2 -> 15, flicker 2->1->2). blkeys emits its own SOURCE=hotkey
  uevent + sysfs_notify(actual_brightness) every 100 ms while gliding and at the end.
- Verified with a uinput virtual keyboard: tap 2 -> 3; measured-cadence hold 3 -> 100
  in ~1.5 s; 15 hotkey uevents, 0 sysfs uevents.
- Live test also set `video.brightness_switch_enabled=N` at runtime (the kernel
  stepped acpi_video0 itself 100 ms after each key).
- brightd: stopped; `run.sh` starts it only when /sys/module/blkeys is absent (fallback).
- Pending (owner decision, reboot): build blkeys into linux-t480, builtin cmdline
  `video.brightness_switch_enabled=0`, and try `acpi_backlight=native`
  (dmesg: "Skipping intel_backlight registration"; acpi_video0 = 101 levels,
  13-25 ms AML per write, dim range is levels 1-5).

## Persistence (udev, no service)
- `/etc/udev/rules.d/90-backlight-state.rules` -> `/usr/local/sbin/backlight-state`:
  `add` restores `/var/lib/backlight/level` ("dev brightness max"; other device =
  same fraction) and removes a stale `locked`; `change` with SOURCE=hotkey saves
  unless `locked` exists, then SIGUSR1s the chadwm bar (instant redraw).
- `/var/lib/backlight` root:video 2775 (syswork applied it root:root; fixed by hand).
- Lock: `bright lock` saves + marks + 100 %, idempotent; `bright unlock` restores + unmarks.
  Sysfs writes (lock's 100 %) never emit SOURCE=hotkey, so they are never saved.
- dwm-lock fix: the "slock-ly already running" early exit now happens before the
  brightness trap (a second lock used to restore the dim level behind the lock).
- Backups: `~/.local/bin/{bright,dwm-lock-bright}.bak-pre-blkeys-20260929`.

## 12-hour clocks
- chadwm bar: `date '+%s %-I:%M %p'` (bar.sh).
- slock-ly: `strftime("%-I:%M %p")`, commit 332bf0a, installed via syswork (4755 kept).

## Screenshots
- `dwm-screenshot screen/gui/full` also copy the saved JPEG to the clipboard as PNG
  (xclip stdout detached so `$(dwm-screenshot ...)` callers do not block).

## Caffeine cup (HyDE idle-inhibitor analogue)
- Bar module `^k10^` (before the clock): outline cup 󰛊 dim = off, filled 󰅶 on an
  inverted pill = on. Left click -> `scripts/caffeine.sh toggle`.
- On = `elogind-inhibit --mode=block --what=sleep:idle:handle-lid-switch:handle-suspend-key:handle-hibernate-key sleep infinity`
  (pid in $XDG_RUNTIME_DIR/caffeine.pid); off = kill it. The lock dies with the
  process, so logout/reboot always return to normal. polkit: allow_active yes.
- Only automatic sleep path on this machine is elogind HandleLidSwitch=suspend-then-hibernate
  (no idle timer, DPMS/screensaver off; lid-delayed-suspend.sh/rtc-hibernate are not wired).
  Handle-* inhibitors are always honoured (LidSwitchIgnoreInhibited only affects
  the high-level "sleep" lock). While on, manual suspend (power menu, lock-screen
  moon, Fn+4) is refused too.

## 2026-09-29 later — lost after reboot into 7.2.8-2 (KVMGT kernel)
- As expected, blkeys was gone after the reboot, so brightd (1 % steps, no hotkey
  uevent -> no bar refresh) came back, together with video.brightness_switch_enabled=Y.
- Rebuilt `blkeys.ko` for 7.2.8-2-t480 (the old .ko had vermagic 7.2.8-1), set
  brightness_switch_enabled=N, insmod, and killed brightd. Uinput tap test: 23 -> 28 -> 23,
  4 SOURCE=hotkey uevents, 0 sysfs. Still live-only: it has to be redone after every reboot until blkeys is built into linux-t480.
- Tap vs hold fix: taps < 700 ms apart counted as a hold and accelerated. Measured on
  event3: EC hold = first repeat 517 ms, then 259 ms (±1 ms); human taps 0.2-0.97 s, irregular.
  `streak_ms` replaced by `delay_ms`=517 / `repeat_ms`=259 / `tol_ms`=20: only presses on
  that cadence count as a hold. accel now {1,2,4,6,8} (was {1,1,2,4,6,8}); full range
  in ~1.3 s held. Backup: blkeys.c.bak-20260929. Replay test: each tap = 1 step; hold 100->1 in 1.3 s.
