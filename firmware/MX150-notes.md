# MX150 (GP108, 10de:1d10) on coreboot — 2026-09-29

Everything is on `mx150` branches; `master`/`t480` are the working C18 setup.

| Where | mx150 branch has | Pre-MX150 |
|---|---|---|
| src/coreboot | 846db20e58 power dGPU before FSP (CMOS 0x6f), 0f49d14d54 power-button override cancels it | branch `t480` |
| src/coreboot/site-local | 3648b3e VBIOS in CBFS (pci10de,1d10.rom -> ACPI _ROM) | branch `master` |
| kernel (recipe) | 0078650 builds linux-t480-headers too | branch `master`, pkg linux-t480-7.2.8-1-pre-mx150.pkg.tar.zst |
| /etc | modesetting X config, nvidia-sleep.conf override | branch `master` (intel/SNA) |
| /usr/local | bin/dgpu, bin/prime-run | branch `master` |
| roms | C19-mx150.rom | C18.rom |

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
`sudo sh ~/t480-build/tools/mx150-revert.sh`, then reboot.

## Update 2026-09-29 evening: working on C32

The sections above describe C19-C23 and are history. Current state:

- Firmware: roms/C32-mx150.rom, branch `c24` of src/coreboot (off `t480`). coreboot powers
  the GPU in the bootblock; root port 1 is a normal port (no hotplug flag).
- `dgpu on|off` still selects it (CMOS 0x6f). Switching from off to on makes the next
  boot do one extra full reset; that is intended.
- Without a driver bound the GPU is powered off at runtime (ACPI power resource). With
  the NVIDIA driver loaded it stays on.
- Tested: cold boot, warm reboot, S3 resume, dgpu off, off->on, `prime-run glxinfo`.
- Upstream series: worktree src/coreboot-upstream (branch `dgpu-upstream`), patches in
  patches-dgpu/. Needs your Signed-off-by before it goes to Gerrit.
- Flash backup from before C24: /root/PRE-C24-backup.rom.
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
  holding the GPU. Source: ~/systemagent/src/gpu-power.
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
  and with the undervolt at 0 the 10-minute GPU test passes on both. `system/etc/thermald.conf`
  now has `uv_* 0`; step it down again only with a test like `freeze-test.sh`.
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
