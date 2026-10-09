# VBIOS and board control

## Full image and ACPI exposure

The existing full VBIOS is **184,320 bytes**, SHA-256
`92936e91fbb591086473366fa6a73c40ec2450e58a90489fba83ce4b58803d82`.
The 119,808-byte reference is its exact prefix, SHA-256
`46f03d4eeb2e8e2c78b7b8c1a98499ef991d5066e0d349011da6ad140f07481f`.
The legacy `55aa` init header declares 119,808 bytes; PCIR at `0x170` declares
**182,272 bytes**, vendor/device `10de:1d10`, x86 image and final-image flag.
The final 2048 bytes of the full file are zero padding. The declared image's
8-bit checksum is zero, which is not proof of cryptographic authenticity or
acceptance after modification.

Archived C55 SHA-256
`63c5ddbe65a9c51da845b83dc4540df706488b76bcee49d8bbca0825ee6b1576`
contains the exact full VBIOS in CBFS `pci10de,1d10.rom`. Source/config disable
option-ROM execution. The intended CBFS `_ROM` path selects 182,272 bytes,
including all nonpadding policy tables. This binds the stored C55 archive;
new installed SPI contents and live `_ROM` length were not measured here.

The short copy can be a valid x86 initialization prefix while missing later
performance policy. It should not replace the full driver-facing ROM.
The earlier bounded image walker did not close every source bound: probe
dereferences occurred before bounded sizing, ID fallback could lose the source
size, and `MAX(init_length, image_length)` could exceed a malformed file.
Source-only [patch 0030](../../../firmware/coreboot/patches/0030-local-pci-rom-bound-cbfs-probe-and-acpi-copy.patch)
validates bounded header/PCIR metadata before access, carries the successful
mapping's length through fallback, and checks the complete image chain before
copying. Following [patch 0031](../../../firmware/coreboot/patches/0031-local-cbfs-report-actual-loaded-size.patch)
corrects the CBFS producer to report actual bytes
produced rather than decompression capacity. Both changes are required for this
bound. The valid full MX150 fixture retains all 182,272 nonpadding bytes.

The [ROM sanitizer fixtures](../../../tools/re-audit/tests/test_pci_rom_bounds.py)
reproduce the baseline unchecked-init copy overread and test
the actual patched probe/walker/SSDT functions. They do not establish a malformed
installed ROM or a live crash. On-device ROM mappings, already-loaded RAM ROMs,
generic execution/copy and VFCT remain outside this correction. The separate
ACPI AML generator also has a `length - 4096` underflow concern for images below
4096 bytes; the copy fixture stubs that generator and does not validate its AML
read behavior. The 182,272-byte MX150 image does not trigger that size case.

The [CBFS producer fixtures](../../../tools/re-audit/tests/test_cbfs_size_contract.py)
exercise actual source definitions with controlled direct mappings, allocators,
hash and decoder results. Compressed allocation still uses declared capacity;
only successful produced bytes become a caller's bound. NONE uses raw extent,
and failed ordinary lookup/type/load paths report zero. The oversized-output
guard also rejects nonzero decoder results above capacity; it does not certify
all decompressor implementations or repair existing allocation ownership.

## Known table locations; unsupported decoder values

Image string `86.08.3b.00.38`, GP108/NV138. BIT P version 2 at `0x216`, data
`0x2ee`, length 196:

| P pointer field | Table | Offset | Version |
|---|---|---|---|
| +0x00 | Performance | `0x1d555` | `0x50` |
| +0x20 | Voltage map | `0x1efc8` | `0x30` |
| +0x28 | Power sense | `0x1fec0` | `0x20` |
| +0x2c | Power budget | `0x202ae` | `0x30` |
| +0x38 | Base clocks | `0x1d6b5` | `0x20` |
| +0x3c | Power topology | `0x1ff5b` | `0x20` |
| +0x68 | Voltage rail | `0x4e02` | `0x10` |
| +0x6c | Voltage device | `0x4d3e` | `0x10` |
| +0x70 | Voltage policy | `0x4e12` | `0x10` |

Voltage-device header/entry/count are 4/24/8; the first entry type is 2,
the other seven are zero. Dedicated legacy-voltage, boost and cstep pointers
are absent; absence of a dedicated boost pointer does not prove no boosting.

Checked [envytools decoder source](https://github.com/envytools/envytools/tree/f102b82381f3f11cee113d16374c87091db039d9/nvbios)
as well as its output. Voltage-map v`0x30` falls outside the decoder's supported
v`0x10`/`0x20` cases and can print uninitialized fields. Voltage-object tables
are dumped without usable calibration semantics. Power-budget selection and
base-clock units also remain unresolved. Printed 5001 W caps, 2936 MHz clocks
or voltage zeros are not established active MX150 limits. This prevents a false
numeric undervolt/overclock recommendation from an unsupported parser.

## Physical controls

The [Windu-2/NM-B501 schematic](https://documents.cdn.ifixit.com/RXTmJIF56BpXZcoC.pdf)
was inspected visually. The actual installed board revision has not been freshly
identified, so the drawing is corroborating hardware evidence with that limit.

| Signal | Schematic and firmware evidence | Meaning established |
|---|---|---|
| `VIDEO_PWM_VID` | GPU U208M GPIO0/C6 → PR603 → NCP81278T VID **pin 5**; VBIOS GPIO0 tag `0x81`, PWM function | GPU supplies a physical dynamic voltage request |
| `-VIDEO_POWER_LIMIT` | EC U23C **M4 ADC14/GPIO216** → GPU GPIO12/B4; VBIOS tag `0x6f` `HW_PWR_SLOWDOWN` | Separate EC throttle input, not the PWM VID net |
| `-VIDEO_THERM_ALERT` | GPU GPIO8/F8 → R10621 → UART_TX net → EC **F9/GPIO104** | GPU-to-EC alert route; alternate pin function and consumer remain untraced |
| Video I²C | EC K10/GPIO131 clock, J10/GPIO130 data via PCA9306 | Board sideband connection; complete transaction policy not recovered |
| Rail request/reset/PWRGD | PCH GPP_E23, E22, F3 respectively | T480 rail/reset sequence uses PCH GPIOs |

The [onsemi NCP81278D/T datasheet](https://www.onsemi.com/download/data-sheet/pdf/ncp81278t-d.pdf)
describes PWM duty conversion through VIDBUF, filtering/resistor network and
REFIN to the output voltage. This supports the physical voltage route, not a
Linux undervolt API. No regulator SMBus voltage-write interface was identified.
The generic PMH7 register-50 helper is not the T480's GPIO rail implementation.

The thermal net was corrected after visually tracing schematic sheet 57 through
R10621; the earlier GPIO023/E1 association was wrong. Negative searches for that
wrong pin say nothing about the actual alert handler. Coreboot's optional
MEC1653 UART code uses GPIO104/105, making its TX mux relevant to this shared
net. The stored production configuration has
`CONFIG_MEC1653_ENABLE_UART` **disabled**, so active interference is not proved.
Do not enable that EC UART as a debugging shortcut without validating the actual
board routing and mux behavior. No pin or EC register was queried or changed.

A bounded follow-up roots GPIO104's native pin mapper at `0xf0c510` and its IRQ
mapping at `0xf0c014`, mask `0x10`. The retained IRQ table's bit 4 entry resolves to
callback `0x28ee`, which immediately returns. This proves that selected native IRQ
slot has a default no-op, not absence of thermal handling. A specific pin
initializer, polling selector and other consumers were not rooted; configuration
and alternate-function handling remain open. Private mapper/table/callback
evidence and a separate hash manifest preserve that limit.

## EC power-limit policy

The supplied N24HT37W ARCompact payload used in the
[lid investigation](../10-ec-firmware-and-lid-wake.md) was analyzed for selected GPU
policy paths. GPIO216 is octal naming: decimal 142. The generic EC pin driver computes
`0xf0c400 + (port-1)*0x20 + 4*bitindex`; the matching descriptor `0x2212e6`
selects **`0xf0c638`**. The actual policy routine at `0xc9fc` changes pin-control
bit 16. With SRAM `0x8000c8` bit 0 set, it returns without changing the pin. Otherwise
it clears bit 16 if `0x8000c9` bit 0, `0x8000c8` bits 1/2/3, or signed policy value
at `0x8000cc` greater than zero is present; only if none of those conditions holds does it set
bit 16. Generic pin initialization selects GPIO, output direction and buffer
mode. [Comparable-family MEC1632 register documentation](https://ww1.microchip.com/downloads/en/DeviceDoc/00001592B.pdf)
(pp 395–398) identifies the output/open-drain
semantics, while exact MEC1653 documentation remains limited. This supports
low/assert and high/release of the active-low throttle line; it is not a live
pin-voltage measurement.

The exact [coreboot MEC1653 UART definitions](https://github.com/coreboot/coreboot/blob/main/src/ec/lenovo/mec1653/uart.h)
independently anchor pin-control base `0xf0c400`; the comparable manual is used
only for register semantics and is not called a complete MEC1653 datasheet.

The dGPU-presence gate reads GPIO062, schematic `VIDEO_ID`: pull-up for the
discrete-GPU board and pull-down for UMA. Initialization at `0xcb70` disables
the writer when that input is low. It samples the input, rather than assuming
the presence of a GPU from a constant.

Another rooted input at `0x208bc` reads GPIO223 (`0xf0c712` bit 3), schematic
the board's `-CHG_PROCHOT` protection line. Low input is inverted and passed through
`0xcaa0` into `0x8000c9` bit 0, causing throttle assertion through the policy above.
Its Q199 connection is identified; complete source arbitration is untraced.
This is a protection path, not a voltage-request knob.

The ordinary EC host setter for byte `0x03`, bit 3 reaches `0x17bc0`, a retained
flag and local `0x8000c8` bit 3. The supplied stock DSDT does not name that bit.
The writable mask and handler are recovered, but its intended policy meaning
is unknown. No command or AML proposal to set/clear it is made.

Other local predicates remain partly decoded: bit 1 has several clearing callers
but no proven setting source; bit 2 comes from a scanner of 16 six-byte records;
known callers select a signed policy value 0/1/2 from state/config masks with hysteresis
and separate event cleanup. Sensor identities/units and complete arbitration
are not established. The policy value is not proved to be a reference counter.
Physical active-low signal naming, register behavior and unknown predicate
meanings remain distinct from current board state.

This work does not disable the EC thermal/power protections, edit firmware or
touch its ports. Policy inputs and their real power/thermal meanings need mapping
before suggesting any change. More GPU performance obtained by bypassing a
shared laptop protection would not establish safe undervolting.

## Editing prerequisites

GP108 VBIOS signature/authentication and acceptance of a modified policy remain
unresolved. The PCI checksum, object parser and physical PWM path do not establish
editability. A credible future firmware proposal needs complete table semantics,
active board calibration, authentication behavior, a bounded build, external
recovery and a reviewed test. No VBIOS or EC edit/flash is proposed here.
