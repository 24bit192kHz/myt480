# Notes

The [T480 improvement wiki](wiki/README.md) records the 2026-10-09 live audit,
reversible thermal/GPU changes, USB-C and fingerprint reverse engineering, tests,
and restoration commands.

Written while the system was built; they explain decisions, not commands. The dated
notes record the state of that day, and later notes or the READMEs supersede them. Paths
such as `~/t480-build/`, `~/systemagent/` or `/root/PRE-*.rom` in them are on the
author's machine; their sources are in this repository under `firmware/` and `src/`.

| File | About |
|---|---|
| `notes/firmware-kernel.md` | firmware and kernel findings |
| `notes/boot-services.md` | what starts at boot and how long it takes |
| `notes/thermal.md` | fan, undervolt, power limits |
| `notes/dwm.md`, `notes/session-tools.md`, `notes/quickshell.md` | the desktop before chadwm (dwm-titus, quickshell) and the session tools |
| `notes/syswork.md` | how `syswork` works |
| `notes/mx150-freeze.md` | the GPU-load freezes: what was tested and that the undervolt was the cause |
| `notes/2026-09-28-*.md` to `notes/2026-10-01-*.md` | what changed on those days (the undervolt and the MX150 clock offsets have their own) |
| `notes/2026-10-04-disk-encryption.md` | measured boot, signed kernel, LUKS with the TPM: the design, the numbers and what went wrong on the way |
