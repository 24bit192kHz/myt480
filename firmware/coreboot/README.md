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
| 0013 | MX150 support as flashed, switched by CMOS 0x6f | local form of patches 2 and 3 of the series |

0003, 0004 and 0006 only change the commit that the submodule points to. They apply
without the submodule's content, but the build needs the libgfxinit patches applied
first, so that those commits exist.

## site-local

| File | Meaning |
|---|---|
| `t480.defconfig` | the configuration: board, blobs, libgfxinit, payloads |
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

coreboot only rebuilds the GRUB payload when `.config` changes: `touch .config` after a
change to GRUB or `grub.cfg`.
