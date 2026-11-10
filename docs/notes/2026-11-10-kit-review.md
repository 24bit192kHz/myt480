# 2026-11-10: a review of the kit itself

The day after the idle-power work the kit was read as a code base: what in it is wrong,
stale or untested. Firmware untouched (C65). Kernel 7.2.8-9 rebuilt once more in place
(`/proc/version` build #10) because the early init changed. Everything below is deployed
on the laptop and was tried there unless it says otherwise.

## What was wrong, and what it is now

| Found | Now |
|---|---|
| The re-audit tests had been red since 2026-10-26: three test classes pointed at patches that had moved to `proposals/` or been folded into patch 0029. 42 of 102 tests never ran. | Paths follow the patches; the retired dGPU fix is kept as a test fixture. 102 run again. |
| No single command checked the kit. `firmware/tools/speedgate.sh` could not even be parsed (an unquoted `<UUID ...>` placeholder). | `make check`: shell syntax of every tracked script, the re-audit suite, the gpu-power tests, the thermald tests, the early init's command-line tests. `speedgate.sh` takes the UUID from `SWAP_UUID` and refuses to run without it. |
| `src/thermald-t480/thermald.conf` still carried the global -130 mV undervolt that caused the freezes of September, next to a stale binary and two log files. | A link to `system/etc/thermald.conf`; no binary and no logs beside the source. |
| The READMEs named C60 and kernel 7.2.8-8 as running, -115 mV on AC, a `slock-ly` directory; `hardware/kernel-cmdline.txt` still said `mitigations=off`. | They say what runs. |
| `03-lid-wake-s4` and `04-btusb-hibernate` removed their marker before the undo had worked: an EC that could not be read or a `modprobe btusb` that failed was silent and never retried. | The marker goes after the undo; a failure is logged and the next wake tries again. Six tests, three of them red before the change. |
| thermald bounded undervolt, power limits and trip points, but not the current limits: `icc_core_ac -64` would have set command bits of the overclocking mailbox word. Time windows, iGPU clock and turbo were unchecked too. | Bounded (1-255 A, 1 ms-128 s, 100-2000 MHz, 0/1); `tests/conf-bounds.c`. |
| `t480-reseal wrap` passed the hex volume key as an argument of `keyctl add`: readable in `/proc/PID/cmdline` while the command ran. | `keyctl padd`, the key comes through a pipe. |
| `t480-reseal rekey` wrote the new master blob first and wrapped the volume keys after it. A wrong passphrase in between left `kmk.blob` and `root.key` under different master keys: a passphrase boot every time until the next rekey. | Everything is made in `/run` and moved to `/boot/t480` at the end. On failure nothing on disk changes and the new master key leaves the keyring. |
| `flashrom.sh` left the policy-less one-shot blob `kmk.next` behind when the flash failed. | It reseals at once, which removes the blob. |
| Early init: when the PCR 8 extension failed and no key had been released, the boot went on, also into a plain root with a sealed blob in reach. | That case powers off. Before a LUKS2 root the boot goes on (the passphrase stands in front of it). The extension is tried five times. |
| Early init: `ro` was only seen with a space on both sides. | Whole words, the last of `ro`/`rw` decides. |
| `upower-battery-heal` (root) fell back to a lock file in `/tmp`. | `/run`. |
| `lid-delayed-suspend.sh` was never wired and contradicted the acpid handler; `mx150-revert.sh` would have installed a kernel that cannot open the encrypted disk. | Both removed. |
| Nothing compared the kit with the laptop. By hand: `.zshrc`, `.gtkrc-2.0`, `btop.conf`, `validity-rs.conf` behind, two udev masks missing. | `tools/kit-drift/kit-drift` (below); the files are caught up. |
| `syswork apply` copied the whiteout of a file deleted in a worktree to the live system: the removed script became a character device 0,0. `src/syswork/syswork` was one version behind the installed one. | Whiteouts are left out of the rsync and the live file is removed; `syswork diff` marks them `(deleted)`. One source. |

## Tried

- `make check`: all suites pass (108 re-audit tests, 3 skipped for want of a coreboot tree).
- `kernel/early-init/qemu/go.sh`, new init as external initramfs and again built into
  the kernel image that was then installed: all eight stages, `RESULT: PASS` in 62-66 s.
  Stage 6 now also runs a rekey without a passphrase and requires every blob unchanged
  and no master key left in the keyring.
- QEMU without a TPM, plain root: boots (logs `PCR 8 extend FAILED`); the same disk with
  a `/boot/t480/kmk.blob`: "the TPM could not be locked for this boot", power-off;
  `... noresume ro` as the last word: root mounted read-only.
- T480: boot on build #10 (`root opened, swap opened, TPM key released, 353 ms`),
  `hibtest.sh` 10.9 s, a real hibernation with RTC wake (6 s from the alarm to the last
  hook), both hooks logged set and clear, btusb back, TCO watchdog re-armed, thermald
  (new binary, `--test` first) in control of the fan.
- `syswork`: a scratch file applied, deleted in a second worktree, applied: file gone, no
  device node, both steps in the `/usr/local` history.

Not tried on the laptop: the failure paths of the two hooks (only with mocked tools), a
`rekey` (it needs the passphrase; rehearsed in QEMU), a failed flash.

## kit-drift

`tools/kit-drift/kit-drift -- ssh HOST ssh root@localhost` hashes the 300 mirrored files
on both sides and lists what differs, what is missing, and files in the hook and rules
directories that the kit does not have. `expected.txt` names the differences that are
meant (placeholders for UUIDs, addresses and the serial number; package files), each with
its reason; an entry that no longer applies is reported as stale. `make drift REACH="..."`
runs it. `/usr/local/bin` and `~/.local/bin` are compared file by file but not searched
for extra files: they also hold binaries, backups and private tools.

## idle-check

`tools/idle/idle-check` (root, read-only) is the budget of the locked, dark machine: at
least 50 % of the time in package C8 or deeper, at most 1.5 W package power, no thread
above 150 wakeups a second. Both regressions of the idle-power note break all three
lines. It refuses to judge a session in use. Started detached, the laptop measures
74 % / 0.63 W on battery / busiest thread 41 a second; through a waiting SSH session the
same minute reads 49 %, so do not run it that way.

Whole-machine draw from the battery in that state (locked, panel off, Wi-Fi associated,
180 s): 2.42 W. The same measurement on 2026-10-26 gave 3.9 W.

## Seen, not changed

- `02-sleep-guard` removes its `wd-off` marker before the watchdog is armed again. If the
  timeout register does not read back as expected the watchdog stays disarmed until the
  next boot, which is logged and was a choice. A retry at the next wake would be a
  change to a hook with a history of resets, and it has no test harness yet.
- containerd wakes 35 times a second with no container running; the fingerprint
  daemon's `verify` thread 40 times while the screen is locked. Together under 30 mW.
- The package never enters C10; C9 is the floor with the panel off.
