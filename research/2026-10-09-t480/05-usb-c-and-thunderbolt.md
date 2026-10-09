# USB-C and Thunderbolt — 2026-10-09

The useful result is a recovered stock UCSI mailbox and transport map, with a
tested offline model. **Native USB-C restoration remains open.** The live kernel
disables TYPEC and the coreboot ACPI tables omit stock `_SB.UBTC` and its `_Q4F`
notification bridge. The working Thunderbolt PCI driver supplies a different
layer; it does not restore connector status, PD control or UCSI notifications.
No raw EC access, connector command, firmware write or dock experiment was
performed during this research.

## What the stock binaries and AML establish

REA 6.1.0 and Ghidra 12.1.4 analyzed copies of stock `UsbTypeCDxe`, `SmmAslSmi` and
`EcIoSmm`; decompiler interpretations were checked against assembly. The DXE
driver allocates a 4 KiB ACPI NVS page below 4 GiB, zeros its first 48 bytes, sets
the software mailbox version to `0x0100` (**UCSI 1.0**) and publishes its pointer.
This is initialization, not an EC capability response. Vendor setup-field names
and the precise enabling protocol selector remain unresolved.

| Shadow mailbox offset | Length | Field |
|---|---:|---|
| `0x00` | 2 | Version |
| `0x04` | 4 | CCI |
| `0x08` | 8 | CONTROL |
| `0x10` | 16 | MESSAGE_IN |
| `0x20` | 16 | MESSAGE_OUT |

AML submits a 37-byte buffer through `HKEY.MHPF`, software-SMI command `0x14`
and its submit/read subcommands. The recovered EC mailbox occupies registers
`0x50..0x74`: command, status, family, selector, a 32-byte payload window and length.
Its transport uses data port `0x1600` and status/command port `0x1604`, with EC
read/write commands `0x80`/`0x81`. Ordinary ACPI EC ports `0x62/0x66` are a separate
interface and cannot be substituted by assumption. C55 source configures the
LPC decode and captured ACPI reserves the vendor ports; real transactions were
not tested.

| UCSI operation | Command | Family | Selector | Length |
|---|---|---|---|---:|
| Write MESSAGE_OUT | `0x0a` | `0x02` | `0x06` | 16 |
| Write CONTROL | `0x0a` | `0x02` | `0x04` | 8 |
| Read MESSAGE_IN | `0x0b` | `0x02` | `0x05` | 16 |
| Read CCI | `0x0b` | `0x02` | `0x03` | 4 |

One corrected interpretation matters: SMI subcommand 0 updates EC register `0x3b`
bits 5/6. It is **not** a UCSI capability query. Capability, connector count and
alt-mode support require later UCSI commands; AML child objects do not establish
those results.

## Completion and notification must be reconstructed carefully

`CHKS` polls command register `0x50` up to 1,000 times with 1 ms sleeps. A cleared
command with status bit 7 set returns 0; bit 7 clear returns `0x8081`; exhausted
polls return `0x8080`. The low seven transport-status bits are unknown and must remain
opaque. These flags are distinct from UCSI CCI busy/error/command-complete flags.
Stock `MHPF` discards the `CHKS` result, and the UCSI AML does not inspect response
status. Lower EC input/output waits check at most 33 times with 30 µs delays but
silently continue after exhaustion; output draining has no finite bound. These
are recovered stock error-path weaknesses, not observed laptop timeouts.

The transaction mutex protects MESSAGE_OUT-before-CONTROL and
MESSAGE_IN-before-CCI ordering. Stock `_Q4F` refreshes the mailbox, waits 1 ms, then
notifies `UBTC` with `0x80`. Refresh-before-notify and UCSI acknowledgements matter
to Linux. Races between EC events and the two-part snapshot remain unresolved.

The [offline model](../../firmware/tools/ucsi-model.py) has no hardware/network
access. Six tests passed for packet sizes/vectors, ordering, busy refusal,
polling boundaries, opaque status, CCI validation and version initialization:

```sh
python3 firmware/tools/ucsi-model.py --self-test
```

A native implementation must allocate its own mailbox, serialize EC access with
other users, validate lengths, propagate errors, bound every wait and deliver
notifications. Do not reuse the stock physical NVS address or turn a printed
register plan into a raw port writer. Firmware exposure plus TYPEC/TYPEC_UCSI and
the selected transport backend are all required. The next milestone is a
controlled read-only capability/status path, followed by both connectors,
charging, role/alt-mode, unplug/replug and sleep tests. None is established yet.
The [USB-C wiki](../../docs/wiki/usb-c.md) contains RVAs, hashes and remaining
uncertainties.

## Thunderbolt health and DMA policy are separate observations

The JHL6240 host binds to Linux's Thunderbolt driver and reports NVM **23.0**.
Lenovo's cited T480 critical-update bulletin lists minimum NVM 20, so this host is
above that minimum. This does not establish the newest release or certify dock
compatibility; no controller update was performed.

The domain reports security `none`, permitting automatic firmware connection,
and `iommu_dma_protection=0`. VT-d is enabled,
but the actual NHI `05:00.0` uses group 13 of type `identity`, shared with `04:00.0`;
`07:00.0` is xHCI. Enabled VT-d and an IOMMU group list therefore do not prove
translated DMA protection. Do not treat blanket dock authorization as a power
optimization. A translated/strict host-DMA profile needs dock and VM tests before
deployment, separately from UCSI restoration.

The [storage/display/Thunderbolt page](../../docs/wiki/storage-display-thunderbolt.md#thunderbolt-and-usb-c-are-separate-layers)
links Lenovo's bulletin and Linux's protection interface. No dock was attached
for charging, display, USB or PCIe validation. These missing interfaces do not
by themselves prove that charging is broken. The
[evidence page](../../docs/wiki/evidence.md) records private manifests and artifact
hashes; this publication includes decoded findings rather than proprietary raw
firmware. This analysis requires no hardware rollback because it made no hardware
changes.
