# MX150 (GP108, 10de:1d10) on coreboot — 2026-09-29

A log of the dGPU work, oldest first. The current state is in the main README (section
"MX150") and in `upstream-dgpu-series/README.md`. The first sections describe the first
working builds (C19 to C23).

Packages added: linux-t480-headers, dkms, nvidia-580xx-dkms, nvidia-580xx-utils,
egl-x11 (1.0.6, mirror no longer had 1.0.5), egl-gbm, egl-wayland, eglexternalplatform.

## Use
- `dgpu on|off|status` — CMOS 0x6f; takes effect at the next boot. Off = C18 behaviour.
- `prime-run <program>` — render on the MX150 (e.g. `prime-run glxinfo -B`).
- A boot that hangs with the GPU on: hold power 4 s. That clears the request and shows the GRUB menu.

## Notes
- NVIDIA 580xx is the last branch with Pascal; no runtime D3 on Pascal, so the GPU
  draws power while on. Keep it off on battery.
- NVreg_PreserveVideoMemoryAllocations=0 (/etc/modprobe.d/nvidia-sleep.conf): the
  package default needs systemd nvidia-suspend services, which s6 does not have.
- \TBTS in acpi/sleep.asl stays disabled (the file was never in the DSDT upstream).

## Going back
`sudo sh firmware/tools/mx150-revert.sh`, then reboot.

## Update 2026-09-29 evening: working on C32

The sections above describe C19-C23 and are history. Current state:

- Firmware: C32. coreboot powers
  the GPU in the bootblock; root port 1 is a normal port (no hotplug flag).
- `dgpu on|off` still selects it (CMOS 0x6f). Switching from off to on makes the next
  boot do one extra full reset; that is intended.
- Without a driver bound the GPU is powered off at runtime (ACPI power resource). With
  the NVIDIA driver loaded it stays on.
- Tested: cold boot, warm reboot, S3 resume, dgpu off, off->on, `prime-run glxinfo`.
- Upstream series: see `upstream-dgpu-series/`.
- A boot that hangs with the GPU on: hold power 4 s (clears the request).

## Update 2026-09-29 21:30: GPU on demand

- Leave `dgpu on` set. The GPU is OFF (power rail off) unless a program uses it.
- `prime-run <program>` turns it on (about 1.5-4.5 s), runs the program on the MX150 and
  turns it off when the last such program has ended.
- `gpu-power on|off|status` does the same by hand (for nvidia-smi, CUDA, ...).
- How: /usr/local/bin/gpu-power (setuid) loads/unloads the NVIDIA modules; without a
  driver the kernel puts the GPU into D3cold through the ACPI power resource.
  /etc/modprobe.d/nvidia-ondemand.conf keeps everything else from loading the driver,
  Option "AutoAddGPU" "false" in /etc/X11/xorg.conf.d/20-intel-fast.conf keeps X from
  holding the GPU. Source: `src/gpu-power/`.
- To go back to "driver always loaded": remove nvidia-ondemand.conf and the AutoAddGPU line.

## Update 2026-09-30: AC/battery policy, and a freeze

- `gpu-power` now has a policy: on AC the driver stays loaded (GPU ready at once), on
  battery the GPU is off unless a program uses it. `gpu-power on|off` by hand wins until
  `gpu-power auto` or the next boot; `gpu-power status` says which mode is active.
  A udev rule on the AC adapter (`/etc/udev/rules.d/90-gpu-power.rules`, hook
  `gpu-power-hook`) applies it on plug/unplug and at boot; `prime-run` uses
  `gpu-power use` / `release` and a shared lock in `/run/gpu-power.users`, so a
  running program is never switched off. Source: `src/gpu-power/`.
- After a GPU benchmark on battery the laptop hung with nothing in the logs. The root
  port had PCIe L1.1/L1.2 substates enabled by FSP (Linux is not granted ASPM control
  by coreboot's `_OSC`, so it cannot change that), and the hang came right after the
  GPU went idle. Firmware C46 (coreboot patch 0018) turns the substates off on that
  port; ASPM L1 stays. Battery impact: none while the GPU is off (its link is powered
  down), well under 0.1 W while it is on.
- Battery: GPU off (rail off) 6.7 W idle, driver loaded and idle 7.7 W, rendering 9.1 W.

## Update 2026-09-30 (evening): the freezes, and a comparison with the vendor firmware

- The silent freezes under GPU load were the CPU undervolt (-130 mV core and cache in
  `thermald.conf`), not the firmware or the GPU: the Lenovo firmware froze the same way,
  and with the undervolt at 0 the 10-minute GPU test passes on both. The undervolt was set
  again later, per power source, after a stress-test sweep (`tools/undervolt/`,
  `docs/notes/2026-09-30-undervolt.md`).
- Vendor firmware versus coreboot, for the MX150 (dumps compared with inteltool, lspci,
  the EC RAM and the decompiled ACPI tables):
  - ASPM: Lenovo runs the GPU port with ASPM off and tells Linux in the FADT that ASPM is
    unsupported. Patches 0018 and 0019 give the same on that port (C47).
  - PCIe payload: Lenovo allows 256-byte TLPs on the GPU, Wi-Fi and SSD ports; patch 0020
    does the same, the GPU and the NVMe drive negotiate 256 bytes (C48).
  - Power sequencing: the vendor's power resource does power, 8 ms, reset release, 16 ms,
    link enable, wait for L0; coreboot's `PGPU` follows the same order. Lenovo also rewrites
    the GPU's subsystem ID (config 0x40) at every power-on; coreboot leaves it, a boot-time
    attempt hung the board twice and nothing depends on it.
  - Vendor ACPI the Linux driver does not need: Optimus `_DSM` (NVOP), the power-sharing
    `_DSM` (GPS: AC/DC notifications, CPU throttling on the GPU's request), GC6 (JT) and the
    MXM table. The driver takes AC/DC from the AC adapter device and reports "Runtime D3
    status: Disabled by default" on this Pascal GPU on both firmwares.
  - EC: the EC reads the GPU temperature itself (EC RAM 0x79) on both firmwares. The
    "SW power cap" seen on AC under Lenovo is the GPU's own power limit at full utilisation,
    not a firmware policy.
  - Wake: an RTC alarm wakes the board from S5 under coreboot (`PM1_STS: WAK RTC`,
    `prev_sleep_state 5` in the console); `tools/coldboot.sh` uses that for cold-boot
    tests. A cold boot with the GPU enabled needs no extra reset; only a warm reset after a
    boot with the port disabled does (patch 0017).
- `gpu-power`: `status`, `on` and `apply` check the PCI vendor (01:00.0 is the Wi-Fi card
  when the port is off), and `off` sets the GPU's runtime PM control to `auto`. PCI devices
  start with it `on`, and then the root port never suspends, so after a fresh boot the
  ACPI power resource never cut the rail after `gpu-power off`.
- Firmware C49 (patch 0021): the SMBIOS BIOS version is now `CBET4000 <version>`. Before,
  thinkpad_acpi stopped at "ThinkPad BIOS t480, EC unknown" and never enabled its thermal
  sensors; now it reports the EC (`EC N24HT37W-3.36`) and the `thinkpad` hwmon has
  temp1 CPU and temp2 GPU (the EC's own reading of the MX150, -128 while the GPU is off),
  and `/proc/acpi/ibm/thermal` exists.
- thermald-t480 (`src/thermald-t480`): the fan curve now follows the hotter of the CPU
  package and that GPU sensor (`gpu_temp_path auto|off|<file>`, default auto), so a GPU
  load raises the fan even while the CPU stays cool.

## 2026-09-30, later: what the vendor firmware still had over coreboot, fixed

A comparison of kernel logs, ACPI device lists and wake sources found these gaps, all
closed by firmware C50 (patches 0022-0024) and kernel 7.2.8-3:

- Bluetooth was hard-blocked on every boot: the generic ThinkPad ASL reported the master
  wireless switch from an EC bit the T480 does not have. WLSW now answers "on", as
  Lenovo's firmware does; Wi-Fi no longer needs the unblock rule either.
- Wake sources: Wake-on-LAN (GbE device with `_PRW`), PME wake on the Thunderbolt root
  port, lid wake from S4, like the vendor firmware.
- thinkpad_acpi no longer sees a tablet mode switch.
- Subsystem IDs 17aa:225d on all PCH devices, the root ports and the system agent devices
  (FSP programs those read/write-once registers before coreboot's drivers run, so it gets
  the board's IDs); GMM at 00:08.0 enabled.
- Kernel: TPM 2.0 (`tpm_tis` on coreboot's MSFT0101 node) and the PCH TCO watchdog. The
  watchdog is armed by `local.d/watchdog.start`; killing the loop cleanly disarms it.
  Lesson from setting it up: a keepalive started from an ssh shell that is cut by a
  suspend dies without the magic close, and the machine resets 30 s after resume.

Not implemented: the USB-C PD controller node (UCSI over the EC), DPTF and WMI; nothing
on this Linux system uses them. The GPU's own subsystem ID stays unset.

Verified afterwards on battery (2026-09-30 18:20-18:26): charge thresholds and cycle counts
work under coreboot (BAT0 85/90 as on stock; BAT1 had lost its thresholds when reinserted,
set to 85/90 again and kept across a reset); the GPU policy parks the MX150 in D3cold about
a minute after boot; idle draw 6.8 W with the GPU off, package C8 34 % of the time. A
deliberate kernel hang (`kernel.panic=0`, sysrq crash) was reset by the TCO watchdog 30 s
later and the next boot re-armed it and brought the GPU back through the one-time reset
path. Hibernate to the swap partition and resume by RTC alarm from S4 work with the GPU
and the watchdog; the X session survives. The Wi-Fi rfkill unblock rule and script were
removed: its log never saw a soft block, and Wi-Fi comes up unblocked on C50.

## 2026-10-01: upstream review, series v4, firmware C51

- v3 (12 patches) got Code-Review -2 on every change from a core developer: "Invalid
  sign-off". coreboot accepts an established alias since 2023 (a "known identity"); the
  alias here is the GitHub account of 2022 whose profile carries the sign-off address.
- Jenkins had found two real bugs: patch 1 broke the arm64 build (a helper outside the
  `HAVE_ACPI_TABLES` block it serves), and the last patch set `SUBSYSTEM_*_ID` in the
  board Kconfig, which lint-stable-024 forbids. A review of the whole series found a third:
  the T480's "no tablet switch" option sat in the common Kconfig and would have removed
  the X380 Yoga's tablet switch.
- sconfig bug: a `subsystemid` in a base devicetree does not survive an override tree
  that names the same device (unset IDs are -1, and the merge copies "non-zero" IDs).
  Every sklkbl variant re-declares the domain, so the IDs in `devicetree.cb` never
  reached a device. The T480 override tree now carries them (patch 0025, series 0017).
- v4: 17 patches (the h8 and T480 patches split into one change each), every patch built for the six variants, every board in the tree at
  the top (966 configurations; 6 need Go and were not built), lint hooks from
  `make gitconfig`. Pushed with Matt DeVillier and Johann C. Rode as reviewers.
- C51 = C50 with the subsystem IDs from the devicetree: cold boot from S5, S3 resume
  with the watchdog armed, 17aa:225d on all PCH and system agent devices, verify48
  identical to C50 (MPS, ASPM, driver, GL/Vulkan, 5 D3cold cycles), DSDT and GRUB
  payload byte-identical to C50.

## 2026-10-26: a T580 on the series, a truncated VBIOS, and the hot-plug port in D3cold

A T580 owner (hustlerone) tried the Gerrit series (patch set 3) on
[CB:95876](https://review.coreboot.org/c/coreboot/+/95876). What his reports taught:

- `dGPU: disabled` with the option's default set to true: there is no option backend on
  sklkbl without the EDK2 setup menu and SMMSTORE, so `get_uint_option()` returns its
  fallback (series README, "Using the series").
- `RmInitAdapter failed! (0x24:0x40:1603)` with a 119296-byte `pci10de,1d10.rom`: the
  VBIOS was cut. The complete image sits in the Lenovo firmware, FFS file
  `17835F4C-5D9B-484D-ABBC-5FBDC11F891C` on the T580 (`8B9BEF90-5FAF-46CF-A32A-7E1B7D33CC87`
  on the T480), 182272 bytes; with it the GPU initialises. Patch 0001 (v6) now says so in the
  log instead of failing silently: `option ROM needs 182272 bytes, but its CBFS file has only
  119296`. `stock-reference/mx150-vbios-10de-1d10.rom` is the same kind of cut dump.
- His `lspci` showed the GPU at `2.5GT/s (downgraded)`, ASPM L0s L1 on, MPS 128. Checked on the
  T480 (C55, driver loaded): after about 20 s idle the GPU is in P8 at 139 MHz and the link
  reads exactly that; it is back at 8 GT/s within a second of load (P0, 1949 MHz, `SW Power
  Cap: Active`, which is normal; `glxfer`: 2.7 GB/s up, 3.1 GB/s down). So the line is normal
  for an idle GPU. His crashes on Wayland and the low game FPS are not explained yet; the
  T480 runs X11.
- Hot-plug root port in D3cold (the open problem of v5, CB:95873): Linux puts a hot-plug
  port into D3 only if it can signal wake and hot-plug events from there
  (`acpi_pci_bridge_d3()`): `_PRW` with a wake GPE, `_S0W` not shallower than D3, and the
  `_DSD` property `HotPlugSupportInD3`. Firmware **C58** = C55 + the hot-plug experiment
  (GPU powered right before FSP-S, `PcieRpHotPlug[0]`) + those three objects on RP01 in the
  SSDT (`_PRW {0x69, 4}` is what the vendor DSDT has on this port). Result on the T480, on AC:
  after `gpu-power off`, the root port is `runtime_status suspended`, `power_state D3cold`,
  ACPI `real_power_state D3cold` (the power resource ran `_OFF`: rail off) within 1 s, and the
  GPU with it; unchanged after 60 s idle; 5 driver on/off cycles OK; no pciehp, Xid or AER
  messages. Patch 0003 (v6) carries the same change. C58 is an experiment image (branch
  `c58-hotplug-d3`, workstation + T480 trees, `roms/C58-hotplug-d3.rom`); the laptop went
  back to C55 afterwards, whose bootblock design reaches D3cold without a hot-plug port.

### 2026-10-26, later: the hot-plug design validated, the setup option checked, Wayland tried

All on firmware C58 (hot-plug port + the three ACPI objects), on AC, remotely:

- 10 min `eglbench` with pciehp bound to the port: 8 GT/s x4 throughout, P0 up to 93 C
  (AC clock offsets +200/+1250), no pciehp, Xid or AER line; 20 driver off/on cycles
  (port D3cold each time); hibernate + RTC resume (`prev_sleep_state 4`, image restored,
  driver loads afterwards); three cold boots from S5 (power good 4.7 ms, link down before
  FSP-S, port kept and link up after, D3cold within 30 s); boot with `dgpu off`:
  `dGPU: disabled`, no 01:00.0, the port stays as an empty hot-plug slot (`PresDet-`),
  the WLAN port keeps 00:1c.6, no power resource emitted; `dgpu on` + warm reboot brings
  the GPU back without the one-time full reset the bootblock design needs; 30 min S3 with
  the port in D3cold: woken by the RTC alarm after 1807 s, nothing earlier.
- Upstream v7: 0003 cuts the power again when `_ON` sees no power good; 0004 makes
  `dgpu_enable` a boolean. Checked without edk2 by reading the CFR records from the
  coreboot table (`tools/cfrdump.py`, firmware C59 = C58 + `DRIVERS_OPTION_CFR` + the
  0004 logic): the option is listed with the GPU enabled and disabled, never SUPPRESSed;
  main had it as an enum with no values (nothing to choose in the menu).
- Wayland (headless `sway -V`, gles2 renderer on the Intel node, own runtime dir):
  `eglinfo -p wayland` with the NVIDIA EGL works; `vkcube --wsi wayland` on the MX150
  fails with "Could not find both graphics and present queues" (the compositor offers no
  way to present NVIDIA buffers); `eglgears_wayland` segfaults in libwayland-client with
  the Mesa EGL as well, so that is the demo, not the driver; GLX through Xwayland with
  `__NV_PRIME_RENDER_OFFLOAD=1 __GLX_VENDOR_LIBRARY_NAME=nvidia` renders on the MX150
  (glxinfo, glxgears 5 s). Firmware-independent; the T580 owner's crashes are not
  reproduced by this.
- P-state note: with the driver loaded and nothing using the GPU, `nvidia-smi` reports
  P0 at 1480 MHz today (the query wakes the GPU from D3cold); the P8 / 2.5 GT/s idle link
  seen in the morning shows only while something keeps the GPU awake without using it.
- The laptop is back on C55. ROMs kept: C58-hotplug-d3, C59-cfr, C59b-cfr-bool,
  backup-before-C58.

### 2026-10-26, evening: C60, the hot-plug design is now the T480's firmware

The user chose the hot-plug design after the C58 validation. C60 = C55 + one clean local
patch (0029 in `coreboot/patches/`): root port 1 hot-pluggable, GPU powered in the ramstage
right before FSP-S, link wait after FSP-S, no bootblock GPU code, no one-time full reset,
`_PRW {0x69, 4}` + `_S0W 4` + `HotPlugSupportInD3` on RP01, power-good abort in `_ON`,
`dgpu_enable` as a boolean in the option list; the power-button override still skips the GPU
for one boot, now read from the power state saved in the romstage (the bootblock read the
live PM1_STS). Flashed 12:32 and checked: port and GPU in D3cold 1 s after `gpu-power off`,
5 driver cycles, the one-boot skip (CMOS 0x6c written by hand, same as the 4 s hold sets:
`dGPU: skipped for this boot`, byte cleared, GPU back on the next boot), a cold boot from S5,
an S3 cycle; TPM key resealed to the new PCR 2. C55.rom stays in `roms/` as the fallback;
the 4 s hold itself is still untested by hand (the skip path behind it is).

Battery idle, same conditions (2026-10-26 13:00, fresh boots, screen locked, backlight 24 %,
driver unloaded, GPU and port in D3cold): C55 8.50 / 8.46 W, C60 8.63 / 8.55 W; package
power 2.1 W on both, no package C8 on either. The two designs draw the same at idle. Both
numbers are about 1.6 W above the 10-04 figures (6.7 to 6.9 W) because PulseAudio now runs
the mic filter chain permanently (LADSPA `noise_suppressor_mono` sink, `rnnoise_mic`
remap source, DeepFilterNet null sink): 28 % of a core on every boot, which keeps the
package out of C8. That is an OS-side item, not firmware.

Follow-up 13:15: the constant PulseAudio load was the old RNNoise chain in `~/.config/pulse/default.pa`
(its loopback keeps the mic and the LADSPA sink running); DeepFilterNet (`dfn-mic`) was already
on demand. The RNNoise block is removed (backup kept, `mic-denoise-toggle.sh on` restores it);
PulseAudio idles at 0 % and DeepFilterNet engages only while something records from
`mic_denoised`. C60 idle on battery afterwards: 8.19 W, package 2.0 W, still no package C8,
so another OS-side wake source remains since 10-04.

Idle power on battery, resolved 13:50: the 2 W above the 10-04 figure was the live globe
wallpaper (`gxwc`, 15 fps), which keeps presenting under the lock screen because the
compositor never reports it obscured: 8.09 W with it, 6.09 W with it killed. Fix in
`gxwc-wallpaper` (`home/local-bin/`): the supervisor pauses the renderer with SIGSTOP while
`slock` runs and resumes it on unlock; measured 6.29 W locked on C60. Not applied, measured
only: USB autosuspend for the touchscreen and the fingerprint reader plus the card reader
unbound saves 0.6 W (needs the fingerprint daemons to release the device when idle); Wi-Fi
power save saves 0.4 W (off on purpose in the NM profile). Thunderbolt driver unloaded costs
1 W more (controller stays in D0), keep it loaded. Package C8 is never reached on this
machine in any of these states (no PSR, i915 DC5/DC6 counts stay 0); the gain comes from
package C6 and the iGPU idling.

Battery policy, 2026-10-26 14:15 (the goal: fast on AC, cheap on battery, never sluggish):
- `lock-powersave` (home/local-bin, started by chadwm run.sh): 120 s into a slock session the
  panel goes off (−1.7 W); on battery Wi-Fi power save is on only while locked (−0.4 W) and
  off again at unlock, so latency never suffers while someone types. Sudo rule
  `system/etc/sudoers.d/lock-powersave` for the two `iw` commands; TLP leaves Wi-Fi power
  save alone now (`WIFI_PWR_ON_BAT=off`), the NM profile keeps its `disable`.
- TLP `CPU_ENERGY_PERF_POLICY_ON_BAT=balance_performance` (was balance_power): same idle
  draw, quicker frequency ramp on battery. AC unchanged (performance governor and EPP,
  64/90 W limits, undervolt, GPU offsets).
- Globe wallpaper cost on battery, panel on, measured 60 s each: none 5.96 W, 6 fps 6.67,
  8 fps 6.99, 10 fps 7.05, 12 fps 7.29, 15 fps 7.64. A floor of ~0.7 W comes from keeping
  the iGPU and compositor awake at all; above 8 fps each frame costs little. Battery setting
  now 10 fps (smooth for a slowly turning globe), AC stays 15; paused while locked.
- Locked on battery with all of it: 4.00 W (8.1 W this morning). Card reader unbound saves
  another 0.2 W, not applied.
