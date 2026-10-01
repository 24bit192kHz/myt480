# MX150 series for upstream coreboot

The dGPU work and the T480 platform fixes as 17 patches on coreboot main
2631796496 (2026-09-30), on Gerrit as topic `t480-dgpu`:
https://review.coreboot.org/q/topic:t480-dgpu

Author and sign-off: `24bit192kHz <24bit192khz@proton.me>`, the same person as
https://github.com/24bit192kHz (whose profile carries that address) and this
repository.

| Patch | What it does |
|---|---|
| 0001 | `_ROM` gets the whole option ROM, not only what the one-byte size field of its header describes; never more than the CBFS file holds |
| 0002 | power the dGPU in the bootblock and wait for its link, so FSP-S finds it on root port 1; full reset on the first boot after enabling it; power off before sleep |
| 0003 | ACPI power resource: the OS can turn the GPU off and on at runtime (D3cold) |
| 0004 | keep the setup option visible after the dGPU was disabled |
| 0005 | documentation of the board |
| 0006 | the full reset of 0002 happens once (CMOS byte 0x6d), never a reset loop |
| 0007 | T480: Max_Payload_Size 256 on the dGPU, WLAN and SSD ports, as the vendor firmware |
| 0008 | T480: no ASPM on the dGPU root port, as the vendor firmware |
| 0009 | ec/lenovo/h8: option for ECs without a radio switch (WLSW returns 1) |
| 0010 | ec/lenovo/h8: option for boards without a tablet switch (no MHKG) |
| 0011 | ec/lenovo/h8: the mainboard can set the deepest state the lid wakes from |
| 0012 | soc/intel/skylake: the GbE ACPI device (Wake-on-LAN in /proc/acpi/wakeup) |
| 0013 | soc/intel/skylake: FSP gets the board's subsystem IDs (Kconfig, else the devicetree) |
| 0014 | T480: selects the options of 0009 and 0010 |
| 0015 | T480: the lid wakes from S4 |
| 0016 | T480: GMM device 00:08.0 on |
| 0017 | T480: subsystem IDs 17aa:225d in the override tree |

## State

| | |
|---|---|
| Builds | every patch on its own for T480, T480s, T580, X280, X380 Yoga and T470s, and with the setup menu (`CONFIG_DRIVERS_OPTION_CFR`) at 0004 and at the top; at the top of the series: every board in the tree, 966 configurations (6 more need the Go compiler for helper tools and were not built) |
| Lint | `make gitconfig` hooks (`check-style`, `lint-stable`, checkpatch on each commit); `lint-stable` and `lint-extended` clean |
| Tested on hardware | T480 (C51 = the flashed build with 0013 and 0017 as here): cold boot from S5, S3 resume with the watchdog armed, all PCH and system agent devices at 17aa:225d, MPS 256 on the dGPU/SSD ports, no ASPM on root port 1, NVIDIA driver with GL and Vulkan, 5 GPU off/on cycles through D3cold. Earlier builds of the same code (C35 to C50): first boot after enabling the GPU, 70 driver load/unload cycles, hibernate/resume |
| Not tested | T480s, T580 and the other variants on hardware; the setup menu of 0004; an actual wake from S4 by opening the lid |

## Found on the way (not in the series)

sconfig drops the `subsystemid` of a base devicetree when an override tree names
the same device: `alloc_dev()` sets unset IDs to -1, and the merge copies the
override's IDs whenever they are "non-zero", which -1 is. Every sklkbl variant
re-declares `device domain 0 on`, so the `subsystemid 0x17aa 0x225d inherit` in
the common `devicetree.cb` never reached a device (0017 works around it for the
T480).

## Gerrit history

- v3, 2026-09-30, changes 95872 to 95883 (12 patches): Code-Review -2 on all from a
  core developer, "Invalid sign-off"; Jenkins: patch 1 broke the arm64 build
  (GOOGLE_CHERRY, unused function), the last patch failed lint-stable-024 (board
  Kconfig must not set SUBSYSTEM_*_ID).
- v4, 2026-10-01: both failures fixed; the T480-only options no longer apply to the
  other variants (the X380 Yoga kept its tablet switch); the subsystem IDs come
  from the devicetree; the h8 and T480 patches split into one change each
  (changes 95872 to 95883 updated, 95912 to 95916 new).

## Using the series

The firmware this laptop runs is built from `../coreboot/` as the main README
describes; its patches 0013, 0014, 0017 to 0020 and 0022 to 0025 are the local form of
this series (with a GRUB payload, and the dGPU switch in CMOS byte 0x6f). The series is the form meant for coreboot itself.
To build it, apply it to coreboot main:

```sh
git clone https://review.coreboot.org/coreboot.git && cd coreboot
git checkout -b t480-dgpu 2631796496
git submodule update --init --checkout
git am ../myt480/firmware/upstream-dgpu-series/*.patch
make crossgcc-i386 CPUS=$(nproc)
make menuconfig          # Mainboard: Lenovo, ThinkPad T480; payload of your choice
make -j4
```

The latest version is the chain on Gerrit; fetching its last change brings all of
them: open the last change of https://review.coreboot.org/q/topic:t480-dgpu and use
its *Download* menu.

Two things the image needs that the series cannot carry:

- **The GPU's VBIOS.** The MX150 has no ROM of its own; the NVIDIA driver reads the
  VBIOS through ACPI `_ROM`. Add the one dumped from Lenovo's firmware
  (`../coreboot/site-local/data/mx150-vbios.rom`, or your own) to CBFS:
  `build/cbfstool build/coreboot.rom add -f mx150-vbios.rom -n pci10de,1d10.rom -t optionrom`
- **The option `dgpu_enable`.** The GPU stays off unless it is set. coreboot reads it
  through its option backend: with the EDK2 payload and SMMSTORE it is in the setup
  menu. Without an option backend the GPU stays off.

The flash descriptor, ME and GbE regions come from your own machine, and flashing
works as in the main README (steps 1 and 2).
