# System

Artix Linux with s6.

| File | Meaning |
|---|---|
| `packages-native.txt` | explicitly installed packages from the repositories |
| `packages-aur.txt` | explicitly installed packages from the AUR or built locally |
| `packages-all-with-versions.txt` | everything, with versions, for reference |
| `s6-active-services.txt` | services that were up |
| `etc/` | files in `/etc` that no package installed or that differ from the package's version |
| `usr-local/` | scripts from `/usr/local` |
| `usr-local/BINARIES.txt` | compiled programs in `/usr/local`; build them from `../src` or their upstream |

## Things in `etc/` worth knowing

| File | Why |
|---|---|
| `modprobe.d/nvidia-ondemand.conf` | nothing loads the NVIDIA driver except `gpu-power` |
| `modprobe.d/nvidia-sleep.conf` | video memory is saved across suspend; without it a running GPU program breaks |
| `X11/xorg.conf.d/20-intel-fast.conf` | modesetting on the Intel GPU; `AutoAddGPU false` keeps X away from the MX150 |
| `elogind/system-sleep/` | hooks around suspend: lock, fingerprint reader, battery |
| `s6/sv/` | own services and changed dependencies |
| `thermald.conf` | fan curve, power limits and the undervolt per power source (`ac_uv_*`, `batt_uv_*`, see `docs/notes/2026-09-30-undervolt.md`) for `thermald-t480` |
| `local.d/watchdog.start`, `modules-load.d/itco-watchdog.conf` | arm the TCO hardware watchdog at boot |
| `local.d/bluetooth.start`, `bluetooth/main.conf` | bluetoothd at boot, adapter auto-enabled |
| `local.d/hibernate-tune.start` | hibernate image size and compression threads |
| `gpu-power.conf` | MX150 clock offsets per power source, set by `gpu-power` after each driver load |
| `sudoers.d/` | what runs without a password |
| `fstab` | partitions by UUID; change them for another disk |
| `nftables.conf`, `local.d/firewall.start` | inbound firewall in its own table (policy drop; SSH from named hosts only). The addresses are examples |
| `ssh/sshd_config.d/50-harden.conf`, `sysctl.d/95-harden.conf` | keys only; kernel pointer, kexec and redirect settings |
| `local.d/tmp-flags.start` | `/tmp` gets `nosuid,nodev` (the s6 service that mounts it ignores `fstab`) |
| `local.d/boot-mount.start` | mounts `/boot` (this boot set has no `mount -a`) and names the dm devices of the early init |
| `local.d/t480-reseal.start`, `usr-local/sbin/t480-reseal` | TPM side of the disk encryption, see `kernel/early-init/README.md` |
| `pacman.d/hooks/95-t480-sign.hook`, `usr-local/sbin/t480-sign-kernel` | sign the kernel for the GRUB in the firmware after every install |
| `initcpio/` (`t480crypt`), `mkinitcpio*.conf` | the distro kernels ask the LUKS passphrase and open root and swap |
| `usr-local/libexec/slock-keyring` | opens the login keyring after a fingerprint unlock with a key sealed in the TPM; must stay mode 0755 (slock checks it with the real uid) |

`/etc` and `/usr/local` are git repositories on the laptop and are changed through
`syswork` (see `../src/syswork`). `syswork apply` does not handle deleted files: remove
those by hand.

Suspend through elogind (`loginctl suspend`, lid, power menu). Writing to
`/sys/power/state` skips the hooks, and the NVIDIA driver then refuses to suspend while
it is loaded.
