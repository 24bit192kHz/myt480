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
