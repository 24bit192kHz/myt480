# Coverage and feature roadmap

The goal is to account for the whole machine and pursue useful, recoverable
improvements. This is the coverage ledger: an inventory item is not marked
reverse engineered merely because its driver loads. Selected paths are complete
enough to explain particular behavior; many proprietary components remain open.
The [work log](00-work-log.md) records completed work and
[daily-use guide](../../docs/wiki/getting-the-most.md) explains the tested settings.
The [complete findings and improvement index](13-findings-and-improvement-index.md)
collects the established results and all 30 improvement directions discussed
through the NVIDIA 610/615 continuation.

## Hardware coverage

The addresses/IDs below come from the repository's
[PCI snapshot](../../hardware/lspci.txt) and
[USB snapshot](../../hardware/lsusb.txt). They are inventory anchors, not a new
live probe. USB bus/device numbers and PCI topology can change with attached
hardware. Every PCI function in that snapshot is included in this table.

| Component / recorded PCI functions | Evidence already obtained | Remaining investigation |
|---|---|---|
| CPU/system agent/DRAM `00:00.0` | i7-8650U, 32 GB; microcode and actual mitigation/thermal/power state checked | Error-checked sustained mixed load, memory timing/stability, FSP initialization and microcode internals |
| Intel graphics `00:02.0` | FBC/DMC state, GLX/EGL/Vulkan routing and GVT fallback inspected | Display feature paths, PSR cause, panel/external-monitor behavior, GVT workload validation |
| Processor/PCH thermal `00:04.0`, `00:14.2` | RAPL/temperature and user controller source reviewed; fan failover deployed | DPTF/thermal-controller interactions, trip ownership and error behavior |
| Gaussian Mixture Model `00:08.0` | Firmware exposure recorded | Actual consumers, driver requirements and useful workload; no performance claim |
| PCH xHCI `00:14.0` | Attached devices and USB power policy recorded | Controller recovery, autosuspend/resume, per-device energy and disconnect behavior |
| PCI root ports `00:1c.0`, `00:1c.6`, `00:1d.0`, `00:1d.2` | Topology/ASPM policy and known GPU-port workaround recorded | Port-specific link/payload/latency and AER/recovery paths under representative load |
| LPC/eSPI `00:1f.0` | EC host interfaces and stock UCSI transport decode reconstructed | EC internal firmware, lid/wake handling, keyboard/battery/charger state machines |
| PMC `00:1f.2` | Package residency and watchdog state inspected | Deep-state blockers across displays/devices and verified sleep transitions |
| HDA `00:1f.3` | AC/battery idle policy inspected | Codec/pin routing, pops, headset detection, latency and suspend reliability |
| SMBus `00:1f.4` | Controller and kernel support inventoried | Client topology and battery/sensor ownership; avoid competing raw transactions |
| Ethernet `00:1f.6` | I219-LM and existing wake policy recorded | Throughput, link power and wake/overnight discharge validation |
| NVIDIA `01:00.0` | Prior CUDA/PRIME/RTD3; approved getters; selected RM voltage clamp/PM, VBIOS and board/EC protections; missing 610/615 GP108 physical selection/registration mapped in [MX150 research](12-mx150/README.md) | Alternative undervolt policy/calibration, remaining RM/microcode, sleep allocations, MX150 Vulkan, energy and display transitions; no working 610/615 port |
| Wi-Fi `02:00.0` | Intel8265/8275 and source-dependent policy inspected | Firmware internals, roaming/reconnect, throughput/latency and measured power |
| Thunderbolt bridges `03:00.0`, `04:00.0`, `04:01.0`, `04:02.0` | Topology, NVM23, security/DMA policy inspected | Dock hotplug, authorization, PCIe tunneling and recovery with translated DMA |
| Thunderbolt NHI `05:00.0` | Correct function/group identified; identity mapping documented | Host/controller firmware protocol and dock/sleep coverage |
| Thunderbolt xHCI `07:00.0` | Distinguished from NHI | USB-C/dock device, unplug and resume behavior |
| NVMe `08:00.0` | SMART/parser/APST observation; bootloader NVMe failure paths reviewed | Linux NVMe residency/error recovery, controller firmware and measured policy comparisons |

| USB/other component | Evidence already obtained | Remaining investigation |
|---|---|---|
| Bluetooth `8087:0a2b` | Present in stored inventory | Radio coexistence, firmware, reconnect and autosuspend |
| Camera `13d3:56a6` | Present in stored inventory | UVC modes/controls, privacy/indicator behavior, resume and power |
| Internal fingerprint `06cb:009a` | Historical driver reset/stop/worker paths and optional Rust cancellation fix | Selected-backend USB lifecycle, recognition testing and bounded drain/phase behavior |
| External fingerprint `2541:0236` | Working backend selection retained | Reader-specific lifecycle/accuracy/USB power; do not conflate with internal sensor |
| Touchscreen `04f3:2398` | Present in stored inventory | HID descriptors, gesture/coordinate behavior, firmware and resume |
| Card reader `0bda:0316` | Present in stored inventory | Removable-media integrity, power and resume |
| USB root hubs | Controller inventory above | Port/device topology under actual docks and peripherals |
| Keyboard, TrackPoint, touchpad and hotkeys | Coreboot EC/ACPI interfaces and kernel support available | Actual input-controller mapping, firmware/event paths and gesture/wake behavior |
| Panel/backlight | Brightness and display feature state observed | Panel capabilities, brightness/energy curve and flicker/resume |
| Two batteries, charger, fan and lid | Source-dependent policy/fan watchdog deployed; retained lid gate recovered; evening owner record reports two S4 lid wakes | Remaining EC/pack/charger protocols, charging/transition accuracy, retained-state/cancellation cleanup and S5 separation |
| TPM, RTC, watchdog, flash descriptor, ME and GbE data | Boot-key source and recovery mechanisms reviewed; RTC/watchdog behavior documented | Remaining TPM policy branches, ME/FSP internals, descriptor bounds and identity-preserving recovery |

These open entries are not known failures. They identify where direct evidence
is still needed. Device IDs alone do not establish controller silicon, an EC CPU
architecture or undocumented pin assignments.

## Software and firmware coverage

| Layer | Completed scope | Still open |
|---|---|---|
| Coreboot/GRUB | C55 source/build policy and selected NVMe/ROM/checker paths reviewed | Wider FSP, memory training, payload recovery and exhaustive failure coverage |
| EC/stock BIOS | Selected transport/ARCompact lid gate; separate evening record reports deployed bit-6 hook and two S4 lid wakes | Remaining callbacks, full live-image identity, retained baseline/completion/cancellation/rollback, S5 separation, EC_WAKE output and other EC policies |
| Built-in early init/TPM | Actual-source trust-boundary fixture, config and migration policy review | Reviewed fix and negative fixtures before signed-kernel replacement |
| Kernel/security | Live-matching config, command line, module and DMA/mitigation choices distinguished | Separate diagnostic/hardened profile, measured cost, driver internals and confinement tests |
| GPU/thermal helpers | Evening record reports GPU lock/error and thermal source/bounds guard deployment; new getter/diagnostic/hook status and PWRGD/ROM corrections checked offline | New source correction deployment, broader policy bounds and live-allocation/failure transitions |
| s6/elogind/power services | Selected service, watchdog, locker and NVIDIA sleep paths inspected | Coherent checked sleep transaction and full startup/failure dependency coverage |
| Desktop/applications | Intel helper/desktop override and actual `bw-screen` session/restart route installed; evening record reports D3cold with Bitwarden open | Broader GPU holders, compositor/input/audio interactions and matched workload energy |
| Network/storage/userspace | Targeted inventory and read-only diagnostics | Network services, package/update integrity, filesystem recovery and application-level performance |

## Features worth pursuing

1. **Lid wake from hibernation: done.** The retained gate traced in the supplied
   EC is real: the `03-lid-wake-s4` sleep hook sets EC byte `0x01` bit 6 before
   each hibernate, and the owner's lid test on 2026-10-09 woke the laptop from S4
   twice ([result](10-ec-firmware-and-lid-wake.md), `docs/notes/2026-10-09-evening.md`).
   The prior GPE, HWLO and vendor-state experiments stay as established negatives
   for those routes; no `_PTS` change was needed.
2. **Native USB-C status/PD control:** implement the recovered serialized transport
   and notifications after proving capability/status behavior. An offline mailbox
   model or TYPEC option alone does not restore the feature.
3. **Reliable GPU sleep and desktop idle:** Intel routing now covers the actual
   Bitwarden restart path and coarse D3cold is recorded. Finish checked sleep
   ordering/failure cleanup, test live allocations/resume, cover remaining GPU
   holders and measure battery energy.
4. **Safer boot and recovery:** fix the encrypted-root trust boundary, NVMe failure
   recovery and checker exit/status contracts before preparing new boot images.
5. **A diagnostic/security boot profile:** retain the tuned kernel while adding
   tracing/mitigation/translated-DMA comparisons supported by dock and VM tests.
6. **Measured peripheral tuning:** use repeatable completed jobs and valid battery
   measurements to choose CPU/display/radio/storage settings, rather than adding
   power switches without a demonstrated bottleneck.

Each delivered feature needs source/protocol evidence, explicit limits, a
recoverable deployment, a meaningful success test and a restoration record.
Reverse engineering can expose an interface or implementation defect; it cannot
guarantee that hardware implements a desired feature or that an undocumented bit
is safe. The current audit has not recovered every instruction in every firmware.
