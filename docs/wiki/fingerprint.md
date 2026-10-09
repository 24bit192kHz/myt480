# Fingerprint drivers

## Live device and backend selection

Two readers are attached: internal Synaptics `06cb:009a` at USB `1-9`, and
Chipsailing CS9711 `2541:0236` at `1-2`. The selected backend is `libfprint`, using
`libfprint-cs9711 1.94.10.r28.g02b285c-2` and `fprintd 1.94.5-2`.

The `validity-rs`, `python3-validity`, and `open-fprintd` services are deliberately
down while this backend is selected. `/usr/local/bin/fprint-backend` manages the
alternatives. Starting all services would introduce competing ownership of the
same D-Bus interface/device. The selected backend was retained.

The sleep hook `05-fprintd` stops/restarts the validity stack only when that
stack is selected. Logs show internal-reader reconnect and later reader resets
around resume, but this alone does not identify a defect. No enrollment,
biometric extraction, or fingerprint database reset was performed.

## Windows driver analysis

The Lenovo archive `nz3gf07w.exe` was downloaded and its SHA512 checked against
the installed [python-validity firmware manifest](https://github.com/uunicorn/python-validity/blob/master/validitysensor/firmware_tables.py).
It was unpacked with `innoextract`, without running the installer. The analyzed
`synaWudfBioUsb.dll` is x86-64 PE, release `5.3.3542.26` with 2020 build paths.
It is a historical protocol reference, not a claim about Lenovo's latest driver.

REA/Ghidra recovered 3,037 procedures and 4,987 strings. String xrefs anchored
the following paths:

| Virtual address | Finding |
|---|---|
| `0x180017e00` | `CBiometricDevice::OnCancel`: serializes access, checks the current request, initiates abort, completes cancellation, clears request ownership |
| `0x18001d338` | Abort helper locks capture state and delegates to `0x180012534` |
| `0x180012534` | Marks pending worker cancellation, stores its mode, calls `SetEvent`; this is not a decoded USB abort packet |
| `0x18001d388` | Waits for the capture thread and closes its handle |
| `0x180014b78` | Idle/resume helper can abort, wait, then initialize/calibrate |
| `0x180027be0` | `DeviceReset` delegates through `0x18002efd0` to `0x180057460` |
| `0x180057460` | Dispatches operation 2; `0x180083410` constructs `05 02 00` for the `DeviceReset` call |
| `0x180065890` | Reset completion callback stores status and signals an event; it does not construct the command |
| `0x180083350` | Operation 10 constructs the one-byte capture-stop command `04` |
| `0x18002d178` | Suspend/capture enable state and virtual callback |

The observed design separates worker cancellation, completion, reset, and
recalibration. The Windows wait uses an infinite timeout; copying that into the
Linux daemon would risk blocking suspend or service shutdown indefinitely.

The continuation recovered the command path through the PAL dispatch and
WinUSB exchange, including optional secure-session wrapping. The recovered
reset and stop payloads agree with the corresponding commands already present
in the Rust backend. [Protocol and lifecycle details](fingerprint-protocol.md)
record the addresses, source comparisons, and remaining unknowns. These are
static findings; this continuation did not send USB commands to either reader.

## How this informs Linux work

The existing Rust backend already has a cancellation flag, daemon-abort status,
device ownership mutex, reopen-on-USB-failure, and a settling period after aborted
captures. The Windows findings help focus investigation on ownership and
completion ordering; they do not justify replacing these with unconditional USB
resets. No backend change was made without a reproduced failure.

Hardware-free injected-reader tests reproduced a Rust cancellation defect:
`wait_int` checked cancel only after a read timed out, so a continuous stream
of successful events could ignore cancellation. The source now checks cancel
before reading and before accepting a successful event. Seven USB unit tests
pass, including cancellation racing a finger-down event. The existing committed
wait, capture-stop, reset, and recovery policy were retained. This source change
was not deployed; the active `libfprint` backend remains unchanged.

Stale-event draining still has no total bound, and capture progress can restart
the committed-read timeout repeatedly. Those remain investigation candidates;
they were not reproduced on hardware or changed in this continuation.

The next useful hardware experiment is a trace of VerifyStart/VerifyStop,
repeated cancel, finger-present cancel, and suspend during capture with **one**
selected backend. Record completion time and command/event ordering; keep
image/template payloads private. `tests/cancel-stress.py` exercises the selected
fprintd device, so verify which reader it chose before attributing a result to
Synaptics. `tests/nofinger-battery.sh` is a hardware test suite, not an energy
measurement: it also attempts enrollment, kills/restarts daemons, and reads
enrollment counts. It was not run in this continuation.

For everyday use, retain the working backend, avoid competing sensor owners,
and use normal cancellation before restarting a stuck service. Do not use
factory reset as a routine recovery action: it discards pairing and enrolled
fingers. Recognition improvements need a controlled enrollment/evaluation
procedure with the user's finger; binary analysis does not establish accuracy.
