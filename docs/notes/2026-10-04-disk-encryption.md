# 2026-10-04: measured boot, signed kernel, disk encryption with the TPM

Goal: a hardened laptop that behaves and boots as before. No prompt on a normal boot
(autologin, lock screen in the background, fingerprint), internal flashing stays possible.

## What was done

- Firewall in its own nftables table, SSH with keys only, a few sysctls, `/tmp` with
  `nosuid,nodev`, stale setuid copies removed.
- Firmware C52: `TPM_MEASURED_BOOT` (PCR 2), GRUB with `pgp`: the default kernel must
  carry a valid signature; the distro kernels, SeaBIOS, the entry editor and the GRUB
  shell need a password. GRUB on coreboot has no `tpm` module, so it cannot measure
  what it starts; that is why the kernel is verified by signature.
- Kernel 7.2.8-7: dm-crypt, AES-NI and the TPM key types built in, plus an early init
  (`kernel/early-init/`).
- Root and swap LUKS2 (aes-xts, 4096-byte sectors), converted in place; `/boot` on a new
  256 MiB partition.
- Firmware C53: boot entries for the encrypted disk, EC options from the devicetree.
  C54: the MX150 option ROM measured into PCR 3 instead of PCR 2.

Not done, on purpose: no SPI flash lock, `iomem=relaxed` stays, `mitigations=off` and
autologin stay.

## Numbers

Cold boots, power-on to the window manager, on AC:

| State | Time |
|---|---|
| C51, plain disk (start of the day) | 3.90 / 3.61 / 3.66 s |
| C52: measured boot + signature check | 3.95 / 3.89 / 3.78 s |
| + early init unsealing on a plain disk | 4.19 / 4.26 s |
| + encrypted disk | 4.23 / 4.16 / 4.19 s |
| C54, kernel command line without `resume=`/`resumewait`/`rootwait` | 3.50 / 3.54 / 3.64 / 3.60 s |

Where the time goes: coreboot hashing and TPM extends about 0.1 s, GRUB's signature
check of the 19 MB kernel 0.10 s, the early init 0.32 s of which 0.24 s is the TPM
(Load and Unseal over salted HMAC sessions on the SLB9670). Dropping the kernel's own
wait for the old resume partition gave about 0.6 s back.

Hibernate and resume by RTC alarm: 6.3 s before and after. Idle on battery with the GPU
in D3cold: 6.7 to 6.9 W.

Raw partition against LUKS2/dm-crypt on the same NVMe (fio, direct, two interleaved
rounds, `no_read_workqueue no_write_workqueue`):

| Test | Raw | Encrypted |
|---|---|---|
| sequential write, 1M | 857 / 1121 MiB/s | 897 / 808 MiB/s |
| sequential read, 1M | 1290 MiB/s | 1280 MiB/s |
| 4k random write, QD1 | 59k iops | 51k iops |
| 4k random read, QD1 | 30.6k iops | 28.2k iops |
| 4k random read, QD32, one thread | 250k iops | 167k iops (207k with workqueues) |
| 4k mixed 70/30, 4 jobs | 86k to 100k + 37k to 43k | 100k to 111k + 43k to 48k |

The in-place encryption of 204 GiB took 21 minutes.

## What went wrong, and what it taught

- The first QEMU test ROM had no key in the memdisk: the Makefile only packed it for a
  relative `grub.cfg` path. The harness caught it before anything was flashed.
- `/tmp` lost `nosuid,nodev` at the next boot and `/boot` was not mounted after the
  conversion: this s6 boot set mounts `/tmp` by hand and has no `mount -a`.
- GRUB's `fallback` only takes entry numbers. A rejected kernel ends in the menu.
- A detached job lost access to the kernel keys: the session keyring of an ssh session
  is revoked when it closes. `t480-reseal` now always runs in its own.
- `/etc/modprobe.d` blocked `dm_mod` (a boot-time tweak), which also broke the distro
  kernels' initramfs until the hook used `modprobe -i`.
- Switching the GPU off changed PCR 2, because the option ROM is only measured while
  the GPU is on. The TPM refused the key and the laptop waited for the passphrase.
  Fix: the ROM is runtime data (PCR 3). Before any boot that may change PCR 2, leave the
  one-shot blob.
- Binding the lock screen's keyring key to PCR 2 as well broke the keyring after every
  test flash; reverted, it is bound to the TPM only. The helper must be executable for
  the user: slock checks it with `access()`, which uses the real uid.
- Hibernate from the session menu was refused while Bitwarden ran: it holds
  `memfd_secret` memory and the kernel then disables hibernation.
- The RTC alarm does not wake the laptop from S5 on battery.
- A root port marked hot-pluggable (tried for the upstream dGPU patch) keeps FSP-S from
  disabling the port, but Linux then never powers the port down: the GPU stayed on with
  no driver bound, 7.3 to 10.5 W against 6.9 W idle. The flashed firmware does not use it.

## Limits

The trust root is the SPI flash. Someone with a clip programmer and time can replace the
firmware and with it the measurements. A running, unlocked machine is protected by the
lock screen and the firewall only.
