# Thermal control

## Why this mattered most

The old controller wrote a manual fan level and then `watchdog 0`. A hung or dead
userspace controller could leave that level in place indefinitely. It also moved
only one fan bracket per sample, even after a large temperature jump, and did not
restore EC automatic control when a CPU sensor read failed.

The revised controller arms `watchdog 120` before every manual level write and
renews the setting every 30 seconds using a monotonic clock. Poll intervals are
capped at 30 seconds. The kernel provides the fallback independently of the
daemon; this matches the documented [thinkpad_acpi fan watchdog](https://www.kernel.org/doc/html/latest/admin-guide/laptops/thinkpad-acpi.html).

## Changes

- Sensor disappearance, invalid readings, and graceful exit return the fan to
  `level auto`. The watchdog is disabled only after that write succeeds.
- A failed or short fan write is detected; the controller attempts EC fallback
  and retries instead of recording a setting that was never applied.
- Large temperature changes cross all relevant fan brackets in one sample.
  Normal hysteresis remains.
- Invalid individual fan levels/ranges are rejected. A wholly invalid curve
  retains the previous/default curve.
- Missing configuration defaults to zero undervolt. Per-machine explicit values
  in `/etc/thermald.conf` continue to apply.
- `--test` performs no fan writes. `--once` leaves the manual setting protected
  by the watchdog, which expires after the process exits.

Sources: [`thermald.c`](../../src/thermald-t480/thermald.c),
[`fan-control.c`](../../src/thermald-t480/tests/fan-control.c).

## Validation

The emulated fan-file test exercises watchdog ordering/refresh, sudden heating,
hysteresis, cooling, release, failed/short writes, retry, dry-run behavior, invalid
curves, and zero undervolt defaults:

```sh
cc -O2 -Wall -Wextra -Wno-missing-field-initializers \
  src/thermald-t480/tests/fan-control.c -o /tmp/test-fan
/tmp/test-fan
```

On the laptop, the replacement was built with the project's musl static-build
flags and dry-run checked against the unchanged configuration. Stopping the s6
service returned the fan to `auto`; restarting restored manual curve control.

For the failure test, PID 29916 was sent `SIGSTOP` for 125 seconds under a cleanup
trap that always sent `SIGCONT`. Samples were taken every 25 seconds. The highest
`temp1_input` sampled was 49°C. The fan changed from level 4 to `auto` by sample
five while the process was still stopped. After `SIGCONT`, it returned to level 4.
This directly verifies the kernel fallback on this T480. The test does not
simulate a kernel lockup; the separate TCO watchdog handles that class of failure.

The live thermal configuration checksum remained
`960b36ec57354117c2449e67425420976714ac13595b25005bf9f1f326c0205a`.
The deployed binary checksum is
`c64159a5f21100e3f55fd8ca8c228c5aaee8d15bc615c76f451b52da0f463f61`.

See [rollback](rollback.md) to restore the original program. Returning to the old
program also restores its old watchdog behavior.
