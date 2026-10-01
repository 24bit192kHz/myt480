# T480 Thermal / Fan / Undervolt Stack

> **2026-09-28:** partly superseded, see [2026-09-28-overhaul.md](2026-09-28-overhaul.md).
> **2026-10-01:** thinkfan and throttled were removed from the system (packages uninstalled; s6 service dirs with their `down` files, `local.d/throttled.start`, `/etc/thinkfan.yaml` and `/etc/throttled.conf` deleted). thermald-t480 is the only thermal daemon. The thinkfan and throttled sections below stay as a record of the values they had.

Recon date: 2026-09-19. Host: Artix Linux, ThinkPad T480, i7-8650U (4C/8T, TjMax 100C).
Init: s6 (s6-rc). All service defs under `/etc/s6/sv/<name>/` (`type` + `run`).
Read-only recon — nothing modified.

## Overview — three daemons, one owner

| Daemon | Binary | Config | s6 state (2026-09-19) | Role |
|---|---|---|---|---|
| thermald-t480 (custom C, "thermald") | `/usr/local/bin/thermald-t480` | `/etc/thermald.conf` (582 B) | **up**, pid 413, ~9.4 days | SOLE ACTIVE owner: fan + MSR undervolt + RAPL PL1/PL2 + TCC trip. Runs `thermald-t480 -c /etc/thermald.conf` |
| thinkfan (stock) | `/usr/bin/thinkfan` | `/etc/thinkfan.yaml` | **down** (`down` flag file present, "not started yet") | Installed but parked. Would run `thinkfan -n -c /etc/thinkfan.yaml` (foreground, pidfile removed first) |
| throttled (python, erpalma/lenovo-throttling-fix fork) | `/usr/bin/throttled` | `/etc/throttled.conf` | **down** (`down` flag file), no process | Legacy/reference config source. Values were copied verbatim into thermald.conf (header says so) |

Why one owner: fan (`/proc/acpi/ibm/fan`) and MSR 0x150/0x1A2 are single-writer
resources. thermald-t480 owns both; thinkfan + throttled are kept down to avoid
fighting over fan level and voltage plane. No `dependencies` files exist for any
of the three services — no s6 ordering constraints. All three `type` files say
`longrun`.

`ps` shows only `s6-supervise` for thinkfan/throttled (no daemon), plus
`thermald-t480 -c /etc/thermald.conf` (pid 413). `s6-svstat` per service
(requires root; read via `ssh root@localhost`) confirms: thermald up, thinkfan
and throttled down.

## thinkfan — curve and levels (PARKED, reference)

`/etc/thinkfan.yaml`:
```yaml
sensors:
  - hwmon: /sys/class/hwmon
    name: coretemp
    indices: [1]            # Package id 0 temp
fans:
  - tpacpi: /proc/acpi/ibm/fan
levels:
  - [2, 0, 44]              # level 2 from 0–44C
  - [4, 42, 54]             # level 4, hysteresis 42–54
  - [5, 52, 64]
  - [7, 62, 32767]          # max at 62C+
```

- 4-step curve, max fan at 62C — conservative/hot-running vs thermald's curve.
- Only watches coretemp index 1 (package). No NVMe/PCH inputs.
- Run script: `rm -f /var/run/thinkfan.pid; exec thinkfan -n -c /etc/thinkfan.yaml`
- Currently superseded: thermald-t480's `fan_levels` use finer steps and it also
  disarms the EC fan watchdog (see `--test` output: "fan watchdog disarmed").

## thermald-t480 — role + config (THE ACTIVE DAEMON)

Custom small C binary (`/usr/local/bin/thermald-t480`; a `/usr/local/bin/thermald`
wrapper/symlink-adjacent ELF also exists). CLI: `thermald [-c conf] [--once] [--test]`.
Reads ONLY `/etc/thermald.conf` (default path baked in; `-c` overrides). If config
missing it falls back to SAFE defaults (UV 0, PL1 15W) — per strings.

`/etc/thermald.conf` (full, 29 lines — unified fan + RAPL + UV + IccMax):
```
temp_path auto
ac_glob /sys/class/power_supply/AC*/online
poll_s 3
ac_pl1_w 44 / ac_pl1_s 28 / ac_pl2_w 70 / ac_pl2_s 0.002 / ac_trip 95 / ac_interval 5
batt_pl1_w 29 / batt_pl1_s 28 / batt_pl2_w 44 / batt_pl2_s 0.002 / batt_trip 85 / batt_interval 30
uv_core -130 / uv_cache -130 / uv_gpu -120 / uv_uncore -120 / uv_analogio 0
icc_core_ac 64 / icc_gpu_ac 31 / icc_cache_ac 6
icc_core_batt 40 / icc_gpu_batt 24 / icc_cache_batt 6
fan_levels 2:0:38,4:36:48,6:46:56,7:54:32767
```

Behavior notes (from `--test` dry-run + strings, no MSR touched):
- Auto power-source detection via `ac_glob`; re-polls every `poll_s` 3s, re-applies
  on drift ("drift detected, re-applying").
- Temp probe: `temp_path auto` resolves coretemp hwmon (`hwmon8/temp1_input` on
  this boot — index moves, hence `auto`). If coretemp absent at start it HOLDS the
  fan instead of dropping it ("holding fan until probe order lands").
- Fan: parses `fan_levels` as `level:lo:hi` triples; writes `level N` to
  `/proc/acpi/ibm/fan`; disarms EC fan watchdog (`watchdog 0`) so EC doesn't
  override. Bad entries skipped, keeps previous curve if all invalid.
- RAPL: programs PL1/PL2 + trip via MSR mailbox ("in-sync, mailbox only").
  Sample line: `AC: PL1 44W/28s PL2 70W/0.002s trip 95C reg=0x42823000dd8160`.
- UV: writes MSR **0x150** per plane (CORE/GPU/CACHE/UNCORE/ANALOGIO), warns if
  outside sane range but applies anyway.
- IccMax: writes MSR 0x150 mailbox too (`0x80000017...`), in Amps.
- Needs `/dev/cpu/*/msr` (msr module). Unprivileged `--test` prints
  "no /dev/cpu/*/msr"; as root it dry-runs with canned values + DRY msr lines.
- Config hot-reload: logs "conf reloaded" (throttled.conf has `Autoreload: True`;
  thermald-t480 re-reads on drift/poll).

thermald fan curve vs thinkfan: `2:0:38, 4:36:48, 6:46:56, 7:54:32767` — reaches
level 6/7 earlier (46/54C vs 52/62C) and tops at 54C instead of 62C. Cooler and
more aggressive than the parked thinkfan curve.

## throttled — undervolt + power limits (REFERENCE, down)

`/etc/throttled.conf` — classic lenovo-throttling-fix INI. Values copied into
thermald.conf per its header comment.

### AC vs BATTERY power limits

| Knob | AC | BATTERY |
|---|---|---|
| Update_Rate_s | 5 | 30 |
| PL1_Tdp_W / PL1_Duration_s | 35 W / 28 s | 29 W / 28 s |
| PL2_Tdp_W / PL2_Duration_S | 60 W / 0.002 s | 44 W / 0.002 s |
| Trip_Temp_C | 90 | 85 |
| cTDP | 0 | 0 |
| Disable_BDPROCHOT | False | False |

Note DIVERGENCE: thermald.conf runs HOTTER/FASTER than throttled.conf —
AC PL1 44 vs 35 W, PL2 70 vs 60 W, trip 95 vs 90 C. Battery matches trip 85C but
thermald PL2 44 W = throttled PL2 44 W while PL1 matches 29 W. So the live daemon
is more permissive on AC than the throttled reference file.

### Undervolt (both AC and BATTERY sections identical in throttled.conf)

| Plane | throttled.conf | thermald.conf (live) |
|---|---|---|
| CORE | -100 mV | **-130 mV** |
| CACHE | -100 mV | **-130 mV** |
| GPU | -50 mV | **-120 mV** |
| UNCORE | -50 mV | **-120 mV** |
| ANALOGIO | 0 | 0 |

Live UV is notably more aggressive (-130/-120 vs -100/-50). Requires MSR-unlocked
firmware (see below); on locked firmware MSR 0x150 writes silently no-op and the
"undervolt" does nothing.

### IccMax (Intel spec values for i7-8650U, comment in file)

| Plane | AC | BATTERY |
|---|---|---|
| CORE | 64 A | 40 A |
| GPU | 31 A | 24 A |
| CACHE | 6 A | 6 A |

Identical in thermald.conf (`icc_*_ac` / `icc_*_batt`). Guards current overshoot
under burst load.

Other throttled.conf bits: `[GENERAL] Enabled: True`, sysfs AC path with glob,
`Autoreload: True`.

## thinkpad_acpi fan_control

- `/etc/modprobe.d/thinkpad_acpi.conf`: `options thinkpad_acpi fan_control=1`
- Live: `/sys/module/thinkpad_acpi/parameters/fan_control` = `Y`.
- Effect: enables write access to `/proc/acpi/ibm/fan` (`level`, `enable`,
  `disable`, `watchdog`). Without it, both thinkfan and thermald-t480 fan writes
  fail read-only. Loaded module confirmed (`lsmod`: thinkpad_acpi, 200704).
- `/proc/acpi/ibm/thermal` does NOT exist on this kernel (fan + `sensors`
  thinkpad-isa-0000 cover readings instead).

## MSR / firmware unlock dependency + TCC blacklist

- `/etc/modprobe.d/32-disable-tcc.conf`:
  ```
  # tcc owned by thermald-t480 via MSR 0x1A2 trip 90; driver probe ~170ms wasted
  blacklist intel_tcc_cooling
  install intel_tcc_cooling /bin/false
  ```
  The in-kernel `intel_tcc_cooling` driver would also program MSR **0x1A2**
  (TCC activation offset / thermal control). Blacklisted + install-blocked so
  thermald-t480 is the single writer of the TCC trip point. Bonus: skips ~170 ms
  wasted driver probe at boot.
- UV path: throttled/thermald-t480 write undervolt via MSR **0x150**
  (OC mailbox: planes CORE/GPU/CACHE/UNCORE/ANALOGIO + IccMax). RAPL limits go
  through the wraparound mailbox path ("in-sync, mailbox only" in test output).
- Firmware context: `~/bin/t480_vfsp_16mb/` holds coreboot/grub ROM images
  (`grub_t480_vfsp_16mb_libgfxinit_corebootfb_<layout>.rom`, 16 MB each) and
  `~/bin/serprog_pico/` holds serprog flasher firmware — i.e. this machine was
  externally flashed (ch341a/Pico serprog class hardware) with a deguarded build
  that leaves the OC mailbox / CFG Lock open so MSR 0x150 writes stick.
  Stock Lenovo firmware (post-plundervolt microcode) locks 0x150; without the
  unlock the whole UV column above is a no-op.
- MSR node present: `/dev/cpu/0/msr` exists (msr module loaded or built in).
  `rdmsr` binary NOT installed (ssh-as-root check: `rdmsr: command not found`),
  so MSR 0x1AA (PKG power clamp / unlock-status peek) could not be read. The
  `--test` dry-run as root shows the daemon resolves temp + parses conf cleanly.

## Live readings snapshot (2026-09-19 ~16:52 UTC, AC online = 1)

- coretemp: Package 44C, Cores 40/44/38/41C (high=crit=100C)
- thinkpad-isa fan1: **3181 RPM, pwm 72%, MANUAL** → later root read: level **6**,
  3348 RPM. (thermald-t480 driving; EC watchdog disarmed.)
- `/proc/acpi/ibm/fan`: `status: enabled`, `level: 4→6` across reads (fan ramping
  with 41C package temp — matches `fan_levels 4:36:48`).
- pch_skylake: 35.5C; nvme composite 27.9C; iwlwifi 49.0C; acpitz 37.0C.
- BAT0 12.07 V / 0.00 A (idle), BAT1 12.29 V / 1.64 A (charging).
- `--test` (root) dry-run at 41C: `DRY fan <- level 4 (41C)` — curve working as
  configured. AC profile selected (PL1 44W/PL2 70W/trip 95C line printed).
- Kernel: `intel_rapl_msr`, `coretemp`, `thinkpad_acpi` loaded;
  `intel_tcc_cooling` absent (blacklist effective). No plundervolt/throttle dmesg
  hits (dmesg read was unprivileged and sparse; re-check as root if needed).

## Tuning knobs

Quieter (tolerate heat, less fan):
- `/etc/thermald.conf` `fan_levels`: raise breakpoints, e.g.
  `2:0:44,4:42:54,6:52:62,7:60:32767` (thinkfan.yaml shape). Then
  `ssh root@localhost 's6-svc -h /run/service/thermald'` (SIGHUP → "conf reloaded").
- Lower AC trip toward 85–90C so RAPL clamps before fan must scream.
- Battery already quiet-ish: `batt_interval 30`, PL1 29 W.

Cooler (more fan, more headroom):
- Current curve already tops at 54C; to push further, widen level 7 down:
  `7:50:32767`, or drop `ac_trip` 95 → 90 to match throttled.conf reference.
- Raise `batt_trip 85` only if battery thermals proven fine — 85C protects the
  rear battery from heat soak.

More performance (AC):
- PL1 44 / PL2 70 already exceed Intel 25W cTDP-up and throttled.conf's 35/60.
  Sustained >44 W on the T480 single-heatpipe cooler will sit at 95C trip —
  watch `sensors` package temp under load before raising.

More undervolt (silicon lottery; freeze = too far):
- Live -130/-130/-120/-120 is already aggressive vs the -100/-50 reference.
  If stable for weeks, could try CORE/CACHE -140 in 10 mV steps; if any freeze,
  back off 10–20 mV. GPU/UNCORE usually crash first — leave at -120 unless
  testing with a GPU load loop. ANALOGIO stays 0 (never UV this plane).
- Verify UV actually applies: needs `rdmsr` (`pacman -S msr-tools`) then
  `ssh root@localhost 'rdmsr 0x150'`-class check or compare package power at
  fixed load before/after. On relocked firmware (BIOS update!) 0x150 writes
  no-op — re-check after any flash.

Structural notes:
- Single-writer rule: keep thinkfan + throttled `down` while thermald-t480 runs.
  To switch control (e.g. test thinkfan curve): stop thermald first
  (`s6-svc -d`), remove thinkfan `down` file, `s6-svc -u` — never run both fans.
- `temp_path auto` is load-bearing: hwmon index moves across boots (hwmon8 today).
  Do not hardcode.
- throttled.conf is the human-editable reference; thermald.conf is the live file.
  Keep them in sync when changing values (or note deliberate divergence like the
  current AC PL1 44 vs 35).
