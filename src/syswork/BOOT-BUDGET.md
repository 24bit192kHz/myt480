# syswork boot budget

Target: keep ~8s boot. No reboot was performed during install — verify on next boot with the commands at the bottom.

## Baseline evidence (captured 2026-09-19, uptime session)

- Kernel: `7.2.5-rt3-arch1-1-rt` (RT kernel default via grub, hidden/timeout-0)
- `boottime` script exists at `/usr/local/bin/boottime` (kernel→sddm-autologin breakdown; run after next boot)
- dmesg initmem free at ~0.29s; s6-rc live DB holds 47–48 services
- Live set includes the new `syswork-remount` (oneshot, exits immediately)

## Added-cost accounting

| Item | Cost | Why negligible |
|---|---|---|
| `syswork-remount` oneshot (`syswork mount-all`, zero worktrees) | **~7 ms** (measured live: `s6-rc -u change` trigger 7 ms; bare `mount-all` 3 ms) | Single shell, one `for` loop over an empty dir, exits. Scales ~5 ms per existing worktree (one overlay mount each). |
| `/etc/.git` + `/usr/local/.git` baselines | **0 ms** | Passive files on disk. Nothing reads them at boot. No hooks installed. |
| `/etc/sudoers.d/20-syswork` | **0 ms** | Parsed once per sudo invocation, not at boot. |
| Overlay mounts at boot | **~5 ms each, only for worktrees left mounted** | Remount loop skips unmounted trees. Normal state (no active experiments) = the 7 ms empty-loop cost only. |
| NFS mirror / timeshift | **0 ms** | On-demand only; timeshift not installed (documented in MIRRORS.md). No timers, no fstab changes. |

**Total added: ~7 ms with no active worktrees. Worst realistic case (3 worktrees left mounted): ~25 ms.**

## Design choices that protect boot

- No daemons, no longrun services, no timers.
- No fstab, initramfs, or grub changes.
- The oneshot is in the `misc` bundle (runs alongside rc-local, not on the critical mount/setup path).
- Git repos have no hooks; `init-repos` was a one-time 1.6 s operation, never repeated at boot.

## Verify on next boot

```sh
sudo /usr/local/bin/boottime
dmesg | grep -E 'Freeing unused kernel image \(initmem\)'
s6-rc -a list | grep -c .
s6-rc -a list | grep syswork-remount
sudo syswork list   # any worktrees auto-remounted?
```
