# Synaptics fingerprint protocol and lifecycle

This continues the [fingerprint audit](fingerprint.md). It reconstructs selected
paths in Lenovo's historical Windows driver and compares them with the existing
Linux Rust implementation. No live reader was claimed, reset, enrolled, or
queried during this continuation.

## Artifact and reproduction

| Item | Value |
|---|---|
| Archive | Lenovo `nz3gf07w.exe`, release `5.3.3542.26` |
| Analyzed file | `synaWudfBioUsb.dll`, 2,160,248 bytes, x86-64 PE |
| SHA256 | `aed9f4246bbaca28776c05c169c734cae9763322b03dc00e4319320e1aa2294d` |
| Analysis | REA 6.1.0 with Ghidra 12.1.4; read-only ephemeral import |
| Address convention | PE virtual addresses at image base `0x180000000`; subtract that base for RVA |
| Private evidence | `/home/btw/test/rea/work/audit-20261009-fingerprint/` |

The directory holds tool results, function assembly, and decompiler output.
The DLL and complete proprietary pseudocode remain outside this repository.
Decompiler function names and structure-field types are inferred. The reset
construction was also checked against assembly, rather than accepting its
pseudocode alone.

To reproduce, verify the hash, open the DLL in REA, batch-decompile the addresses
below, and read 128 bytes at `0x180116950`. Interpret that dispatch table as
little-endian 64-bit function pointers. Importing a new DLL version requires
fresh xrefs; these addresses are specific to this hash.

## Reset command recovered

The earlier audit stopped at an operation dispatcher. The continuation resolved
the table entry and the command builder:

```text
CBiometricDeviceUSB::DeviceReset (0x180027be0)
  -> wrapper 0x18002efd0
  -> API 0x180057460, requested reset mode = 0
  -> dispatcher 0x18005e0d0, operation = 2
  -> table 0x180116950 + 2 * 8
  -> command builder 0x180083410
  -> command exchange 0x18007f730
```

`0x180083410` allocates three bytes through `0x180087ed0`, which zeroes the
allocation and writes its first byte. It sets opcode `0x05`, clamps the requested
16-bit mode to at least `2`, and copies that word at offset 1. For this caller,
the resulting buffer is **`05 02 00`**. This is the same payload used for a sensor
reboot during wedged-session recovery in [device.rs](../../src/validity-rs/src/device.rs).
The interpretation as the same recovery operation is supported by that source
comparison; this analysis did not measure the command's effects on hardware.

The builder requests a two-byte response before possible secure-session
adjustment. The reset callback `0x180065890` only stores the operation status and
calls `0x18003ebb0`, whose successful path uses `SetEvent`. It is a completion
notification, not the reset transport itself.

The assembly at `0x180083472`–`0x180083523` corroborates the allocation length,
opcode, mode clamp, word copy, and exchange call. The mode is a 16-bit value;
there is no evidence here that arbitrary modes are interchangeable. Only the
mode used by this `DeviceReset` caller was characterized.

## Capture stop is a separate command

`0x1800582e0` submits operation 10 through the same dispatcher. Table slot 10
resolves to `0x180083350`, which allocates **one byte `04`** and requests a
two-byte reply. The capture-cleanup routine `0x180030130` calls this API when
its session-state checks permit it. This agrees with the Linux
[`Device::capture`](../../src/validity-rs/src/sensor.rs) cleanup command
`self.app(&[0x04])`.

That does not prove every Windows cancellation edge sends `04`. The recovered
worker has additional state and cleanup branches. What is established is the
stop command's construction and its use by capture cleanup. The complete
cancellation-to-stop path remains a useful trace target.

## Command transport recovered

| Address | Observed responsibility |
|---|---|
| `0x18007f730` | Exchange, then response callback and buffer release |
| `0x18007f880` | Optional secure-session preparation, response allocation, transport invocation, response handling |
| `0x180078ea0` | Can replace the plaintext command buffer with a secure-session buffer |
| `0x180103470` → `0x180105fc0` | Submit the internal exchange request `0x938ae058` |
| `0x1801065c0` | Decode request `0x938ae058` and validate input/output descriptor sizes |
| `0x180106c00` | Write on pipe index 0, then read on pipe index 1; retry once on internal read status `0xdb` |
| `0x1801079a0` → `0x180108a10` | Query the pipe, then invoke WinUSB write |
| `0x180107920` → `0x180108890` | Query the pipe, then invoke WinUSB read |
| `0x180107b40` | Resolve the WinUSB function table using `GetProcAddress` |

The loader fixes the otherwise ambiguous indirect calls: table offsets `0x20`,
`0x28`, and `0x30` are `WinUsb_QueryPipe`, `WinUsb_ReadPipe`, and
`WinUsb_WritePipe`. Pipe indices are descriptor indices, not USB endpoint
addresses. The Linux implementation uses command endpoints `0x01` and `0x81`,
data endpoint `0x82`, and interrupt endpoint `0x83`; those values come from
[usb.rs](../../src/validity-rs/src/usb.rs), not from this recovered Windows
descriptor lookup.

The buffers `05 02 00` and `04` are commands **before optional secure wrapping**.
They should not be assumed to appear literally in every USB capture. Secure
session state and the TLS transcript matter. The reason for retry status `0xdb`
was not independently established; the retry behavior was recovered without
assigning a new protocol meaning to that code.

## Cancellation and recovery have different scopes

| Action | Recovered behavior or source evidence |
|---|---|
| Client cancellation | `0x180012534` marks cancellation, stores its mode, and signals the worker event |
| Finger-wait completion | `0x1800125dc` wakes, checks cancellation before processing finger events, returns internal cancelled status `0x67`, and removes the event registration |
| Capture stop | Application command `04`; distinct from reset |
| Sensor reset | Application command `05` plus a 16-bit mode; this caller uses `05 02 00` |
| Endpoint reset | The WinUSB open helper `0x180107e80` queries pipes and attempts pipe reset |
| Host device restart | Hard-reset path `0x18003bac0` → `0x18005e530` → `0x180103690` → `0x180105a70` submits `0x938ae004`; the WinUSB backend closes handles and requests a Windows device restart |
| Factory reset | Linux `factory-reset --yes` wipes pairing/firmware/fingers; no equivalent destructive path was exercised or needed here |

For endpoint reset, Microsoft's API documentation describes clearing the pipe's
stall condition and data toggle. This is a different scope from restarting the
sensor's application session. [WinUsb_ResetPipe documentation](https://learn.microsoft.com/en-us/windows/win32/api/winusb/nf-winusb-winusb_resetpipe).

The host-restart path uses `SetupDiCallClassInstaller` with request `0x12` and a
property-change structure in `0x18010a8b0`. Mapping the structure to the Windows
API identifies a device stop/restart request, not a fingerprint-database erase.
[SP_PROPCHANGE_PARAMS documentation](https://learn.microsoft.com/en-us/windows/win32/api/setupapi/ns-setupapi-sp_propchange_params).

Windows' USB recovery guidance calls for completing/cancelling outstanding
transfers before resetting an endpoint or port, and escalating recovery when
the narrower operation does not resolve the failure. That supports serializing
recovery with the capture worker rather than resetting underneath it.
[USB recovery guidance](https://learn.microsoft.com/en-us/windows-hardware/drivers/usbcon/how-to-recover-from-usb-pipe-errors).

## Linux source fix and remaining candidates

The current Rust backend already owns the device under a mutex, retains cancel
requests, sends capture stop after capture returns, drains stale events, waits
150 ms after aborted capture, reopens failed sessions, and sends disconnected
status when the daemon aborts. The recovered Windows lifecycle supports keeping
these phases explicit. An unconditional sensor reset for every cancellation is
not justified by this evidence.

The cancellation defect was reproduced without a USB device. A private helper
accepts an injected interrupt reader while the real `Usb::wait_int` supplies the
existing endpoint read with its 50 ms poll timeout. With the previous policy,
four fixtures passed and two failed: a cancel already set still accepted an
event, and a continuous successful-event stream ignored cancellation entirely.

The source now checks cancel before starting a read and again before accepting
a successful result. A timeout retries through the same check. Thus cancellation
can win over a finger-down event received during the cancellable wait, matching
the recovered vendor worker's cancel-before-event-processing order. USB errors
retain their `UsbFailure` type. The helper adds no reset or device access to the
tests, and the committed `wait_int_for` behavior is unchanged.

Seven fixtures pass: normal event bytes, timeout followed by an event, cancel
before reading, cancel during timeout, cancel racing finger-down, successful
event-stream cancellation, and USB device removal. Reproduce with:

```sh
cd src/validity-rs
cargo test --locked usb::tests -- --nocapture
```

The build used the existing lockfile and emitted six existing dead-code warnings
in the host matching code. No lockfile or dependencies were changed. This is a
tested **source change**, not a live-backend deployment or measured hardware
latency improvement. The laptop continues using its selected `libfprint`
backend; `validity-rs` was not started or installed.

The remaining source-review findings still need emulated and hardware evaluation:

| Source behavior | Adversarial fixture | Status / improvement to validate |
|---|---|---|
| Previous `Usb::wait_int` checked cancel only after `Timeout` | Continuous non-finger events with cancel already set | Fixed in source; before/after injected-reader regression demonstrated |
| `Usb::drain_int` runs until a read stops succeeding | Unlimited immediately available stale events | Add a total deadline/event limit and report whether draining completed |
| Committed capture applies 10 seconds to each event | Endless well-formed progress events without the completion bit | Use one phase deadline and pass each read its remaining time |

The latter two are reachable source behaviors under the stated hypothetical
fixtures. They were not observed on this laptop in this continuation, and no
driver change was deployed.
A phase deadline must preserve the existing rule that a committed capture is
completed or recovered coherently; it should not silently turn a finger-present
cancel into a transport reset.

Test acceptance should include cancellation before the first event, cancellation
during an event stream, late events from the preceding capture, missing final
progress, a USB disconnect, and stop-command failure. Record one terminal client
status, ordered cleanup before the next capture, and a finite shutdown time.
Do not infer a complete shutdown bound by adding the nominal per-read timeouts:
retries and repeated progress can extend the operation.

## Getting the best everyday result

Keep one backend selected and retain password/PIN access while testing. Confirm
which reader fprintd selected before measuring cancellation or recognition; the
external CS9711 and internal Synaptics reader use different drivers. Prefer
repeatable VerifyStart/VerifyStop tests to re-enrollment when investigating a
session failure. Preserve the working pairing identity and calibration when
changing firmware/DMI, since the Rust backend explicitly checks pairing against
host identity.

The useful success metrics are normal recognition time, false rejects under
consistent conditions, cancellation latency, resume recovery time, idle CPU,
and USB disconnects. Clocking a no-finger test or reading a decompiled algorithm
does not measure recognition accuracy. Keep raw fingerprint images, templates,
pairing keys, and full USB payload traces private; publish command names,
timestamps, lengths, and lifecycle outcomes when those are sufficient.

The existing `nofinger-battery.sh` is named as a test battery. It is not a
battery-energy tool, and it includes enrollment and service-kill operations.
Use individual tests after checking their actual side effects. No such tests,
backend switches, or fingerprint data reads were performed here.
