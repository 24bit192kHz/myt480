# myt480

A ThinkPad T480 rebuilt from source: coreboot firmware, a kernel made for this one
machine, Artix Linux with s6, a chadwm desktop, and the NVIDIA MX150 switched on only
when a program needs it. This repository has everything to build the same laptop.

| Part | What runs |
|---|---|
| Hardware | ThinkPad T480, i7-8650U, 32 GB, GeForce MX150, NVMe |
| Firmware | coreboot with patches, GRUB 2 payload, SeaBIOS and toys as secondary payloads |
| OS | Artix Linux, s6 init, no initramfs on disk; root and swap LUKS2-encrypted, opened by the TPM without a prompt |
| Kernel | `linux-t480`: Linux 7.2.8, CachyOS PKGBUILD, BORE, only the modules this laptop uses, a 62 kB early init built in |
| Boot chain | coreboot measures itself and GRUB into the TPM; GRUB only starts a signed kernel without a password; the kernel unseals the disk key |
| Desktop | X11, chadwm, picom, st, rofi, slock-ly |
| Graphics | Intel UHD 620 for everything; MX150 on demand through `prime-run` |

Results on this machine: with measured boot, the signature check and the encrypted disk
the desktop is up 3.5 to 3.6 s after power-on (3.6 to 3.9 s before all of that, see
`docs/notes/2026-10-04-disk-encryption.md`), the laptop idles at 6.7 to 6.9 W on battery
with the MX150 off, the MX150 is ready about 2 s after a program asks for it, hibernate
resumes 6 s after the wake alarm, and a tested undervolt (-115 mV core on AC) gives about
15 % more CPU throughput at the same temperature.

## Branches

| Branch | Firmware | State |
|---|---|---|
| `speed` | C32: the fastest boot, nothing that costs time | flashed and tested: cold boot, warm reset, suspend, GPU on and off |
| `main` | C55 (2026-10-08): on coreboot main of that day, all patches in `firmware/coreboot/README.md` (0001 to 0028): link wait and `_ROM` check, GRUB runtime config and NVMe fixes, panel power before FSP-S, one reset at most, vendor ASPM and payload settings on the GPU port, SMBIOS version for thinkpad_acpi, and the vendor platform setup (radios, wake sources, subsystem IDs, GMM); C52 to C54 (2026-10-04): TPM measured boot, GRUB signature check and password, boot entries for the encrypted disk, h8 options from the devicetree, GPU option ROM measured into PCR 3 (`site-local`); C55: rebased on upstream main (Fn hotkey events from upstream), the power-button override keeps the GPU choice, lid wake GPE fixed, GPE routing explicit (patches 0026 to 0028) | flashed and tested: RTC-alarm cold boots, S3 with the watchdog armed, GPU off/on cycles; hibernate, a watchdog hang test and a 10-minute GPU load test on the builds before it; C54 also: suspend, hibernate, firmware flash with re-seal |
| `testing` | C55 retained; no firmware or kernel replacement in the 2026-10-09 audit | reversible fan/GPU fixes and Intel routing deployed; diagnostics, protocol recovery and remaining defects recorded in [research](research/2026-10-09-t480/README.md) |

`main` and `speed` differ in `firmware/` and `hardware/kernel-cmdline.txt`.
`main` supplies the running C55 firmware; `speed` is the state before that review.
`testing` adds the 2026-10-09 audit changes and research without rewriting either branch.

## Layout

Every directory has its own README.

| Directory | Content |
|---|---|
| `hardware/` | `lspci`, `lsusb`, disk layout, kernel command line |
| `firmware/coreboot/` | base commit, patches, `.config` as built, `site-local/` (defconfig, `grub.cfg`, blobs, VBIOS) |
| `firmware/libgfxinit/` | patches for the `3rdparty/libgfxinit` submodule |
| `firmware/grub/` | base commit and patches for the GRUB payload |
| `firmware/upstream-dgpu-series/` | the MX150 and T480 work as 15 patches on coreboot main, on Gerrit as topic `t480-dgpu` |
| `firmware/stock-reference/` | ACPI tables and VBIOS dumped from Lenovo's firmware |
| `kernel/` | PKGBUILD, config, module database |
| `system/` | package lists, s6 services, every `/etc` file that is not a package default, `/usr/local` scripts |
| `src/` | the programs written for this laptop, chadwm as configured, changes to third-party programs |
| `home/` | dotfiles and `~/.local/bin` scripts |
| `docs/` | notes on why things are the way they are |
| `research/` | dated subject reports, complete audit work log, validation and recovery |

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

### 3a. Signed kernel, GRUB password, disk encryption (optional, in this order)

`docs/notes/2026-10-04-disk-encryption.md` explains the design and what it cost.

1. Signing key: a GnuPG key without passphrase in a root-only directory; export the
   public key to `site-local/data/boot.pub`. `system/usr-local/sbin/t480-sign-kernel`
   signs `/boot/vmlinuz-linux-t480`, and the pacman hook in `system/etc/pacman.d/hooks/`
   runs it after every kernel install.
2. GRUB password: `grub-mkpasswd-pbkdf2`, then `site-local/data/auth.cfg` with
   `set superusers="you"` and `password_pbkdf2 you <hash>`. Neither file is in this
   repository. Build and flash; from then on GRUB refuses an unsigned default kernel.
3. Encryption: `kernel/early-init/README.md`. Make a verified backup first.

After that, before every firmware flash: `t480-reseal next-boot` (or use
`firmware/tools/flashrom.sh`), or the next boot asks for the LUKS passphrase.

### 4. Desktop

Copy the dotfiles in `home/` (`ls -a`) to `~/`, `home/config/` to `~/.config/`,
`home/local-bin/` to `~/.local/bin/` and `home/local-share/` to `~/.local/share/`.
The Bitwarden desktop override requires the `igpu-run` helper installed by
`src/gpu-power`. tty1 logs in automatically and `~/.zprofile`
starts X.

## MX150

- `dgpu on|off|status` sets whether the firmware powers the GPU at boot (CMOS 0x6f). Leave it on.
- The GPU is off (D3cold, power rail off) until a program uses it:
  `prime-run <program>` turns it on, runs the program on it and turns it off afterwards.
  `gpu-power on|off|status` does it by hand, for `nvidia-smi` and the like.
- On AC `gpu-power` raises the clocks: graphics +200 MHz, memory +1250 MHz
  (`system/etc/gpu-power.conf`), retained from the self-checking CUDA tuning runs.
  `prime-run` restores those offsets after runtime wake. On battery it uses stock clocks.
- On battery: 6.7 W idle with the GPU off, 7.7 W with it on and idle.
- Driver: `nvidia-580xx-dkms`, the last branch that supports this GPU.
- Details and history: `firmware/MX150-notes.md`.
- 2026-10-09: idle RTD3 is enabled with clock restoration in `prime-run`, and fan
  control has a tested kernel-watchdog fallback. See the [improvement wiki](docs/wiki/README.md)
  for live results, reverse-engineering findings, remaining work, and rollback.

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

- The disk key is bound to the firmware (TPM PCR 2), not to a secret you type. Someone
  who can rewrite the flash chip with a clip programmer can defeat that; the flash is
  not write-locked, on purpose, so that it can be updated from Linux.
- Bitwarden desktop holds `memfd_secret` memory, and the kernel refuses to hibernate
  while any program does. `home/local-bin/hibernate-safe` quits it, hibernates and
  starts it again; other ways to hibernate are refused while it runs.
- The RTC alarm does not wake this laptop from S5 on battery; `firmware/tools/coldboot.sh`
  needs AC.

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
