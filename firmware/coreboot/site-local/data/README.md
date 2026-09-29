# Data files

Four files are not here. Three belong to one machine: Take them from a dump of
your own flash chip and put them in this directory before you build:

| File | What it is | How to get it |
|---|---|---|
| `ifd.bin` | flash descriptor | `ifdtool -x dump.bin`, the file `flashregion_0_flashdescriptor.bin` |
| `me.bin` | Intel ME | `flashregion_2_intel_me.bin`, then neutered and deguarded for the T480 |
| `gbe.bin` | Ethernet configuration with the MAC address | `flashregion_3_gbe.bin` |

`ifdtool` is in `util/ifdtool` of the coreboot tree. The defconfig expects these names.

The fourth is `doom.wad`, the game data for the Doom payload. It is not free to pass
on. Put the shareware `doom1.wad` here under that name, or take the two
`CONFIG_COREDOOM_` lines out of `t480.defconfig`.

The other files are the same on every T480 with this configuration:

| File | What it is |
|---|---|
| `mx150-vbios.rom` | VBIOS of the MX150 from Lenovo's firmware, read by the driver through ACPI `_ROM` |
| `background.png`, `font.pf2` | GRUB menu |
| `bootorder` | SeaBIOS boot order |
| `pacman.elf`, `*.img`, `*.flp` | toys in the boot menu |
