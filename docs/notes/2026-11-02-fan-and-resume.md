# 2026-11-02: quiet fan through hibernation, smaller images, Bluetooth in suspend-then-hibernate

## Fan at full speed while waking from S4

Sampling `/proc/acpi/ibm/fan` every 0.2 s across a hibernation showed the fan
`disengaged` (EC 0x2f bit 6: full speed, unregulated) right after the thaw at
38 °C, then `0`, then `auto`. Two causes, both in firmware:

- The EC powers up with the fan disengaged and keeps it so until the host picks a
  mode. The vendor firmware sets it early; coreboot never did, so every boot and
  every restore boot ran the fan flat out until Linux took over. Patch 0032 writes
  0x80 (automatic, thinkpad_acpi's `level auto`) in the mec1653 bootblock.
- Linux's ACPI fan driver (`CONFIG_ACPI_FAN=y`) has `.freeze = acpi_fan_suspend`,
  which puts the fan in D0 on every hibernation freeze, also in the restore
  kernel just before it jumps to the image. coreboot's H8 `FPWR._ON` calls
  `FANE(1)`: disengaged. `FANE` writes the two bits one at a time, hence the
  passing `0` (fan stopped). Patch 0033 adds `H8_FAN_PWR_NO_EC` and sets it on
  sklkbl_thinkpad: the power resource keeps only its bookkeeping. `_CRT` and
  `_PSV` stay; the EC's automatic mode and thermald-t480 cool.

thermald-t480 renewed the fan level every 30 s on `CLOCK_MONOTONIC`, which stops
during sleep, so after a resume it believed its level was still set for up to
30 s. It now uses `CLOCK_BOOTTIME`: the level is written again at the first poll
(0.4 s after the thaw in the test).

C65 = C64 + 0032 + 0033. After it the fan log shows no `disengaged` at all; the
EC sits in automatic mode until thermald sets its level.

## Image size

`/sys/power/image_size` defaulted to 2/5 of RAM (13 GB), so the image kept the
whole page cache. Resume times (RTC alarm to thaw, LZ4, 8 threads):

| Image | Resume |
|---|---|
| 1.8-2.6 GB | 7.3-9.4 s |
| 3.2 GB | 12.3 s |
| 11 GB (8.8 GB cache) | 21.5 s |
| same 8.8 GB cache, image_size 2 GiB: 2.1 GB | 7.4 s |

`01-hibernate-image-size` sets 2 GiB before every sleep (the value does not
survive a boot). Price: the kernel drops cache before the snapshot (2.8 s
instead of 0.5 s with 8.8 GB cached) and the cache is cold after the resume.

## Bluetooth in suspend-then-hibernate

After the S3 wake the 8265 downloads its firmware again (~1.6 s). The hibernate
phase came at once and unloaded btusb in the middle of the download: the
download failed, the driver reset the chip, and the unload waited for a 10 s HCI
timeout (-110). `04-btusb-hibernate` now unloads btusb already in the suspend
phase of suspend-then-hibernate and loads it 2 s after the S3 wake unless the
hibernate phase claims it; test: S3 wake to hibernate in 1 s instead of 11 s,
Bluetooth back after the S4 resume. Plain suspend still leaves the adapter alone.

## Cron jobs at home

`stealth-vpn apply` and `phone-hotspot check` (cron, every 30 s each; private,
not in this repo) tore everything down again on every tick at home: 100
programs a minute. Both now look first (state file, /sys/class/net, /proc) and
skip the teardown when nothing is up: 25 + 9 a minute. What remains is cron's
own PAM sessions (each asks the kernel for the audit netlink family this kernel
lacks: `modprobe net-pf-16-proto-9`) and the bar.
