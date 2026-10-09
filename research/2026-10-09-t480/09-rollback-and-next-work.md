# Recovery and prioritized next work

## Verified restoration paths

Original thermal/GPU executables and configs are retained on the laptop under
root-only `/root/myt480-audit-20261009`. The
[full rollback script](../../tools/re-audit/rollback.sh) checks expected backup
hashes/endpoints before replacement. Preflight and full restoration were tested;
the original hashes matched, then the improvements were reapplied and verified.
The detailed [restoration record](../../docs/wiki/rollback.md) lists original
and deployed hashes and subsystem behavior.

```sh
# On the T480, as root; preflight makes no changes.
sh /root/myt480-audit-20261009/rollback.sh all --check
# Restore the selected subsystem. Close GPU applications before gpu/all.
sh /root/myt480-audit-20261009/rollback.sh thermal
sh /root/myt480-audit-20261009/rollback.sh gpu
# Alternatively restore both:
sh /root/myt480-audit-20261009/rollback.sh all
```

The thermal path stops/replaces/restarts its s6 service; reverting the program
also reverts its fan-watchdog behavior. GPU restoration refuses observed clients
and unloads/reloads the driver, so a newly arriving client can still prevent it.
The recorded Bitwarden-held reload refusal was handled without stopping the app.
Config text alone does not change an already loaded driver's active parameter.

The Intel helper/desktop override were originally absent. Their absence markers
and installed hashes are retained separately under
`/root/myt480-audit-20261009-routing`. The
[routing rollback](../../tools/re-audit/rollback-routing.sh) refuses later edits,
removes only the two matching added files and refreshes the desktop database.
Full removal/reinstallation was tested. It does not reload drivers or stop apps.

```sh
sudo sh /root/myt480-audit-20261009-routing/rollback-routing.sh --check
sudo sh /root/myt480-audit-20261009-routing/rollback-routing.sh
```

The measurement/SMART tools are read-only and the UCSI model is offline; they
need no hardware-policy restoration. The Rust fingerprint change is source-only,
with the live libfprint backend retained. Its preceding source is available in
Git history. The later cross-stack assessment and this publication are research
and documentation changes. The later GPU policy and QEMU verdict fixes are
source-only; no laptop rollback is needed for them. Revert their commit before
building to restore the preceding source. Installed executables are not replaced
by a Git checkout.

## What to address next

| Order | Work | Evidence required before deployment |
|---|---|---|
| 1 | Require expected encrypted root before production-secret unseal; separate provisioning | Actual-source fixtures for plain/wrong/missing headers, valid encrypted root, passphrase and hibernation; then a recoverable signed-kernel test |
| 2 | Repair GRUB NVMe status checking and timeout/queue recovery | Inject full status, queue-full, late completion and timeout cases; verify controller quiescence and buffer lifetime before cold boots |
| 3 | Make sleep hooks a checked sequence | Done 2026-10-09 evening for the NVIDIA resume (synchronous, bounded), the TCO disarm/re-arm checks and `rtc-hibernate`; session-lock readiness still open; elogind cancellation rejected (see the review) |
| 4 | Make firmware checkers and wrappers truthful | QEMU verdict exit/selector contract fixed and installed; `flashrom.sh`/`flashrom-warm.sh` now exit 1 when the flash is not verified; exact config/key/auth and malformed option-ROM checks remain |
| 5 | Close policy-helper failure gaps | Deployed 2026-10-09 19:01: the node/unload/users-lock helper, and thermald keeps the last power source (battery at start) when AC detection fails and rejects out-of-range undervolt, power-limit and trip values |
| 6 | Restore native UCSI connector control | Serialized bounded EC transport, recovered packet ordering, ACPI notifications and a compatible kernel; validate on recoverable hardware without assuming the model proves it |
| 7 | Compare a tracing/security kernel profile | Mitigations, translated host DMA, supported signatures/confinement; compare workload cost and dock/VM behavior while retaining the tuned profile |
| 8 | Measure practical performance and energy | App restart/holders, controlled battery discharge, fixed completed jobs, display/dock/radio consistency and real sleep/resume |
| 9 | Continue fingerprint and wake research | Selected-backend cancellation/resume without biometric publication; lid gate now traced in supplied EC, with host route/rollback eligibility checks and approval required before testing |

The first three entries address concrete trust/reliability findings; later tuning
depends on measured constraints. Full rationale and evidence are in the
[cross-stack review](../../docs/wiki/cross-stack-review.md).

## Daily use while research continues

Start with CPU plan `auto`; explicit plans persist across battery changes and
reboots. `balanced` can raise battery limits above the automatic battery profile.
Use Intel for ordinary applications and the MX150 deliberately for compute or
rendering. Retain the tested undervolt/offset margins, fan fallback and TCO
recovery. The [daily-use guide](../../docs/wiki/getting-the-most.md) gives profile
values, inspection commands and repeatable measurement procedures.

The audit did not flash firmware or change the kernel. The published `testing`
branch preserves the complete audit history without rewriting `main` or `speed`.
Repository checkout and installation are separate actions: reading this research
or switching a Git branch does not deploy it onto another machine.
