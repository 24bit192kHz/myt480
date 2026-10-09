# Complete findings and improvement index

Publication update: **2026-10-10**. This index preserves the complete improvement
list discussed after the NVIDIA 610/615 investigation and connects it to the
subject reports, source fixes, evidence and recovery instructions. It uses the
existing recorded observations; this publication ran **no T480 command** and
changed no installed component.

The recorded baseline is an i7-8650U T480 with 32 GB RAM, UHD620, GP108M MX150
`10de:1d10`, Toshiba XG6, coreboot C55, Artix/s6, kernel `7.2.8-7-t480` and
proprietary NVIDIA `580.178.04`. This is the last inspected combination, not a
fresh October 10 hardware inventory. The [coverage ledger](11-coverage-and-feature-roadmap.md)
accounts for every PCI function and other device in the stored snapshots.

## How to read status

- **Deployed/observed:** recorded laptop behavior or an installed improvement.
  Separate evening/owner results are identified as such.
- **Source-only:** an authored fix with offline checks, awaiting deployment and
  hardware validation. Git changes do not install it on the laptop.
- **Known defect:** reproduced source/control-flow failure requiring a fix; this
  does not assert the laptop has experienced that failure in normal operation.
- **Measurement/validation:** a plausible improvement whose practical benefit
  or lifecycle correctness needs a controlled comparison.
- **Research:** implementation or hardware prerequisites are unresolved.

These categories describe what the evidence supports. Open inventory entries
are not automatically faults, and no unmeasured FPS, wattage or battery-runtime
gain is promised.

## Findings already established

| Subject | Preserved result and scope | Detailed report |
|---|---|---|
| Fan control | Kernel watchdog fallback, EC automatic-mode release, immediate curve transitions, write-error handling and conservative missing-config behavior were deployed. A 125-second daemon pause demonstrated fallback. | [Thermal](04-thermal-and-power.md) |
| Thermal source/configuration | The evening run deployed last-valid AC/battery-source fallback and numeric undervolt/power/trip bounds. Existing voltage offsets, curve and limits were retained. | [Evening deployment](../../docs/notes/2026-10-09-evening.md) |
| MX150 lifecycle | Coarse RTD3 reached D3cold with loaded modules on AC; five cold-wake CUDA rounds and PRIME OpenGL passed. The wrapper holds the device and restores configured frequency offsets. Node-wait, unload-error and manual-off locking fixes were later deployed. | [Graphics](03-nvidia-and-intel-graphics.md), [GPU wiki](../../docs/wiki/nvidia.md) |
| Intel routing | `igpu-run`, the desktop override and the actual `bw-screen boot` session/hibernate restart route were deployed. The evening record reports D3cold with Bitwarden open. | [Routing](../../docs/wiki/gpu-routing.md) |
| Sleep infrastructure | Package `NoExtract` resolved the vendor hook masking the site hook under elogind 257. Correct hibernate dispatch, synchronous bounded resume, watchdog handling and RTC entry through `loginctl` were deployed. Recorded idle-GPU S3/S4 cycles passed. | [Evening deployment](../../docs/notes/2026-10-09-evening.md), [Sleep](12-mx150/04-sleep-and-firmware.md) |
| Lid wake | Lenovo N24HT37W was identified as little-endian ARCompact. EC byte `0x01` bit 6 controls a retained gate for conditional lid-open/PWRBTN behavior. The owner subsequently woke S4 twice using the installed hook; no `_PTS` edit or firmware flash was needed. | [EC/lid](10-ec-firmware-and-lid-wake.md) |
| NVIDIA voltage | Exact 580 RM clamps the recovered signed overvoltage request below zero to zero and reports minimum offset zero. This interface cannot undervolt the MX150; alternate V/F policy remains research. | [Voltage](12-mx150/01-voltage-and-controls.md), [Closed RM](12-mx150/02-closed-driver.md) |
| Approved capability queries | NVML frequency getters succeeded; modern P0–P15 results do not establish independent per-state setters. Usage/current power-limit getters were unsupported, and the 5001 W default metadata is not a physical limit. Existing X exposed no NV-CONTROL. GPU returned to D3cold. | [Getter observations](12-mx150/01-voltage-and-controls.md#approved-getter-observations) |
| GPU firmware/board | The complete retained VBIOS and CBFS binding were checked. PWM VID reaches the NCP81278T regulator; EC GPIO216 power-limit and GPIO104 thermal-alert routes are separate. Selected PROCHOT/throttle policy paths were recovered; full sensor arbitration and board calibration remain open. | [VBIOS and EC](12-mx150/03-vbios-and-ec.md) |
| Newer NVIDIA branches | As verified October 9, 615.78.08 was the newest public Linux display release and 580.178.04 the newest compatible release found. Both newer kernel flavors exist. Proprietary 610/615 reject Pascal, clear its physical HAL entries and omit GP108 object registration. Shared Pascal fragments survive; an ID/check patch alone cannot restore the shipping path. | [Upgrade research](12-mx150/07-driver-upgrade-and-pascal-support.md), [hash manifest](12-mx150/08-upgrade-evidence.json) |
| GPU firmware source corrections | Patches 0029–0031 abort failed PWRGD power-on, bound CBFS ROM probe/copy and propagate actual produced bytes. Actual-source/AML fixtures passed; these fixes were not flashed. Wider ROM/AML paths remain outside their scope. | [Power/sleep](12-mx150/04-sleep-and-firmware.md), [ROM bounds](12-mx150/03-vbios-and-ec.md) |
| USB-C | Stock UCSI 1.0 shadow layout, software-SMI dispatch, EC mailbox ordering, transport ports and completion/notification behavior were recovered. The model is offline; native connector control is not implemented. | [USB-C/Thunderbolt](05-usb-c-and-thunderbolt.md) |
| Fingerprint | Historical Windows reset `05 02 00`, capture-stop `04`, cancellation/worker order and optional secure wrapping were traced. Rust interrupt-stream cancellation starvation was reproduced and fixed in source. The selected libfprint backend was retained; the optional Rust backend was not installed. | [Fingerprint](06-fingerprint.md), [protocol](../../docs/wiki/fingerprint-protocol.md) |
| Boot trust | Actual-source fixtures reproduced production-master load followed by plain-root init execution, assuming a successful matching-policy TPM unseal. Migration also relies on deleting an unbound blob. No key extraction or physical exploit was performed. | [Firmware/boot trust](01-firmware-and-boot-trust.md) |
| GRUB NVMe | The reviewed module matches the archived C55 candidate. Incomplete status checking and timeout/retry/late-completion ownership defects were identified; no live timeout was induced. | [Firmware/boot trust](01-firmware-and-boot-trust.md) |
| Build/deployment integrity | The evening run installed truthful QEMU scenario exit status and flash-wrapper failure status, without flashing. It also synchronized deployed files/source through `syswork` and removed a stale worktree. Exact firmware config/key/auth checking and debug-policy consistency remain open. | [Evening deployment](../../docs/notes/2026-10-09-evening.md), [cross-stack review](../../docs/wiki/cross-stack-review.md) |
| Kernel/security | The final captured config, disabled mitigations/tracing/enforcement choices and Thunderbolt identity DMA mapping were distinguished from the base config. These are retained choices; a separate profile needs measured dock/VM/workload validation. | [Kernel](02-kernel-and-security.md) |
| Storage/display | SMART showed no recorded media/error-log entries, 26% endurance used and 855 historical unsafe shutdowns; this is not a lifespan forecast or attribution. Intel FBC and DMC worked; unavailable PSR status and a GVT fallback were not demonstrated faults. Thunderbolt NVM23 exceeds the cited critical-update minimum, not proof it is newest. | [Other hardware](07-storage-and-other-hardware.md) |
| Measurement/recovery | Passive dual-battery/RAPL and read-only SMART tools were added. The short charging-AC observation measured CPU-package power only. Backup/hash-checked restoration was exercised; later deployments have updated hashes and endpoints. | [Power](04-thermal-and-power.md), [recovery](09-rollback-and-next-work.md) |

The negative Lenovo SMM/GPE/HWLO experiments remain preserved in the
[EC report](10-ec-firmware-and-lid-wake.md#established-negatives-retained).
They were not repeated or discarded when the separate retained EC gate was found.

## Complete improvement inventory

The first 26 rows preserve every direction in the preceding user-facing list.
The additional rows make the wider memory/controller/userspace coverage explicit.

| ID | Area and useful improvement | Current status and next evidence |
|---|---|---|
| I01 | **MX150 idle power:** extend Intel routing to any remaining unnecessary GPU holders and measure battery savings | Routing/coarse D3cold work. Broader application coverage and matched discharge energy remain unmeasured. [Guide](12-mx150/05-use-and-validation.md) |
| I02 | **GPU system sleep:** preserve live CUDA/graphics allocations across S3/S4 and failed transitions | Idle-GPU cycles passed; live allocation correctness, backing storage and one effective sleep owner remain unvalidated. [Sleep](12-mx150/04-sleep-and-firmware.md) |
| I03 | **NVIDIA hook:** preserve actual helper/timeout errors and restore console state on failure | Status correction deployed 2026-10-10 (14 mock tests, one RTC S3 cycle). Nonzero exit alone does not cancel this elogind configuration. [Sleep](12-mx150/04-sleep-and-firmware.md) |
| I04 | **Coreboot GPU power:** abort rail startup without PWRGD and investigate truthful power/link state contracts | Patch 0029 is AML-tested/source-only; electrical timing, `_STA`/request versus PWRGD and link-wait failure behavior remain open. [Power](12-mx150/04-sleep-and-firmware.md#acpi-power-on-timeout-defect) |
| I05 | **VBIOS/CBFS:** prevent truncated/oversized source reads and use actual produced lengths | 0030/0031 are sanitizer-tested/source-only. On-device/RAM/generic/VFCT and short-image AML-generator paths are outside this fix. [ROM](12-mx150/03-vbios-and-ec.md) |
| I06 | **Games/applications:** validate compatible DXVK/Proton, actual Vulkan feature/limit reports and OpenCL work | R580 clears current documented DXVK baseline version; device features and titles still need testing. Optional descriptor paths are not automatically faster. [Upgrade](12-mx150/07-driver-upgrade-and-pascal-support.md) |
| I07 | **CUDA:** use CUDA 12.9 for `sm_61` builds, reduce transfers/allocations and choose working sets that fit VRAM | Useful workload-specific path. Framework/library architecture support and checked outputs must be verified separately. [CUDA guide](12-mx150/05-use-and-validation.md#cuda-build-compatibility) |
| I08 | **GPU efficiency:** compare FPS limits, graphics settings, CPU/GPU balance and completed useful work | Needs matched performance/energy comparisons. Do not increase the retained offsets merely because getter ranges are larger. [Measurement](12-mx150/05-use-and-validation.md#measure-and-restore) |
| I09 | **GPU undervolting:** investigate a usable alternative V/F policy with calibrated telemetry | Research. Recovered overvoltage offset cannot do it; physical PWM/internal machinery is not a delivered undervolter. [Voltage](12-mx150/01-voltage-and-controls.md) |
| I10 | **Runtime PM/PCIe/reset:** investigate finer idle behavior, offset lifetime, AER/reset and recovery | Research/validation. Coarse mode works; fine behavior and independent per-state controls are unproved. Keep the known freeze workaround pending evidence. [Coverage](12-mx150/06-coverage-and-evidence.md) |
| I11 | **Alternative graphics driver:** study Nouveau/NVK and GP108 power/reclocking | Source-accessible research. No performance parity, reliable reclocking or NVIDIA CUDA replacement was established. [Alternatives](12-mx150/05-use-and-validation.md#alternative-driver-and-kernel-work) |
| I12 | **NVIDIA maintenance:** maintain a matched R580 stack and backport a relevant exposed kernel-interface fix | Supported direction; match a demonstrated failure and check existing backports first. No working 610/615 Pascal port is delivered. [Upgrade](12-mx150/07-driver-upgrade-and-pascal-support.md) |
| I13 | **Boot security:** require expected encrypted root before production-secret release | Known source/control-flow defect. Separate provisioning/recovery; validate encrypted/plain/wrong/missing root, passphrase and hibernate cases before signed-kernel replacement. [Trust](01-firmware-and-boot-trust.md) |
| I14 | **TPM migration:** bind migration authorization appropriately instead of file-deletion-only one-shot use | Design work. Preserve passphrase/known-good recovery and handle flash/reseal failures; an old copied unbound blob has no TPM single-use guarantee. [Trust](01-firmware-and-boot-trust.md) |
| I15 | **GRUB/NVMe:** correct full status checking, timeout/queue recovery, late completion identity and buffer ownership | Known defects. Inject failures offline before recoverable controller/cold-boot validation. [GRUB](01-firmware-and-boot-trust.md#grub-defects-are-tied-to-the-archived-c55-build) |
| I16 | **Firmware verification:** verify extracted config, keys/authentication and production-equivalent debug policy | Checker/debug gaps remain. QEMU verdict and flash-wrapper status were fixed/deployed evening; that is not a new firmware boot/flash test. [Build gates](01-firmware-and-boot-trust.md#make-the-build-and-flash-gates-truthful) |
| I17 | **Sleep coordination:** confirm intended-session locker readiness and undo fingerprint/VM/GPU/watchdog/RTC preparation on cancellation | Partly improved. `AllowSuspendInterrupts` remains off because cancelled pre-hooks get no post cleanup in the reviewed elogind version. A coherent transaction remains needed. [Sleep review](../../docs/wiki/cross-stack-review.md#3-make-sleep-one-coherent-checked-operation) |
| I18 | **EC/lid:** preserve correct retained-state baseline, completion and cancellation/rollback behavior; separate S5 | S4 lid wake worked twice. A host-shadow read does not recover the retained baseline, so the hook clears the bit unconditionally once it armed it; since 2026-10-10 the opt-out file only stops arming and never skips the post cleanup. Further experiments require separate approval. [EC limits](10-ec-firmware-and-lid-wake.md) |
| I19 | **USB-C:** restore connector status/notifications, then supported role/PD/alt-mode control | Recovered transport needs serialized bounded EC ownership, firmware/ACPI notifications and compatible kernel support. TYPEC options alone and AML child counts do not establish EC capabilities. [UCSI](05-usb-c-and-thunderbolt.md) |
| I20 | **Thunderbolt security/docks:** validate translated DMA, authorization, tunneling, hotplug and resume | Identity mapping is recorded. Actual dock/VM compatibility and recovery must be tested; NVM23 is not a newest-version claim. [Thunderbolt](07-storage-and-other-hardware.md#thunderbolt-host-and-usb-c-transport) |
| I21 | **Kernel diagnostics/security:** add a separate tracing/mitigated/hardened profile | Comparison project. Final config disables selected enforcement/tracing; compiled/active features and application policy must remain distinct. Retain tuned recovery boot. [Kernel](02-kernel-and-security.md) |
| I22 | **Fingerprint:** bound cancellation/draining/phase deadlines and validate disconnect/resume | One optional Rust source fix passed fixtures; live selected libfprint was retained. Test each reader/backend separately without conflating reset scopes or recognition quality. [Fingerprint](06-fingerprint.md) |
| I23 | **CPU/cooling:** measure sustained mixed load and optimize plans/fan behavior within tested stability margins | Fan fallback/source guards work; thermal/DPTF ownership, CPU/GPU contention and sustained performance remain to measure. Cooling maintenance is conditional on an actual bottleneck. [Thermal](04-thermal-and-power.md) |
| I24 | **Batteries/display/storage:** verify applied charge thresholds and compare brightness/display states/APST against latency and energy | Measurement. Two-pack/source transitions, Linux NVMe recovery, PSR cause/flicker/external displays and charge/controller policy remain open. [Hardware](07-storage-and-other-hardware.md) |
| I25 | **Other peripherals:** Wi-Fi/Bluetooth coexistence, audio/headset wake, Ethernet wake, USB autosuspend, camera, touch/input and card-reader resume | Inventory/validation. No throughput, recognition, audio or energy improvement is established merely from a power option. [All devices](11-coverage-and-feature-roadmap.md) |
| I26 | **Virtualization:** validate existing Intel GVT and GPU passthrough/reset/isolation | Research. The GVT fallback is not a proved failure; VFIO availability does not establish safe MX150 reset or usable passthrough. [Kernel](02-kernel-and-security.md), [MX150 limits](12-mx150/05-use-and-validation.md#alternative-driver-and-kernel-work) |
| I27 | **Memory/platform internals:** check RAM/mixed-load stability and investigate FSP training, microcode, ME/descriptor and identity-preserving recovery | Open implementation work. No memory-timing or opaque-firmware speed improvement is established. [Coverage](11-coverage-and-feature-roadmap.md) |
| I28 | **EC/charger/controller internals:** continue keyboard/battery/charger state machines, thermal arbitration and GPIO/sideband consumers | Selected lid/GPU paths are recovered; full installed-image identity, IRQ/polling consumers, sensor units and protection arbitration remain open. [EC](10-ec-firmware-and-lid-wake.md), [GPU/EC](12-mx150/03-vbios-and-ec.md) |
| I29 | **Services/update/filesystem integrity:** check startup dependencies, package exclusions, service ownership, recovery and application resource use | Selected paths already reviewed. Wider network/userspace/package/filesystem recovery coverage remains open; there is no general replacement/tuning recommendation without evidence. [Software ledger](11-coverage-and-feature-roadmap.md#software-and-firmware-coverage) |
| I30 | **Media acceleration:** validate Intel profiles/workloads and resolve the conflicting MX150 decode documentation | No supported MX150 NVENC unlock is established. Exact 580 VDPAU entries and current codec matrix disagree; real profile/output/energy tests remain needed. [Media](12-mx150/05-use-and-validation.md#media-support-an-unresolved-documentation-conflict) |

CPU undervolting already exists: retained core/cache offsets are -115 mV AC and
-100 mV battery. The retained iGPU/uncore offsets are -80/-70 mV AC and
-60/-50 mV battery ([configuration](../../system/etc/thermald.conf)).
Prior AC -124 mV produced errors and battery -121 mV froze.
MX150 policy remains +200/+1250 frequency offsets on AC and zero on battery;
+250 core previously produced silent CUDA errors. These values describe this
machine's retained settings, not a fresh sweep or universal recommendation.
Positive frequency tuning is not proof of lower GPU voltage.

## What the new-driver result does and does not establish

The [610/615 report](12-mx150/07-driver-upgrade-and-pascal-support.md) preserves
the exact proprietary legacy/physical/HAL registration findings, open-driver
GSP dependency, version matching and rejected simple patch approaches. A genuine
port needs GP108 implementation/registration plus compatible controls/classes,
graphics/CUDA, reset, memory, protection and power-state validation. Some
Pascal helpers and compiler strings remain, so total removal of every old
routine is not claimed.

Several advertised R610 fixes already reached R580. Compatible application
updates and targeted host-driver/firmware fixes remain useful avenues; no port
benefit was measured. New software cannot add missing Tensor/RT silicon, VRAM,
bandwidth or cooling capacity. GPU VBIOS authentication/edit acceptance,
calibration and usable alternative undervolt control remain unresolved.

## Priority, validation and recovery

The first priorities are **I13/I14 boot trust, I15 NVMe boot recovery and I17/I02
checked sleep**, followed by reviewed source-fix deployment, native USB-C and
matched battery/application measurements. Follow
[the prioritized recovery plan](09-rollback-and-next-work.md) and
[the daily guide](../../docs/wiki/getting-the-most.md).

Recorded checks include 44 Rust tests with seven USB cancellation fixtures,
earlier GPU/helper and scenario-verdict fixtures, and the later **102-test**
MX150/power/SMART/AML/ROM/CBFS suite. The latter used actual-source/AML paths,
controlled hardware/decoder stubs, sanitizers and complete matching DSDT
compilation; it was not a full firmware build or live 610/615 compatibility
test. [Work log](00-work-log.md), [validation](08-tools-evidence-and-validation.md)
and [MX150 evidence](12-mx150/06-coverage-and-evidence.md) retain precise scopes.
This documentation publication checks links/status/provenance; it does not
rerun or extend those hardware results.

Use exact backups and the updated hashes for the component actually deployed;
the original audit rollback is not automatically valid for later binaries.
Source-only patches 0031, 0030 and 0029 can be reversed in that order before
rebuilding. Kernel/driver trials need a coherent matched package stack, known-good
boot and restoration path. Full firmware images, raw proprietary decompilation,
keys, biometric captures and private machine logs remain in the recorded private
evidence locations; public reports preserve derived findings, authored code/tests
and hashes.

The user's requirement to ask before any new T480 command remains in force.
The limited approved getter/file/existing-session queries are complete; they did
not authorize deployment, tuning, EC transactions, module operations or sleep.
The separate evening/owner cycles are retained as reported results. Publishing
this index and pushing `testing` adds no new hardware permission.
