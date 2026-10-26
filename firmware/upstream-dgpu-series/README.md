# MX150 series for upstream coreboot

The dGPU work and the T480 platform fixes as 15 patches on coreboot main
6e7766c710 (2026-10-02), on Gerrit as topic `t480-dgpu` (v6, 2026-10-26):
https://review.coreboot.org/q/topic:t480-dgpu

Author and sign-off: `24bit192kHz <24bit192khz@proton.me>`, the same person as
https://github.com/24bit192kHz (whose profile carries that address) and this
repository.

| Patch | What it does |
|---|---|
| 0001 | `_ROM` gets the whole option ROM, not only what the one-byte size field of its header describes; never more than the CBFS file holds, and a warning in the log when the file ends before its last image (a truncated VBIOS dump) |
| 0002 | root port 1 is marked hot-pluggable, so FSP-S keeps it without a link; the dGPU gets its power right before FSP-S and is found in the PCI scan; power off before sleep |
| 0003 | ACPI power resource: the OS can turn the GPU off and on at runtime (D3cold); `_PRW`, `_S0W` and `HotPlugSupportInD3` on the root port, so that Linux puts the hot-plug port into D3cold too |
| 0004 | keep the setup option visible after the dGPU was disabled |
| 0005 | documentation of the board |
| 0006 | T480: Max_Payload_Size 256 on the dGPU, WLAN and SSD ports, as the vendor firmware |
| 0007 | ec/lenovo/h8: devicetree register `no_wireless_switch` for ECs without a radio switch (WLSW returns 1) |
| 0008 | ec/lenovo/h8: the tablet mode switch (MHKG) only for boards with the register `has_tablet_mode_switch`; set for X60, X200, X201, X220, X230/X230t and X380 Yoga |
| 0009 | ec/lenovo/h8: the mainboard can set the deepest state the lid wakes from |
| 0010 | soc/intel/skylake: the GbE ACPI device (Wake-on-LAN in /proc/acpi/wakeup) |
| 0011 | soc/intel/skylake: FSP gets the board's subsystem IDs (Kconfig, else the devicetree) |
| 0012 | T480: sets `no_wireless_switch` |
| 0013 | T480: declare lid wake as S4-capable in ACPI; actual S4 lid wake remains unimplemented |
| 0014 | T480: GMM device 00:08.0 on |
| 0015 | T480: subsystem IDs 17aa:225d in the override tree |

## State

| | |
|---|---|
| Builds | in the coreboot-sdk container (the toolchain of coreboot's Jenkins): every patch on its own for all sklkbl_thinkpad variants, X230 variants and X60; at the top of the series all 54 Lenovo configurations |
| Lint | checkpatch on each commit, `lint-stable` clean |
| Tested on hardware | T480, with the same code in the local tree (which reads a CMOS byte where 0002 reads the option): 0002: link down before FSP-S, port kept, link up before the PCI scan; cold boot, warm reset, first boot after enabling the GPU (no extra reset), GPU disabled, S3 resume; Linux binds pciehp to the port, NVIDIA driver with GL and Vulkan, 5 GPU off/on cycles through D3cold (driven by my own tool; see the fixed problem below). v6 of 0003 on firmware C58 (the local tree with the same hot-plug port and ACPI objects): with the driver unloaded, Linux puts the root port and the GPU into D3cold within a second (ACPI real power state D3cold, the rail off through the power resource), both stay there at idle (60 s), 5 off/on cycles of the driver, no pciehp or Xid events. 0007, 0008, 0012: "radio switch found; radios are enabled", Bluetooth and WWAN unblocked, no tablet mode switch. Earlier builds: subsystem IDs 17aa:225d, MPS 256, hibernate/resume |
| Not tested | this exact series booted on top of main; T480s, T580 and the other variants on hardware; the setup menu of 0004; an actual wake from S4 by opening the lid |
| Fixed in v6 | with root port 1 marked hot-pluggable (0002, v5), Linux kept the port in D0 when no driver was bound, so the power resource of 0003 was never turned off and the GPU stayed powered at idle (T480 on battery: 6.9 W without the flag, 7.3 to 10.5 W with it). Linux puts a hot-plug port into D3 only if it can signal wake and hot-plug events from there (`acpi_pci_bridge_d3()` in `drivers/pci/pci-acpi.c`): v6 of 0003 adds `_PRW` (the PCIe PME GPE, as the vendor firmware has on this port), `_S0W` = D3cold and the `_DSD` property `HotPlugSupportInD3` to the root port. Measured on C58, see "Tested on hardware". Reported on [CB:95873](https://review.coreboot.org/c/coreboot/+/95873) on 2026-10-04 |

## Found on the way (not in the series)

sconfig drops the `subsystemid` of a base devicetree when an override tree names
the same device: `alloc_dev()` sets unset IDs to -1, and the merge copies the
override's IDs whenever they are "non-zero", which -1 is. Every sklkbl variant
re-declares `device domain 0 on`, so the `subsystemid 0x17aa 0x225d inherit` in
the common `devicetree.cb` never reached a device (0015 works around it for the
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

- v5, 2026-10-04, after review by Patrick Rudolph: the h8 options moved from Kconfig
  to devicetree registers, and the tablet switch is now opt-in for the tablets; the
  dGPU's root port is marked hot-pluggable instead of powering the GPU in the
  bootblock, which made the one-time full reset unnecessary (95877 abandoned);
  "no ASPM on the dGPU root port" abandoned (95879): the port has CLKREQ0#, ASPM L1
  with L1 substates passed a 10 minute load test and S3, and the hangs once blamed
  on ASPM were a CPU undervolt. 95872 still carries a Code-Review -2 about the
  provenance of AI-assisted patches.
- v6, 2026-10-26, after a T580 owner tried the series on [CB:95876](https://review.coreboot.org/c/coreboot/+/95876):
  0001 warns when the VBIOS file in CBFS ends before the end of its image and never
  copies more than the file (his first dump was 119296 bytes and failed silently with
  `RmInitAdapter failed! (0x24:0x40:1603)`); 0003 adds `_PRW`, `_S0W` and
  `HotPlugSupportInD3` to the root port, which lets Linux put the hot-plug port and
  the GPU into D3cold (the idle power problem of v5, measured fixed on the T480).

## Using the series

The firmware this laptop runs is built from `../coreboot/` as the main README
describes; its patches 0013, 0014, 0017 to 0020 and 0022 to 0025 are the local form of
an earlier version (v4) of this series (with a GRUB payload, the dGPU switch in CMOS byte 0x6f, and the GPU still powered in the bootblock). The series is the form meant for coreboot itself.
To build it, apply it to coreboot main:

```sh
git clone https://review.coreboot.org/coreboot.git && cd coreboot
git checkout -b t480-dgpu 6e7766c710
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
  - It must be the **complete image**, 182272 bytes for this VBIOS. A dump read from the
    running GPU (nouveau's debugfs, `vgabios_finder`, the ROM BAR) is cut off at the
    size byte of its header (119808 bytes, or less), and the driver then fails with
    `RmInitAdapter failed! (0x24:0x40:1603)`. `../stock-reference/mx150-vbios-10de-1d10.rom`
    is such a cut dump, kept as a reference only.
  - Take it from the Lenovo firmware image instead: `UEFIExtract <image> all`, then the
    raw section of the FFS file `8B9BEF90-5FAF-46CF-A32A-7E1B7D33CC87` (T480) or
    `17835F4C-5D9B-484D-ABBC-5FBDC11F891C` (T580, found by its owner); its body starts
    with `55 AA` and has a `PCIR` structure for 10de:1d10.
  - Check: the 16-bit image length at `PCIR + 0x10` is in 512-byte units (0x164 × 512 =
    182272 here), and the file must be at least that long; `cbfstool coreboot.rom print`
    must show that size for `pci10de,1d10.rom`. With v6, a cut file is reported in
    `cbmem -c`: `option ROM needs 182272 bytes, but its CBFS file has only ...`.
- **The option `dgpu_enable`.** The GPU stays off unless it is set. coreboot reads it
  through its option backend: with the EDK2 payload and SMMSTORE it is in the setup
  menu. Without an option backend (for example with SeaBIOS or GRUB as the payload)
  `get_uint_option()` always returns its fallback and the GPU stays off; changing
  `.default_value` in `cfr.c` does not help, it is only the default the setup menu
  shows. For a quick test without EDK2, change the fallback in `dgpu_power_on()` from
  `get_uint_option("dgpu_enable", 0)` to `get_uint_option("dgpu_enable", 1)`. The
  firmware log then shows `dGPU: power good after ... us`, `dGPU: link after ... ms`
  and `dGPU: enabled`; `dGPU: disabled` means the rail was never switched on.
- **No VGA option ROM run.** The MX150 has no display outputs (muxless Optimus), so
  `CONFIG_VGA_ROM_RUN` and a payload that runs option ROMs are not needed; the ROM in
  CBFS is only handed to the driver through `_ROM`.

The flash descriptor, ME and GbE regions come from your own machine, and flashing
works as in the main README (steps 1 and 2).
