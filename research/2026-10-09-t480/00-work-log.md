# Complete work record

Audit and publication date: **2026-10-09**. The source baseline was `d89f79d`.
Work was first committed on `re-audit-20261009`; `testing` was created from its
final audit commit to publish the complete work with this subject-based research
folder. Existing history was retained.

## 1. Initial inspection and reverse engineering

Inspected the laptop over SSH: firmware/version, running kernel and command line,
GPU driver/runtime state, fan and thermal settings, power policies, ACPI tables,
USB/backend selection, storage, display, Thunderbolt, services, and selected logs.
Compared live state with repository snapshots rather than assuming every stored
configuration was active.

Used REA successfully after the installed older workflow failed; analyzed the
thermal executable, selected fingerprint DLL functions, and stock USB-C SMM/EC
components. Used Ghidra MCP after the full NVIDIA REA job timed out. Recovered
selected RM power paths, fingerprint worker/reset/cancel paths, and the stock
UCSI mailbox/transport. Inspected the awesome-reverse-engineering index to select
appropriate tools. Tool versions and artifact hashes are in the
[evidence report](08-tools-evidence-and-validation.md).

## 2. Fan and NVIDIA improvements — `2cb13a3`

[Commit: Make T480 fan and GPU control fail safely; document driver audit](https://github.com/24bit 192kHz/myt480/commit/2cb13a3c133df037f55a46a4bb4493fdc34f0c20).

| Change | Repository | Laptop |
|---|---|---|
| Fan watchdog, EC fallback, curve transitions, config validation | [`thermald.c`](../../src/thermald-t480/thermald.c), rebuilt tracked executable, emulated tests | Replaced `/usr/local/bin/thermald-t480`; existing config retained |
| Offset restoration, honest runtime status, independent NVML setters | [`gpu-power.c`](../../src/gpu-power/gpu-power.c), fixtures | Rebuilt and replaced `/usr/local/bin/gpu-power` |
| Held-open device, signal forwarding, cleanup, descriptor isolation | [`prime-run`](../../src/gpu-power/prime-run), system snapshot, fixtures | Replaced `/usr/local/bin/prime-run` |
| Coarse RTD3 on AC | [`gpu-power.conf`](../../system/etc/gpu-power.conf) | Set `rtd3 1`; active module parameter checked as `0x01` |
| Inspection, hardware validation, recovery | [`tools/re-audit`](../../tools/re-audit/) collector, RTD3 test, rollback | Original programs/configs preserved under root-only backup directory |
| Research wiki | Initial thermal, NVIDIA, USB-C, fingerprint, subsystem, evidence and rollback pages | Research saved; no firmware or kernel replacement |

Validation included emulated fan I/O, wrapper signal/stdin/exit/descriptor
fixtures, mocked NVML errors, static builds and shell checks. A live 125-second
daemon pause demonstrated independent kernel fallback from manual fan level 4
to EC auto; the maximum sampled CPU temperature was 49°C. Resuming the daemon
restored curve control.

Five live D3cold-to-CUDA rounds passed self-checking `vectorAdd` and `scan`, with
the retained +200/+1250 MHz offsets present inside a held-open job. PRIME OpenGL
rendered on the MX150 and idle returned to D3cold. These checks establish behavior
on this machine; total cold-job durations are not isolated wake latency or watts.

The full rollback restored original executable/config hashes, after which the
improvements were reapplied and revalidated. A later reload attempt was refused
while Bitwarden held GPU handles; cleanup/reapplication restored the intended
files and driver mode. The application was left running.

## 3. Protocol recovery, Intel routing, and diagnostics — `fe58a80`

[Commit: Extend T480 protocol audit, Intel routing and measurement wiki](https://github.com/24bit 192kHz/myt480/commit/fe58a802fc08913b1d7c9147196d1dd1e6756527).

| Change | Repository | Laptop |
|---|---|---|
| Explicit Intel GLX/EGL/Vulkan routing | [`igpu-run`](../../src/gpu-power/igpu-run), install target, system snapshot, four fixtures | Installed `/usr/local/bin/igpu-run` |
| Bitwarden future-launch routing | [`bitwarden.desktop`](../../home/local-share/applications/bitwarden.desktop) | Added user desktop override; running instance unchanged |
| Routing restoration | [`rollback-routing.sh`](../../tools/re-audit/rollback-routing.sh) | Original-absence markers and installed hashes saved; removal/reinstallation tested |
| Passive energy/source measurement | [`measure-power.py`](../../tools/re-audit/measure-power.py), 21 fixtures | Captured a 20-second observational AC baseline without policy changes |
| Fixed read-only SMART request | [`nvme-health.py`](../../tools/re-audit/nvme-health.py), four fixtures | Read SSD health; no storage setting changed |
| Offline UCSI packet/completion model | [`ucsi-model.py`](../../firmware/tools/ucsi-model.py), six fixtures | No EC/mailbox transactions performed |
| Fingerprint event-stream cancellation | [`usb.rs`](../../src/validity-rs/src/usb.rs), seven USB fixtures within Rust suite | Source-only; selected libfprint backend retained |
| Extended documentation | Five new wiki subjects, prior-page updates, usage/installation notes | Wiki copied into the routing research backup directory |

Intel GLX, targeted X11 EGL, and Vulkan checks passed. The first Intel selector
produced a Mesa warning; it was corrected to the explicit PCI selector and
revalidated. A broad EGL probe reported unsupported contexts, so the report does
not claim universal platform support. Routing rollback/reapply passed again after
the final inherited-environment cleanup.

The AC capture yielded 21 samples across 19.995 seconds, package energy 63.678 J
and average CPU-package power 3.185 W. AC was online, so whole-system discharge
was deliberately unavailable. NVIDIA was active with application holders.
No battery-runtime improvement or isolated GPU wattage was measured.

Recovered additional Windows fingerprint reset/stop and secure-transport paths,
plus USB-C DXE shadow initialization, SMM dispatch, EC mailbox and lower transport
wait semantics. Corrected the earlier interpretation of SMM subcommand 0: it
rewrites EC bits, rather than querying capability. Native USB-C restoration
remains open.

Continuation tests passed: **25 power/SMART + 4 Intel launcher + 6 UCSI model +
44 Rust tests = 79 tests**. Seven USB cancellation fixtures are included in the
44 Rust tests, not additional to them. The Rust run retained six existing
dead-code warnings; optional private Python golden data was absent, so conditional
tests were not fresh cross-language verification. Desktop validation, shell
syntax, relative wiki links and whitespace checks also passed.

## 4. Cross-stack assessment — `7f5a430`

[Commit: Document cross-stack boot, driver and sleep defects](https://github.com/24bit 192kHz/myt480/commit/7f5a430d6091280f9c8d127a1b17096dbf1a0b78).

This pass added the [cross-stack review](../../docs/wiki/cross-stack-review.md)
and corrected the coreboot, kernel and early-init READMEs. It inspected the
actual running configuration, installed sleep hooks, final patched sources and
archived C55 payload. It made no live policy changes.

| Work | Result and boundary |
|---|---|
| Early-init control-flow harness using actual source | Reproduced master load → plain-root mount → init under mocked successful unseal; no live key release/extraction |
| GRUB NVMe completion/queue vectors | Found status-type masking and timeout/retry failure paths; matching module bound to archived C55 payload, no induced live timeout |
| Sleep-hook/helper inspection and mocked VT failure | Found wrong hibernate mapping, asynchronous resume, locker/watchdog readiness gaps and status loss; no live sleep test |
| Kernel and DMA inspection | Distinguished actual final config from base; confirmed mitigation tradeoffs, host identity mapping, GVT fallback and current driver state |
| Firmware-check fixtures | Reproduced UUID-only checker acceptance and zero exit on printed test failure; found flash-status propagation and option-ROM bounds candidates |
| GPU/thermal robustness review | Identified remaining error reporting, manual-off lock race candidate, AC-detection fallback and numeric-bounds gaps |
| Documentation correction | Recorded findings, evidence limits, validation required and prioritized next fixes |

Remaining findings are analysis and proposed work. They were not fixed by this
documentation-only assessment.

## 5. Publication on `testing`

Created `testing` from the final cross-stack audit, retaining all preceding source, tools,
tests and wiki changes. Added this folder with subject reports, this work record,
validation/evidence notes and recovery priorities. Linked it from the repository
and documentation indexes; updated branch-publication statements that described
the earlier local-only phase. The publication introduces no further hardware
changes. It is committed and pushed as a separate documentation commit.

GitHub rejected the initial push with `GH007` because the commit email was
protected. Only the four unpublished `testing` commits were recreated using
the authenticated account's GitHub no-reply address. Their file trees matched
before this work-log/link update. The original `re-audit-20261009` branch and
`testing-before-email-fix-20261009` backup remain local. Commit links above use
the public equivalents; no remote history was rewritten and email protection
was retained.

## 6. Supplied EC image and whole-machine coverage

Extracted the supplied `N24UR36W` ISO's full El Torito disk image and the two
identical FL2 copies. Hashed the ISO, container and payload; version `N24HT37W`
is present. Used the explicitly requested Ghidra MCP with a pinned local ARC
processor contribution, then independently checked selected instructions and
tables with a private GNU Binutils 2.45 ARC build. The payload is little-endian
ARCompact, not an H8 CPU merely because the compatibility driver has that name.

The [EC report](10-ec-firmware-and-lid-wake.md) records physical lid/PWRBTN#/wake
signals, VCI configuration, the power-button sequencer and a conditional deep-state
lid-open request. Ordinary channel-0 commands reach a host-bit setter:
EC byte `0x01`, bit 6 (`BTPC`) controls a retained flag at `0xf0cd08` bit 0.
The flag gates VCI lid detection and the lid request, while other power-policy
predicates still control the eventual pulse. This is a reviewable `_PTS(4)`
candidate, not a demonstrated S4 wake fix.

Independent review caught and corrected two important interpretation hazards:
vendor channel-1 command semantics alone did not prove ordinary ACPI access;
the separate channel-0 handler/BAR trace does. Host byte `0x01` reads a resettable
shadow, not the retained flag, so saving that bit alone does not prove reversible
restoration. S4/S5 separation and active complete-image identity remain open.
The supplied earlier SMM/GPE/HWLO experiment results were retained without repeats.

The [coverage ledger](11-coverage-and-feature-roadmap.md) accounts for all 24 PCI
functions in the stored inventory, recorded USB devices, other hardware and
software layers. Each entry distinguishes evidence already obtained from open
work; inventory is not called complete reverse engineering. It links useful
feature opportunities to their remaining implementation/validation requirements.

The private Ghidra project/results were saved; its temporary server stopped and
processor link removed. **No T480 command, EC transaction or hardware test was
performed in this continuation.** The user's read-only and ask-before-test rules
remain in force. Public changes contain derived findings, not raw firmware dumps.

## 7. Additional source-only reliability fixes

| Change | Offline validation | Deployment |
|---|---|---|
| GPU load fails when its node never appears; automatic policy propagates unload failure | Fake sysfs/modprobe fixtures and warning-free `-Werror` build | Installed 2026-10-09 19:01 (built on the laptop, through `syswork`; `docs/notes/2026-10-09-evening.md`) |
| Manual GPU off checks the users lock before unloading, refusing a starting/active wrapper without a blocking lock inversion | Real advisory-lock fixtures cover the launch interval before device open and later policy application | Installed with the above |
| QEMU scenario runner returns 1 on failure and 2 on unknown selectors | Six verdict/entrypoint fixtures, without starting a VM | Copied to `~/t480-build/tools/qemu/` the same evening; no firmware build/flash |

The GPU suite passed **12 new policy fixtures**, the existing **5 wrapper
fixtures**, and the NVML checks. The firmware-runner suite passed **6 new
fixtures**. These 18 new tests are separate from the earlier 79-test continuation;
they are not new hardware, QEMU boot or battery results. These fixes began as
source-only; the merged evening record reports their later deployment as shown
above. Git and the updated deployment record provide distinct rollback paths.
Updated the wiki and corrected the old missing-SMM lid attribution and forced
power-off GPU-choice description. All continuation work is published on `testing`.

## 8. MX150 voltage, closed-driver and EC protection continuation

Added the [MX150 subject folder](12-mx150/README.md) with separate voltage/API,
closed RM, VBIOS/EC, sleep/firmware, application/measurement and coverage reports.
Used the requested Ghidra MCP again on the exact retained 580.178.04 module,
saved a persistent project and selected-function/evidence manifests, and checked
important paths against original ELF relocations/GNU disassembly.

| Work | Result | Boundary |
|---|---|---|
| Public control/header + exact NVML analysis | ABI/units/support constraints; legacy transport and corrected NVOC handler record | Selected paths recovered, not the whole RM |
| Exact X→RM voltage dispatch | Signed µV frontend reaches an exact kernel setter that clamps negative requests to 0; getter min 0 | Recovered overvoltage control cannot undervolt; active GP108 max/telemetry unresolved |
| Internal RM voltage objects | Type2→internal3 parser/constructor and allowed-point PWM setter | Alternative full V/F-policy implementation/calibration remains open |
| Full VBIOS/archived C55 | Prefix/init versus full PCIR length, CBFS byte identity, table metadata and unsupported decoder hazards | No new live `_ROM`, ROM edit or flash |
| Board voltage/EC protection | GPU PWM→NCP81278T VID pin 5; GPIO216 throttle policy, board PROCHOT and unnamed host bit; corrected alert GPIO104/F9 | Active pin/policy state and complete sensor meanings unmeasured; optional EC UART disabled in stored config |
| Getter inventory | New [`mx150-capabilities.py`](../../tools/re-audit/mx150-capabilities.py); approved getters returned legacy ranges and 32 modern successes, power usage/limit unsupported |13 fake-library tests; no setter; GPU D3cold before/after |
| Existing-session voltage exposure | `nvidia-settings`/libXNVCtrl absent; Xlib opens existing :0 and finds no NV-CONTROL extension | No package/X/Coolbits changes; voltage telemetry unread |
| Sleep diagnostic | Truthful file observations for Intel/offload/s6; approved stdin invocation found no detected failures/unknowns |8 fake-root tests; not installed or a real sleep test |
| Sleep-hook error reporting | Observed live site hook already has hibernate/phase/synchronous resume; source-only snapshot now preserves real failure codes |14 mock lifecycle tests; not deployed; elogind may ignore nonzero status |
| Runtime DGON failure | Source-only patch 0029 leaves failed rail/link/reset off on PWRGD timeout |10 patch/actual-AML/complete-DSDT checks; no firmware deployment |
| CBFS ROM source bounds | Patches 0030/0031 validate mapped metadata/image chain and actual produced size; all 182272 MX150 nonpadding bytes preserved | Actual-source C sanitizer fixtures; no on-device/RAM/generic-load/VFCT or complete AML-handler repair claim |
| Software limits/use | CUDA 12.9 versus 13 build support, Pascal 580 lifecycle, September 2026 fixed-version threshold and media conflict | No new workload performance or feature claims |

The first combined power/SMART/new diagnostic/AML suite passed 56 tests with the matching
workstation coreboot DSDT integration enabled and no skips. Actual baseline and
patched DSDTs compiled with 0 errors, 0 warnings and unchanged 29 remarks.
Temporary owned Ghidra services were stopped after saving. Public material adds
derived facts, authored tools/tests and patches; raw proprietary output remains
private. The exact read-only capability observation is reviewable in the
[voltage report](12-mx150/01-voltage-and-controls.md#approval-boundary-and-concrete-next-observation).
Approval was requested because the user's earlier instruction requires asking
before any T480 command. The user explicitly approved those queries only.
NVML initialization/shutdown and getter calls, the file diagnostic and fixed
metadata reads ran without installation. The existing X session was queried
without exposing authentication contents or changing the server. Current site
sleep files were captured privately. No tuning, EC transaction, module reload,
process termination, stress/sleep/reboot or flash was performed.

Final combined validation passed **102 tests**, with the matching workstation
coreboot integration enabled and no skips: 21 power, 4 SMART, 13 NVML ABI/guard,
8 diagnostic, 10 patch/AML/DSDT, 14 hook lifecycle, 15 ROM bounds and 17 CBFS size
contract fixtures. Clang ASAN/UBSAN checks the extracted actual C functions;
decoder outcomes and storage are controlled stubs. The complete matching DSDT
compilation is separate from a full firmware build. Patches 0029–0031 also applied
sequentially with `git am` in a disposable source fixture and reversed to all
three exact baseline files. Shell syntax, Markdown links and non-patch whitespace
passed; applied-source whitespace was checked separately from mandatory diff
context spaces in the serialized patch files.
These are source/control-flow checks, not new electrical, sleep or energy results.

## 9. Integrated evening deployment and lid-wake record

Before publication, `testing` had advanced through
[the evening deployment](https://github.com/24bit192kHz/myt480/commit/0c6b22b9f98f0047bb881173f007b88e03b916c8),
[the successful lid tests](https://github.com/24bit192kHz/myt480/commit/cc8a961fdcd444b44b4f319dbf49c65bd429c409)
and a README correction. This unpublished MX150 work was rebased onto that
history without rewriting the remote commits. The tested MX150 code/fixtures
were preserved; deployment descriptions were reconciled with the new record.

The [separate evening notes](../../docs/notes/2026-10-09-evening.md) report the
new GPU/thermal guards, `bw-screen` Intel routing, effective NVIDIA hook/package
exclusion, watchdog/RTC changes, idle-GPU RTC S3/S4 cycles, and two owner-operated
lid wakes from S4 with `WAK PWRBTN`. Those are another run's recorded hardware
results, not actions or independent retests in the approved getter pass.
The lid hook enables EC byte `0x01` bit 6 without a firmware or ACPI change.
Its ordinary success is established by the reported tests; retained-state
baseline, asynchronous completion, cancellation/failure cleanup and complete
rollback remain separate questions. Current query-only approval authorizes no
additional EC access or sleep test.

## Final recorded state and preserved boundaries

The thermal daemon, GPU helper/wrapper, coarse RTD3 configuration, Intel helper
and desktop override were deployed. Backup/hash-guarded restoration was tested.
At the end of the earlier deployment pass, Bitwarden held NVIDIA devices and
the GPU was active/D0; its next normal relaunch adopts Intel routing. In the
later approved query pass the GPU was suspended/D3cold before and after NVML.
That observation supersedes the older D0 state without attributing a process
change to this continuation. CPU undervolt, retained GPU offset policy, fan
curve and configured power limits were unchanged. The last checked CPU plan
was `auto`; it was not queried again under the limited NVIDIA approval.

Firmware was not flashed; kernel/NVIDIA packages were not replaced; the selected
fingerprint backend was not switched; biometrics were not enrolled or deleted.
The integrated evening record reports idle-GPU S3/S4 and owner-operated lid wake.
Live-allocation GPU preservation, external docks, USB-C role swaps and controlled
battery discharge comparisons remain outstanding. No full recovery of proprietary
drivers or the EC firmware is claimed.

Raw analysis and selected live logs remain in private workstation evidence
directories. This publication adds derived research and authored fixes/fixtures,
without adding raw DLLs, full firmware images, keys or biometric captures.
