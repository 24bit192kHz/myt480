# Voltage and supported interfaces

## What can be undervolted now

The i7-8650U is already configured with CPU/cache offsets of −115 mV on AC
and −100 mV on battery. iGPU/uncore settings and prior error-checked sweeps are
recorded in the [daily guide](../../../docs/wiki/getting-the-most.md#keep-the-tested-margin)
and [CPU research](../../../docs/notes/2026-09-30-undervolt.md).
Those are CPU voltage planes; they do not undervolt the discrete MX150.
This pass leaves them unchanged. CPU instability can also corrupt a GPU workload,
so a future GPU comparison must keep the existing CPU settings fixed.

For the MX150, a physical voltage-control route and internal driver implementation
are established. A supported negative user adjustment is not. NVIDIA's exact
NV-CONTROL header exposes an **overvoltage** offset and a conditional voltage
reading. Exact kernel analysis now establishes that its global voltage backend
clamps negative requests to zero and reports a minimum offset of zero. This
recovered interface cannot supply a negative undervolt offset.

Static analysis of the exact X driver now confirms that the frontend forwards
a signed 32-bit µV offset and reports backend min/max values without a fixed
positive-only clamp in the frontend. The lower RM handler performs that clamp.
Support still depends on the selected performance interface, a returned voltage
descriptor flag and Coolbits 16; active GP108 support/max/telemetry remain
unqueried. An alternative V/F-policy route is not delivered by bypassing the
frontend. See
[the recovered dispatch](02-closed-driver.md#nv-control-voltage-reaches-rm-transport).

## Exact interface inventory

| Interface | Documented behavior | Evidence for this MX150 |
|---|---|---|
| Legacy NVML GPC/memory VF offsets | Signed frequency offsets and range getters | Previously accepted AC +200/+1250 MHz and battery 0/0; restored by `prime-run` after runtime wake |
| Modern NVML `Get/SetClockOffsets` | Versioned domain/P-state record; Maxwell+ on fully supported devices | Approved getters accepted all 32 domain/P-state queries; independent per-state behavior and setters untested |
| GPU locked clocks | Volta+ on fully supported devices | Pascal is outside documented generation; RTX fixed-clock recipes cannot be assumed to work |
| Memory locked clocks | Ampere+ in the contemporary NVML header | Pascal is outside documented generation |
| Application clocks | Exact 580 CLI documents Maxwell-based GeForce and Kepler+ Tesla/Quadro/Titan | No demonstrated Pascal MX150 support; saying all GeForce lacks the interface would be incorrect |
| NVML power cap | Getter/setter support depends on the device; units mW | Fresh usage/current-limit getters return `NOT_SUPPORTED` (3); default/constraints report implausible 5001 W metadata; no usable cap/draw established |
| NV-CONTROL 409/410, 424/425 | Graphics/memory frequency offsets; Coolbits 8 | NVML tuning works; current Intel X session has no NV-CONTROL extension |
| NV-CONTROL **412** | `GPU_OVER_VOLTAGE_OFFSET`, units **µV**, Coolbits 16 | Exact lower RM handler clamps negative requests to 0 and reports min 0; active MX150 eligibility/max unresolved |
| NV-CONTROL **413** | `GPU_CURRENT_CORE_VOLTAGE`, read-only, conditional on support for 412 | Usable MX150 voltage telemetry unresolved |
| Public NVML voltage or full V/F point API | None found in the contemporary public header or exact 580 exports | Internal RM voltage machinery exists; public API absence is not proof of fixed silicon voltage |
| Public Windows NVAPI Pstates20 | Getter includes voltage/clock records; documented Windows interface | Not evidence of a Linux setter or a usable Linux full-curve editor |

Sources: [exact NV-CONTROL header](https://github.com/NVIDIA/nvidia-settings/blob/580.178.04/src/libXNVCtrl/NVCtrl.h),
[contemporary NVML development archive](https://developer.download.nvidia.com/compute/cuda/redist/cuda_nvml_dev/linux-x86_64/cuda_nvml_dev-linux-x86_64-13.0.87-archive.tar.xz),
[exact Coolbits documentation](https://download.nvidia.com/XFree86/Linux-x86_64/580.178.04/README/xconfigoptions.html),
and the exact package's `nvidia-smi.1.gz`. The CUDA 13 header is an interface/ABI
reference, not a recommendation to compile Pascal programs with CUDA 13 or a
claim that it was the driver's exact build header.

## Units and claims that matter

The legacy memory setter halves the requested offset before internal clock
scaling. Prior +1250 changed reported memory clock from roughly 3003 to
3628 MHz, approximately +625 MHz. NV-CONTROL describes memory offsets in transfer
rate MHz. Do not compare a transfer-rate offset directly with physical memory
clock, or multiply units again without checking the reporting API.

A positive core offset changes frequency at a given voltage; automatic boost
may use that headroom for more work. It does not command fewer millivolts or
prove lower energy. A defensible undervolt result needs measured lower voltage
at matched frequency/performance, checked output and energy measurements.
If voltage telemetry remains unavailable, report performance/energy tuning
without calling it a demonstrated undervolt.

The [VBIOS/board report](03-vbios-and-ec.md) identifies GPU GPIO0 → PWM VID →
NCP81278T regulator. The EC's power-limit pin is separate. Clearing an EC
throttle predicate, forcing a raw PWM tuple, enabling a registry string, or
editing ROM checksums is not an established reversible undervolting method.

## Prepared inventory, not a tuning script

[`mx150-capabilities.py`](../../../tools/re-audit/mx150-capabilities.py) allows
only NVML getters plus initialization/shutdown. It checks the selected bus and
product name, records individual API return codes, signed offset ranges,
P0–P15 modern offsets, clocks and power metadata. A missing symbol, unsupported
operation, permission error, bad argument, version mismatch and GPU loss remain
distinct. Failed getters never become fabricated zero values.

It does not call voltage/clock/power setters, load an absent driver explicitly,
enable Coolbits, change X configuration or issue undocumented RM controls.
NVML initialization/queries can nevertheless wake an idle GPU. Product-name
verification is not an independent PCI-ID capture, successful power metadata is
not proof of a usable cap, and unsupported P-states are not hardware failures.
Thirteen fake-library tests cover the ABI, statuses, identity guards and cleanup;
the approved live getter results are recorded below. The tool was fed over SSH
without installing a file or changing a setting on the laptop.

## Approved getter observations

The user explicitly approved only the reviewed NVML, file-only diagnostic and
existing-session voltage/range queries. Read-only PCI metadata confirmed
`10de:1d10`; active driver is 580.178.04 and NVML 13.580.178.04. Before the probe
the GPU was suspended/D3cold. The capability inventory initialized, queried and
shut down successfully. Later sysfs observation again showed suspended/D3cold.

| Observation | Approved result | Interpretation |
|---|---|---|
| Legacy frequency offsets |0/0 MHz after query wake; core range −200..1200, memory−2000..2000 | Ranges are accepted API limits, not validated stable settings; retained policy still +200/+1250 AC |
| Modern frequency getters | Graphics and memory P0–P15 all return success, 0 offsets and the same respective ranges |32 queries accepted; may expose common legacy policy, not proof of 16 independent states or setter behavior |
| P-state/clock reports |P0; graphics/SM 1480, memory 3003, video 1328 MHz | Momentary driver reports, not a measured workload/stability result |
| Power usage/current limit |`NOT_SUPPORTED`, code 3 for both | No GPU wattage or adjustable cap proved |
| Default limit/constraints |5,001,000 mW default; constraints 1..5,001,000 mW | Implausible metadata; not a physical 5001 W board limit |
| Existing X display |`:0` opens; Xlib reports no `NV-CONTROL` extension | Existing-session voltage query unavailable; no X configuration changed |

The expected `nvidia-settings` executable was absent (exit 127), as was
`libXNVCtrl`. A direct getter-only Xlib extension query in the existing user's
session succeeded and confirmed the missing extension. No package was installed,
no X screen/Coolbits was enabled and no alternate X server was started. Missing
session exposure does not prove absence of the GPU's physical voltage control.
Attribute 413 voltage remains unread; the exact static backend's negative clamp
remains independently established.

The unwrapped query wake observes zero offsets because runtime offset lifetime
is separate from the retained config/wrapper. No tune/reset command was run to
restore them during the observation; future intentional `prime-run` jobs use
the existing restoration policy. The range maximum is not a recommendation:
the earlier +250 core silent-error result still limits daily tuning.

## Approval boundary and concrete next observation

The user explicitly required asking before **any** T480 command and subsequently
approved the following query-only scope. Its results are above; the unavailable
existing-session NV-CONTROL query was bounded without changing the session:

1. Read PCI identity, module version, sysfs runtime state and existing sleep
   configuration using fixed file reads and the diagnostic in
   [the sleep report](04-sleep-and-firmware.md).
2. During an intentional active-GPU window, feed the reviewed capability script
   to `python3 - --bus-id 0000:01:00.0` over SSH. It changes no tuning settings,
   but may wake the GPU. Stop on an identity mismatch or failed initialization.
3. If the existing graphical session exposes an NVIDIA target, run existing
   `nvidia-settings --query GPUOverVoltageOffset --query GPUCurrentCoreVoltage`
   in that session's already authorized environment. Record target identity,
   validity/range, telemetry and errors. Negative-offset support is already ruled
   out for this recovered backend. Do not add an X screen, enable Coolbits, change
   authentication or interpret missing display/target as proof of no voltage hardware.

Approval of these observations does not authorize a setter, EC transaction,
module reload, workload stress, process termination or sleep/reboot. A future
settings trial needs a separate concrete candidate, full snapshot/restore
procedure and explicit approval. No negative request through the recovered
overvoltage control is proposed; it would be clamped to zero. A different
undervolt policy needs separate implementation/calibration/acceptance proof.
