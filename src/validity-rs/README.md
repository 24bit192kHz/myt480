# validity-rs

A Rust port of [python-validity](https://github.com/uunicorn/python-validity)
for one sensor: the Synaptics/Validity **06cb:009a** match-on-chip fingerprint
reader in the ThinkPad T480. It serves the same
`io.github.uunicorn.Fprint.Device` D-Bus interface as python-validity's
`dbus-service`, registers with
[open-fprintd](https://github.com/uunicorn/open-fprintd), and so works
unchanged with fprintd clients, PAM (`pam_fprintd`) and lock screens.

It speaks the sensor's protocol natively (USB, the TLS-like session with
ECDH pairing, SPI-flash partitions, firmware-extension upload, calibration,
capture programs, on-chip template database) in a single ~2 MB binary that
links only libusb, with no Python stack.

## How it differs from python-validity

- **T480 only.** Only 06cb:009a (sensor type 0x199) and its blobs are
  supported. python-validity also handles 138a:0090/0097 and friends; this
  port does not.
- **Pairing survives firmware changes.** The pairing blob on the sensor is
  sealed with keys derived from the host's SMBIOS identity (product name +
  serial). python-validity always uses the live DMI values, so switching
  between the stock Lenovo BIOS and coreboot breaks the pairing, and
  recovery means a factory reset that loses every enrolled finger.
  validity-rs tries pinned identities (`hwkey`), then the live DMI identity,
  then fallbacks (`hwkey_fallback`), and uses the first that unlocks the
  pairing. A blank sensor is paired with the first identity, so pinning keeps
  future pairings stable too. See `packaging/validity-rs.conf.example`.
- **Wedged-sensor recovery.** A sensor that stops answering (typically after
  an interrupted session or a bad resume) is recovered without a reboot or
  replug: USB port reset first, then a sensor reboot command plus port reset,
  up to four attempts. A D-Bus call that hits a USB failure reconnects once
  and retries. If the sensor is really gone the daemon exits (after a 5 s
  pause at startup) and the supervisor restarts it.
- **Cancel-safe.** `Cancel`/`VerifyStop` are honoured while the sensor waits
  for a finger; once a finger is down the capture, match or enrollment step
  runs to completion (as in python-validity). After any aborted capture the
  daemon stops the capture program, drains stray interrupt events and lets the
  sensor settle 150 ms before starting the next one. python-validity's cancel
  flag is cleared at every interrupt wait, so a cancel that races a new
  operation is silently lost; here it never is. Sensor access is serialized:
  a call that arrives during a running capture waits briefly, then gets
  `AlreadyInUse`. On exit the sensor is always rebooted, which stops it from
  accumulating TLS sessions and running out of memory. For CLI commands the
  first Ctrl-C cancels the capture, a second one exits.
- **Clients are told when the daemon aborts.** On `Suspend` or shutdown a
  running verify/enroll ends with `verify-disconnected`/`enroll-disconnected`,
  so pam_fprintd returns at once and a locker such as slock-ly re-arms after
  resume, instead of waiting out its PAM timeout (up to 10 min) against a
  capture that no longer exists. open-fprintd itself sends nothing when a
  device daemon goes away, and python-validity sends nothing either. The
  daemon also re-registers by itself whenever open-fprintd restarts.
- **Fast bring-up.** About 0.6 s from start to ready on a paired sensor
  (log line `sensor ready in N ms`), and the same path is used on resume.
- **Plain state handling.** Calibration data lives in `/var/lib/validity-rs`
  and is read and written in place (python-validity's s6 service copied it
  to `/run` and back). On the first start `calib-data.bin` is imported from
  `/var/lib/python-validity` after checking it against the sensor's flash, so
  switching over needs no recalibration and no re-enrollment. Users map to
  the same SIDs as python-validity, so existing fingers keep working.

## Install

Arch/Artix packaging lives in `packaging/`:

- `packaging/PKGBUILD`: builds this checkout (`cargo build --release --locked`)
  and installs `/usr/bin/validity-rs`, the D-Bus policy, an s6-rc service
  (`/etc/s6/sv/validity-rs`, depends on `open-fprintd`) and the docs.
- `packaging/INSTALL.md`: switch-over from python-validity on the T480
  (syswork + s6), verification, rollback.
- `packaging/validity-rs.conf.example`: commented `/etc/validity-rs.conf`.

Never run validity-rs and python-validity at the same time; both drive the
same USB device.

## Configuration

`/etc/validity-rs.conf` (override with `-c`), `key = value` lines, `#`
comments; every key is optional:

| key | meaning |
|---|---|
| `hwkey = NAME SERIAL` | pinned pairing identity, tried first (repeatable) |
| `hwkey_fallback = NAME SERIAL` | tried after the live DMI identity (repeatable) |
| `sid.USER = S-1-5-...` | SID for USER (default `S-1-5-21-111111111-1111111111-1111111111-<uid>`) |
| `data_dir = PATH` | calibration / firmware directory (default `/var/lib/validity-rs`) |
| `retry_delay_ms = N` | pause after a failed identify capture (default 1000) |

## Usage

```
validity-rs [-v|-vv] [-c CONFIG] COMMAND

commands:
  daemon                 serve io.github.uunicorn.Fprint.Device for open-fprintd (default)
  info                   show sensor, firmware, pairing and enrolled fingers
  identify               wait for a finger and match it on the chip
  list USER              list USER's enrolled fingers
  enroll USER FINGER     enroll a finger (e.g. right-index-finger)
  delete USER            delete all of USER's fingers
  calibrate              recalibrate and store a new clean-slate image
  led                    flash the sensor LED
  raw HEX                send a raw command over TLS and print the reply
  factory-reset --yes    wipe pairing, firmware and fingers on the sensor
```

All commands need root (USB access and `/sys/class/dmi/id/product_serial`).
The standalone commands need the sensor to themselves, so stop the daemon
first (`s6 live stop validity-rs`). `-v` logs debug, `-vv` trace with
timestamps. Logs go to stderr; under s6 they land in the catch-all logger
(`/run/uncaught-logs/current`).

`factory-reset` wipes the pairing and every enrolled finger; the next start
pairs the sensor again with the first configured identity and uploads the
firmware extension (`6_07f_lenovo_mis_qm.xpfwext`, obtainable with
python-validity's `validity-sensors-firmware`), searched in `data_dir` and
then `/var/lib/python-validity` and `/var/run/python-validity`.

## Testing

```sh
cargo test
```

Pure-logic tests (TLS PRF, handshake and PSK key derivation, pairing-blob
unlock with the right and a wrong identity, key wrapping, pairing
certificate, bit packing, capture-command building, calibration pipeline,
template-DB reply parsing, SID mapping) compare byte for byte against **golden
vectors** produced by python-validity on a real T480. Generate them with
python-validity installed and its service stopped, as root:

```sh
s6 live stop python3-validity
python3 tests/golden/extract.py tests/golden/golden.json
```

`tests/golden/golden.json` is gitignored because it contains the sensor's
pairing blob (the TLS flash partition) and this machine's calibration data.
Without it the golden tests print `golden.json missing; skipped` and pass.

Hardware tests (run as root against the running daemon; they need a user
with enrolled fingers):

- `tests/cancel-stress.py N HOLD USER` — Claim, VerifyStart, sleep HOLD
  seconds, VerifyStop, Release, ListEnrolledFingers, N times; reports cycles
  with errors. Watch `dmesg` for `usb 1-9: USB disconnect` while it runs.
- `tests/cancel-matrix.sh` — restarts the daemon and runs cancel-stress,
  counting sensor disconnects.
- `tests/suspend-abort.py USER [kill]` — starts a verify, then runs
  open-fprintd's suspend hook (or SIGTERMs the daemon) and checks that the
  client receives `verify-disconnected`.

## Credits

- [python-validity](https://github.com/uunicorn/python-validity) by uunicorn
  and contributors: the protocol reverse engineering, the blobs in `blobs/`,
  and the reference implementation this port follows (and is tested against).
- [open-fprintd](https://github.com/uunicorn/open-fprintd), which this
  daemon plugs into.

## License

MIT, see `LICENSE`. validity-rs is derived from python-validity,
MIT (c) 2020 uunicorn.
