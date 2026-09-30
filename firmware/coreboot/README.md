# coreboot

| File | Meaning |
|---|---|
| `BASE-COMMIT` | the upstream commit the patches apply to |
| `patches/` | `git am` these in order |
| `dot-config-as-built` | the full `.config` of the flashed build, for reference |
| `site-local/` | copy into the coreboot tree; holds everything that is specific to this laptop |

## Patches

| # | What it does | Upstream? |
|---|---|---|
| 0001 | EC: tell the OS about AC changes only when the state changed | could go upstream |
| 0002 | cache the SPD data in flash (`RW_SPD_CACHE`), faster boot | could go upstream |
| 0003, 0004, 0006 | move `3rdparty/libgfxinit` to the patched commits (see `../libgfxinit`) | with the libgfxinit patches |
| 0005 | set SWF18 after libgfxinit lit the panel, so i915 takes the mode over without a flicker | could go upstream |
| 0007 | ASPM: only endpoints limit the exit latency | could go upstream |
| 0008 | enable the SMBus controller (touchpad RMI4, SPD) | could go upstream |
| 0009 | Lenovo's PL1/PL2 power limits | could go upstream |
| 0010 | log how long Thunderbolt mailbox commands take | debugging aid |
| 0011 | power-button override asks GRUB for its menu (CMOS 0x6e) | local |
| 0012 | pass the whole option ROM to ACPI `_ROM` | patch 1 of `../upstream-dgpu-series` |
| 0013 | MX150 support as flashed in C32, switched by CMOS 0x6f | local form of patches 2 and 3 of the series |
| 0014 | wait up to 100 ms for the GPU link before FSP-S; check the image lengths of an option ROM against the size of the CBFS file | the review fixes of the series |
| 0015 | put `grub.cfg` into the payload's memdisk (no CBFS walk at every GRUB start); after a reset without power loss trust the SPD cache without reading the DIMM serial numbers (130 ms) | local |
| 0016 | request eDP panel power before FSP-S, so the panel's 210 ms power-up overlaps FSP-S (graphics init 259 -> 13 ms); pull-ups on the HDMI DDC pads, an empty HDMI port answers in 3 ms instead of a 65 ms timeout | could go upstream |

0003, 0004 and 0006 only change the commit that the submodule points to. They apply
without the submodule's content, but the build needs the libgfxinit patches applied
first, so that those commits exist.

## site-local

| File | Meaning |
|---|---|
| `t480.defconfig` | the configuration: board, blobs, libgfxinit, payloads |
| `t480-debug.defconfig`, `t480-debug.fmd` | the same with coreboot's log written to the flash, see below |
| `grub.cfg` | GRUB menu and the kernel command line |
| `Makefile.mk` | adds the data files below to CBFS |
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
sh ../../tools/romcheck.sh build/coreboot.rom     # prints "romcheck: OK"
```

## Debug build

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

coreboot only rebuilds the GRUB payload when `.config` changes: `touch .config` after a
change to GRUB or `grub.cfg`.
