# Tools

| File | Use |
|---|---|
| `bootmenu` | sets CMOS 0x6e so that GRUB shows its menu at the next boot, and reboots. Only argument: `--no-reboot`. Any other argument also reboots |
| `t480-init` | PID 1 wrapper: mounts `/proc` and `/sys`, then starts s6. Needed because the kernel boots without an initramfs. Install as `/usr/local/sbin/t480-init` |
| `mx150-revert.sh` | removes the NVIDIA driver setup |
| `tscmono.c`, `pm_ab.py` | boot time measurement |
| `boot-times.sh` | one line of boot times for the current boot |
| `romcheck.sh` | refuses a ROM whose GRUB payload does not carry `site-local/grub.cfg`; run it before every flash |
| `cmos6d.py` | shows CMOS 0x6d, the "GPU reset tried" flag of coreboot patch 0017 (0 after a good boot) |
| `coldboot.sh [on\|off] [seconds]` | sets the `dgpu` request, arms an RTC alarm and powers off: a cold boot without touching the power button (root) |
| `undervolt/` | the CPU undervolt sweep kit; its README explains the runs |
| `gpu/` | the MX150 clock offset kit (sweeps, load and bandwidth tests); its README explains the runs |
| `bench-all.sh` | the benchmark set used before and after the undervolt |
| `hib-measure2.sh` | hibernate cycle timing |
| `flashrom.sh NAME`, `flashrom-warm.sh NAME` | flash `roms/NAME.rom` (FMAP + COREBOOT) after `romcheck.sh` and `t480-reseal next-boot`, then cold boot (RTC alarm, needs AC) or reboot. Root, detached |
| `bootmeasure.sh LABEL` | one line per boot: firmware time, GRUB kernel load, power-on to chadwm, PCR 2 |
| `idlepower.py LABEL [s]` | idle power of the current state: battery draw, RAPL, package C-states, GPU and root port power state |
| `speedgate.sh` | raw partition against LUKS2/dm-crypt with `fio`, interleaved; it was run on the swap partition before the disk was encrypted |
| `qtest52.py` | QEMU scenarios for the GRUB boot policy: bad, missing and tampered signatures, password prompts; exits1 on a failed scenario and2 on an unknown selector |
