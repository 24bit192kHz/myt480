# Sleep and power failure handling

## Runtime D3 and system sleep are separate

Prior live tests demonstrated coarse RTD3/D3cold and five cold-wake CUDA rounds
with offset restoration. They did not establish preservation of live allocations
through suspend, hibernate or suspend-then-hibernate. NVIDIA's
[580 power-management manual](https://download.nvidia.com/XFree86/Linux-x86_64/580.178.04/README/powermanagement.html)
describes different kernel-callback and `/proc/driver/nvidia/suspend` mechanisms.
Full VRAM preservation requires coherent userspace calls and sufficient backing
storage. A parameter and a present proc file alone do not prove that lifecycle.

The newly integrated [evening record](../../../docs/notes/2026-10-09-evening.md)
reports idle-GPU RTC S3/S4 cycles and two owner-operated S4 lid wakes. Those are
useful hardware results from a separate run. They do not test preservation of
live GPU allocations, and this approved query-only continuation did not repeat
sleep, EC writes or reboot.

The retained installed hook inspection found fixed `suspend` before every sleep
action and background `resume`. Correct hibernate dispatch and awaiting resume
matter when preservation is enabled. Current elogind source also has optional
built-in NVIDIA handling. Its installed version, defaults and effective owner
must be established before adding a replacement hook; two owners can duplicate
operations. The [cross-stack report](../../../docs/wiki/cross-stack-review.md#3-make-sleep-one-coherent-checked-operation)
records the observed defects and locker/watchdog/VT dependencies.

The later approved file observation now finds `/etc/elogind/system-sleep/nvidia`
with separate hibernate dispatch and synchronous bounded resume; the previously
observed vendor `/usr/lib` hook and `/usr/libexec` counterpart are absent.
That change was already present when queried, not deployed by this continuation.
The observed site-hook SHA-256 is
`3be9535de5f90bf655e782d254312e7ef425b6047312e07e4d102bf07004759d`,
mode 0755. Its comments describe package exclusion, but current package-update
enforcement/default owner precedence was not newly proved by these queries.
The separate evening record identifies elogind 257 and the package-exclusion fix;
the merged repository now contains that configuration. The hook still hides
helper failure behind logging/exit 0 and logs `$?` after logical negation on the
resume failure path.

A source-only [replacement snapshot](../../../system/etc/elogind/system-sleep/nvidia)
preserves the observed action/phase dispatch and synchronous resume while saving
the helper/timeout status before logging. It returns the real failure, restores
the saved VT after failed resume, and prevents logger/cleanup failures from
masking the result. Fourteen mock lifecycle tests include actual GNU timeout
against a mock process; no real NVIDIA helper or sleep action is called.
elogind may continue sleep despite a nonzero hook, so this corrects reporting,
not the entire sleep abort/readiness contract. The existing 30-second timeout
uses SIGTERM semantics and is not a guarantee that an unresponsive process is
forcibly killed. This file is not deployed and package configuration is unchanged.

## Corrected configuration diagnostic

[`nvidia-suspend-test.sh`](../../../system/usr-local/bin/nvidia-suspend-test.sh)
now inspects files rather than prescribing systemd units, persistence or early
NVIDIA KMS. Intel drives this desktop; NVIDIA KMS/fbdev disabled and an unloaded
on-demand driver are valid states. The old generic readiness advice was unsuitable
for the stored Artix/s6/Intel-offload policy.

It reports PCI identity, runtime state, driver flavor/version, preservation
parameters, proc interface, observed elogind configuration and selected hook
content. It flags preserve=1 without the proc interface and an open NVIDIA
kernel module on Pascal. Fixed-pre dispatch, asynchronous resume and potential
owner coexistence are warnings based on file heuristics.

It performs no NVML calls, `nvidia-smi`, privilege escalation, external service
operations or device writes. It never calls sleep or reports that all hardware
is ready. Exit 1 means a detected failure; exit 2 means unresolved required
observations; exit 0 can include warnings/skipped unloaded-driver checks and is
not proof of suspend success. Eight fake-root fixtures exercise the actual script.

`--root` is a path prefix for offline fixtures, not a filesystem jail; symlinks
can escape it. Hook discovery selects filenames containing `nvidia`, so arbitrary
wrappers can be missed. Drop-in precedence/defaults, executable hook ordering,
backing capacity, CUDA correctness and live transitions remain separate checks.
The updated script was fed over SSH for the approved file observation; it was
not installed. It reported 0 detected failures/unknown required observations,
with preservation=1, proc interface present, DPM 1 and KMS=N. This is not a passed
sleep test; default owner behavior and VRAM/locker readiness remain unresolved.

## ACPI power-on timeout defect

The actual runtime `DGON` method polls `DGFX_PWRGD` for 100 ms but previously
continued to release reset and enable the PCIe link even if the rail never became
ready. Bootblock already powers down on the equivalent timeout. This is a static
failure-handling discrepancy; no failed rail was induced or observed.

A narrow source-only [patch 0029](../../../firmware/coreboot/patches/0029-local-dgpu-abort-runtime-power-on-without-power-good.patch)
now rechecks PWRGD, disables the link, asserts reset, clears the rail request,
waits the existing power-off settling time and returns before reset release/link
enable on timeout. A signal ready at the final recheck can proceed normally.

The [focused test](../../../tools/re-audit/tests/test_dgpu_power_policy.py)
reconstructs the exact method from existing patch 0012, verifies forward/reverse
application, and executes the actual patched AML with fake GPIO helpers using
ACPICA. Eight execution scenarios cover the original bug, perpetual low,
inconsistent initial reset/link, immediate/near-boundary/final-recheck success,
power-good dropping at recheck and an already-powered no-op. The complete existing
baseline and patched T480 DSDTs also compile with 0 errors, 0 warnings and the same
29 remarks. Ten focused tests passed with `iasl`/`acpiexec`20251212.

The inspected source ref is `c57exp` commit
`84126e3fc1e4129f5be9af44d633736cea68d169`; method-file SHA-256
`1cd672880aca951c7801e4d7076472fc96e76fbb7ce7d72f8af1c27094201afa`.
The matching actual preprocessed DSDT SHA-256 is
`a3ba8b977b03fcfe5b2b6c189bba73db66b8f0d56eb82c990ee48d00ebcd3b26`.
Those are workstation build artifacts, not fresh installed firmware identity.
The wider `_STA` request-latch versus actual PWRGD status and `DGLW` link-wait
result contracts remain open. `DGON` remains void; the power resource observes
the cleared request latch through `DGST`, rather than receiving a rich error
return. The fixture cannot validate electrical timing, Linux ACPI unwind or
actual hardware failure/recovery. No full coreboot firmware build or flash was
performed. Reverse 0029 before rebuilding to restore its source behavior.

## Recovery and the next hardware test

The helper's earlier node-wait/unload-error/manual-off-lock fixes began source-only;
the integrated evening record reports their deployment and updated hashes in
[the test/deployment record](../../../docs/wiki/nvidia.md#later-source-only-policy-fixes).
This continuation's diagnostic, hook-status correction and firmware patches
remain uninstalled; earlier sleep results use the preceding hook behavior.

Before an allocation-preserving sleep test, select one verified NVIDIA sleep
owner, correct action/phase/error propagation, verify VRAM backing storage,
locker and watchdog readiness, and preserve exact old files with checksums.
Use checked CUDA/graphics allocations and synchronous resume; record exit codes,
Xids, state, outputs and restoration. Separate runtime D3, S3 and S4 cases.
These are requirements for a later explicitly approved test, not actions performed.
The user's ban on current reboot/suspend/hibernate/EC writes remains in force.
