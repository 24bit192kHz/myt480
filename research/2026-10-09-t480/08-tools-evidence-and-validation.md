# Tools, evidence and validation

The audit used offline binary analysis, source fixtures and selected reversible
hardware checks. Each answers a different question: reconstructed control flow
is not hardware validation, and a hardware state transition is not a measured
battery saving. This summary consolidates the recorded results; it does not
represent a new deployment or test run.

The authoritative history is `2cb13a3` (fan/GPU fixes and initial analysis),
`fe58a80` (protocol continuation, routing, measurement and Rust cancellation),
and `7f5a430` (cross-stack assessment). The maintained
[evidence page](../../docs/wiki/evidence.md) contains artifact hashes, reproduction
commands and private log references; the [wiki index](../../docs/wiki/README.md)
links the subject reports.

## Tool attempts and fallback

| Tool/workflow | Actual outcome |
|---|---|
| [REA](https://github.com/morluto/rea), installed 3.2.1 | Could not start this environment's native workflow. This was a tool-start failure, not evidence that the target binaries could not be analyzed. |
| `rea-agents@6.1.0` | Worked with explicit `GHIDRA_INSTALL_DIR` and `JAVA_HOME` pointing to Ghidra 12.1.4/JDK 21. Doctor passed; native fingerprint, thermal, SmmAslSmi and EcIoSmm analysis succeeded. CLI and temporary stdio MCP clients were used. |
| REA NVIDIA native workflow | Timed out after 330 seconds on the 143,576,728-byte `nvidia.ko`. |
| [bethington/ghidra-mcp](https://github.com/bethington/ghidra-mcp) fallback | Built dev version 7.0.0 at checkout `9cc29c0f1efb6c63a7d6898c9a23aff39397f992` with its Gradle wrapper. A localhost headless server, 8 GiB heap, imported without auto-analysis and served targeted function requests. Full auto-analysis subsequently completed. |
| [awesome-reverse-engineering](https://github.com/alphaSeclab/awesome-reverse-engineering) | Inspected as a tool index. Ghidra, ELF symbols/objdump, UEFI extraction, `iasl` and `innoextract` were useful; every linked tool was not installed or evaluated. |
| Later EC Ghidra MCP continuation | Supplied N24HT37W imported with a locally built, pinned ARC processor contribution; selected vectors, host/cache tables, lid callbacks and power-button sequence analyzed. |
| GNU Binutils 2.45 ARC decoder | Private workstation build independently corroborated vectors, conditional instructions, INI3 initialization, dispatch tables and selected wake paths. |

The fallback recovered 61,109 symbol-backed NVIDIA functions. This count is not
61,109 independently verified implementations. ELF relocation warnings, including
unhandled `R_X86_64_PC64`, and unresolved kernel externals limit type/control-flow
interpretation. Ghidra addresses are relocated analysis addresses, not file offsets
to patch. [NVIDIA findings](../../docs/wiki/nvidia.md) record the selected RM paths.

A temporary Python MCP helper saved successful results before raising a reporting
exception because it used `isError` instead of the SDK's `is_error`. Saved output
and server status were checked directly; this exception was not counted as failed
native analysis. Temporary servers/clients were stopped after saving results.
The fallback listened only on `127.0.0.1:8099`, not the LAN, and was not registered
as a permanent MCP service.

## Public findings and private artifacts

Public material consists of narrow derived findings, hashes, source changes,
fixture tests and reproduction instructions. Exact artifact identity matters:
the DLL hash in [fingerprint research](06-fingerprint.md) binds its addresses;
the [evidence hash table](../../docs/wiki/evidence.md#evidence-anchors) binds NVIDIA,
live ACPI and extracted stock firmware modules. The archived C55 payload/module
hashes bind the NVMe review to that candidate build, not a fresh live SPI dump.

Private evidence remains outside the repository under
`/home/btw/test/rea/work/`, in these audit directories:

| Directory | Retained evidence |
|---|---|
| `audit-20261009` | Initial analysis, deployment, fan pause, five-round GPU tests and rollback/restoration logs |
| `audit-20261009-fingerprint` | Fourteen successful REA result bundles, protocol/worker reconstruction and cancellation regressions |
| `audit-20261009-usbc` | DXE/SMM/EC reconstruction, source capture, hashes and offline model results |
| `audit-20261009-power` | Identity-free 20-second AC capture/report and manifest |
| `audit-20261009-routing` | GLX/EGL/Vulkan, SMART, routing rollback/reapply and final checks |
| `audit-20261009-cross-stack` | Mocked early-init harness, NVMe vectors, firmware checker fixtures and selected live-state logs |
| `audit-20261009-ec` | Supplied ISO/EC extraction, hashes, ARC processor/build, Ghidra project/results, independent GNU code/table checks and publication review |

The publication boundary excludes proprietary DLLs, full firmware images,
complete proprietary decompiler output, keys, laptop identities and biometric
material. Private artifact paths document where evidence was retained; they are
not downloadable repository assets.

## Validation results and what they mean

These counts overlap where stated and should not be added into one score.

| Check | Recorded result | Interpretation |
|---|---|---|
| Power/SMART Python suite | **25 tests passed: 21 power + 4 SMART** | Fixtures cover units, counter validity/wrap, source/battery rules and parsers; not fuel-gauge accuracy or energy savings. |
| Intel launcher | **4 fixtures passed** | Inherited routing/filter removal, missing-driver failure, process semantics and usage. Live Intel GLX, X11 EGL and Vulkan checks also passed. |
| Offline UCSI model | **6 tests passed** | Packet layout, ordering, busy/completion boundaries, CCI and initialization agree with the recovered model; no live EC/USB-C transport was exercised. |
| Rust suite | **44 tests reported passing**, including **7 new USB fixtures** | Hardware-free cancellation regressions passed. Optional Python-generated golden data was absent; conditional paths did not freshly validate cross-language agreement. Six existing dead-code warnings remained. |
| Cancellation before/after | Old policy: **4 passed, 2 failed**; fixed policy: **7 passed** | Demonstrates the source starvation defect and fix; does not measure reader shutdown latency or deploy the Rust backend. |
| Initial fan/GPU source checks | Emulated fan and NVML harnesses, PRIME wrapper fixtures and compilation passed | No real MSR/fan/GPU access in these fixtures; detailed cases are in [thermal](../../docs/wiki/thermal.md) and [NVIDIA](../../docs/wiki/nvidia.md). |
| Live fan fallback | Daemon paused for **125 seconds**, sampled every **25 seconds**; fan reached `auto` by sample 5, maximum sampled CPU temperature **49°C** | Demonstrates kernel fan-watchdog fallback on this T480, not recovery from a kernel lockup. |
| Live GPU checks | **5 cold-wake rounds** passed CUDA `vectorAdd`/`scan`, held-job offsets and idle D3cold; PRIME OpenGL passed; no new Xid found | Confirms this firmware/driver/GPU combination. Wrapper-plus-job times **1.249–1.277 seconds** are not isolated hardware wake latencies. |
| Power observation | **21 snapshots**, **19.995 seconds**, CPU package **63.678 J / 3.185 W** on AC | CPU-package observation only; system discharge was correctly unavailable. No whole-laptop watts or battery-runtime gain claimed. |
| Cross-stack fixtures | Early-init ordering, NVMe status/queue vectors and failing firmware-checker cases reproduced offline | Early-init assumes successful matching-policy unseal and mocks hardware; no live TPM exploit, controller timeout or flash was performed. |
| Later GPU policy fixes | **12 new fixtures passed**, existing **5 wrapper fixtures** and NVML checks passed; warning-free `-Werror` build | Fake sysfs/modprobe and real users locks; source-only, no laptop deployment or NVIDIA hardware access. |
| Later QEMU verdict fix | **6 fixtures passed** | Failure/unknown-selector exit contract checked without starting QEMU; no new firmware boot-policy result. |
| EC static continuation | Independent ARC vector/INI3/table/branch agreement | Selected firmware paths recovered; live flag state, rollback and successful S4 wake remain unverified. |

Desktop-file validation, shell syntax, local wiki links and whitespace checks
passed in the recorded continuation. Original hash restoration and post-restoration
checks passed; a GPU reload was correctly refused while an application held the
device. Intel routing also passed a complete removal/reinstallation round trip.
See [rollback](../../docs/wiki/rollback.md) and
[routing restoration](../../docs/wiki/gpu-routing.md#revert).

## Reproduction and remaining work

Use the [verification commands](../../docs/wiki/evidence.md#verification-commands)
for hardware-independent checks. Analyze artifact copies after checking hashes;
target a small function bundle before expensive full analysis. ACPI comparisons
must distinguish actual runtime tables from stock reference tables. Read the
individual hardware script's side effects and restoration prerequisites before
running it; the RTD3 script reloads NVIDIA and is not a passive inventory tool.

No recognition-accuracy, real suspend/hibernate, dock, battery-transition or
battery-runtime result was established by this audit. The running Bitwarden
instance retained NVIDIA handles despite the new next-launch Intel override.
Measure its ordinary relaunch separately before claiming improved idle behavior.
The [cross-stack review](../../docs/wiki/cross-stack-review.md) prioritizes mocked
failure tests and reviewed fixes for boot trust, NVMe recovery and sleep ordering
before further hardware experiments. The UCSI reconstruction still needs a
serialized transport and notifications; setting kernel options alone is insufficient.
