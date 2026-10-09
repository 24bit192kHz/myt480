# Fingerprint lifecycle and cancellation

The useful result was a reproduced cancellation defect and a narrow Rust source
fix. The laptop's working `libfprint` backend was retained. Reverse engineering
clarified reset, capture-stop and worker completion; it did not establish better
recognition accuracy or justify routine sensor resets.

This summary follows audit commits `2cb13a3` and `fe58a80`. Detailed evidence is in
the [fingerprint overview](../../docs/wiki/fingerprint.md) and
[protocol reconstruction](../../docs/wiki/fingerprint-protocol.md).

## Selected hardware and backend

Two readers were attached: internal Synaptics `06cb:009a` at USB `1-9`, and
external Chipsailing CS9711 `2541:0236` at `1-2`. The selected backend used
`libfprint-cs9711 1.94.10.r28.g02b285c-2` and `fprintd 1.94.5-2`.
The alternative `validity-rs`, `python3-validity` and `open-fprintd` services were
intentionally down. Running competing services would introduce device/D-Bus
ownership conflicts. The backend selector and `05-fprintd` sleep hook were
inspected without switching the selected stack.

No enrollment, fingerprint database/template access, factory reset or live USB
protocol experiment was performed in this investigation.

## What was recovered

The Lenovo `nz3gf07w.exe` archive was checksum-verified against the existing
python-validity manifest and unpacked with `innoextract`, without running its
installer. The historical Windows driver `synaWudfBioUsb.dll`, release
`5.3.3542.26`, is a 2,160,248-byte x86-64 PE with SHA256:

```text
aed9f4246bbaca28776c05c169c734cae9763322b03dc00e4319320e1aa2294d
```

REA 6.1.0/Ghidra 12.1.4 recovered 3,037 procedures and 4,987 strings. Fourteen
successful continuation result bundles traced selected dispatch, transport and
worker paths. Counts describe analyzed material, not complete protocol coverage.

| Recovered path | Finding and practical meaning |
|---|---|
| `OnCancel` at `0x180017e00`, worker flag/event at `0x180012534` | Cancellation serializes ownership and wakes the worker; setting the flag is not itself a USB abort packet. |
| Finger wait at `0x1800125dc` | Checks cancellation before processing finger events, returns cancelled status `0x67`, and removes event registration. |
| Reset `0x180027be0` → operation 2 → builder `0x180083410` | Constructs `05 02 00` for this caller, matching the Rust recovery command. Callback `0x180065890` signals completion. |
| Operation 10 → builder `0x180083350` | Constructs capture-stop byte `04`, matching Rust capture cleanup. This does not prove every cancellation branch sends it. |
| Exchange `0x18007f730` → `0x180106c00` | Optional secure wrapping precedes WinUSB write/read. Pipe indices are descriptor indices, not endpoint addresses. |

The payloads are application commands **before optional secure wrapping**; they
need not appear literally in a USB capture. Addresses belong to this exact DLL
and PE image base `0x180000000`. Assembly corroborated the reset construction.
Endpoint reset, application reset, host device restart and destructive factory
reset have different scopes. The recovered Windows infinite wait should not be
copied into a Linux shutdown/suspend path.

## Source fix and validation

Previously, [`Usb::wait_int`](../../src/validity-rs/src/usb.rs) checked the cancel
flag only after `Timeout`. An interrupt stream returning successful events could
therefore starve cancellation. An injected-reader fixture reproduced the old
policy: **four tests passed and two failed**, covering a pre-existing cancel
accepting an event and cancellation ignored during a successful-event stream.

The helper now checks cancel before each read and before accepting a successful
event. A timeout retries through the same check. The real reader retains its
existing 50 ms interrupt poll; USB errors retain `UsbFailure`. Committed
`wait_int_for`, stale-event draining, capture-stop, reset and recovery semantics
were left unchanged.

**Seven USB fixtures passed:** normal event, timeout then event, pre-cancel with
no read, timeout cancellation, cancel racing finger-down, successful-stream
cancellation, and USB device removal. The later complete Rust run reported
**44 passing tests**, including these seven. Optional golden data was absent, so
conditional golden-data paths provide no fresh cross-language validation.
The lockfile/dependencies were unchanged; six existing dead-code warnings remained.

```sh
cd src/validity-rs
cargo test --locked usb::tests -- --nocapture
```

This is a tested **source change**, not a deployed fingerprint-driver change.
`validity-rs` was not installed or started; live `libfprint` remained selected.
Rollback is restoring the preceding Rust source revision, as recorded in the
[rollback guide](../../docs/wiki/rollback.md#repository-and-evidence).

## Remaining work and everyday use

Unbounded stale-event draining and capture progress that repeatedly restarts a
per-read timeout remain source-review candidates. Reproduce those with injected
streams before adding total phase deadlines; preserve coherent committed-capture
completion and cleanup. Hardware validation should then use one selected backend
and known reader, measuring VerifyStart/VerifyStop, repeated cancel, finger-present
cancel, disconnect and suspend-during-capture outcomes.

Keep the working backend, normal password/PIN access and existing pairing state.
Prefer ordinary cancellation to disruptive resets. Recognition quality needs
controlled user testing; decompiled algorithms and no-finger timing cannot prove
it. `tests/nofinger-battery.sh` is a test suite with enrollment/service side effects,
not an energy benchmark, and was not run here.

The DLL, complete proprietary pseudocode and analysis outputs remain private.
Do not publish raw images, templates, pairing keys or full biometric payload
traces. Publish lifecycle timing, status and command metadata when sufficient;
see the [evidence boundary](08-tools-evidence-and-validation.md).
