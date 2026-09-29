# MX150 (GP108, 10de:1d10) on coreboot — 2026-09-29

Everything is on `mx150` branches; `master`/`t480` are the working C18 setup.

| Where | mx150 branch has | Pre-MX150 |
|---|---|---|
| src/coreboot | 846db20e58 power dGPU before FSP (CMOS 0x6f), 0f49d14d54 power-button override cancels it | branch `t480` |
| src/coreboot/site-local | 3648b3e VBIOS in CBFS (pci10de,1d10.rom -> ACPI _ROM) | branch `master` |
| kernel (recipe) | 0078650 builds linux-t480-headers too | branch `master`, pkg linux-t480-7.2.8-1-pre-mx150.pkg.tar.zst |
| /etc | modesetting X config, nvidia-sleep.conf override | branch `master` (intel/SNA) |
| /usr/local | bin/dgpu, bin/prime-run | branch `master` |
| roms | C19-mx150.rom | C18.rom |

Packages added: linux-t480-headers, dkms, nvidia-580xx-dkms, nvidia-580xx-utils,
egl-x11 (1.0.6, mirror no longer had 1.0.5), egl-gbm, egl-wayland, eglexternalplatform.

## Use
- `dgpu on|off|status` — CMOS 0x6f; takes effect at the next boot. Off = C18 behaviour.
- `prime-run <program>` — render on the MX150 (e.g. `prime-run glxinfo -B`).
- A boot that hangs with the GPU on: hold power 4 s. That clears the request and shows the GRUB menu.

## Notes
- NVIDIA 580xx is the last branch with Pascal; no runtime D3 on Pascal, so the GPU
  draws power while on. Keep it off on battery.
- NVreg_PreserveVideoMemoryAllocations=0 (/etc/modprobe.d/nvidia-sleep.conf): the
  package default needs systemd nvidia-suspend services, which s6 does not have.
- \TBTS in acpi/sleep.asl stays disabled (the file was never in the DSDT upstream).

## Going back
`sudo sh ~/t480-build/tools/mx150-revert.sh`, then reboot.

## Update 2026-09-29 evening: working on C32

The sections above describe C19-C23 and are history. Current state:

- Firmware: roms/C32-mx150.rom, branch `c24` of src/coreboot (off `t480`). coreboot powers
  the GPU in the bootblock; root port 1 is a normal port (no hotplug flag).
- `dgpu on|off` still selects it (CMOS 0x6f). Switching from off to on makes the next
  boot do one extra full reset; that is intended.
- Without a driver bound the GPU is powered off at runtime (ACPI power resource). With
  the NVIDIA driver loaded it stays on.
- Tested: cold boot, warm reboot, S3 resume, dgpu off, off->on, `prime-run glxinfo`.
- Upstream series: worktree src/coreboot-upstream (branch `dgpu-upstream`), patches in
  patches-dgpu/. Needs your Signed-off-by before it goes to Gerrit.
- Flash backup from before C24: /root/PRE-C24-backup.rom.
- A boot that hangs with the GPU on: hold power 4 s (clears the request).

## Update 2026-09-29 21:30: GPU on demand

- Leave `dgpu on` set. The GPU is OFF (power rail off) unless a program uses it.
- `prime-run <program>` turns it on (about 1.5-4.5 s), runs the program on the MX150 and
  turns it off when the last such program has ended.
- `gpu-power on|off|status` does the same by hand (for nvidia-smi, CUDA, ...).
- How: /usr/local/bin/gpu-power (setuid) loads/unloads the NVIDIA modules; without a
  driver the kernel puts the GPU into D3cold through the ACPI power resource.
  /etc/modprobe.d/nvidia-ondemand.conf keeps everything else from loading the driver,
  Option "AutoAddGPU" "false" in /etc/X11/xorg.conf.d/20-intel-fast.conf keeps X from
  holding the GPU. Source: ~/systemagent/src/gpu-power.
- To go back to "driver always loaded": remove nvidia-ondemand.conf and the AutoAddGPU line.
