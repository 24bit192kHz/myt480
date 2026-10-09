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
3 invokes `XDCE`. `_STA` depends on `OSYS >= 2015`, `TBTS`, and `USTC`.
The stock EC query `_Q4F` invokes `UBTC.NTFY`, which reads the mailbox, sleeps
1 ms, and sends ACPI notification `0x80`.

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
| 0 | `0x1384` | Capability query |
| 1 | `0x13c0` | Submit mailbox request when idle |
| 2 | `0x14b8` | Read response/status |
| 3 | `0x1528` | Separate general interface path; not reconstructed here |

The request submitter checks EC register `0x50`. When idle, it writes family,
selector, length, optional payload, and finally the command byte. When busy, it
reads status into the NVS buffer instead of submitting another command.

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

## Implementation route and remaining work

A native implementation can replace this narrow vendor-SMM bridge. It must own
and serialize EC transactions, check the busy/status protocol, bound all waits,
validate lengths, and preserve notification/ack ordering. Ensure the LPC bridge
decodes the `0x1600` transport and define arbitration with other EC users.
Firmware must expose
a correctly allocated mailbox and ACPI/UCSI interface; the kernel needs TYPEC,
TYPEC_UCSI, and the chosen ACPI/custom transport backend. A stock physical NVS
address must never be copied into coreboot unchanged.

Still unresolved: version/mailbox initialization, exact completion/error semantics, transaction timing, the
notification path under coreboot, command cancellation, reset, and behavior with
real chargers/docks. First validate read-only connector capability/status through
a serialized transport. Then test negotiated power, both connectors, unplug,
replug, suspend/resume, and DP alt-mode before making it the default.

This audit produced the transport map, not a working UCSI driver. Charging and
display behavior cannot be inferred solely from Linux's missing class interface.
