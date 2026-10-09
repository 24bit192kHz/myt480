# EC firmware and lid wake — static continuation

The supplied Lenovo image is now extracted and decoded as **little-endian
ARCompact**. A conditional lid-open-to-power-button path is recovered:
**EC byte `0x01`, bit 6 (`BTPC`) enables a retained flag used by a deep-state
lid-open callback.** This is a concrete host-accessible candidate for `_PTS(4)`.
Successful S4 wake, the installed image's complete identity and all runtime
eligibility conditions remain unverified. No ACPI/EC patch is deployed.

**Result (2026-10-09 evening): it works.** The candidate was tried as a sleep hook
instead of an ACPI change: `system/etc/elogind/system-sleep/03-lid-wake-s4` sets
the bit through coreboot's `ectool` before every hibernate and clears it after
the resume. The owner hibernated, closed and opened the lid twice (19:23 and
19:24); the machine powered on and resumed both times, `PM1_STS: WAK PWRBTN`,
`prev_sleep_state 4`, no RTC alarm armed. The host-visible byte reads `0x05`
again after each S4 cycle, as predicted (shadow, not the retained flag), so the
hook clears bit 6 unconditionally. See `docs/notes/2026-10-09-evening.md`.

This continuation runs on the workstation. No command was run on the T480,
and no EC transaction, firmware write, reboot, suspend or hibernate was performed.
The user's requirement to ask before a hardware test remains in force.

## Established negatives retained

The user supplied these completed results; this continuation does not repeat them:

- Vendor SMM `AWON` is a no-op, `SLTP` arms a sleep SMI, and `SmmSleepEvent`
  normally converts S4 to S5 and updates indexed register `0x41`. No lid-specific
  control was identified there.
- Copying the vendor real-S4 indexed/EC RAM state and arming `HWLO` through `_PSW`
  did not enable lid wake.
- GPE `0x16`, `0x17`, or both, EC RAM `0x3d = 0x5f`, and matching PCH pad state
  did not enable it. Boot-latched GPE status bit22 is not evidence of a lid event.
- This model's setup has no wake-on-lid option. C56/C57 experiments are complete.

The [historical experiment record](../../docs/notes/2026-10-08-port-fixes.md)
provides context. The coreboot README's earlier statement that missing vendor
SMM explained S4 lid wake has been corrected; that attribution was unsupported.

## Exact extraction and hashes

Input: `/home/btw/Downloads/n24ur36w (1).iso`, volume `N24UR36W`.
Its El Torito catalogue is at 2,048-byte sector 20. The hard-disk image starts
at sector 27, byte 55,296. The catalogue's one-sector load count describes boot
loading, not the complete disk image. MBR partition 1 starts at 512-byte sector 32
and spans 67,552 sectors; the maximum partition end gives a 34,603,008-byte image.

The FAT paths `FLASH/N24ET74P/$0AN2400.FL2` and
`FLASH/N24ET74W/$0AN2400.FL2` contain identical EC images. Version `N24HT37W`
appears at container offset `0x260`, payload offset `0x240`. It matches the
version observed in the earlier live audit; this extraction is not a fresh
comparison with the installed EC's complete contents.

| Artifact | Bytes | SHA256 |
|---|---:|---|
| Supplied ISO | 34,975,744 | `c3a2a987e7d3925f7dfbe147d5c3bdafb894c4b7896075bade6a6d8a7130004c` |
| `$0AN2400.FL2` | 286,752 (`0x46020`) | `e04bb05556ead0e7db28249a98ed5eb242779e43f8ebef606762a74832e7d772` |
| FL2 payload after 32-byte wrapper | 286,720 (`0x46000`) | `befcb425b9f2a83b959c0ef3ee490420b7c5e5ffa2a792fda821d0ff355e77cf` |

The wrapper begins `_EC` plus byte 01. Its eight little-endian 32-bit words are
`0143455f, 46020, 46000, 1, 1, 6a4f90e3, 0, 0`. Two length fields match exactly.
The other fields are unresolved; `6a4f90e3` does not match the payload's standard
CRC32 `499089b8`. The populated tail after the long FF gap is also unclassified.
Selected executable regions are plainly decodable; that does not identify every
container/security field or prove a valid modified image could be installed.

For this exact hashed ISO, reproduce extraction in a private workstation folder:

```sh
umask 077
ec_work=$(mktemp -d /tmp/t480-ec-static.XXXXXX)
cd "$ec_work"
ec_iso='/home/btw/Downloads/n24ur36w (1).iso'
dd if="$ec_iso" of=eltorito-harddisk.img bs=512 skip=108 count=67584 status=none
# Requires mtools; partition offset 32*512 = 16384.
mcopy -i 'eltorito-harddisk.img@@16384' \
  '::/FLASH/N24ET74W/$0AN2400.FL2' ec-container.fl2
python3 - <<'PY'
from pathlib import Path
from hashlib import sha256
image = Path('ec-container.fl2').read_bytes()
assert len(image) == 0x46020
assert sha256(image).hexdigest() == 'e04bb05556ead0e7db28249a98ed5eb242779e43f8ebef606762a74832e7d772'
Path('ec-payload.bin').write_bytes(image[0x20:])
PY
```

These offsets apply to the audited ISO, not every Lenovo update. This copies
files only; it does not execute an updater or communicate with hardware.

## Architecture and tool validation

The [production build configuration](../../firmware/coreboot/dot-config-as-built)
selects `CONFIG_EC_LENOVO_MEC1653=y`. The `h8` compatibility-interface name is
not its CPU architecture. Original
[MEC1653 research](https://airbus-seclab.github.io/embedded_controller/BH2019-Slides-Breaking_Through_Another_Side_Bypassing_Firmware_Security_Boundaries_from_Embedded_Controller-matrosov-gazet.pdf),
PDF page 14, identifies ARC-625D on its P50 target; the T480 code was corroborated
directly by independent decoders. New MEC1653B product information must not be
substituted for the older MEC1653 part.

Used the requested [Ghidra MCP](https://github.com/bethington/ghidra-mcp) build
with Ghidra 12.1.4/JDK 21. The installed Ghidra lacked ARC, so the processor
contribution from [Ghidra PR3006](https://github.com/NationalSecurityAgency/ghidra/pull/3006)
was pinned at `d3fbf109ada6d051750e973779170c1758622530`, built locally and added
through a removable processor-directory link. Its build emitted seven
NOP-constructor warnings. Analysis uses `ARCompact:LE:32:default`, payload base 0.

The first 24 eight-byte vectors decode coherently: reset at 0 jumps to `0x2080`;
subsequent targets include `0x27a0`, `0x27a4` and `0x27ac`. Reset contains
coherent SRAM initialization and internal calls. Addresses here are payload
analysis addresses; add `0x20` for container offsets. This model does not prove
every runtime remapping or bank configuration.

Built GNU Binutils 2.45 privately for `arc-linux-gnu`, independently confirming
those vectors and selected reset/MMIO instructions. The
[GNU release source](https://lists.gnu.org/archive/html/info-gnu/2025-07/msg00009.html)
archive SHA256 was
`c50c0e7f9cb188980e2cc97e4537626b1672441815587f1eab69d2a1bfbef5d2`.
The ARC decoder prints raw `.data` as words; an ELF wrapper with `.text` marked
code is required for the independent raw-image check:

The commands below assume target-prefixed ARC tools are on `PATH`. This audit
used the uninstalled private build's `binutils/objcopy` and `binutils/objdump`.

```sh
arc-linux-gnu-objcopy -I binary -O elf32-littlearc -B arc:ARC600 \
  --rename-section .data=.text,alloc,load,readonly,code,contents \
  ec-payload.bin ec-analysis.elf
arc-linux-gnu-objdump -d -EL --start-address=0 --stop-address=0xc0 ec-analysis.elf
```

Linear disassembly can decode embedded data as instructions. Function boundaries,
literal pools, delay slots, callers and branches must be corroborated before
assigning meanings. Decompiler parameter recovery and loop/jump-table warnings
are retained limitations; this is not a complete firmware emulation.

## Physical signals and register evidence

The original [Windu-2/NM-B501 rev0.1 drawing](https://documents.cdn.ifixit.com/RXTmJIF56BpXZcoC.pdf),
sheets 8, 13, 26, 55–57, connects these signals. The installed board revision has
not been rechecked.

| Signal | EC connection | Host connection |
|---|---|---|
| Lid input `-LID_CLOSE` | U238 A1, `VCI_IN3#/GPIO000` | Lid connector |
| Power-button output `-PWRSW_EC` | G2, `GPIO106` | GPD3/PWRBTN# |
| EC wake `-EC_WAKE` | N7, `SMI#` | GPP_C23, GPE `0x17` |
| EC SCI | K6, `EC_SCI#` | GPP_C22, GPE `0x16` |
| Sleep-state inputs | GPIO110/G11 and GPIO111/G12 | SLP_S3# and SLP_S4# |

The separately named `-LID_CLOSE_EC` output is not the lid input. Host power-button
and EC wake are separate outputs; an SCI notification is another path.

Exact [MEC1653 coreboot UART source](https://github.com/coreboot/coreboot/blob/main/src/ec/lenovo/mec1653/uart.h)
establishes GPIO configuration base `0xf0c400` and its UART offsets. The full
[MEC1632 manual](https://ww1.microchip.com/downloads/en/DeviceDoc/00001592B.pdf)
is an explicitly comparable ARC-family reference, not an exact MEC1653 manual.
Its GPIO/VCI layouts supplied search candidates. Actual firmware instructions,
tables and schematic pin assignments provide the additional cross-checks below.
Undocumented offsets are not proposed live write commands.

## Selected recovered paths

| Path / analysis address | Observed code behavior | Interpretation boundary |
|---|---|---|
| Power sequencer `0xc424`, table `0xc454` | Dispatches 17 substates using SRAM byte `0x8000c2` | Internal substates are not ACPI sleep-state numbers |
| Output helpers `0xcd14` / `0xcd9c` | Clear/set control bit 16 at `0xf0c518`, with software request arbitration | Strong GPIO106/PWRBTN# mapping from instruction/address/pin evidence; not an enable flag |
| Sequencer substates 4–7 | Assert output, set timer value 50, wait, release output | Timer units and all request sources must be established before assigning a pulse duration |
| Sequencer substate 9 | Reads `0xf0c709` bit 1 | Correlates with SLP_S4# input; not alone proof of S4 lid eligibility |
| Sequencer substate 11, read `0xc608` | After several helper conditions, tests `0xf0c700` bit 0; high returns toward substate 2, low selects substate 12 | Reads the lid candidate input, but cannot yet be called the complete lid event handler |
| VCI lookup `0x142d8`, table `0x308b0` | Token `0x200100` resolves to table index 3 | Firmware descriptor, not a raw register mask |
| VCI query `0x14520` | Returns 1 when status bit 3 is clear and latch-enable bit 3 is set | Corroborated with ARC delay slots; consistent with VCI_IN3, not an OR of the two bits |
| VCI setup/edge helpers `0x14330`, `0x14454`, `0x144e4` | Configure, inspect and clear selected VCI edge/latch state | Electrical detection is separate from host wake permission |
| Transition callback `0xd100` | Flag descriptor `0x20` chooses enabled VCI lid detection, with polarity token `0x200100` or `0x200120`; clear flag disables it | Transition preparation, not by itself an interrupt handler |
| Lid callback `0xc960` | Requires input argument 1, descriptor `0x20` set, and internal state 8 or 1; sets request bit 3 in `0x8000c0` | Conditional lid-open request in deep-state-related code; runtime debounce/masks and further gates matter |
| Sequencer substates 14–16 | Can consume that request and return to substate 2 | State 14 can first advance on timer expiry; this is not unconditional immediate wake |

These statements are corroborated selected control-flow findings. Remaining
helper predicates and the separate EC_WAKE/SMI output still need investigation.
The recovered request uses the PWRBTN# path; it does not depend on proving that
arming GPE `0x17` supplies the missing EC policy.

## Host bit to retained lid flag

Two independent decoders agree on this chain:

```text
ACPI EC write: byte 0x01, bit 6 (BTPC)
  -> 0x15fe8 updates host shadow and queues asynchronous work
  -> 0x175e0 decodes the per-byte/per-bit dispatch tables
  -> 0x17c00 selects set/clear for flag descriptor 0x20
  -> 0x20ad0 / 0x20b14 update cache index 10, bit 0
  -> 0x209bc writes retention byte 0xf0cd08
  -> 0xd100 arms/disarms lid VCI detection
  -> 0xc960 can request the deep-state power-button sequence
```

The dispatch anchors are table `0x31be4[0] = 0`, descriptor `0x4008` at
`0x31aa6`, callback index 10 at `0x31a3a`, and callback pointer `0x17c00`
at `0x31964`. `0x17c00(1)` sets descriptor `0x20`; other values clear it.
These are private-image analysis addresses, not instructions to write those
addresses on the host.

Initialization explains the internal mapping. `INI3` at `0x2e56c` selects
decoder `0x2defc`; its first record expands 320 encoded bytes to 1,408 bytes
at SRAM `0x800000`. Offset `0x518` supplies signed halfwords
`{0, 1, 7, 9, 10, 22}`. Descriptor `0x20` means group 4, bit 0, mapping to
cache index 10. The later cache fill at `0x2091c` reads indices 0–1 through
indexed registers and indices 2–21 through retention registers `0xf0cd00` onward.
Predicated ARC comparisons skip an apparent second indexed-register loop.
Consequently this flag is **not EC indexed register `0x18`**.

Ordinary channel 0 parser `0x16fcc` reads `0xff0904/08`. Commands `0x80/0x81`
dispatch through table `0x30938` to read handler `0x17214` and write handler
`0x17234`; the write's second phase at `0x17254` calls `0x15fe8`. Read results
come through `0x15fa0` and `0x17208` to `0xff0900`. These command meanings agree with
[ACPI's EC interface specification](https://uefi.org/specs/ACPI/6.6/12_Embedded_Controller_Interface_Specification.html).
Initialization `0xd978` programs channel 0 BAR through selector 2 with port
`0x62` and mask 4; `0x10e60` separately programs selector 3 with `0x1600` and
mask 4. Helper `0xfa64` writes `0x00628004` to `0xff336c` and `0x16008004` to
`0xff3370`. These are computed writes, not a new live register readback.
Stock `_CRS` exposes data `0x62` and command/status `0x66`; stock ECOR declares
`BTPC` at byte `0x01`, bit 6.

Vendor channel 1 parser `0xabe0` instead reads `0xff0d04/08` and shares
`0x15fa0/0x15fe8`. Its `0x80/0x81` commands alone would not prove ordinary
ACPI reachability; the independent channel 0 handler and BAR trace close that
static mapping. No reader/writer of `BTPC` was found in the retained DSDT/SSDTs.
Its acronym expansion is unknown.

**Readback/rollback limitation:** `0x15fa0(0x01)` reads host shadow byte
`0x800959`, rather than reading retention register `0xf0cd08` directly.
Saving the old EC byte preserves the observed host value, but does not yet
prove that value reflects the initial retained flag. Startup callback `0x17464`
zeros all 256 host shadow bytes without resetting this retention byte there;
the two values can therefore disagree. Writes are dispatched asynchronously.
Persistence, completion and shadow/retention synchronization
must be established before calling a hardware experiment fully reversible.

## Power-state predicates and remaining gates

Internal state byte `0x8003a0` is not an ACPI sleep-state argument:

| Internal state / callback | Signal-grounded evidence | Qualified interpretation |
|---|---|---|
| 3 / `0xc164` | Moves toward 5 when VCC_PWRGD is low, SLP_S3# low and SLP_S4# high; low SLP_S4# instead leads toward 8 | Running-related state; not a complete definition of S0 |
| 5 / `0xc238` | VCC_PWRGD high leads toward 3; both sleep inputs low with power-good low leads toward 8 | S3-related state |
| 8 / `0xc424` | Deep-off-related power sequencing and SLP_S4# checks; lid callback accepts this state | S4/S5-related; available inputs do not distinguish the two |
| 1 | Also accepted by `0x3c84` for the lid callback | Startup/transition meaning remains open |

`0xc960` is one of eight callbacks for the lid status bit. It is excluded in
states 3 and 5; that does not mean there is no separate S0 notification or S3
wake callback. Those other paths have not all been reconstructed.
Scheduler slot `0x38` maps to `0xc424` in internal state 8, but is empty in
state 1. The proved request-to-power-button worker route is consequently the
state-8 route; accepting a callback in state 1 does not prove it pulses there.

When the request reaches substate 2, timer value 200 is armed. Substate 3 can
advance immediately when `0x17f0c()==1 && 0x17e9c(0xff)==1`. Otherwise its timer
and a second `0x17f0c()` check must pass. Further predicates involving
`0x24500`, `0x17e9c`, `0x1a5b4`, or request bit 8 can divert to substate 12.
Substate 4 calls `0xcd14(2)` to assert PWRBTN#, with later substates releasing
it through `0xcd9c(2)`. Function meanings and timer units remain partly open.
The retained flag is a required gate for this branch, not proof all gates pass.

## Review-only `_PTS` proposal and approval boundary

An actionable finding needs the exact lid-open branch, the power-state predicate,
the output it drives, and a proven translation from the relevant control to host
EC RAM or indexed ports. Internal SRAM `0x8000xx`, GPIO configuration, a VCI
descriptor and EC RAM byte offsets are different address spaces. A software
request bit is not automatically a writable wake-enable flag.

Stock AML already exposes lid status through EC `0x46` bit 2 and wake control
through EC `0x32` bit 2 (`HWLO`); prior tests established that arming the latter
does not enable S4 wake. See [stock AML](../../firmware/stock-reference/DSDT.dsl).
The indexed [helper](../../firmware/tools/ecidx.py) uses `0x15ec/0x15ee`; even its
read operation writes the selector. It was not run in this continuation.

The candidate is **set `BTPC = One` for `_PTS(4)`**, preserving the other bits
of EC byte `0x01`, because the host setter enables both lid VCI configuration
and the deep-state lid-open request. This is distinct from the already tested
`HWLO` bit and indexed `0x41` state. A review sketch is:

```asl
// Illustration only: integrate with the existing _PTS/_WAK; never duplicate them.
// BTPC must be an EmbeddedControl Field at byte 0x01, bit 6,
// ByteAcc, Preserve, using the existing EC OperationRegion.
Name (LBOS, Zero) // Saved HOST SHADOW value, not proven retained-state readback.
Name (LBSV, Zero)

// Within existing _PTS, after ordinary sleep preparation:
If (Arg0 == 4) {
    If (LBSV == Zero) {
        LBOS = \_SB.PCI0.LPCB.EC.BTPC
        LBSV = One
    }
    \_SB.PCI0.LPCB.EC.BTPC = One
}

// Within existing _WAK after EC availability is established:
If (LBSV) {
    \_SB.PCI0.LPCB.EC.BTPC = LBOS
    LBSV = Zero
}
```

This is neither a compiled patch nor a safe deployment recipe. Linux's
[ACPI sleep preparation](https://github.com/torvalds/linux/blob/master/drivers/acpi/sleep.c)
and [hibernation restore](https://github.com/torvalds/linux/blob/master/kernel/power/hibernate.c)
can restore ACPICA state with the image; a fresh firmware boot alone does not
determine these globals' lifetime. Actual preparation/snapshot/finish
ordering, repeated preparation, interrupted sleep and cold boot without resume
need a reviewed cleanup plan. No rollback ordering test was performed here.
The asynchronous setter and readback caveat above prevent promising complete
restoration from this sketch alone. No S5 enablement is proposed.

**Stop before testing.** The next permitted request is approval for a limited
live **read-only baseline**, separately from any write or sleep test: inspect the
active DSDT's EC resources/field, read existing byte `0x01` through the kernel's
EC interface if already available, and capture current firmware/version state.
No raw port probe, EC write, driver bind, reboot, suspend or hibernate is part of
that baseline. If the interface is unavailable, stop rather than enabling it.
The recorded running configuration disables `CONFIG_ACPI_EC_DEBUGFS`, so that
part of the baseline may be unavailable; no module/config change is authorized.
Even a zero shadow bit does not prove the retained flag is clear. A later
hardware experiment requires explicit authorization overriding the current
read-only restriction, a persistence/synchronization result, and a concrete
rollback plan; none was performed in this continuation.

Private extraction, manifests, Ghidra project/results and independent GNU output
are retained under `/home/btw/test/rea/work/audit-20261009-ec`. Raw EC images and
complete proprietary disassembly are not added to GitHub. The
[coverage ledger](11-coverage-and-feature-roadmap.md) records other open firmware
and hardware paths and how they relate to useful features.

The private Ghidra project was saved, the temporary localhost server stopped,
and the added processor-directory link removed. Ghidra identified 1,225
functions; that is an analysis count, not 1,225 fully reconstructed routines.
