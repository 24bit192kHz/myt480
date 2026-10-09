# Storage, display, and other inspected hardware

These are targeted observations from audit commits `fe58a80` and `7f5a430`, not
a claim that every device, driver routine, or workload has been audited. The
checks did not alter NVMe power policies, force Intel graphics features, write
Thunderbolt firmware, or exercise a dock. See the
[detailed hardware wiki](../../docs/wiki/storage-display-thunderbolt.md) and
[subsystem inventory](../../docs/wiki/subsystems.md).

## NVMe health: a read-only result, not a lifespan forecast

The inspected drive is a Toshiba XG6 `KXG6AZNV256G`, firmware `5108AGLA`.
The live APST latency budget is **25,000 µs**. This permits eligible power
states; it does not measure their residency or justify tightening that budget.

The new [`nvme-health.py`](../../tools/re-audit/nvme-health.py) opens the
controller read-only and issues only Get Log Page for the 512-byte SMART log,
with a five-second command timeout and Retain Asynchronous Events set. It
does not issue Set Features, firmware, format, sanitize, or self-test commands,
and excludes drive serials. A health query can wake storage, so it was kept
separate from the passive energy capture.

| SMART observation, 2026-10-09 | Value |
|---|---|
| Critical warning | 0 |
| Composite temperature | Approximately 27°C |
| Available spare / threshold | 100% / 10% |
| Estimated endurance used | 26% |
| Media/data-integrity errors / error-log entries | 0 / 0 |
| Warning / critical temperature time | 0 / 0 minutes |
| Power-on hours | 10,651 |
| Historical unsafe shutdowns | 855 |

The live read succeeded. Four
[fixture tests](../../tools/re-audit/tests/test_nvme_health.py) validated the
x86-64 request/ABI, large 128-bit fields, temperature conversion and sparse
sensor numbering, and malformed log rejection. Missing sensors remain `null`
in their original slots.

Zero error counters are a snapshot, not a future reliability guarantee; 26%
endurance used is not a remaining-life countdown. The historical unsafe-shutdown
count cannot be assigned to one firmware version, crash, or this audit. Keep
independently recoverable backups and dated health observations. Latency/error
and energy measurements are needed before an APST change or firmware update.

## Intel graphics: the actual feature state

The live UHD620/i915 debug state showed **FBC enabled and compressing**, with an
eligible display plane. DMC firmware `i915/kbl_dmc_ver1_04.bin` version 1.4 was
initialized and loaded. DC3→DC5/DC5→DC6 counters were zero at the observation;
the Intel GPU was idle while its active display kept the PCI device in D0.
The PSR status endpoint returned `ENODEV`.

Automatic parameters (`-1`) do not imply a feature is disabled: FBC was already
working. The unavailable PSR status does not establish a panel fault or its
cause. Likewise, display-driving Intel D0 is distinct from an otherwise unused
MX150 held awake by another application. No PSR/GuC/DC override was deployed.

The deployed [`igpu-run`](../../src/gpu-power/igpu-run) and application override
are covered by [GPU routing](../../docs/wiki/gpu-routing.md). Intel GLX,
X11 EGL, and Vulkan selection were verified there. Compare display state,
brightness, workload, and package residency before forcing a display feature;
later tests must cover flicker, playback, external monitors, and resume.

The additional cross-stack pass confirmed GVT's configured mdev and captured
firmware state. A missing `golden_hw_state` file followed the supported fallback,
not a demonstrated virtualization failure. A DP adapter warning and repeated
TSC-adjust restoration messages remain events to correlate with connectors and
resume. See the [cross-stack findings](../../docs/wiki/cross-stack-review.md).

## Thunderbolt host and USB-C transport

The JHL6240 Thunderbolt NHI is bound at **05:00.0**, with readable NVM **23.0**.
The Lenovo critical-update bulletin's T480 minimum is NVM 20, so this observation
is above that minimum; it does not establish the newest firmware or dock
compatibility. No controller firmware was written.

Domain security is `none`, and `iommu_dma_protection=0`. VT-d is independently
enabled, but the NHI's current IOMMU group 13 uses **identity mapping**, shared
with 04:00.0. Therefore the presence of VT-d does not prove translated DMA
isolation. **07:00.0 is the xHCI controller, not the NHI.** These observations
are separate from the missing USB-C UCSI connector-control/notification path.
The [USB-C reconstruction](../../docs/wiki/usb-c.md) documents the recovered EC
mailbox and the transport/synchronization work still required.

There was no attached dock in this pass. Charging, USB devices, external
display, PCIe peripherals, hotplug, and suspend/resume need actual dock/topology
coverage before changing authorization or DMA policy.

## Other policy observations and their limits

| Area | Inspected state | Next useful validation |
|---|---|---|
| Wi-Fi | Intel 8265/8275; iwlwifi power saving off on AC; TLP requests it on battery | Throughput, latency, reconnect/resume, and discharge under the battery profile |
| Audio | HDA idle power saving 0 on AC; TLP requests 1 second on battery | Pops/clicks, stream stability, and wake latency |
| Ethernet | I219-LM; existing AC-only wake policy retained | Wake sources, link behavior, and overnight discharge |
| PCIe | Global ASPM `powersupersave`; GPU-port workaround retained | Device-specific measurements before removing a known freeze workaround |
| Batteries | Two packs; configured 85/90% charge thresholds | Verify applied thresholds and repeat valid discharge measurements |
| USB/fingerprint | Two readers; libfprint selected, alternate services intentionally down | Reader-specific lifecycle/recognition tests; [dedicated protocol work](../../docs/wiki/fingerprint-protocol.md) |
| Recovery/sleep | Current TCO and soft/NMI detectors confirmed; `deep` selected | Real suspend/hibernate and checked error-path sequencing |

These entries describe policy/status inspection, not completed throughput,
audio-quality, recognition-accuracy, or sleep-reliability tests. No targeted
current Xid, NVMe timeout/reset, iwlwifi error, or AER failure appeared in the
inspected retained log; this does not cover every previous boot.

The [collector](../../tools/re-audit/collect.sh) and
[power sampler](../../tools/re-audit/measure-power.py) make future comparisons
repeatable. Keep storage health queries and GPU renderer/NVML probes outside
idle-energy captures. Follow the [daily-use guide](../../docs/wiki/getting-the-most.md)
and [restoration procedures](../../docs/wiki/rollback.md); prioritize a measured,
reversible change rather than treating every available power knob as a defect.
