## SPDX-License-Identifier: GPL-2.0-only
# T480 site-local additions (upstream coreboot build, no lbmk).
# Everything built from source is selected in the defconfig; this file only
# adds data files and the one out-of-tree payload binary (Pac-Man).

SITE_DATA := site-local/data

# GRUB2 primary payload: splash for gfxterm
cbfs-files-y += background.png
background.png-file := $(SITE_DATA)/background.png
background.png-type := raw

# GRUB2 menu font: Unifont 16 (the look of GRUB's built-in ASCII-only font)
# plus the arrows and box drawing the menu uses. Loaded only for the menu.
#   grub-mkfont -o site-local/data/font.pf2 \
#     -r 0x20-0x7E,0xA0-0xFF,0x2190-0x21FF,0x2500-0x25FF unifont.pcf.gz
cbfs-files-y += font.pf2
font.pf2-file := $(SITE_DATA)/font.pf2
font.pf2-type := raw

# MX150 (GP108, 10de:1d10) VBIOS. The GPU has no usable ROM BAR on this muxless
# Optimus board; the NVIDIA driver reads the VBIOS through ACPI _ROM, which
# pci_rom_ssdt() writes from this file. It is never executed (no VGA_ROM_RUN).
# Source: stock Lenovo firmware, dumped via nouveau debugfs (182272-byte image).
cbfs-files-y += pci10de,1d10.rom
pci10de,1d10.rom-file := $(SITE_DATA)/mx150-vbios.rom
pci10de,1d10.rom-type := optionrom

# Pac-Man (libpayload); no public source, the only prebuilt payload
cbfs-files-y += img/pacman
img/pacman-file := $(SITE_DATA)/pacman.elf
img/pacman-type := payload
img/pacman-compression := none

# Floppy images booted by SeaBIOS from CBFS (etc/bootorder lists them)
cbfs-files-y += floppyimg/kolibri.img
floppyimg/kolibri.img-file := $(SITE_DATA)/kolibri.img
floppyimg/kolibri.img-type := raw
floppyimg/kolibri.img-compression := LZMA

cbfs-files-y += floppyimg/michalos.img
floppyimg/michalos.img-file := $(SITE_DATA)/michalos.flp
floppyimg/michalos.img-type := raw
floppyimg/michalos.img-compression := LZMA

cbfs-files-y += floppyimg/floppybird.img
floppyimg/floppybird.img-file := $(SITE_DATA)/floppybird.img
floppyimg/floppybird.img-type := raw
floppyimg/floppybird.img-compression := LZMA

# SeaBIOS runtime integers (64-bit LE), SeaBIOS is only reached from GRUB:
#   boot-menu-wait -1 : always wait in the SeaBIOS menu
#   pci-optionrom-exec 2 : run VGA option ROMs only
$(obj)/site-local/boot-menu-wait.bin:
	mkdir -p $(dir $@)
	printf '\377\377\377\377\377\377\377\377' > $@

$(obj)/site-local/pci-optionrom-exec.bin:
	mkdir -p $(dir $@)
	printf '\002\000\000\000\000\000\000\000' > $@

cbfs-files-y += etc/boot-menu-wait
etc/boot-menu-wait-file := $(obj)/site-local/boot-menu-wait.bin
etc/boot-menu-wait-type := raw

cbfs-files-y += etc/pci-optionrom-exec
etc/pci-optionrom-exec-file := $(obj)/site-local/pci-optionrom-exec.bin
etc/pci-optionrom-exec-type := raw
