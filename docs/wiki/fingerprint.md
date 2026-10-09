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

| RVA / address | Finding |
|---|---|
| `0x180017e00` | `CBiometricDevice::OnCancel`: serializes access, checks the current request, initiates abort, completes cancellation, clears request ownership |
| `0x18001d338` | Abort helper locks capture state and delegates to `0x180012534` |
| `0x180012534` | Marks pending worker cancellation, stores its mode, calls `SetEvent`; this is not a decoded USB abort packet |
| `0x18001d388` | Waits for the capture thread and closes its handle |
| `0x180014b78` | Idle/resume helper can abort, wait, then initialize/calibrate |
| `0x180027be0` | `DeviceReset` delegates through `0x18002efd0` to `0x180057460` |
| `0x180057460` | Queues an internal operation/callback; the final USB reset command was not recovered |
| `0x18002d178` | Suspend/capture enable state and virtual callback |

The observed design separates worker cancellation, completion, reset, and
recalibration. The Windows wait uses an infinite timeout; copying that into the
Linux daemon would risk blocking suspend or service shutdown indefinitely.

## How this informs Linux work

The existing Rust backend already has a cancellation flag, daemon-abort status,
device ownership mutex, reopen-on-USB-failure, and a settling period after aborted
captures. The Windows findings help focus investigation on ownership and
completion ordering; they do not justify replacing these with unconditional USB
resets. No backend change was made without a reproduced failure.

The next useful experiment is a trace of VerifyStart/VerifyStop, repeated cancel,
finger-present cancel, and suspend during capture with **one** selected backend.
Use the existing tests under `src/validity-rs/tests/`, record USB request/response
ordering and bounded completion time, and keep image/template payloads private.
Compare the trace against the worker lifecycle above before changing transport
commands. Actual recognition accuracy still requires the user's finger and a
controlled enrollment/evaluation procedure.
