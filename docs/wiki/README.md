# T480 reverse-engineering and improvement wiki

Audit date: **2026-10-09**. Source baseline: `d89f79d`; work branch:
`re-audit-20261009`. The laptop was inspected over SSH. Its live firmware is C55,
its kernel is `7.2.8-7-t480`, and its proprietary NVIDIA driver is `580.178.04`.

The most important improvement was making manual fan control fail safely. GPU
tests confirmed D3cold on AC while restoring the existing clock offsets before
each `prime-run` job. Both are deployed. The continuation added Intel application
routing, deeper protocol recovery and better measurement tools.

Start with [getting the most out of this T480](getting-the-most.md) for daily
profiles, GPU selection, repeatable measurements and recovery.
The [cross-stack review](cross-stack-review.md) ranks remaining boot-trust,
firmware, kernel and sleep-path defects with evidence and next validation steps.

| Area | Result | Status |
|---|---|---|
| [Thermal control](thermal.md) | Kernel fan watchdog, immediate curve transitions, EC fallback, conservative missing-config undervolt | Deployed; emulated I/O, graceful shutdown, and live 125-second pause tested |
| [NVIDIA](nvidia.md) | Coarse RTD3 with clock restoration; signal cleanup; truthful runtime status | Deployed; five cold-wake CUDA rounds and PRIME OpenGL passed |
| [Intel application routing](gpu-routing.md) | `igpu-run` and Bitwarden desktop override | Deployed; Intel GLX/EGL/Vulkan and full rollback/reapply passed; existing app still running |
| [USB-C](usb-c.md) | Recovered stock UCSI ACPI layout, SMM dispatch, EC mailbox, and transport ports | Reverse engineered; native implementation remains open |
| [Fingerprint](fingerprint.md) | Traced Windows cancellation, reset, and idle/resume paths; checked Linux backend selection | Analysis completed for selected paths; backend unchanged |
| [Fingerprint protocol](fingerprint-protocol.md) | Recovered reset `05 02 00`, stop `04`, secure transport and worker ordering | Static recovery; source cancellation regression addressed separately from live backend |
| [Power measurement](power-measurement.md) | Passive counters, correct dual-battery/source handling, repeatable capture | 21 fixture tests; live AC CPU-package observation, no whole-system watts claim |
| [Storage/display/Thunderbolt](storage-display-thunderbolt.md) | SMART health, working FBC/DMC, NVM 23.0 and domain policy | Read-only checks; no firmware or power-policy changes |
| [Cross-stack review](cross-stack-review.md) | Early-init trust boundary, C55 NVMe failure paths, live sleep hooks and kernel tradeoffs | Offline/source defects and current settings distinguished; live changes deferred to validated fixes |
| [Other subsystems](subsystems.md) | CPU, iGPU, NVMe, Wi-Fi, audio, Thunderbolt, sleep, firmware and crash diagnostics inspected | Inventory and next experiments documented |
| [Evidence and tools](evidence.md) | REA used successfully; Ghidra MCP fallback used on NVIDIA | Reproduction details and artifact hashes recorded |
| [Rollback](rollback.md) | Original binaries/configs retained; checksum-checked restoration script | Backup preflight passed on the laptop |

## Actual changes

- `/usr/local/bin/thermald-t480` replaced with the rebuilt controller.
- `/usr/local/bin/gpu-power` and `/usr/local/bin/prime-run` replaced.
- `/etc/gpu-power.conf`: `rtd3 0` changed to `rtd3 1` with an explanatory comment.
- Matching source, the tracked thermal binary, the `prime-run` system snapshot,
  and GPU config updated in this branch.
- `/usr/local/bin/igpu-run` added, plus a user Bitwarden desktop override for
  future launches. No running password-manager instance was stopped.
- Added hardware-independent tests, a read-only inventory collector, a temporary
  RTD3 validation script, power/SMART diagnostics, and rollback scripts under
  `tools/re-audit/`. Added an offline UCSI packet/completion model.
- Fixed cancellation starvation in the optional Rust fingerprint backend source,
  with a reproduced fixture failure and passing regressions. The active libfprint
  backend was not replaced.

The existing CPU undervolt, GPU offsets, fan curve, and power limits were retained.
This audit did not flash firmware, change the kernel, enroll/delete fingerprints,
or replace NVIDIA packages. USB-C restoration is **not implemented**. Hardware
sleep/resume, external docks, USB-C role swaps, and battery discharge measurements
still need separate validation; this session does not establish those outcomes.

## What to work on next

1. Implement the recovered USB-C mailbox through a serialized kernel/firmware EC
   transport, with timeouts and proper notifications. This addresses a missing
   interface that userspace tuning cannot supply.
2. Validate GPU RTD3 across real suspend/resume and AC/battery transitions. The
   tests here establish idle entry, cold wake, correct CUDA results, and offsets.
3. Capture fingerprint cancellation/resume USB traces using the selected backend,
   then compare them with the recovered worker lifecycle.
4. Measure energy on battery with charging stopped and a repeatable workload;
   only change CPU/iGPU/NVMe policies when the measurement identifies a bottleneck.

This is an audit of the observable system and selected relevant binary paths,
not a claim to have recovered every proprietary driver or EC firmware routine.
