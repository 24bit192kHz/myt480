# myt480

A ThinkPad T480 rebuilt from source: coreboot firmware, a kernel made for this one
machine, Artix Linux with s6, a chadwm desktop, and the NVIDIA MX150 switched on only
when a program needs it. This repository has everything to build the same laptop.

| Part | What runs |
|---|---|
| Hardware | ThinkPad T480, i7-8650U, 32 GB, GeForce MX150, NVMe |
| Firmware | coreboot with patches, GRUB 2 payload, SeaBIOS and toys as secondary payloads |
| OS | Artix Linux, s6 init, no initramfs |
| Kernel | `linux-t480`: Linux 7.2.8, CachyOS PKGBUILD, BORE, only the modules this laptop uses |
| Desktop | X11, chadwm, picom, st, rofi, slock-ly |
| Graphics | Intel UHD 620 for everything; MX150 on demand through `prime-run` |

Results on this machine: firmware hands over to GRUB about 0.6 s after power-on with the
MX150 enabled (0.4 s without), the desktop is up about 2.6 s after a reset, the laptop
idles at 6.7 W on battery with the MX150 off, the MX150 is ready about 2 s after a
program asks for it, hibernate resumes 5 s after power-on, and a tested undervolt
(-115 mV core on AC) gives about 15 % more CPU throughput at the same temperature.

## Branches

| Branch | Firmware | State |
|---|---|---|
| `speed` | C32: the fastest boot, nothing that costs time | flashed and tested: cold boot, warm reset, suspend, GPU on and off |
| `main` | C51: all patches in `firmware/coreboot/README.md` (0001 to 0025): link wait and `_ROM` check, GRUB runtime config and NVMe fixes, panel power before FSP-S, one reset at most, vendor ASPM and payload settings on the GPU port, SMBIOS version for thinkpad_acpi, and the vendor platform setup (radios, wake sources, subsystem IDs, GMM) | flashed and tested: RTC-alarm cold boots, S3 with the watchdog armed, GPU off/on cycles; hibernate, a watchdog hang test and a 10-minute GPU load test on the builds before it |

Everything outside `firmware/` and `hardware/kernel-cmdline.txt` is the same on both.
`main` is what runs on my laptop now; `speed` is the state before the review round.

## Layout

Every directory has its own README.

| Directory | Content |
|---|---|
| `hardware/` | `lspci`, `lsusb`, disk layout, kernel command line |
| `firmware/coreboot/` | base commit, patches, `.config` as built, `site-local/` (defconfig, `grub.cfg`, blobs, VBIOS) |
| `firmware/libgfxinit/` | patches for the `3rdparty/libgfxinit` submodule |
| `firmware/grub/` | base commit and patches for the GRUB payload |
| `firmware/upstream-dgpu-series/` | the MX150 and T480 work as 17 patches on coreboot main, on Gerrit as topic `t480-dgpu` |
| `firmware/stock-reference/` | ACPI tables and VBIOS dumped from Lenovo's firmware |
| `kernel/` | PKGBUILD, config, module database |
| `system/` | package lists, s6 services, every `/etc` file that is not a package default, `/usr/local` scripts |
| `src/` | the programs written for this laptop, chadwm as configured, changes to third-party programs |
| `home/` | dotfiles and `~/.local/bin` scripts |
| `docs/` | notes on why things are the way they are |

## What you need

- A ThinkPad T480. With a dGPU for the MX150 part; everything else works without one.
- An external SPI programmer (CH341A or a Raspberry Pi Pico with serprog) and a SOIC-8
  clip. Lenovo's firmware write-protects the flash chip, so the first flash is external.
- A second computer to build on and to look things up while the laptop is open.

## Build it

### 1. Dump your own firmware first

Read the flash chip twice with the external programmer and compare the two files.
Keep them: they are your way back, and they hold three things that belong to your
machine and must come from it:

| Blob | What it is |
|---|---|
| `ifd.bin` | flash descriptor |
| `me.bin` | Intel ME; neuter and deguard it (see the coreboot and deguard documentation for the T480) |
| `gbe.bin` | Ethernet configuration with your MAC address |

They are not in this repository, and neither is `doom.wad`. Put yours in `firmware/coreboot/site-local/data/`;
`README.md` there says how to cut them out of the dump.

### 2. Firmware

```sh
git clone https://review.coreboot.org/coreboot.git && cd coreboot
git checkout -b t480 "$(cat ../myt480/firmware/coreboot/BASE-COMMIT)"
git submodule update --init --checkout
( cd 3rdparty/libgfxinit && git am ../../../myt480/firmware/libgfxinit/patches/*.patch )
git am ../myt480/firmware/coreboot/patches/*.patch
cp -r ../myt480/firmware/coreboot/site-local .
make crossgcc-i386 CPUS=$(nproc)          # with Ada, for libgfxinit
make defconfig KBUILD_DEFCONFIG=site-local/t480.defconfig
make -j4
```

Before `make`, set up the GRUB tree as described in `firmware/grub/README.md`, and
fill in the partition UUIDs in `site-local/grub.cfg` (`blkid` shows them).

Write `build/coreboot.rom` with the external programmer. Later updates go from Linux:

```sh
flashprog -p internal -r backup.rom
flashprog -p internal --fmap-file build/coreboot.rom -i FMAP -i RW_SPD_CACHE -i COREBOOT -w build/coreboot.rom
```

A boot that hangs: hold the power button for 4 s. That clears the dGPU request and
asks GRUB for its menu on the next boot.

### 3. System

1. Install Artix (s6) on the NVMe; partitions as in `hardware/lsblk.txt`.
2. `pacman -S --needed - < system/packages-native.txt`, then `system/packages-aur.txt` with `yay`.
3. Build and install the kernel: `cd kernel && makepkg -si`.
4. Copy `system/etc/` over `/etc` and `system/usr-local/` over `/usr/local`.
   Then fill in what is yours: the UUIDs in `fstab` and `default/grub`, `hostname`,
   `hosts`, and the serial number in `validity-rs.conf`.
5. Build the programs in `src/`; `system/usr-local/BINARIES.txt` lists where each goes.
6. Enable the services in `system/s6-active-services.txt`.
7. Create your user, add your own Wi-Fi networks and SSH keys.

The kernel boots without an initramfs through `init=/usr/local/sbin/t480-init`; the
command line is in `hardware/kernel-cmdline.txt` and is set in `site-local/grub.cfg`.

### 4. Desktop

Copy the dotfiles in `home/` (`ls -a`) to `~/`, `home/config/` to `~/.config/` and
`home/local-bin/` to `~/.local/bin/`. tty1 logs in automatically and `~/.zprofile`
starts X.

## MX150

- `dgpu on|off|status` sets whether the firmware powers the GPU at boot (CMOS 0x6f). Leave it on.
- The GPU is off (D3cold, power rail off) until a program uses it:
  `prime-run <program>` turns it on, runs the program on it and turns it off afterwards.
  `gpu-power on|off|status` does it by hand, for `nvidia-smi` and the like.
- On AC `gpu-power` raises the clocks: graphics +150 MHz, memory +1000 MHz
  (`system/etc/gpu-power.conf`): about +4 % in shader-bound and +16 % in memory-bound work,
  tested in `docs/notes/2026-10-01-mx150-tuning.md`. On battery it runs at stock clocks.
- On battery: 6.7 W idle with the GPU off, 7.7 W with it on and idle.
- Driver: `nvidia-580xx-dkms`, the last branch that supports this GPU.
- Details and history: `firmware/MX150-notes.md`.

Compared with Lenovo's firmware (its ACPI tables and hardware state were dumped and
compared, see `firmware/MX150-notes.md`): the GPU port runs with the same link settings
(ASPM off, 256-byte payloads), the EC reads the GPU temperature on both and thinkpad_acpi
shows it as `temp2`, and the fan follows it. Missing on purpose: GC6, the Optimus and
power-sharing `_DSM`s (Windows only) and the GPU's subsystem ID. Rendering is not affected.
Bluetooth, Wake-on-LAN, wake from the Thunderbolt port and from the lid in S4, the TPM and
the Lenovo subsystem IDs on the PCH devices came with firmware C50 and kernel 7.2.8-3.
Firmware C51 (2026-10-01) takes those IDs from the devicetree, as the upstream series does.
Cold boots for testing: `firmware/tools/coldboot.sh` (RTC alarm wake from S5 works).

## Known limits

- Tested on one T480. The firmware patches also build for the T480s and T580, untested.
- `system/etc/thermald.conf` undervolts per power source: -115 mV core and cache on AC,
  -100 mV on battery, found on this CPU with the sweep kit in `firmware/tools/undervolt/`
  (-124 mV gave machine-check errors on AC, -121 mV froze on battery). Every CPU differs:
  set `ac_uv_*` and `batt_uv_*` to 0 on another machine and sweep it.
- The kernel only has the modules my hardware needs. For other hardware add the module
  to `kernel/extra-modules.txt` and rebuild.
- The PCH TCO watchdog is armed at boot (`system/etc/local.d/watchdog.start`, 30 s, petted
  every 5 s): a kernel hang ends in a reset. Killing the keepalive loop disarms it.

## Licences

Patches follow the licence of the project they are for; the programs in `src/` carry
their own. The files in `firmware/coreboot/site-local/data/` and `firmware/stock-reference/`
contain firmware from Lenovo and NVIDIA: the VBIOS of the MX150, which the driver
cannot work without, and Lenovo's ACPI tables as the reference for the power sequence.
They remain the property of their owners. Nothing in this repository identifies a
particular laptop: flash descriptor, Intel ME, MAC address, serial number and partition
UUIDs are left for you to fill in.
