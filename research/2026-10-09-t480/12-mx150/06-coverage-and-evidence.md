# MX150 coverage and evidence

## Coverage ledger

This table accounts for the relevant boundaries of the MX150 stack. **Reviewed**
means a bounded source or selected control path is understood; it does not mean
every routine, failure case or workload has been recovered. Open entries identify
the next evidence needed, rather than asserted failures.

| Area | Established in this or the prior audit | Open dependency |
|---|---|---|
| Identity/software | Approved PCI `10de:1d10` and active 580.178.04; exact retained artifacts, earlier C55/kernel baseline | Fresh installed firmware/board identity |
| CPU versus GPU undervolt | Existing CPU voltage planes; GPU PWM regulator; recovered overvoltage handler clamps negative to zero | Alternative V/F policy, voltage telemetry, calibration and stability |
| Public clock/power controls | Approved legacy ranges and 32 modern getter successes; current power usage/limit unsupported | Independent P-state behavior/setters, application clocks, valid cap and energy measurements |
| NV-CONTROL voltage | Attributes 412/413, signed frontend, exact kernel min 0/clamp, support/Coolbits gates; existing X has no extension | Active GP108 voltage object/support/max and telemetry |
| Closed RM | Corrected dispatch record; exact voltage descriptor setter/getter, selected parser/PWM paths | Remaining legacy-clock downstream policy, calibration, table locator and full newer dispatch |
| Runtime D3 | Earlier coarse mode/cold CUDA checks; console guard/ref bookkeeping traced | Fine-mode eligibility, wider transitions, holder behavior and measured idle energy |
| Offset lifetime/resume | Earlier loss/restoration observed; selected state restore/error paths traced | Exact reset/store instruction and all failure unwind paths |
| Full VBIOS | Exact full/prefix/archive hashes, PCIR/init lengths, table metadata | Supported v0x30 semantics, active selection, signature/edit acceptance |
| ROM-to-ACPI source | Intended 182272-byte exposure; source patches 0030/0031 bound CBFS probe/copy and actual produced size | Hardware validation/live `_ROM`; on-device/RAM/generic load/VFCT and small-image AML contracts |
| Board voltage/rail/reset | Schematic + VBIOS PWM/throttle agreement; coreboot GPIO sequence | Installed drawing revision, active electrical/calibration behavior |
| EC GPU policy | GPIO216 throttle mapping/gates; corrected thermal alert GPIO104/F9; optional EC UART disabled in production | Sensor/state meaning, GPIO104 consumer/mux, policy callers, arbitration and active transitions |
| ACPI rail timeout | DGON discrepancy reproduced; patch 0029 passes actual AML and complete DSDT checks | Hardware unwind, OS failure propagation and DGLW result contract |
| System sleep | Approved site-hook/files; separate evening idle S3/S4 and S4 lid results; source-only truthful hook status | Effective default owner, storage/locker/failure readiness and live-allocation preservation |
| PRIME/desktop | Previous MX150 OpenGL, Intel GLX/EGL/Vulkan, holder and routing evidence | MX150 Vulkan, Wayland, display/dock and application coverage |
| CUDA/OpenCL/frameworks | Earlier checked CUDA rounds; Pascal compiler lifecycle and Nouveau GP108 constructors reviewed | Installed compiler/library architecture audit, alternative-driver power and workload-specific support |
| Media | No supported NVENC; official decode documentation conflict recorded | Device profile queries and actual checked decode behavior |
| Thermal/power | Shared EC path and earlier fan failover/clock stability margin | Joint CPU/GPU energy/thermal behavior, EC policy meanings and battery A/B/A |
| PCIe/error recovery | Earlier ASPM/payload/reset workaround source; failure concerns documented | AER/Xid/timeout fault unwind, bus/rail recovery and VFIO isolation requirements |
| Security/update | September 2026 NVIDIA 5861 bulletin names 580.178.04 fixed threshold; legacy/open-module limits and source bounds fixes | Applicable entry classification, broader ioctl/backend audit and later bulletins |

This is an explicit boundary map, not a promise that the 61,679-function
proprietary driver has been fully reconstructed. No complete GPU microcode,
memory-controller training, signed firmware authentication or entire EC scheduler
implementation has been recovered. VFIO/vGPU, a new BIOS or a bypass of EC
protections is not delivered by inventorying an interface.

## Evidence identities

| Artifact | Size / identity | SHA-256 |
|---|---|---|
| Exact kernel module |143576728B; NVIDIA 580.178.04 |`99946c338065a775ce13fdb49179440d2ae9732767545e77524c3078fb2a0ce1` |
| Exact NVML library |2291896B; NVIDIA 580.178.04 |`451ad740fe549ed7023570fda15ba2762f6fb29ea50afe468b0eb5e74246a4c1` |
| Exact X driver |6832688B; NVIDIA 580.178.04 |`20a24f9ee63f491860419c9d12b5699288379494572b197abee9eb326013c33b` |
| Full VBIOS |184320B |`92936e91fbb591086473366fa6a73c40ec2450e58a90489fba83ce4b58803d82` |
| Init-prefix reference |119808B |`46f03d4eeb2e8e2c78b7b8c1a98499ef991d5066e0d349011da6ad140f07481f` |
| Archived C55 image |16777216B; archive binding only |`63c5ddbe65a9c51da845b83dc4540df706488b76bcee49d8bbca0825ee6b1576` |
| Supplied EC payload |N24HT37W; see parent EC extraction record |`befcb425b9f2a83b959c0ef3ee490420b7c5e5ffa2a792fda821d0ff355e77cf` |
| Exact NV-CONTROL header |580.178.04 |`bd8363bfcea8e9b7df98c78d5d29afcc0077a8db7ff726d1d4d0849525edcbae` |
| Contemporary NVML header |CUDA 13.0.87 interface reference |`28b51fbd44df16adf1e58229778414a4d1e7e05fdd4a74526ef0affb75f18416` |
| Public schematic |Windu-2/NM-B501; installed revision unmeasured |`28f0b458a8a0c510e8bd884b274823b1a5b8fd7d3b456a243b997285d4a7aca2` |
| onsemi regulator datasheet |NCP81278D/T |`bc8a6996b260ae71edc44efd83248a112c5c48880eeb93f7bff41ac580ecdff1` |
| Current security bulletin 5861 |NVIDIA primary-source snapshot |`a8a11d16854917716f52e5f86257caaf66c1019eb08cbbe04515a258d31c02ef` |

Private evidence lives in the workstation's `audit-20261009-mx150` and parent EC
analysis folders: supported-interface audit, selected-function manifest, saved
Ghidra project, independent ELF relocation/disassembly, VBIOS metadata,
primary-source hashes and EC policy notes. The closed-driver bundle has a hashed
135-file manifest. The later voltage continuation records 34 selected functions
and a 56-file hash manifest; these sets overlap and are not additive recovery
counts. Corrected NVOC metadata and independent GNU instruction checks establish
the negative clamp and min 0 result. Private approved getter/file/Xlib captures
record the limited target observations. Temporary owned MCP server/client
processes were stopped after saving. Public reports contain derived facts and
authored code, not full proprietary decompilation or raw machine captures.

## Offline validation and limits

The new capability probe is verified with 13 fake-library tests: ABI/version,
domain/P-state inputs, getter allowlist, units, unsupported/missing/permission/
argument/version errors, wrong/unknown identity, initialization/lookup failures
and shutdown. The corrected diagnostic has 8 fake-root tests for actual script
behavior without device access. The existing 21 power and 4 SMART tests also pass.
Patch 0029 adds 10 focused patch/actual-AML/complete-DSDT tests.
That first combined suite passed 56 tests with the actual workstation coreboot
integration enabled and no skips. The final suite passed **102 tests**, including
14 sleep-hook, 15 actual-source ROM and 17 CBFS size fixtures, with no skips.
Clang ASAN/UBSAN checks the actual C paths; decoders/storage are controlled stubs.
Patches 0029–0031 applied sequentially with `git am` in a disposable fixture and
reversed to the exact baseline. See
[the work log](../00-work-log.md#8-mx150-voltage-closed-driver-and-ec-protection-continuation).
Firmware checks and limits are recorded in [the sleep report](04-sleep-and-firmware.md)
and [VBIOS report](03-vbios-and-ec.md).

The user approved the reviewed query-only scope, and actual NVML getters,
fixed file reads and existing-session Xlib extension queries were run. The GPU
was observed suspended/D3cold before and after; no new setting was assigned.
There were no voltage/clock setters, EC reads/writes, module reloads, process
termination, sleep/reboots or flashes in this continuation.
No additional watts, performance gain, battery life, S4 lid wake or complete
allocation-preserving sleep result is claimed by this continuation. The
integrated evening notes separately report idle-GPU S3/S4 and two successful
owner-operated lid wakes; this pass did not repeat them. The completed approved
observations and the remaining approval boundary are specified in
[the voltage report](01-voltage-and-controls.md#approval-boundary-and-concrete-next-observation).

## Later 610/615 upgrade investigation

The [upgrade report](07-driver-upgrade-and-pascal-support.md) and
[artifact manifest](08-upgrade-evidence.json) add official release verification,
cross-branch READMEs/public-source snapshots, static extraction of exact 610/615
packages and comparison with retained 580. Three proprietary initialization
blocks were recovered: legacy rejection, cleared Pascal physical HAL entries,
and omitted GP108 object registration after slots are zeroed. Ghidra MCP and
independent GNU/ELF analysis corroborate selected lookup/registration paths.
Shared Pascal fragments remain; complete removal/recovery of every backend
routine is not asserted. Kernel/userspace bridging, complete GP108 restoration,
actual Vulkan feature reports and measured application/energy benefit remain open.

This later pass ran no T480 command and built/installed no patched driver.
The preceding 102-test suite covers earlier authored fixes, not a 610/615 port.
New documentation, JSON structure, source anchors and local Markdown links were
checked separately; no new GPU compatibility or performance test is claimed.
