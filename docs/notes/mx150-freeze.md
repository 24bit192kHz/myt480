# MX150: silent hard freezes with the NVIDIA driver loaded

## 2026-09-30

- Freezes 1-2: battery, `gpu-power on`, nvtop polling (C45/C46 firmware). Also hit twice by
  an mpv benchmark that went to the GPU by itself (nvdec profile + CUDA crop sidecar; mpv
  config fixed since, plain mpv stays on the Intel GPU).
- C46 (no L1 substates on RP01) and C47 (no ASPM on RP01): **no effect**. C47 froze at
  09:06:46 in a scripted test (`~/.cache/mpv-bench/freeze-test.sh`): driver loaded 09:05:04,
  nvidia-smi poll 1/s + nvtop + renders on the GPU in an unmapped window; renders 1-4 fine,
  freeze at the moment render 5 (720p) ended, 102 s after the driver load.
- Suspect now: runtime D3. The driver default `NVreg_DynamicPowerManagement=3` suspends an
  idle GPU (video memory < 200 MB) to D3cold through the coreboot power resource `PGPU` on
  RP01 (`acpi/dgpu.asl`: link disable, reset, rail off / on) and every poll wakes it again.
  Not proven: no log survives a freeze (kernel has no ramoops).
- Change: `/etc/modprobe.d/nvidia-rtd3.conf` sets `NVreg_DynamicPowerManagement=0x00`
  (syswork `gpu-rtd3-off`, applied). GPU stays in D0 while the driver is loaded; unloading
  the driver still powers it off. **Not yet tested**: needs `dgpu on` + reboot first (the
  4 s power hold cleared CMOS 0x6f).
- Side finding: with the GPU disabled in firmware, 0000:01:00.0 is the Wi-Fi card and
  `gpu-power status` wrongly prints "GPU on" (it only reads power_state, not the vendor).
- 09:15 test with runtime D3 off (`Runtime D3 status: Disabled`, runtime_status stayed
  `active`, logged every 0.5 s): **froze again**, 09:17:38, 101 s after the driver load and
  9 s into render 5 -- the same point as the C47 run (102 s, render 5). So neither ASPM nor
  runtime D3. No kernel message in /var/log/kernel before either freeze. Same script both
  times, so "time since driver load" and "fifth render" are not separated yet. All freezes
  so far were on battery. Next: the same test on AC, with battery voltage/power, CPU temp,
  nvidia-smi and dmesg logged and synced (`freeze-pwr.log`, `freeze-smi.log`,
  `freeze-dmesg.log`). `nvidia-rtd3.conf` is still in place (kept to change one thing at a time).
- 09:26 two-phase test (battery, RTD3 off): 180 s of polling without rendering passed;
  froze 09:31:01, 85 s into the render phase, 9 s into render 4 (1080p, GPU 51 %, 56 C,
  213 MiB). So it needs GPU load, not time. Battery at the freeze: BAT1 11.18 V, 28.8 W, flat
  for the last seconds (no sag); CPU 46 C. Not thermal, not a collapsing pack.
- Root port 00:1c.0 has SERR+ in Command and BridgeCtl (coreboot default). Suspect: a link
  error under load -> SERR -> NMI/SMI nobody clears -> silent stall. Next run: AER counters
  logged every 0.5 s (`freeze-aer.log`), link speed logged, and SERR cleared on the port and
  GPU at runtime (`SERR_OFF=1`), so an error would reach the kernel log instead.
- 09:44 test with SERR cleared on 00:1c.0 and the GPU (setpci, verified) and AER counters
  logged: froze 09:44:56, 5 s into render 2, the first real GPU load of that run. All AER
  counters 0 to the last sample, link 8 GT/s x4. So no counted link error and SERR is not
  the path. Freeze point varies (render 2, 4, 5, 5).
- The lockup detectors were never on in any freeze: the firmware GRUB cmdline has
  `nowatchdog nmi_watchdog=0`, and `sysctl -w kernel.nmi_watchdog=1` alone does nothing while
  `kernel.watchdog` is 0 (and reading a watchdog sysctl writes the effective state back, so
  order matters). "The hard-lockup detector did not fire" in the 08:00 handoff is therefore
  no evidence for an all-CPU/SMM stall. A driver spinning on one CPU with IRQs off would
  look the same. Working sequence (verified, dmesg "NMI watchdog: Enabled"):
  `sysctl -w kernel.watchdog=1 kernel.nmi_watchdog=1 kernel.soft_watchdog=1`.
  The kernel has CONFIG_DRM_PANIC (kmsg screen), so a lockup panic is shown on the panel.
  Next run: `WD=1 ./freeze-test.sh 600` (panic on hard/soft lockup and unknown NMI, panic=60).
- 09:51 test with the lockup detectors on (readback 1 1 1, hardlockup_panic=1, thresh 5,
  panic=60, "NMI watchdog: Enabled"; i915 registers drm panic planes): froze 09:52:22, 5 s
  into render 3. The owner saw **no panic screen** in ~75 s and held power. So no CPU could
  take an NMI or run the soft-lockup timer: the stall is below the kernel.
- Read-only look at the chipset (GPU absent): SMI_EN=0x10000033 (global, EOS, SLP, APMC,
  eSPI; no TCO, no GPIO SMI), NMI_SC=0x24 (SERR NMI off), TCO timer halted, SMI count 4 since
  boot, no MCE lines. Few SMI sources, so an SMI storm is not obvious; the test now logs the
  SMI count (MSR 0x34) every 0.5 s (`freeze-smicount.log`).
- Battery voltage at the freezes: 11.18 V / 11.24 V / 10.86 V on BAT1 (clone 45N1057, 55-66 %),
  idle 11.5 V. All freezes ran on BAT1 alone. Untested: AC, and BAT0 (genuine) alone.
- 10:06 test on BAT0 alone (BAT1 pulled), detectors on: froze 10:08:28, 2 s into render 7
  (longest run so far, 6 renders). BAT0 dipped to 9.3-9.75 V at 13-19 W several times
  without a freeze and read 10.5 V at the freeze: battery voltage does not line up with the
  freezes, and both packs freeze. (BAT0 sags a lot for 85 %: weak pack, separate matter.)
  SMI count stayed 4 to the last sample, AER 0. No panic screen again.
- Still possible: an SMI storm that starts at the freeze (the logger gets no CPU after that).
  SMI_LOCK is set (GEN_PMCON_A e08432f0) so GBL_SMI_EN cannot be cleared, but the single
  enables are writable (tested bit 28). Next run `SMI_OFF=1 WD=1`: clears SLP/APMC/eSPI in
  SMI_EN and PMME/HPME in the root-port MPC (0xd8), and saves the RP1 config dump
  (`rp1-config-*.txt`) from a GPU boot.
- 10:17 test with every reachable SMI source off (SMI_EN 10000033 -> 00000003; RP1 MPC
  099e0008 already has PMME/HPME clear), detectors on, both batteries in: froze 10:18:28,
  8 s into render 4 (GPU 55 %, 46 C, system 27.6 W, peak 35.5 W in render 3). SMI count 4,
  AER 0, runtime PM active. RP1 dump from a GPU boot: `~/.cache/mpv-bench/rp1-config-1017.txt`
  (8 GT/s x4, ASPM not supported, completion timeout 50us-50ms enabled, no hot-plug).

### Where this stands (2026-09-30 10:20)

Eight scripted reproductions, all on battery, all a silent stall 2-120 s into real GPU load.
Ruled out by test: ASPM/L1 substates (C46/C47), runtime D3, time since driver load, battery
sag and the clone pack, SERR, counted PCIe errors, kernel lockup (no panic with detectors
on), SMIs. Not tested: AC power; light GPU load (timed playback); nouveau.

Working theory (not proven): the GPU itself stops under load while its link stays up, and
the stalled PCIe traffic backs up through the PCH until every core that touches it blocks.
On Lenovo firmware the EC is told about the GPU and the GPU gets an AC/battery power level;
coreboot does neither (`nvidia-smi`: 5001 W limit, "SW thermal slowdown active" at idle), so
on battery the GPU may run past what its supply allows. First test when a charger is at
hand: the same script on AC. Firmware side: compare GPP_F1 GC6_FB_EN / GPP_F2 -GPU_EVENT /
EC GPU bits and the power sequence against the vendor firmware and the schematic.

State left: `dgpu` off in CMOS after the last hold (no NVIDIA device, no freeze risk);
`/etc/modprobe.d/nvidia-rtd3.conf` still sets DynamicPowerManagement=0 (disproved as the
cause, kept so later tests change one thing; remove to get runtime D3 back); test kit in
`~/.cache/mpv-bench/` (`freeze-test.sh DUR [IDLE]`, env WD=1 SMI_OFF=1 SERR_OFF=1).

### 2026-09-30 10:45: stock firmware flashed for a control run

Owner's call. `~/firmware/myt480/stock-deguarded-flashopen.bin` (stock N24ET74P + deguarded ME,
as run on 2026-09-17, plus PchSetup BIOS Lock and FPRR cleared in the NVRAM store) written
full-chip from coreboot C47, VERIFIED, readback identical. Live C47 chip saved before:
`~/firmware/roms/backups/pre-stock-C47-live-20260930.rom`. Coreboot baseline dump (GPU off):
`~/t480-build/probe/coreboot-C47-nogpu/`; same script for stock: `~/t480-build/probe/probe.sh`.
Disk GRUB (MBR) checked in QEMU before the flash. Machine shut down after the flash.

### 2026-09-30 11:05: it is not coreboot -- stock freezes too; CPU undervolt is the suspect

- Control run on stock N24ET74P (flashopen image; flash is still locked, the two NVRAM bytes did
  not take): same failure at 10:55:47, 6 s into render 4. With the detectors on, the owner saw
  the Caps Lock LED blinking and a dark screen = kernel panic (rebooted by itself after 60 s).
  So the earlier "stall below the kernel" reading was wrong: the DRM panic screen does not
  show here; on coreboot the power button was held before the 60 s ran out.
- The machine then crashed again at ~11:00 on stock with the NVIDIA driver not loaded, while
  only shell commands were running.
- Found: thermald-t480 applies a CPU undervolt on every boot, on both firmwares (MSR 0x150
  read back: core -130 mV, cache -130 mV, iGPU -80 mV, uncore -80 mV; OC lock bit clear),
  plus IccMax 40 A core and PL1 15 W / PL2 25 W on battery. /etc/thermald.conf has had
  uv_core/uv_cache -130 since the 09-19 baseline; 09-28 commit already records "77 i915 GPU
  hangs at -120" for the GPU plane. Every freeze was on battery and at a load change.
- 11:05: undervolt set to 0 for this boot (syswork `uv-off`, shadowed over /etc/thermald.conf,
  thermald restarted, MSR read back 0.0 mV on all planes). The shadow is gone after a reboot,
  so the undervolt comes back until the overlay is applied. Freeze test without undervolt: pending.
- 11:06-11:17, stock firmware, undervolt 0 (`/etc/thermald.conf` uv_* = 0, applied through
  syswork `uv-off`; previous file kept as `/etc/thermald.conf.bk-uv130`), battery, detectors on:
  **PASSED** 60 s polling + 600 s, 40 renders, GPU up to 63 C, no AER, no panic. Every one of
  the nine runs with the undervolt failed within 2 minutes of rendering. Cause: CPU undervolt
  (-130 mV core/cache) from thermald-t480, not coreboot, not the MX150. C46/C47 (ASPM) and
  `nvidia-rtd3.conf` were never needed. Still to confirm under coreboot once it is flashed back.
- 12:45: the crashes at 11:01 and 11:27 on stock are explained: both hit while an agent dump
  script was reading chipset sideband (PCR) ports raw through /dev/mem (partial output dirs
  ended at pcr-bd / pcr-ef). Not a hardware or undervolt problem. The complete dump was redone
  with inteltool only: `~/t480-build/probe/stock-full-ac-gpuoff`, `stock-full-ac-gpuon`
  (index in `~/t480-build/probe/README.md`). The 11:22 display freeze was `i915 GPU HANG in
  Xorg` from an uncapped off-screen NVIDIA benchmark; the system stayed up.
- 12:45-12:55, stock, AC, undervolt 0: PASSED 600 s, 30 renders, GPU max 72 C, CPU up to 88 C,
  no AER, no panic. Battery and AC both pass without the undervolt.
- 12:59: `/etc/modprobe.d/nvidia-rtd3.conf` removed (owner's call): runtime D3 is back to the
  driver default. syswork bug seen: deleting a file in an overlay and applying it copies the
  overlayfs whiteout (char device 0,0) to the live path instead of deleting the file; the node
  was removed by hand as root and the deletion committed in /etc.
- 13:00-13:12, stock, AC, undervolt 0, no nvidia-rtd3.conf (driver reports "Runtime D3 status:
  Disabled by default" on stock ACPI): PASSED 60 s idle + 600 s load, GPU max 72 C, no AER,
  no lockup. One of 22 renders hung until its 60 s timeout with `Xid 13` (graphics exception in
  mpv's vo thread); the same happened once in the 12:45 AC run. System unaffected. Complete
  dump afterwards: `~/t480-build/probe/stock-full-ac-gpuon-2`.
- 14:44, stock, AC (BAT1 charging at 17 W, thermald AC limits PL1 64 W / PL2 90 W): under GPU
  load the MX150 is power-capped on and off (`clocks_event_reasons` 0x4 = SW power cap, clocks
  590-1770 MHz, utilisation 96 %), and the battery charge rate falls to 0. On battery the same
  load ran a steady 1771 MHz at 54 % with no cap. Samples with EC RAM and GPIO every 2 s:
  `~/t480-build/probe/stock-ac-load-samples` vs `stock-batt-load-samples`. Reading: the charger
  cannot cover CPU + GPU + charging, and Lenovo's firmware caps the GPU; coreboot has no such
  path, so compare the EC bytes that differ between the two sample sets.
- 15:05-15:12, stock, battery, lid closed: on-screen `prime-run mpv` (1080p24, 160 s): MX150
  renders, 0 dropped / 0 delayed frames, GPU 17 %, no Xid. S3 suspend (deep) with the NVIDIA
  driver loaded and RTC wake after 50 s: resumed cleanly, GPU renders afterwards, GPU-related
  GPIO pads unchanged. Dump: `~/t480-build/probe/stock-full-batt-resume`.

### 2026-09-30 15:47-17:00: back on coreboot, C48/C49 verified

- The owner reflashed C35 with the SPI programmer; C48 (= C47 + Max_Payload_Size 256 on the
  GPU, Wi-Fi and SSD root ports, the vendor firmware's values) and C49 (+ SMBIOS BIOS version
  "CBET4000 t480") were flashed internally and tested over RTC-alarm cold boots
  (`~/t480-build/work/coldboot.sh`; RTC wake from S5 works: `PM1_STS: WAK RTC`).
- Freeze test on coreboot with the undervolt at 0: PASSED 600 s, 47 renders, 0 Xid, 0 AER,
  GPU max 72 C. Same as on the Lenovo firmware: the undervolt was the whole story.
- gpu-power: `off` now sets the GPU's runtime PM control to `auto` (PCI default `on` kept the
  port awake, so no D3cold after a fresh boot); `status/on/apply` check the PCI vendor.
- thinkpad_acpi now reads the EC ("EC N24HT37W-3.36") thanks to the CBET4000 BIOS version,
  so the thinkpad hwmon exposes temp1 CPU / temp2 GPU; thermald-t480 uses the hotter of the
  two for the fan (`gpu_temp_path auto|off|<file>`).
- Stock-vs-coreboot summary for the MX150: ASPM off on the GPU port (as vendor), payload 256
  (as vendor); vendor's NVOP/GPS/GC6/MXM ACPI and the subsystem-ID rewrite are not needed by
  the Linux driver; the EC reads the GPU temperature itself on both firmwares.

### 2026-09-30 17:00-18:00: the rest of the stock comparison, fixed (C50 + kernel 7.2.8-3)

- Bluetooth was hard-blocked under coreboot (the generic ThinkPad ASL read a wireless-switch
  EC bit the T480 does not have); C50 answers WLSW = on like Lenovo. Wake-on-LAN, wake from the
  Thunderbolt port and from the lid in S4 are back; no bogus tablet switch; subsystem IDs
  17aa:225d on all PCH devices; GMM 00:08.0 on.
- Kernel 7.2.8-3: TPM 2.0 (`tpm0`) and the PCH TCO watchdog, armed at boot by
  `/etc/local.d/watchdog.start` (30 s, petted every 5 s). Disarm with
  `pkill -TERM -f 'watchdog-keepalive$'`; never start the loop from an ssh shell that may be
  cut (it died without the magic close twice while being set up and the machine reset 30 s
  after the next resume, "TCO_STS: SECOND_TO" in cbmem).
- S3 tests: run `loginctl suspend` as root; the earlier user-level runs from detached scripts
  never suspended. Verified on C50 + 7.2.8-3 with the watchdog armed.
