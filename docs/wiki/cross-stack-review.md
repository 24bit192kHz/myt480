# Cross-stack defects and improvement priorities

Review date: **2026-10-09**, following audit commit `fe58a80`. This pass checked
the current laptop, cumulative firmware sources, archived C55 payload, kernel
recipe/configuration, and driver/service hooks. It did not change live policies,
flash firmware, suspend, switch backends, or read secret/biometric material.

The highest priorities are the early-init trust boundary, GRUB's failure handling,
and the sleep path. Several kernel settings are deliberate performance/security
tradeoffs. Those choices need a separate comparison profile rather than being
mistaken for broken drivers.

| Priority | Finding | Evidence/status | Useful next action |
|---|---|---|---|
| High | TPM master can be loaded before executing an unauthenticated plain root | Actual early-init control flow reproduced with mocked hardware; physical exploit not performed | Separate provisioning from daily boot; require the expected encrypted root before unsealing |
| High | GRUB NVMe timeout/retry can dereference `NULL`; some errors pass as success | Final source and matching archived C55 module; no live timeout observed | Inject failures offline, repair queue recovery and status checking before the next payload build |
| Fixed 2026-10-09 | NVIDIA sleep hook requests `suspend` for hibernation and backgrounds resume (an `/etc` override existed but elogind masked it with the package file) | Package hook removed and kept out by `NoExtract`; `system/etc/elogind/system-sleep/nvidia`: hibernate mapping, synchronous bounded resume, VT put back on failure | Exercised by RTC-wake S3 and S4 cycles the same evening, see `docs/notes/2026-10-09-evening.md` |
| Partly fixed | Sleep proceeds without proving locker readiness or successful watchdog disarm | Disarm now has a forced fallback and the re-arm checks that the keepalive started; locker readiness unchanged | elogind cancellation deliberately not enabled: with `AllowSuspendInterrupts=yes` any failing pre-hook cancels and the post hooks never run |
| Security choice | CPU mitigations disabled | Live vulnerabilities explicitly report vulnerable states | Compare a recoverable boot profile with mitigations enabled |
| Security choice | Thunderbolt domain has automatic connection and identity DMA mapping | Live NHI group13 `identity`, domain `none`, advertised protection0 | Test translated/strict host DMA with dock and VM coverage |
| Feature gap | UCSI connector control/notifications absent | Live-matching kernel disables TYPEC; coreboot omits the recovered bridge | Implement serialized transport plus ACPI notifications; options alone are insufficient |
| Fixed 2026-10-09 | Lid did not wake S4 hibernation | EC decode found the retained flag behind EC byte `0x01` bit 6; the `03-lid-wake-s4` hook sets it before each hibernate; owner-tested twice (wake by the EC's PWRBTN# pulse, no RTC) | Nothing further; no firmware change needed |

## 1. The TPM boot path needs a stricter root boundary

[`init.c`](../../kernel/early-init/init.c) classifies root/swap headers at lines
524–525, then attempts to load `t480-kmk` at 536–537 regardless of whether root
is encrypted. `add_key()` at 224–226 puts it in root's user keyring. Plain-root
boot remains supported; line 584 mounts that root and line 620 executes its init.
There is no intervening revocation of the loaded master.

On a clean boot, a replacement plain filesystem can therefore become the next
privileged userspace under the original signed kernel. If copied sealed/wrapped
blobs satisfy the unchanged firmware/TPM policy, that userspace inherits access
to a usable master. Conditional encrypted-root-key loading at line 551 does not
solve this: the retained master can authorize loading the wrapped volume-key
blob later. Extending PCR8 prevents a new TPM unseal against the original policy;
it does not invalidate the master already loaded in the kernel.

An offline harness included the actual source and mocked device, mount, key and
exec calls. Under the explicit assumption of a successful matching-policy
unseal, it confirmed **master load → plain root mount → root init execution**.
This is a reproduced control-flow defect, not a demonstration of key extraction
or an attack on the running machine. A valid hibernation image can resume the
legitimate system before replacement init; the clean-boot path remains open.
Trusted-key non-exportability does not prevent use by kernel consumers.
[Linux trusted/encrypted-key documentation](https://docs.kernel.org/security/keys/trusted-encrypted.html).

The daily kernel should reject unexpected plain/missing root headers before
secret release. Keep provisioning/plain-disk support in an explicit recovery
path that does not automatically unseal production secrets. Verify this with
plain-root, wrong-header, missing-disk, correct encrypted-root, passphrase and
hibernate fixtures before deploying a signed replacement kernel.

There is also a migration-policy limitation: `t480-reseal next-boot` writes
`kmk.next` without a PCR policy; `load_trusted()` accepts it first. File deletion
is the one-shot mechanism, so a previously copied unbound blob has no TPM-enforced
single-use guarantee. No such file remains on the laptop now. Prefer a migration
policy bound to approved measurements/authorization; handle flash failure and
reseal failure explicitly. This review did not copy blobs or change the TPM.

## 2. Repair the bootloader failure paths before more firmware experiments

The final GRUB tree is `fb40995250874853982cb5cc8416cad4fce11706`. Its reviewed
`nvme.mod` occurs byte-for-byte in the archived C55 candidate payload. This binds
the source findings to that build; no new live SPI dump was taken.

- [`0001 NVMe patch`](../../firmware/grub/patches/0001-Add-native-NVMe-driver-based-on-SeaBIOS-out-of-tree-.patch),
  line 364, checks only the eight-bit status code, ignoring status-code type.
  Raw CQE status `0x201` passes although decoded status `0x100` is a
  command-specific error. [Linux NVMe status definitions](https://github.com/torvalds/linux/blob/master/include/linux/nvme.h).
- [`0015 timeout patch`](../../firmware/grub/patches/0015-nvme-real-timeouts-pci-scan-only-existing-buses-memd.patch),
  lines 134–136, returns failure without quiescing/resetting the controller or
  recovering the submission. With queue head0/tail1/mask1, the next submission
  returns `NULL`; patch0001 line694 dereferences it. The three-attempt
  [GRUB fast path](../../firmware/coreboot/site-local/grub.cfg) can reach that retry.
- Completion consumption does not check command/submission identity. Recovery
  must prevent a late completion from becoming another request's result; merely
  clearing software queue indices while the controller is active is insufficient.

Offline vectors confirmed the status predicate and post-timeout queue condition.
No live controller timeout, reset or read failure was induced. Add injected
timeout/late-completion/queue-full/status fixtures, then verify controller recovery
and buffer lifetime before hardware cold-boot tests.

## 3. Make sleep one coherent, checked operation

The packaged `/usr/lib/elogind/system-sleep/nvidia` calls `nvidia-sleep.sh suspend`
for every `pre` event and `resume &` for `post`. It ignores the requested sleep
method. The live driver reports `PreserveVideoMemoryAllocations: 1`, so correct
proc-interface sequencing matters. NVIDIA distinguishes hibernate from suspend
and requires resume immediately after a successful or failed transition.
[NVIDIA 580 power-management documentation](https://download.nvidia.com/XFree86/Linux-x86_64/580.178.04/README/powermanagement.html).

Follow-up (2026-10-09 evening): an override
[`/etc/elogind/system-sleep/nvidia`](../../system/etc/elogind/system-sleep/nvidia)
with the hibernate mapping had existed since 2026-10-05, but it had never run.
elogind 257 executes `/usr/lib/elogind/system-sleep` first and masks `/etc` by
file name (`dirs[]` in its `sleep.c`), the opposite of what the override assumed;
no `nvidia-sleep` syslog line had ever been written. The finding above was
therefore right in effect. The package file is now removed and kept out by
`NoExtract = usr/lib/elogind/system-sleep/nvidia` in `pacman.conf`, and the
override runs the resume synchronously with a 30 s bound, switching the VT back
itself if the helper is cut short. Checked with RTC-wake S3 and S4 cycles the
same evening (`docs/notes/2026-10-09-evening.md`).
The vendor helper also has an exit-status defect: after failed `chvt`, its
`exit $?` can return the condition test's success rather than the original error.
That was reproduced with mocked commands, without switching a real VT.

Other sleep-path issues are:

| Path | Current defect / limitation | Remedy |
|---|---|---|
| [`00-slock`](../../system/etc/elogind/system-sleep/00-slock), lines5/9/12 | Accepts any `slock` process, starts locking in the background, always succeeds | Confirm the intended session's successful input grabs with a bounded handshake |
| [`02-sleep-guard`](../../system/etc/elogind/system-sleep/02-sleep-guard) | Ignored a watchdog-disarm failure before hibernate preparation | Fixed 2026-10-09: a keepalive that will not exit is killed and the timer stopped from a fresh descriptor (magic close); a still-armed watchdog is logged loudly |
| Same hook | Logged successful rearming without verifying keepalive startup | Fixed 2026-10-09: waits for the keepalive and logs "left disarmed" if it did not start |
| [`rtc-hibernate`](../../system/usr-local/bin/rtc-hibernate) | Direct sysfs entry bypassed elogind's NVIDIA, locker and watchdog hooks (and is refused outright while the driver is loaded) | Fixed 2026-10-09: arms the RTC, then `loginctl hibernate` |

An ordinary hook failure does not cancel elogind sleep. Its supported mechanism
requires `AllowSuspendInterrupts=yes` in sleep configuration plus a stdout error
message beginning with a supported cancellation keyword. In elogind 257 that
setting also makes any non-zero hook exit cancel the sleep, and a cancelled sleep
runs no `post` hooks, so earlier pre-hook changes (stopped fingerprint driver,
paused VM, RTC alarm) would stay in place. It was therefore left off.
[Official elogind hook documentation](https://raw.githubusercontent.com/elogind/elogind/main/man/loginctl.xml).

The currently observed TCO watchdog and soft/NMI detectors are working. These
findings concern error/ordering paths; this pass did not demonstrate an unlocked
resume or another watchdog reset.

## 4. Separate the tuned kernel from an audit/security profile

The live config matches [`config-7.2.8-7-t480`](../../kernel/config-7.2.8-7-t480),
SHA256 `bad21a17f884dcad8726dea817cba49f3384840cace55459b44296210cce3b1e`.
[`kernel/config`](../../kernel/config) is the base before tailoring, not the final
running config.

`mitigations=off` is live and deliberately retained in the
[earlier hardening decisions](../notes/2026-10-04-disk-encryption.md). The CPU
reports vulnerable Meltdown, Spectre, MDS, SRBDS and other applicable states.
Mitigations are compiled in, so a separate boot profile can compare their actual
cost. [Linux parameter semantics](https://docs.kernel.org/admin-guide/kernel-parameters.html).

VT-d is enabled and 18 IOMMU groups exist. However, the actual Thunderbolt NHI
`05:00.0` uses group13, type `identity`, shared with `04:00.0`; the domain reports
`none` and `iommu_dma_protection=0`. `07:00.0` is the xHCI controller, not the NHI.
[`PKGBUILD`](../../kernel/PKGBUILD), lines537–542, deliberately selects host
passthrough. Test a translated/strict host-DMA profile with both dock and VM
workflows before changing this default. Do not describe the presence of VT-d as
proof of isolation. [Thunderbolt protection interface](https://www.kernel.org/doc/html/latest/admin-guide/thunderbolt.html).

Tailoring also disables module signatures, lockdown, audit, major MAC stacks,
FTRACE/BTF and allocation initialization. The active LSMs are
`capability,landlock,yama`; compiled options in the base do not establish active
policy. A separate tracing/hardened kernel is useful for investigating failures
and testing confinement while retaining the tuned kernel as a recovery choice.
Keep `iomem=relaxed`/internal flashing as an explicit maintenance requirement.
Measure each profile; no speed or battery gain from changing these settings was
established here.

## 5. Other concrete source/build improvements

| Area | Finding | Status / next check |
|---|---|---|
| Debug firmware | Stored debug defconfig selects older GRUB branch and omits production PGP modules/measured boot | Derive it from production and check equivalent boot policy before use |
| Pre-flash checker | [`romcheck.sh`](../../firmware/tools/romcheck.sh) accepts matching UUID text without checking actual config/key/auth | Stale/missing-policy fixture returned OK; compare exact extracted artifacts |
| Option ROM | [`coreboot patch0013`](../../firmware/coreboot/patches/0013-local-dGPU-wait-for-the-link-before-FSP-S-bound-the-.patch), lines80–81, can select a copy length exceeding CBFS file size | Malformed-input defect; current VBIOS fits. Reject oversized lengths and test truncated images |
| Firmware tests | [`qtest52.py`](../../firmware/tools/qtest52.py) printed failures but exited0 | Later source fix returns1 for scenario failures and2 for unknown selectors; six hardware-free verdict fixtures passed, no new QEMU boot run |
| Flash wrappers | [`flashrom.sh`](../../firmware/tools/flashrom.sh) and [`flashrom-warm.sh`](../../firmware/tools/flashrom-warm.sh) end failed verification branches successfully | Propagate failure and resolve the open migration blob; no flash was run |
| GPU policy | [`gpu-power.c`](../../src/gpu-power/gpu-power.c) reported success after missing node or failed automatic unload | Later source fix returns failure; twelve fake-device/modprobe/lock fixtures passed, not deployed |
| GPU launch race | Manual `off` bypassed the wrapper's shared users lock | Later source fix refuses unload while the launch/job lock is held; hardware-free launch-window fixture passed, not deployed |
| Thermal failure path | [`thermald.c`](../../src/thermald-t480/thermald.c), line320, defaults to AC on detection failure; numeric config lacks complete bounds | Retain last valid source/default conservatively; reject invalid values before hardware writes |
| Reference consistency | Kernel README omitted enabled VFIO/IOMMU; package description advertises disabled AutoFDO/Propeller | README corrected in this review; use final config for feature claims |

The setuid GPU helper uses an exact command whitelist, fixed `execve` arguments
and environment, an absolute NVML path, and root-controlled parents/config.
No command/path injection was found in this pass. Symlink/type checks and error
reporting can still improve its robustness; observed `/run` permissions do not
establish an unprivileged symlink exploit.

## What checked out, and what remains unmeasured

Microcode0xf6, loaded DMC1.4, active FBC, matching NVIDIA DKMS/userspace580.178.04,
the fan watchdog and current TCO/NMI recovery were confirmed. Retained logs had no
matching current Xid, NVMe timeout/reset or AER failure. SSD health is documented
on the [storage page](storage-display-thunderbolt.md).

GVT is present: `kvmgt`, a configured mdev and exposed captured firmware state.
Its missing `golden_hw_state` message follows a supported fallback returning
success; it is not proof of failed virtualization.
[i915 GVT implementation](https://github.com/torvalds/linux/blob/master/drivers/gpu/drm/i915/gvt/firmware.c).
TSC-adjust restoration and a DP adapter warning need event correlation before
being assigned a fault. The inactive `/etc/default/grub` reference does not
describe current PSR/APST settings.

The Intel launcher is deployed, but the existing Bitwarden process still holds
NVIDIA handles. Adopt it on the next normal app restart and compare device state
and energy. [Routing](gpu-routing.md), [measurement](power-measurement.md),
[UCSI reconstruction](usb-c.md) and [fingerprint source work](fingerprint-protocol.md)
contain their own limitations and restoration steps.

Private evidence is retained under
`/home/btw/test/rea/work/audit-20261009-cross-stack`: the source control-flow
harness, NVMe vectors, firmware checker fixtures and selected live-state logs.
No claim is made that every proprietary routine or every possible hardware
workload has been validated.
