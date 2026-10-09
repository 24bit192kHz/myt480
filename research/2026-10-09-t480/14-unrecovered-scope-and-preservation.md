# What remains unrecovered, and how to preserve the work

Publication date: **2026-10-10**. Yes, there are parts of this T480 that have not
been reverse engineered. The saved inventories are accounted for, but the
firmware and driver implementations are only partly reconstructed. Publishing
every recorded finding does not turn the remaining unknowns into completed work.

The practical goal is to preserve enough evidence that the established findings
can be checked and reused without starting from scratch. No archive can promise
that a changed firmware, driver, device or workload will never need new analysis.
This continuation used local evidence only and ran **no T480 command**.

Before publication, `testing` advanced through the separate
[October 10 deployment review](../../docs/notes/2026-10-09-evening.md#2026-10-10-01000150-the-mx150-push-audited-three-files-deployed).
Its hook/diagnostic and lid opt-out cleanup deployment and one idle-GPU RTC S3
cycle are incorporated as reported results. Patches 0029–0031 remain unflashed:
the valid uncompressed MX150 ROM's ordinary copy is unchanged, and the new
PWRGD abort has no stock `HGON` precedent or real fault validation. Their source
hardening checks do not establish a performance benefit or justify a flash.

## Inventory coverage and remaining reconstruction

The stored [PCI snapshot](../../hardware/lspci.txt) contains **24 functions** and
**21 distinct vendor/device ID pairs**. The [USB snapshot](../../hardware/lsusb.txt)
contains **10 entries: four root hubs and six physical devices**. All are
represented in the [coverage ledger](11-coverage-and-feature-roadmap.md).
Those counts describe the saved configuration, not every optional T480 device
or a fresh live inventory. A driver binding or device ID is not full protocol,
firmware or failure-path recovery.

| Area | Existing understanding | Still unrecovered or unvalidated |
|---|---|---|
| CPU/microcode | Recorded identity, voltage planes, limits and mitigation state | Microcode implementation, every MSR/firmware interaction and sustained mixed-load margin |
| Memory/FSP | Published SPD/MRC cache and boot-time work | FSP memory training, full silicon initialization, memory-controller internals and exhaustive DIMM compatibility |
| ME/flash/board | Recorded coreboot configuration, descriptor/identity recovery workflow | Complete ME firmware, every descriptor/security path and exact installed board/schematic revision |
| EC | N24HT37W ARCompact; selected host/UCSI, lid/PWRBTN and GPU protection paths | Entire scheduler, keyboard/charger/pack state machines, every interrupt/poll consumer, sensor units/arbitration and EC_WAKE output route |
| Lid hook | Separate owner record reports two successful S4 wakes | Original retained-state baseline, asynchronous completion, cancellation/rollback and S5 separation |
| NVIDIA RM | Selected voltage, clock/power, runtime-PM and newer GP108 selection/registration paths | Remaining RM interfaces/classes, full calibration/policy, engine microcode, memory training, firmware authentication and an alternative undervolter |
| GPU firmware/board | Full retained VBIOS identity and selected regulator/throttle routes | Active table selection, all table semantics, edit acceptance, live electrical behavior and complete protection arbitration |
| Intel graphics/media | Intel routing, FBC/DMC observations and published panel-init work | Full display/media/GVT behavior, PSR cause, exact panel features and external-display transitions |
| Wi-Fi/Bluetooth/Ethernet | Controller identities and selected source-dependent policies | Firmware internals, complete reconnect/coexistence/wake/error behavior and measured energy |
| NVMe | XG6 identity/SMART/APST, selected GRUB status/queue defects | Controller firmware, Linux recovery/residency and real fault/late-DMA behavior |
| Thunderbolt/USB-C | NVM/security/DMA observations and recovered UCSI software/EC transport | Controller/PD firmware, native UCSI implementation, actual capabilities/roles/notifications and dock/alt-mode transitions |
| Fingerprint | Selected historical reset/stop/worker/secure-transport paths and Rust cancellation fix | Complete sensor protocol/firmware, selected live-backend lifecycle, bounded drain/phase behavior and recognition-quality testing |
| Camera/touchscreen/card reader | Recorded IDs | Complete descriptors/controls, firmware, lifecycle, power and error recovery |
| Keyboard/TrackPoint/touchpad/hotkeys | Historical RMI4 intertouch, GRUB scan-code and hotkey work | Exact peripheral firmware, full event/wake state machines and comprehensive lifecycle coverage |
| Audio | Selected HDA power policy | Exact codec/pin routing, full headset/jack/codec behavior, pops and resume/error recovery |
| Batteries/charger/sensors | Source-aware policy, fan watchdog and selected EC protection work | Pack/controller firmware, complete charge/protection arbitration, sensor calibration and applied threshold/transition validation |
| TPM/boot trust | Selected unseal/root and migration paths, reviewed recovery workflow | Remaining policy branches, reviewed repairs and negative hardware/recovery validation |
| Kernel/services/applications | Recorded configuration plus selected sleep, helper, startup and routing paths | All other driver/service/package/filesystem/network/application failure paths and workload behavior |

The [MX150 boundary map](12-mx150/06-coverage-and-evidence.md) gives finer GPU
scope. The [complete findings index](13-findings-and-improvement-index.md)
connects these unknowns to the 30 improvement directions. None of the open
entries alone establishes a broken component.

## Work that should not be rediscovered

Preserve the pre-audit history as well as the October reports. The
[firmware overview](../../firmware/README.md) records panel power/DDC, SPD caching,
GRUB scanning/read-size and boot-time work. The
[September deep dive](../../docs/notes/2026-09-28-deepdive.md) records verified
`i801_smbus`/`rmi4_smbus` input routing and earlier initialization work.
The [EC report](10-ec-firmware-and-lid-wake.md#established-negatives-retained)
preserves the completed SMM/GPE/HWLO experiments. The
[work log](00-work-log.md) and [evening notes](../../docs/notes/2026-10-09-evening.md)
preserve later deployments, idle sleep and owner-operated lid results.

Failure to identify a new lid bit in the old SMM route was an established result;
it should not be repeated merely because another route later succeeded. Likewise,
automatic function counts are analysis output, not reviewed-function counts.

## What a fresh clone preserves

`testing` contains the subject reports, address/symbol explanations, important
negative results, artifact hashes, source pins/patches, offline models/fixtures,
deployment/recovery records and the recorded stock ACPI/VBIOS reference files.
The newly published [static-analysis readers](../../tools/re-audit/static-analysis/README.md)
reproduce the exact EC initialization map and cross-branch NVIDIA physical/legacy
tables from matching input artifacts. Their provenance records the original
authored scripts and the changes needed to remove workstation path assumptions.
The same directory preserves the authored actual-source plain-root fixture,
with strengthened mocks and an exact-source guard. It assumes successful
matching-policy unseal and reproduces ordering; it does not exercise a real TPM.

A fresh clone does **not** contain every raw input, private capture or saved
analysis database. In particular, three saved Ghidra projects remain private:

| Audit directory below `/home/btw/test/rea/work/` | Project pair to retain |
|---|---|
| `audit-20261009-ec` | `ec-static.gpr` and the entire `ec-static.rep/` |
| `audit-20261009-mx150` | `mx150-static.gpr` and the entire `mx150-static.rep/` |
| `audit-20261009-mx150-upgrade` | `mx150-upgrade-static.gpr` and the entire `mx150-upgrade-static.rep/` |

The `.gpr` files alone are empty project markers; the `.rep` trees hold the
databases. The three saved pairs occupied **802,524,864 bytes** when checked.
Earlier fingerprint/SMM/UCSI work is preserved as result bundles in the recorded
audit directories; equivalent standalone Ghidra project pairs were not located
there. Those bundles preserve selected findings, not a complete annotated database.

## Private preservation snapshot made in this continuation

The [preservation inventory](15-analysis-preservation.json) identifies a private
archive under `/home/btw/test/rea/work/audit-20261010-preservation/`. It retains
all three saved Ghidra project trees, the pinned ARC processor and selected small
source/analysis/results from the nine original audit directories. This includes
the original plain-root C fixture, shell helpers and selected reference headers.
The JSON specifies exact selection rules, counts, sizes and hashes. Every retained
source member was streamed and checked against its recorded source size/SHA-256;
the embedded archive manifest was separately compared byte-for-byte.

This is a snapshot of that stated scope, not a complete raw-artifact or workstation
backup. Original packages, raw firmware, some binary analysis outputs, toolchains
and large/excluded files remain at their original private locations. The archive
is on the **same workstation**, so an independent storage copy is still needed
for protection against disk loss. No Ghidra reopen or independent-device restore
test is claimed. Existing audit manifests are kept as historical evidence rather
than overwritten when a later note changes. The earlier MX150 manifest matches
136 of 137 entries: `closed-rm-static-notes.md` changed during the later voltage
correction already published in the [RM report](12-mx150/02-closed-driver.md).
The inventory retains both digests, and the new archive hashes the current note.

For the private archive, check its recorded digests locally before copying:

```sh
cd /home/btw/test/rea/work/audit-20261010-preservation
sha256sum -c SHA256SUMS-v2
```

Keep the archive, `archive-files-v2.json` and `SHA256SUMS-v2` together on private storage.
If restoring, list the archive first and extract into a new empty private
workstation folder. Keep each `.gpr` beside its matching `.rep` directory, preserve
the ARC processor contribution, and use the recorded Ghidra 12.1.4/JDK21 tool
baseline. Close Ghidra before making a future project snapshot; copying a changing
database does not establish a coherent backup.

## Reuse contract and remaining identity gaps

For every reused finding retain:

1. Exact input version, size/SHA-256, source commit and patch order.
2. Architecture/language, load base, relocation assumptions and address space.
3. Selected function/symbol boundaries, independent corroboration and uncertainty.
4. Expected output or fixture, established negative results and tested failure scope.
5. Deployment state, backup identity and recovery instructions.

Tool pins and private evidence locations are in
[tools/evidence](08-tools-evidence-and-validation.md). The EC uses
`ARCompact:LE:32:default`, payload base zero; container offsets add `0x20`.
NVIDIA Ghidra addresses are relocated analysis addresses, not file offsets to
patch. The upgrade [manifest](12-mx150/08-upgrade-evidence.json) binds branch-specific
symbols/tables to exact objects; it is not a recipe for patching another release.

The public inventory does not establish a complete identity set for the installed
board revision, panel EDID, DIMM/SPD details, HDA codec, TPM model/firmware,
PD controller, battery-pack/controller and sensors. Historical working paths do
not replace those identities. A firmware/device replacement or new workload
needs comparison with the recorded baseline before a result is carried forward.

Completing the remaining work would require additional static inputs and, for
many interfaces, approved device observations and controlled tests. The earlier
getter-only approval is complete and does not authorize any new T480 command,
EC access, tuning, installation or sleep. This publication adds no such action.
