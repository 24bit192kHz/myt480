# USB-C, UCSI, and the recovered EC mailbox

## The missing layers

The live kernel has `CONFIG_TYPEC` disabled and no `/sys/class/typec` interface.
More fundamentally, the live coreboot ACPI tables omit the stock `_SB.UBTC` UCSI
device. Thunderbolt's PCI NHI binds to the `thunderbolt` driver; that does not
provide the missing USB-C connector/PD control interface.

Stock `SSDT10.aml` (OEM `UsbCTabl`, 1,656 bytes) provides `UBTC` with HID
`USBC000`, CID `PNP0CA0`, and a 4 KiB memory resource based on `UBCB`. Its 56-byte
operation region has this layout:

| Offset | Length | Field |
|---|---:|---|
| `0x00` | 2 | UCSI version |
| `0x04` | 4 | CCI |
| `0x08` | 8 | CONTROL |
| `0x10` | 16 | MESSAGE_IN |
| `0x20` | 16 | MESSAGE_OUT |

The `_DSM` UUID is `6f8398c2-7ca4-11e4-ad36-631042b5008f`.
Function 0 advertises mask `0x0f`; function 1 writes, function 2 reads, and function
3 returns the GNVS byte `XDCE` (its precise meaning is still unresolved).
`_STA` depends on `OSYS >= 2015`, `TBTS`, and `USTC`.
The stock EC query `_Q4F` invokes `UBTC.NTFY`, which reads the mailbox, sleeps
1 ms, and sends ACPI notification `0x80`.

## How stock firmware creates the mailbox

Further offline analysis on 2026-10-09 recovered the stock `UsbTypeCDxe` driver,
file GUID `15B985C5-7103-4F35-B59D-2235FC5F3FFE`. Its 2,080-byte PE image has
seven analyzed functions. The main function, RVA `0x2d4`, performs these steps:

1. Extract its RAW section and install its `UsbCTabl` SSDT through the ACPI table
   protocol. It changes the extracted table's OEM ID to `INTEL ` before install.
2. Read the `Setup` variable, requesting `0x5eb` bytes. Query a vendor protocol
   (`11b34006-d85b-4d0a-a290-d5a571310ef7`), method offset `0x30`, selector 8.
   Mailbox allocation requires that method's byte result and `Setup[0x4ad]` to be
   nonzero. The selector and setup field are identified by location, not by a
   recovered human-readable name.
3. Locate the global NVS protocol
   (`074e1e48-8132-47a1-8c2c-3f14ad9a66dc`). Allocate **one 4 KiB ACPI NVS page**
   using `AllocateMaxAddress`, with maximum address `0xffffffff`.
4. Explicitly zero the first **48 bytes**, then write little-endian `0x0100` at
   offset 0. This is **UCSI 1.0**, not an EC version read or a capability response.
5. Store the 32-bit allocation address at GNVS offset `0x615`, matching `UBCB`
   in the stock DSDT. Publish that four-byte pointer as variable `UsbTypeC`, GUID
   `fc876842-d8f0-4844-ae32-1ff843797b17`, attributes `0x3`.

UEFI constants identify memory type 10 as `EfiACPIMemoryNVS` and attributes
`0x3` as **nonvolatile plus boot-service access**, without runtime access.
These interpretations were checked against
[EDK2's UEFI definitions](https://github.com/tianocore/edk2/blob/master/MdePkg/Include/Uefi/UefiMultiPhase.h)
and [allocation definitions](https://github.com/tianocore/edk2/blob/master/MdePkg/Include/Uefi/UefiSpec.h).
Linux's [UCSI header](https://github.com/torvalds/linux/blob/master/drivers/usb/typec/ucsi/ucsi.h)
identifies `0x0100` as version 1.0.

The allocation is a **software shadow mailbox**; AML exchanges its contents with
the EC. Reproducing its layout is insufficient without the transport. The driver
does not query the EC's connector count, charging capability, or alt-mode support
here. Those come from later UCSI commands. The two AML connector child objects
describe physical locations, not a captured `GET_CAPABILITY` result.

## ACPI-to-SMM path

The methods send a 37-byte buffer through `HKEY.MHPF`. Under mutex `BFWM`, that
method copies the buffer to the stock NVS field `BFWB` and invokes `BFWP`/`BFWL`.
Those are software-SMI command `0x14`, subcommands 1 and 2. The existing stock
analysis identifies the software-SMI trigger as `0xf5`; see the
[earlier SMM investigation](../notes/2026-10-08-port-fixes.md).

REA/Ghidra analysis of `mod-SmmAslSmi.pe` recovered the command table at RVA
`0x3c40`. Its entries are **8-byte pointers**. Command `0x14` points to `0xe48`,
which dispatches subcommands through the table at `0x3dd0`:

| Subcommand | Function RVA | Purpose |
|---|---|---|
| 0 | `0x1384` | Rewrite EC RAM `0x3b` bits 5/6 from argument bits 5/6 |
| 1 | `0x13c0` | Submit mailbox request when idle |
| 2 | `0x14b8` | Read response/status |
| 3 | `0x1528` | Separate general interface path; not reconstructed here |

The request submitter checks EC register `0x50`. When idle, it writes family,
selector, length, optional payload, and finally the command byte. When busy, it
reads status into the NVS buffer instead of submitting another command.

Subcommand 0 was previously described here as a capability query. Its decompiled
implementation instead computes `(old & 0x9f) | (argument & 0x60)` for register
`0x3b`. `HKEY.MHCF` separately stores `argument >> 5` in battery-status bookkeeping
`BSWR`. It is not a UCSI capability query and is unnecessary for the four UCSI
mailbox operations below.

## Recovered mailbox mapping

| Buffer byte(s) | Stock NVS offset | EC register(s) | Role |
|---|---|---|---|
| 0 | `0xdd1` | `0x50` | Command; bit 0 selects response payload behavior |
| 1 | `0xdd2` | `0x51` | Status/return value |
| 2 | `0xdd3` | `0x52` | Family |
| 3 | `0xdd4` | `0x53` | Selector |
| 4..35 | `0xdd5..0xdf4` | `0x54..0x73` | Payload window |
| 36 | `0xdf5` | `0x74` | Payload length |

The UCSI AML uses the following requests, with family `0x02`:

| Operation | Command | Selector | Length |
|---|---|---|---:|
| Write MESSAGE_OUT | `0x0a` | `0x06` | 16 |
| Write CONTROL | `0x0a` | `0x04` | 8 |
| Read MESSAGE_IN | `0x0b` | `0x05` | 16 |
| Read CCI | `0x0b` | `0x03` | 4 |

`SmmAslSmi` calls the first two slots of the EC I/O protocol. Analysis of
`mod-EcIoSmm.pe` shows those slots are functions `0x7d0` and `0x7e0`, using
**data port `0x1600`, status/command port `0x1604`**. Read uses EC command `0x80`;
write uses `0x81`, with input/output-buffer polling between stages. Separate
protocol slots use `0x62/0x66`. These addresses were recovered offline; no raw EC
port access was performed during this audit.

The C55 source baseline `b5fbbcff8a` already sets `gen1_dec` to
`LPC_IO(0x1600, 0x80)` in `sklkbl_thinkpad/devicetree.cb`. Captured live DSDT
device `ECMM` reserves `0x1600` and `0x1604`. Thus the ports are configured in
source and described to the OS; actual mailbox transactions were not tested.
The existing ordinary ACPI EC uses `0x62/0x66`. Access to the vendor mailbox
through those ordinary EC ports cannot be assumed merely because both interfaces
use EC read/write opcodes.

## Completion, errors, and notification ordering

Stock DSDT field `HMPR` is the full EC command byte at `0x50`. Field `HMDN` is
**bit 7** of status byte `0x51`. `EC.CHKS` polls `HMPR`, sleeping 1 ms for each
nonzero value, at most 1,000 times:

| Observation | `CHKS` return |
|---|---|
| Command cleared, status bit 7 set | `0` |
| Command cleared, status bit 7 clear | `0x8081` |
| Command remains nonzero for 1,000 polls | `0x8080` |

The raw status byte also has seven low bits whose meanings have **not** been
recovered. `CHKS` does not inspect them. They must be retained as opaque status,
not guessed to mean success. These vendor transport flags are distinct from
UCSI's four-byte CCI, whose busy/error/command-complete flags occupy bits 28/30/31.

`MHPF` calls `CHKS` after an accepted submission but **discards its return**,
then reads the response through SMM anyway. `ECWR` and `ECRD` also do not inspect
the response status byte. This establishes a weakness in the stock error path;
it does not establish that a timeout occurred on this laptop.

The lower EC I/O layer has separate limitations. `EcIoSmm` RVA `0xd2c` waits for
input-buffer empty and `0xd9c` waits for output-buffer full. Each checks at most
33 times with a 30-microsecond delay (`0x2dc4` converts its argument through a
one-million divisor). Both return without reporting exhaustion, and their callers
continue I/O. RVA `0xd68` drains output bytes until OBF clears with **no finite
bound**. A replacement should propagate polling failures and bound draining,
rather than duplicate these behaviors.

The outer `UBSY` mutex encloses each two-part transaction:

- `ECWR`: write the 16-byte MESSAGE_OUT first, then the 8-byte CONTROL that starts
  the UCSI command.
- `ECRD`: refresh the 16-byte MESSAGE_IN first, then the 4-byte CCI.
- `_Q4F` → `NTFY`: perform `ECRD`, wait 1 ms, and notify `UBTC` with `0x80`.

Linux's [ACPI UCSI implementation](https://github.com/torvalds/linux/blob/master/drivers/usb/typec/ucsi/ucsi_acpi.c)
reads the cached CCI when processing notifications. Thus the **refresh-before-notify**
ordering matters. Its polling path invokes `_DSM` function 2; writes invoke
function 1. A native transport must also preserve standard UCSI command-complete
and connector-change acknowledgements. The proprietary mailbox analysis does not
resolve all races between a new EC event and a two-part snapshot.

## Tested offline model

[`firmware/tools/ucsi-model.py`](../../firmware/tools/ucsi-model.py) creates the
four exact 37-byte packets, lists ordered EC register writes, models `CHKS`'s
poll-count boundaries, initializes the 48-byte shadow, and decodes CCI. It has
**no device access, networking, sleep, or hardware execution**. Its submission
planner rejects a busy mailbox; its completion model preserves the unknown low
status bits. These checks are more explicit than the stock AML but do not prove
hardware behavior.

```sh
python firmware/tools/ucsi-model.py --self-test
python firmware/tools/ucsi-model.py packet write-control --data 0600000000000000
python firmware/tools/ucsi-model.py cci 0x80001004
```

The packet example encodes UCSI `GET_CAPABILITY`; it only prints a fixture.
Six offline tests passed, covering all four packet vectors, exact lengths,
payload/command ordering, busy refusal, pair ordering, 999/1,000-poll boundaries,
opaque status retention, CCI limits, and version initialization. Do not feed the
printed register plan into a raw port writer.

## Implementation route and remaining work

A native implementation can replace this narrow vendor-SMM bridge. It must own
and serialize EC transactions, check the busy/status protocol, bound all waits,
validate lengths, and preserve notification/ack ordering. Ensure the LPC bridge
actually decodes the configured `0x1600` transport and define arbitration with
other EC users.
Firmware must expose
a correctly allocated mailbox and ACPI/UCSI interface; the kernel needs TYPEC,
TYPEC_UCSI, and the chosen ACPI/custom transport backend. A stock physical NVS
address must never be copied into coreboot unchanged.

Version, allocation, transport completion bit, stock polling limits, and stock
notification ordering are now recovered. Still unresolved: low EC status bits,
all vendor configuration-field names, real transaction timing, runtime LPC
decoding and EC arbitration, delivery of query `0x4f` under
coreboot, UCSI cancellation/reset behavior, and real charger/dock behavior.
Neither the captured live DSDT nor coreboot's generic H8 `ec.asl` defines
`_Q4F`; adding `UBTC` alone therefore leaves the stock notification chain absent.
First validate read-only connector capability/status through a serialized
transport. Then test negotiated power, both connectors, unplug, replug,
suspend/resume, and DP alt-mode before making it the default.

This audit produced the transport map, not a working UCSI driver. Charging and
display behavior cannot be inferred solely from Linux's missing class interface.

## Evidence and reproducibility

Analysis used REA **6.1.0**, Ghidra **12.1.4**, and JDK 21 on extracted stock
firmware; original bytes were not modified. Local evidence is retained at
`/home/btw/test/rea/work/audit-20261009-usbc/`. It includes `typec-functions.json`,
`typec-entry-assembly.json`, `typec-data.json`, `smm-functions.json`,
`ecio-polling.json`, and `ecio-primitives.json`. Addresses above are RVAs in the
zero-based PE imports, not live addresses.

| Extracted artifact | SHA-256 |
|---|---|
| UsbTypeCDxe PE, GUID `15B985C5-7103-4F35-B59D-2235FC5F3FFE` | `71b192a866d4d5aebe09b7272401db24e760f96f9e94ecf9ddbda5cfe9622f7d` |
| UsbTypeCDxe embedded RAW AML | `2ea2144620a27fd42ec2407184b86c2f7113462a2998991040b2157b8b42e967` |
| SmmAslSmi PE | `149a282a9ca05baa6ad659a09a9aeda2f6143612221cc2adb2828f7774312e10` |
| EcIoSmm PE | `b44f9cb7fcc7a798b967ff5769bd9002981ec331cf7085f0a6fc62fc5feddf00` |

The embedded AML and the installed Lenovo SSDT have different hashes/OEM headers;
the DXE driver edits the header while installing. Decompiler output was checked
against assembly for allocation arguments, setup offset, version, pointer width,
and variable attributes. Status interpretation beyond the recovered bit and
standard CCI fields remains explicitly unverified. This continuation made no live
hardware or firmware changes, so it requires no hardware rollback.
