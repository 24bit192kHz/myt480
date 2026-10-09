# 2026-10-08 port fixes

OS side (syswork wake-ac, /etc commit 783f3ad):
- /etc/elogind/system-sleep/02-sleep-guard: on battery, GLAN wake (Wake-on-LAN, PCI 00:1f.6
  power/wakeup) is disabled before every sleep and restored after the wake, as Lenovo's
  "Wake On LAN: AC Only" default (stock think-lmi dump). RP09 (Thunderbolt) stays armed: stock
  has "Wake by Thunderbolt: Enable". Reason: 9 %/day drain in hibernation reported by battwatch;
  the control is one night in plain poweroff (S5) to see the EC + clone-pack baseline.
- /etc/local.d/lockup-detect.start: kernel.watchdog, soft/nmi watchdog, softlockup_panic and
  hardlockup_panic on (the firmware cmdline has nowatchdog nmi_watchdog=0; the TCO only fires
  when the keepalive dies), and msr allow_writes=on (thermald-t480's mailbox and TCC writes
  tainted the kernel and spammed dmesg on every resume). Applied live too.
- `dgpu on`: the MX150 had been off in firmware since the 2026-10-05 power-button hold.

Firmware C55 (workstation branch c55): c54 rebased on coreboot main b5fbbcff8a (2026-10-08),
libgfxinit patches rebased on 2c446dbc; local SMBus and CBET4000 commits dropped (upstream has
64579f74e4 and 28cfb2125d); planned: power-button override = one-shot boot without the GPU
(option kept), lid wake GPE 0x17 as the vendor DSDT (_PRW {0x17, 4}; GPP_C23 EC_WAKE),
gpe0_dw0..2 = GPP_C/D/E explicitly (the live PMC GPIO_GPE_CFG is 0x432 already).

## 14:15 C55 built and gated, NOT flashed
- Workstation tree branch c55 = 197b599dbd (28 commits on coreboot main b5fbbcff8a of 2026-10-08;
  libgfxinit t480-c55 cc48be1 on upstream 2c446dbc); fetched into the T480 tree as the same
  branch names. site-local unchanged (014c5bd). `make olddefconfig` only added
  SOC_INTEL_COMMON_BLOCK_GPIO_LOCK_USING_SBI=y (upstream select).
- roms/C55.rom sha256 63c5ddbe...b1576 on both machines; romcheck OK; QEMU gate: A1-A3 boot,
  B/C/E menu, D fails by design, flag test pass; DSDT has LID _PRW {0x17, 4}; static.c has
  gpe0_dw0..2 = GPP_C/D/E.
- Flash when the user is at the machine and on AC: `sh ~/t480-build/work/flashrom.sh C55`
  as root (does t480-reseal next-boot, then a cold boot via RTC; the RTC alarm does not wake
  from S5 on battery, so on battery use flashrom-warm.sh). After the boot: verify48.sh C55,
  cbmem -c must not show "Duplicate GPE DW", `dgpu status`, lid-open wake from S4 test
  (rtcwake -m no + loginctl hibernate, then open the lid), a 4 s power hold must boot without
  the GPU once and keep `dgpu status` = on.
- myt480 is not updated for C55 yet (patches must be regenerated against the new base).
- /usr/local/bin/dgpu knows the one-shot byte (status line); /usr/local commits f06c0c8, 4570611.

## 14:18 C55 FLASHED (warm reboot, on battery, user's go) and verified
- flashprog VERIFIED 14:17:58, backup roms/backup-before-C55.rom. Boot 14:18: TPM key released with the
  one-shot blob, kmk.blob resealed 14:18:18 to the new PCR 2 B5B658BF...; coreboot 755 ms to payload;
  no "Duplicate GPE DW" line; dGPU enabled, D3cold when idle, prime-run glxinfo renders on the MX150,
  5/5 D3cold cycles; thinkpad_acpi "BIOS CBET4000 t480, EC N24HT37W-3.36"; LID _PRW is gpe17 now.
- verify48.sh run as root lost its ssh-filled fields (it expects to run as btw); checked by hand.
- TLP turned the NMI watchdog off again (/etc/tlp.conf NMI_WATCHDOG=0 -> 1, syswork tlp-nmi).
- myt480 28fbdd9 pushed: patches 0001-0028 on main b5fbbcff8a, libgfxinit on 2c446dbc, READMEs,
  system files, this note.
- Still for the user: lid-open wake from hibernate, a 4 s power hold (must boot once without the GPU
  and keep `dgpu status` on), one night in plain poweroff for the drain baseline, fallback GRUB entries.

## 14:35-15:05 lid wake from S4: what the EC does under coreboot
- With C55 (_PRW GPE 0x17 = EC_WAKE# on GPP_C23, armed: gpe0_en[0] 0x00800000) a lid open in S4
  does nothing: gpe0_sts[0] shows only bit 22 (EC SCI, GPP_C22) latched, never bit 23. Pad config
  of GPP_C22/C23 is byte-identical to stock (inteltool). In S3 the lid wakes by a power-button
  pulse from the EC (PM1_STS WAK PWRBTN, no GPE armed for it).
- Stock arms the EC for S4/S5 wake in SMM at sleep entry (OPTS: SLTP = SMI 0x05, AWON(4) =
  SMI 0x12); the ACPI-visible EC writes (HWLO = 0x32 bit 2 = h8 WKLD, HWLB, HCMU) match coreboot.
  EC RAM 0x3d is 0x5f on stock and 0 on coreboot; writing 0x5f (ectool -w 0x3d -z 0x5f) did not
  help and the EC reset it to 0 across the S4 cycle. Dead end.
- 14:55:33 "crash": TCO reset exactly 30 s after "Performing sleep operation 'hibernate'": the
  keepalive is frozen while the image is written. 02-sleep-guard now disarms the TCO in pre
  (hibernate only, marker /run/sleep-guard.wd-off) and post re-arms it (syswork wd-hib);
  exercised with pre/post by hand and by a real hibernate at 14:59 (image restored, re-armed).
- C56 = C55 + LID _PRW GPE 0x16 (the EC SCI GPE armed as a wake GPE for S3/S4). To check after
  the flash: hibernate + lid open; S3 + plug/unplug the charger (spurious wake?); S3 + lid.

## 15:22 lid wake from S4: not achievable without the vendor SMM; back on C55
- C56 (LID _PRW GPE 0x16) and C57-exp (0x16 + 0x17 both armed, gpe0_en[0] 0x00c00000, EC 0x3d
  written to 0x5f right before the sleep): hibernate + lid open = nothing, no GPE latched, PM1
  WAK PWRBTN only after the user's press. gpe0_sts[0] bit 22 is set on EVERY boot (EC SCI at
  power-on), so it was never evidence of a lid SCI in S4. The EC clears 0x3d by itself.
- Conclusion: the T480 EC does not treat a lid open as a wake event in S4/S5 unless Lenovo's SMM
  (SLTP = SMI 0x05, AWON = SMI 0x12 at _PTS) arms it; the ACPI-visible EC bits (HWLO/WKLD,
  HWLB, HCMU) and the PCH side (pads identical to stock, GPEs armed) are not enough. In S3 the
  EC pulses PWRBTN# on lid open, which is why S3 lid wake works. Lenovo's own BIOS has no
  "wake/power on by lid" setting on this model (think-lmi dump), so the vendor _PRW {0x17, 4}
  is likely boilerplate. Not pursued further (would need the SMM handlers reverse-engineered).
- Final state: C55 flashed back 15:22 (PCR 2 B5B658BF..., resealed), lid _PRW {0x17, 4} kept as
  the vendor's; experiment branches c56 / c57exp and roms C56.rom, C57-exp-bothgpe.rom kept.
  The sleep hook keeps: WoL AC-only, TCO disarm around hibernate (the real fix of the day).

## 15:25-15:45 stock SMM analysed (workstation /mnt/ssd-cachy/t480-smm, UEFIExtract + capstone)
- ACPI `SMI(cmd, ...)` = mailbox at MNVS+0xFC0, SW SMI 0xF5, handled by module SmmAslSmi
  (jump table at RVA 0x3c40). Case 0x12 (AWON) is `xor eax,eax; ret`: a no-op. Case 0x05 with
  PAR0=0 (SLTP) only sets SMI_EN.SLP_SMI_EN (PMBASE+0x30 bit 4) and clears SLP_SMI_STS: it arms
  the sleep SMI. The real sleep work is in module SmmSleepEvent (SLP_EN trap):
  - S4 handler: GetVariable(Setup/LenovoConfig); unless NVS WIWK (0xD8B bit 0) or a setup bit
    says otherwise, it rewrites PM1_CNT SLP_TYP to S5 (0x1c00) and writes CMOS 0x74 = 0x55:
    Lenovo hibernates as S5 by default. In the real-S4 branch it sets EC indexed register 0x41
    bit 2 (ports 0x15EC index / 0x15EE data, decoded by coreboot's gen2_dec 0x15E0-0x15EF),
    EC RAM 0x3a |= 0x20 unless WLAC (WakeOnLAN) == 2, EC RAM 0x3b &= ~0x10, and the helper
    0x12ec sets 0x41 bit 1 from NVS NPME (0xD1A bit 4) and clears idx 0x26 bit 2.
  - S5 handler: idx 0x41 &= ~4, EC RAM 0xcf &= ~0x40.
  - S3 handler: idx 0x41 |= 4 under the same flag.
- Replicated the real-S4 EC state from the sleep hook (idx 0x41 = 0x0f, bits survive the
  S4 cycle) + HWLO via _PSW: hibernate + lid open still nothing (15:33-15:38). Nothing in any
  handler is lid-specific. CONCLUSION: the T480 cannot wake from hibernation by lid open, on
  Lenovo's firmware too (it is S5 there by default, and the EC does not treat the lid as an
  S4 wake even when armed as Lenovo arms it). Closed.
  SUPERSEDED 2026-10-09: this was true of the host side only. The EC firmware itself has the
  path, behind a flag Lenovo never sets (EC RAM 0x01 bit 6); set from a sleep hook it works,
  see 2026-10-09-evening.md and research/2026-10-09-t480/10-ec-firmware-and-lid-wake.md.
- Tool kept: ~/t480-build/tools/ecidx.py (read/setbit/clearbit of the 0x15EC index space).
  Sleep hook restored (no experiment lines), idx 0x41 back to 0x09.
