# coreboot

| File | Meaning |
|---|---|
| `BASE-COMMIT` | the upstream commit the patches apply to |
| `patches/` | `git am` these in order |
| `dot-config-as-built` | the full `.config` of the flashed build, for reference |
| `site-local/` | copy into the coreboot tree; holds everything that is specific to this laptop |

## Patches

On coreboot main of 2026-10-08. Two earlier local patches are upstream now and gone from the list: the SMBus controller (commit 64579f74e4) and the `CBET4000` BIOS version for thinkpad_acpi (28cfb2125d, which also brought the Fn hotkey events).

| # | What it does | Upstream? |
|---|---|---|
| 0001 | EC: tell the OS about AC changes only when the state changed | could go upstream |
| 0002 | cache the SPD data in flash (`RW_SPD_CACHE`), faster boot | could go upstream |
| 0003, 0004, 0006 | move `3rdparty/libgfxinit` to the patched commits (see `../libgfxinit`) | with the libgfxinit patches |
| 0005 | set SWF18 after libgfxinit lit the panel, so i915 takes the mode over without a flicker | could go upstream |
| 0007 | ASPM: only endpoints limit the exit latency | could go upstream |
| 0008 | Lenovo's PL1/PL2 power limits | could go upstream |
| 0009 | log how long Thunderbolt mailbox commands take | debugging aid |
| 0010 | power-button override asks GRUB for its menu (CMOS 0x6e) | local |
| 0011 | pass the whole option ROM to ACPI `_ROM` | patch 1 of `../upstream-dgpu-series` |
| 0012 | MX150 support as flashed in C32, switched by CMOS 0x6f | local form of patches 2 and 3 of the series |
| 0013 | wait up to 100 ms for the GPU link before FSP-S; check the image lengths of an option ROM against the size of the CBFS file | the review fixes of the series |
| 0014 | put `grub.cfg` into the payload's memdisk (no CBFS walk at every GRUB start); after a reset without power loss trust the SPD cache without reading the DIMM serial numbers (130 ms) | local |
| 0015 | request eDP panel power before FSP-S, so the panel's 210 ms power-up overlaps FSP-S (graphics init 259 -> 13 ms); pull-ups on the HDMI DDC pads, an empty HDMI port answers in 3 ms instead of a 65 ms timeout | could go upstream |
| 0016 | a GPU whose link never comes up resets the machine once, not forever (CMOS 0x6d remembers the attempt); the second time it boots without the GPU | belongs in the series |
| 0017 | no PCIe L1 substates on the GPU's root port (ASPM L1 stays). Linux is not given ASPM control here, so the port ran with FSP's L1.1/L1.2; the machine once hung without a trace right after a GPU benchmark, the signature of a failed L1.2 exit | belongs in the series |
| 0018 | no ASPM at all on the GPU's root port, as on the vendor firmware (its FADT declares ASPM unsupported and root port 1 advertises none) | belongs in the series |
| 0019 | 256-byte PCIe payloads on the GPU, Wi-Fi and SSD root ports, the vendor firmware's values (the Thunderbolt port stays at 128) | belongs in the series |
| 0020 | ec/lenovo/h8: options for boards without a wireless switch (the T480 EC has no such bit; WLSW read 0 and thinkpad_acpi hard-blocked Bluetooth and WWAN) or a tablet switch, and a lid wake-state macro | belongs in the series |
| 0021 | soc/intel/skylake: the GbE ACPI device with its wake entry (Wake-on-LAN in `/proc/acpi/wakeup`), PME wake on root port 9 (Thunderbolt), and the board's subsystem IDs handed to FSP for the read/write-once registers it programs first | belongs in the series |
| 0022 | t480: radios never hard-blocked, no tablet switch, lid wake declared as S4-capable like the vendor DSDT (only S3 works, see 0027), GMM 00:08.0 on, subsystem IDs 17aa:225d on every PCH and system agent device, as on the vendor firmware | belongs in the series |
| 0023 | t480: the subsystem IDs come from the devicetree (`subsystemid 0x17aa 0x225d inherit` in the T480 override tree; sconfig drops the one in `devicetree.cb`) and soc/intel/skylake hands the system agent's IDs to FSP, instead of `CONFIG_SUBSYSTEM_*`, which upstream's lint rejects in a board Kconfig. Flashed as C51 | in the series (v4) |
| 0024 | payloads/GRUB2: `site-local/data/boot.pub` and `auth.cfg`, when present, go into the GRUB memdisk next to the runtime config, so the signing key and the password hash are part of the payload that coreboot measures into PCR 2 | local |
| 0025 | ec/lenovo/h8: the wireless and tablet switch come from devicetree registers (`no_wireless_switch`, `has_tablet_mode_switch`) instead of the Kconfig options of 0022; the T480 sets the first | patches 7, 8 and 12 of `../upstream-dgpu-series` |
| 0026 | the power-button override (power held 4 s) makes the next boot run without the MX150 once (CMOS 0x6c) and keeps the choice in 0x6f; before, it cleared the choice and the GPU stayed off until `dgpu on` | belongs in the series |
| 0027 | ec/lenovo/h8: the mainboard can set the lid's wake GPE; the T480 uses 0x17 (EC_WAKE# on GPP_C23, as Lenovo's `_PRW {0x17, 4}`) instead of the hard-coded 0x18, which maps to no pad here. S4 lid wake remains unimplemented: arming the SCI/wake GPEs and copying the vendor EC state did not enable it. Subsequent SMM analysis found no lid-specific arming path, so the earlier attribution to missing vendor SMM was unsupported. From S3 the lid wakes by the EC's power-button pulse; static EC firmware investigation is recorded in the [research folder](../../research/2026-10-09-t480/README.md) | belongs in the series |
| 0028 | t480: `gpe0_dw0..2 = GPP_C, GPP_D, GPP_E` in the devicetree, what the PMC fell back to from MISCCFG (no more "Duplicate GPE DW register values" warning) | belongs in the series |
| 0029 | runtime MX150 power-on stops when PWRGD remains low: disable link, assert reset and clear rail request before returning | source-only research correction; actual AML/DSDT checked, not flashed |
| 0030 | validate bounded CBFS ROM headers/PCIR and complete image chains; retain mapping size through fallback and ACPI copy | source-only; sanitizer checked; requires 0031's actual-size producer contract |
| 0031 | CBFS returns actual loaded/decompressed bytes, uses raw extent for NONE and rejects output beyond capacity | source-only producer correction; no firmware deployment |

Patch 0029 was added by the [MX150 research continuation](../../research/2026-10-09-t480/12-mx150/04-sleep-and-firmware.md).
It applies after 0028 and is absent from `dot-config-as-built`/the archived C55
firmware's behavior. Forward/reverse application, eight ACPICA execution cases
and complete matching T480 DSDT compilation were checked offline. Building/flashing
or inducing a failed rail on the laptop requires separate approval. Reversing
the patch restores source behavior; it does not alter already installed firmware.

Patches 0030 and 0031 form the [CBFS ROM bounds correction](../../research/2026-10-09-t480/12-mx150/03-vbios-and-ec.md).
Apply both in order after 0029. The original CBFS allocator could publish declared
decompression capacity rather than actual output; using that value as a parser
bound would leave 0030 incomplete. Actual-source host fixtures use ASAN/UBSAN,
including baseline failure reproduction and full MX150 nonpadding preservation.
They do not validate on-device/RAM ROM mappings, generic execution/VFCT, every
ACPI AML read contract, the full firmware build or hardware behavior. Reverse
0031 then 0030 in a source tree before rebuilding to remove them. Existing flashed
firmware remains unchanged.

Review of 2026-10-10 (independent of the author): the three patches apply on `c55`
and `pci_rom.c`/`cbfs.c` compile cleanly with coreboot's own toolchain and `-Werror`.
They have no effect on this board: the VBIOS is stored uncompressed in CBFS
(`site-local/Makefile.mk`, 184320 bytes), so the stored and mapped sizes are already
equal and the `_ROM` copy is byte-identical with or without 0030/0031; and Lenovo's own
`HGON` never checks power-good at all (8–16 ms delay, then reset release), so the
abort path of 0029 is new behaviour without a vendor precedent and has never been
exercised. Treat them as hardening for a future build, gated by the QEMU scenarios;
they do not justify a flash on their own.

0003, 0004 and 0006 only change the commit that the submodule points to. They apply
without the submodule's content, but the build needs the libgfxinit patches applied
first, so that those commits exist.

## site-local

| File | Meaning |
|---|---|
| `t480.defconfig` | the configuration: board, blobs, libgfxinit, payloads, TPM measured boot (PCR 2; the MX150 option ROM is listed as runtime data and goes to PCR 3, because it is only loaded while the GPU is on) |
| `t480-debug.defconfig`, `t480-debug.fmd` | older debug profile with flash logging; GRUB/signature/measured-boot settings differ from production, see below |
| `grub.cfg` | GRUB menu, the kernel command line and the boot policy: signature enforced on the default kernel, password on everything that can start something else |
| `Makefile.mk` | adds the data files below to CBFS |
| `data/boot.pub`, `data/auth.cfg` | public key the kernel is signed with, and GRUB superuser + password hash. Not included: make your own (main README, step 3a). Without them the build still works and GRUB checks nothing |
| `data/ifd.bin`, `me.bin`, `gbe.bin` | flash descriptor, Intel ME and Ethernet configuration. Not included: they belong to one machine, see `site-local/data/README.md` |
| `data/mx150-vbios.rom` | VBIOS of the MX150, from Lenovo's firmware. The driver reads it through ACPI `_ROM`; it is never executed |
| `data/background.png`, `font.pf2` | GRUB menu |
| `data/bootorder` | SeaBIOS boot order |
| `data/pacman.elf`, `*.img`, `*.flp` | toys in the boot menu; `doom.wad` is not included |


## Build

```sh
make crossgcc-i386 CPUS=$(nproc)      # once; builds GCC with Ada for libgfxinit
make defconfig KBUILD_DEFCONFIG=site-local/t480.defconfig
make -j4
```

Before flashing, check that the image carries this tree's `grub.cfg` and not the
QEMU harness one (`site-local/Makefile.mk` rebuilds the shared payload on every
`make`; the check is the belt to those braces):

```sh
sh ../myt480/firmware/tools/romcheck.sh build/coreboot.rom   # prints "romcheck: OK"
```

## Debug build

The stored debug defconfig predates the production boot policy: it selects the
older `t480` GRUB branch, omits PGP modules and does not enable measured boot.
Before using the commands below, derive the debug profile from the production
configuration, add its logging region, and verify equivalent boot policy. The
[cross-stack review](../../docs/wiki/cross-stack-review.md) records this mismatch
and the pre-flash checker's remaining limitations.

A boot that hangs leaves nothing behind: the log is in RAM and the way out is a
forced power-off. The debug build writes the log to a 128 KiB region `CONSOLE` of the
flash chip as well.

```sh
make defconfig KBUILD_DEFCONFIG=site-local/t480-debug.defconfig
make -j4
flashprog -p internal --fmap-file build/coreboot.rom -i FMAP -i CONSOLE -i COREBOOT -w build/coreboot.rom
# ... the boot that hangs, power off, boot again ...
flashprog -p internal --fmap -i CONSOLE:console.bin -r /dev/null && strings console.bin | less
```

The log is appended until the region is full, about two boots. Writing the debug image
again empties it. It costs boot time and wears the flash: go back to the normal build
when the problem is found.

