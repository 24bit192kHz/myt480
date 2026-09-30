# T480 Firmware + Kernel Workspace Notes

> **2026-09-28:** partly superseded, see [2026-09-28-overhaul.md](2026-09-28-overhaul.md).

- Recon date: 2026-09-19, READ-ONLY (no binaries read; sizes/hashes from
  text notes and `ls`/`stat` only).
- Machine: ThinkPad T480, i7-8650U (Kaby Lake-R), MX150 (muxless Optimus,
  iGPU primary), 32 GB DDR4, Artix Linux.
- Brief correction: the text files (`README.md`, `patches.txt`,
  `patches-misc.txt`, `deguard-dug9.txt`) live in `~/firmware/myt480/`, NOT
  `~/build/` (which holds only suckless `dwm-scratch/` + `st/` sources).
- Live firmware per notes: `N24ET74P (1.49)`, stock Lenovo + deguarded ME,
  flashed 2026-09-17, flashprog VERIFIED.

## 1. Workspace map (dir → role)

- `~/firmware/myt480/` — firmware collection root. All SPI images 16 MiB. Flash:
  Macronix MX25L12805D. ME 11.x, LP PCH, 2M SKU. Holds the authoritative
  README, all staged images, the patcher, patch sets, three subdirs.
- `~/firmware/myt480/t480/` — OS tuning docs (not firmware): `README.md` (TLP +
  throttled apply), `t480.md` (deep-tuning research, 2026-03-05:
  Plundervolt caveats, PL1/PL2 targets, Arch stack design, MX150
  integrated-on-battery, validation protocol), `throttled.conf`
  (Core/Cache −80 mV starter), `10-t480-tlp.conf`, two `.html` guides.
- `~/firmware/myt480/thinkpad-firmware-patches/` — upstream collection
  (digmorepaka). T480 set: `xx70_xx80_patches_v7.txt` (Advanced menu,
  iGPU OC, LCDControl, WWAN-whitelist removal). T480 = supported
  ("flash chip next to SOC, don't touch memory settings"), TPM = No.
  Tamper fix: `4C 4E 56 42 42 53 45 43 FB` → `…FF` via hex edit.
- `~/firmware/myt480/thunderbolt/` — 7× 1 MiB Thunderbolt NVM dumps (`tb.bin`,
  `thunder*.bin`, `after.bin`, `null.bin`). Untouched (`after == tb`).
- `~/firmware/myt480/` support files — `UEFIPatch` 0.28.0 (LongSoft, used for all
  images); `patches.txt` (stock sample MSR set, legacy use only);
  `patches-misc.txt` (2 unrelated lines: Gemini Lake SGX, ASUS Z87);
  `deguard-dug9.{pdf,txt}` (3mdeb talk notes).
- `~/firmware/t480-kernel/` (owner's spelling) — custom kernel build output:
  `bzImage` (Linux 7.0.0-rc4, 15 MB), `.config` (148 KB), `System.map`
  (8 MB), all Mar 21 2026, one build session.
- `~/firmware/linux-linuxboot/` — kernel source tree, 7.0.0-rc4 ("Baby Opossum
  Posse"). Version-matches the custom build → almost certainly its
  source; name suggests LinuxBoot-payload interest.
- `~/firmware/lbmk/` — LibreBoot Make, "a coreboot distribution" with automated
  ROM build system. Role: coreboot ROM builder / reference. Ships
  `src/deguard` + `config/deguard` — deguard support integrated.
- `~/firmware/flashprog/` — self-built flashrom fork (binary + source, Mar 20
  2026). Used for every SPI read/write/verify.
- `~/bin/serprog_pico/` — 14 RP2040 `.uf2` firmwares (`serprog_pico` +
  13 Adafruit/other variants). Turns a Pico into the SPI programmer.
- `~/bin/t480_vfsp_16mb/` — 30 prebuilt 16 MB coreboot images, GRUB +
  libgfxinit for this board: `corebootfb` × 15 keymaps + `txtmode` × 15.
  `corebootfb_usqwerty` rebuilt Mar 29 (active keymap; matches the
  `-fb-usqwerty` ROM suffix).
- `~/slock-ly/` — suckless `slock` fork + `pam_auth.diff` (see §6).
- `~/build/` — only suckless X sources. `~/Documents/` — empty.

## 2. deguard + UEFIPatch + MSR-unlock story (from the txt files)

- **Deguar**d (Mate Kukri, 3mdeb DUG#9): bypasses Intel BootGuard on
  SKL/KBL/CFL via CVE-2017-5705 in vulnerable ME 11.x — downgrade ME
  over SPI; vulnerable BUP shadows BootGuard fuses in SRAM each boot
  (fuses untouched). `rdmsr 0x13a` non-zero = enforcing; live box `0x0`.
  Out-of-box on T480 + Dell OptiPlex 3050; enabled the T480(s) coreboot
  port. Flow: read SPI → `generatedelta.py` → donor ME 11.6.0 →
  `finalimage.py` + fake FPFs → `ifdtool` → flash back.
- **Owner recipe** (`~/firmware/myt480/README.md`): Dell Inspiron 5468 donor
  `me.bin` (ME 11.6.0.1126 LP 2M, `/home/bup/ct` byte-exact, `eom=00`)
  via `ifdtool -p sklkbl -i ME:me.bin`, then `-M 1` (HAP bit,
  `0x102: 0x50→0x51`). Diff ≈ 838 KB ME-only; BIOS/GbE/FD identical.
  `stock-deguarded-t480.bin` (md5 `593d86ac…`) is LIVE; ME hidden (no
  `00:16.x` PCI, no `/dev/mei*`). Unsigned coreboot proven bootable
  (B66 ran pre-stock-flash); reflash re-establishes the base anytime.
- **MSR unlock** (built, NOT flashed): UEFIPatch 0.28.0 + one KBL SiInit
  line (`299D6F8B… P:81E10080000033C1:9090909090909090`), tamper fix
  `0x89D008 FB→FF`. `stock-deguarded-msr-unlock.bin` (md5 `3f2e3c59…`).
  Expected: `rdmsr 0xE2` `0x1e008008 → 0x1e000008`. Does NOT open
  internal flashing (SMM/PR locks independent).
- **Full unlock v2** (built, NOT flashed — USE, not v1): +7 upstream
  lines (FormBrowser ×2 text-Advanced, Setup iGPU-freq + LCDControl,
  WmaPolicy ×3 WWAN-whitelist). v1 SUPERSEDED: review caught UEFIPatch
  pad-destroy wiping 1301 B (KM + BPM manifests); v2 restores byte-exact
  (md5 `82678590…`).
- **Legacy (do NOT flash)**: `patched.bin` = `backup-after-patch.bin`
  (old generic-`patches.txt` run, ME untouched, IBB mod under enforcing
  BootGuard); `deguard-test.bin` (name lies — ME/FIT/MN2 pristine,
  BIOS-only delta). **Gold pair**: `backup.bin` + `backup2.bin`,
  byte-identical pristine (md5 `012040d8…`, ME 11.8.60.3561) — restore
  point only.
- **Flashing reality**: external SPI (`flashrom -p ch341a_spi -w`),
  dump-twice + md5 first, cold off-machine backup. Internal works only
  under coreboot; stock SMM blocks it (BIOS_CNTL `0xA2`, PR0/PR1
  latched, FLOCKDN; S3-resume bypass tried twice = dead end). Staged
  NVRAM path: `setup_var.efi` on USB ESP → `PchSetup 0x17` (BIOS Lock)
  + `0x612` (FPRR) + `CpuSetup 0x3C` (CFG Lock) → warm-reboot, flash
  before cold boot.

## 3. ROM naming convention + experiment pattern (66 `*.rom` in `~`)

- **B-series** (B39–B65, Sep 15 single-day sprint, all 16 MiB, common
  `-fb-usqwerty` = framebuffer console, US-QWERTY):
  - Timing phase (B39–B55): `950ms-shift-infinite` → `ps2fix` →
    `2s-sleep-interrupt` → `1s-clean-wait` → `zero-wait-shift-latch` →
    `500ms-sleep-shift/fixed` → `precheck/warmup/posix-loops` → a
    `250/400/450/465/475/500ms-clean` convergence sweep →
    `500ms-zero-menu`. A binary search for minimal reliable bootmenu
    wait, converging at 450–500 ms.
  - Payload phase (B56–B65): `tableos-500ms`, `diet-audio`,
    `tableos-5s`, `fixedaudio`, `grubfix`, `menuonly-750ms`,
    `notempledoom-lite`, `scale5-sound`, then `pristine-tint`,
    `tint-scale2/centered`, `tintelf-doomscale2`. Menu/audio/display
    iteration on coreboot+GRUB.
  - B66 (referenced, since removed) was the unsigned-coreboot proof.
- **Safety discipline**: same-day `pre-Bxx-backup-<timestamp>.rom`
  readback per build (B39–B65, few gaps). B39–B55 are mode `600 root`,
  B56+ `644 root` (workflow change ~18:00 mid-sprint).
- **Prior generation** (Sep 11 era): `bios.rom`/`bios2.rom` (Mar 20,
  likely original stock), `flashed-20260911-1841-b7fd49cc.rom`,
  `pre-B1/B7/fixall/flash/mainstream/revertB52`.
- No flash scripts or notes beside the ROMs — procedure lives in
  `~/firmware/myt480/README.md`; no `~/*.sh` is firmware-related.

## 4. Kernel situation

- **Custom build staged, not booted**: `bzImage` = 7.0.0-rc4
  (`-g8a30aeb0d1b4`, `#3 SMP PREEMPT_DYNAMIC`, gcc 15.2.1). Config +
  symbols installed (`/boot/config-t480`, `/boot/System.map-t480`) but
  **no `vmlinuz-t480`** — not yet wired into GRUB.
- **Bootable**: `vmlinuz-linux` (Sep 11) + `vmlinuz-linux-rt` (Sep 12),
  each with initramfs, plus `intel-ucode.img`. Brief says RT
  (7.2.5-rt3-arch1) is default; `grub.cfg` is root-only, unverified.
- Source `~/firmware/linux-linuxboot/` (7.0.0-rc4) version-matches the build.

## 5. Flashing setup (serprog_pico)

- Pico/RP2040 + `serprog_pico` firmware (14 `.uf2` variants kept) =
  USB SPI programmer; `~/firmware/flashprog/flashprog` (self-built Mar 20)
  speaks to it. Target: MX25L12805D 16 MiB, chip next to SoC.
- Loop: double-dump + md5 → modify → full-16 MB external write →
  verify. Internal flashing only under coreboot.
- Ready inventory: 30 keymap-variant coreboot images +
  4 staged stock-based images (1 live, 3 pending).

## 6. slock-ly custom locker

- suckless `slock` + `pam_auth.diff`: links `-lpam`, authenticates via
  the `login` PAM service instead of shadow; adds 4th state `PAM` in
  purple (`#9400D3`, "waiting for PAM"); keeps `failonclear = 1` and
  `nobody:nogroup` privilege drop.

## 7. How firmware state enables undervolt

- Chain: **deguard live** → BootGuard neutered → **MSR CFG-lock NOP
  (built, pending flash)** → `MSR 0xE2` clears → offsets writable →
  **`throttled`** applies Core/Cache −80 mV starter, stepping −10 mV
  toward ≈ −100…−130 mV, PL2 44 W / PL1 22–25 W, Tau 28 s.
- Why: Plundervolt mitigation (SA-00289) locks undervolt out — the
  `~/firmware/myt480/` pipeline (gold pair → deguard base → MSR unlock → full
  unlock) re-opens it. OS side staged; only the reflash is pending.
- Corroboration (2026-09-18): `0xE2=0x1e008008` (lock still set —
  MSR image unflashed), `0x3A=0x40005`, `0x13A=0x0`; disk has no ESP,
  boots USB. MX150 under coreboot stays offload-only (needs
  RP01.PEGP ACPI + BAR work).

## 2026-09-30: linux-rt replaced by linux-lts
- `pacman -Syu` (98 pkgs; log /root/pacman-syu-2026-09-30.log), then `linux-lts` + `linux-lts-headers`
  6.18.54-1 installed and `linux-rt` removed (log /root/pacman-lts-2026-09-30.log). nvidia 580 DKMS
  built for lts. `linux-lts.preset` uses the default /etc/mkinitcpio.conf (udev), as rt did.
- Disk `/boot/grub/grub.cfg`: `linux-rt` → `linux-lts` by sed (backup `grub.cfg.bak-pre-lts`), not
  regenerated (disk GRUB core is still 2.14; package is 2.16).
- Firmware menu: `site-local/grub.cfg` entry changed to `Artix Linux (linux-lts) [L]` in the source
  only. NOT rebuilt or flashed: the flashed payload still has the `[R]` entry pointing at the removed
  `/boot/vmlinuz-linux-rt`. Default `[t]` linux-t480 and `[l]` stock are unaffected.
  (Rebuilt and flashed later that day: firmware C48 and up carry the `[L]` entry.)
