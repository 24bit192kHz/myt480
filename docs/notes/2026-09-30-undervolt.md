# 2026-09-30: CPU undervolt sweep, benchmarks, Bluetooth, hibernation

## Undervolt (i7-8650U, coreboot C50, on AC, PL1 64 W / PL2 90 W, thermal limit 95 C)

Method: `firmware/tools/undervolt/`. Each candidate ran about four minutes: all-core AVX with
verification and MX150 renders at the same time, load/idle bursts, a single-core turbo
burst, memory and cache pressure, then 20 s idle. Failure = verification error, machine
check, changed offset, or a freeze (TCO watchdog reset). The TCO watchdog and `panic=10`
made unattended runs safe.

| core = cache | result |
|---|---|
| 0, -80, -100, -110, -120 | pass (coarse) |
| -121, -122, -123 | pass (1 mV steps) |
| -124 | machine check logged during the all-core phase, no crash |

iGPU plane passed to -100 under an Intel render load (the cap of the sweep), uncore to
-100 (cap). Chosen with margins: AC core/cache -115, iGPU -80, uncore -70. A 300 s soak
with the MX150, 200 s with the Intel GPU and a suspend/resume passed. thermald-t480 now
takes `ac_uv_*` and `batt_uv_*`; battery uses -100/-100/-60/-50 until `sweep-batt.sh`
has run there (the old -130 froze on battery within two minutes). The old global
`uv_core -130` is what caused every freeze this week, on Lenovo's firmware too.

Benchmarks (`firmware/tools/bench-all.sh`), 0 mV versus the AC set:

| test | 0 mV | -115/-80/-70 |
|---|---|---|
| sysbench cpu, 8 threads, events/s | 6949 | 8022 (+15 %) |
| sysbench cpu, 1 thread | 1079 | 1245 (+15 %) |
| 7-Zip total MIPS | 30144 | 32851 (+9 %) |
| sustained all-core at the 95 C limit | 2.5-3.6 GHz, 27-39 W | 3.3-3.9 GHz, 29-40 W |
| render test, Intel / MX150 (fps) | 162 / 135 | 167 / 135 |
| idle package power | 5.0-5.4 W | 4.3-4.6 W |
| NVMe sequential read | 1.65 GB/s | 1.62 GB/s |

## Bluetooth

bluez-utils installed; `local.d/bluetooth.start` starts bluetoothd at boot (no s6 service
on this machine), `bluetooth/main.conf` has `AutoEnable=true`. chadwm bar module 11 shows
the adapter state and the connected device; `btmenu.sh` is the rofi picker (power, scan,
connect/disconnect/pair). Firmware C50 made this possible (radios were hard-blocked
before).

## Hibernation

Measured with `firmware/tools/hib-measure2.sh` (a wall-clock loop finds the freeze and thaw
instants; a short RTC alarm makes the wake immediate after power-off):

| kernel, image_size | command to freeze | freeze to thaw (short alarm) | power-on to thaw |
|---|---|---|---|
| 7.2.8-3 LZO, image_size 0 | 0.6 s | 16.3 s | 6.6 s |
| 7.2.8-3 LZO, 40 % of RAM | 0.6 s | 14.4 s | 6.6 s |
| 7.2.8-4 LZ4, 40 % of RAM | 0.6 s | 15.6 s (larger image after a fresh boot) | 5.1 s |

image_size 0 made the kernel shrink memory for 2-3 s before the snapshot; 40 % avoids
that. LZ4 (kernel 7.2.8-4, `HIBERNATION_DEF_COMP=lz4`) cuts the resume by 1.5 s. The
tune runs from `local.d/hibernate-tune.start` (the rc.local call did not run).

## 2026-10-01: battery sweep

`sweep-batt.sh -100` on battery (external battery draining first, charger out, PL1 15 W,
package around 62 C, each candidate 180 s of the same verified stress plus MX150 renders;
core = cache only, iGPU and uncore at 0 during the sweep):

| core = cache | result |
|---|---|
| -100 to -120 | pass, 1 mV steps |
| -121 | froze in the burst phase, TCO watchdog reset, no machine check logged |

So the battery limit is 3 mV tighter than on AC and fails as a freeze instead of a logged
machine check, which matches the old -130 freezing there within minutes. The installed
battery set stays at -100/-100/-60/-50 for now; -110 for core/cache would keep the same
margin as the AC set. thermald-t480 does not need to be stopped for a sweep: it rewrites
the offsets only on a power-source change or when the power-limit registers drift, so
after a sweep `touch /etc/thermald.conf` makes it re-apply its profile (the sweep leaves
the offsets at 0). Note that `sweep-batt.sh` checks only BAT0 for its 20 % stop while
BAT1 drains first.

Also fixed: the kit scripts referred to `uvtest3.sh`, which is `uvtest.sh` here.
