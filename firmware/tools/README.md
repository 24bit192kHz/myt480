# Tools

| File | Use |
|---|---|
| `bootmenu` | sets CMOS 0x6e so that GRUB shows its menu at the next boot, and reboots. Only argument: `--no-reboot`. Any other argument also reboots |
| `t480-init` | PID 1 wrapper: mounts `/proc` and `/sys`, then starts s6. Needed because the kernel boots without an initramfs. Install as `/usr/local/sbin/t480-init` |
| `mx150-revert.sh` | removes the NVIDIA driver setup |
| `tscmono.c`, `pm_ab.py` | boot time measurement |
- `romcheck.sh`: refuses a ROM whose GRUB payload does not carry `site-local/grub.cfg` (run before every flash).
- `boot-times.sh`: one line of boot times for the current boot.
