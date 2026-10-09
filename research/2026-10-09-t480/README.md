# T480 research, changes, and recovery — 2026-10-09

This folder records the reverse engineering, live inspection, reversible changes,
and cross-stack review carried out on this T480. It accompanies the source and
the detailed [improvement wiki](../../docs/wiki/README.md); it is a dated research
record rather than an automatic deployment procedure.

The `testing` branch includes all three audit commits from `re-audit-20261009`,
starting at baseline `d89f79d`, followed by this organized publication. Read the
[complete work log](00-work-log.md) for what was actually changed and tested.

| Subject | Research report |
|---|---|
| Complete work record | [Changes, deployments, tests, and commit history](00-work-log.md) |
| Firmware and boot trust | [Coreboot, GRUB NVMe, TPM early init, and build checks](01-firmware-and-boot-trust.md) |
| Kernel and security | [Actual running configuration, DMA, mitigations, and sleep](02-kernel-and-security.md) |
| Graphics | [NVIDIA reverse engineering, RTD3, PRIME, and Intel routing](03-nvidia-and-intel-graphics.md) |
| Thermal and power | [Fan failover, retained tuning, measurement, and daily plans](04-thermal-and-power.md) |
| USB-C and Thunderbolt | [Recovered UCSI protocol and missing native interface](05-usb-c-and-thunderbolt.md) |
| Fingerprint | [Recovered commands, worker lifecycle, and source cancellation fix](06-fingerprint.md) |
| Storage and other hardware | [NVMe health, display, peripherals, and remaining checks](07-storage-and-other-hardware.md) |
| Tools and evidence | [REA/Ghidra workflow, validation record, and evidence limits](08-tools-evidence-and-validation.md) |
| Recovery and next work | [Verified rollback paths and prioritized remaining fixes](09-rollback-and-next-work.md) |
| EC and lid wake | [Supplied EC image, ARCompact decoding and selected wake paths](10-ec-firmware-and-lid-wake.md) |
| Whole-machine coverage | [Every recorded PCI function, other hardware and feature roadmap](11-coverage-and-feature-roadmap.md) |
| MX150 deep continuation | [Voltage controls, closed RM, VBIOS, EC throttle, sleep and CUDA coverage](12-mx150/README.md) |

## What improved

Manual fan control now has a kernel watchdog fallback, rapid temperature changes
cross the appropriate curve brackets immediately, and sensor/write failures
attempt EC automatic control. NVIDIA coarse RTD3 was demonstrated on this exact
MX150 while `prime-run` restores the existing clock offsets after wake. Intel
application routing was added to keep ordinary graphics discovery off NVIDIA.
Those changes were deployed with tested restoration paths.

The continuation also added passive power/SMART tools, an offline UCSI model,
and a source-only fingerprint cancellation fix. The later cross-stack review
identified remaining early-init, bootloader, sleep, and build-check defects;
those findings are documented and are not presented as completed fixes.
The workstation continuation also fixes GPU node/unload error reporting and
manual-off locking, plus the QEMU scenario runner's failure/selector exit status.
Twelve GPU policy and six runner-verdict fixtures passed. The subsequently
merged [evening record](../../docs/notes/2026-10-09-evening.md) reports deployment
of those fixes, thermal source/bounds guards and the effective NVIDIA hook.

The EC continuation uses the supplied Lenovo ISO for static Ghidra MCP and
independent GNU analysis. It identifies a retained lid gate and conditional
power-button path while keeping
the requested hardware-test approval boundary. A separate evening run subsequently
installed the bit-6 sleep hook and records two successful owner-operated S4 lid
wakes. No firmware flash or `_PTS` change was needed. This query-only continuation
did not repeat those EC writes or sleep tests; full cleanup/rollback cases remain
distinct from the two successful cycles.

The [MX150 continuation](12-mx150/README.md) maps actual voltage/clock interfaces,
selected closed-driver backends, the full VBIOS and regulator/EC throttle wiring.
It adds a getter-only capability probe, corrects the Intel/offload sleep diagnostic
and prepares truthful hook status and coreboot power-good/CBFS ROM bounds fixes.
Direct MX150
undervolting remains unresolved through alternate policy routes; the recovered
overvoltage backend clamps negative offsets to zero. Offline checks are
distinguished from hardware results. After explicit approval, getter/file/X-session
observations verified current capabilities without tuning or EC changes.

## Reading the evidence

**Deployed** means files/policy were changed on the laptop and checked there.
**Source-only** means the repository changed without replacing the live component.
**Offline reproduction** means a fixture or model reproduced a source behavior,
with assumptions stated. **Static recovery** means selected binary paths were
analyzed without sending their commands to hardware. **Open** means validation
or implementation remains outstanding.

Hardware baseline: T480, i7-8650U, 32 GB, Intel UHD620, GP108M MX150, Toshiba XG6,
coreboot C55, Artix/s6, Linux `7.2.8-7-t480`, NVIDIA `580.178.04`. Results apply to
this inspected combination. No claim is made that every proprietary routine or
every workload was recovered or tested.

For day-to-day use, see [getting the most out of this T480](../../docs/wiki/getting-the-most.md).
For restoration, start with [recovery](09-rollback-and-next-work.md).

The public research contains derived findings, code, test fixtures, and hashes.
Raw proprietary analysis output, full firmware images, keys, biometric data,
and private machine captures from this audit remain outside this publication.
