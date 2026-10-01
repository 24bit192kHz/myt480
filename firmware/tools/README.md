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
| `bench-all.sh` | the benchmark set used before and after the undervolt |
| `hib-measure2.sh` | hibernate cycle timing |
