# Thermal control, power measurement, and daily plans

This summary covers the thermal/power work in audit commits `2cb13a3` and
`fe58a80`, plus the remaining failure paths recorded in `7f5a430`. The detailed
[thermal wiki](../../docs/wiki/thermal.md),
[measurement wiki](../../docs/wiki/power-measurement.md), and
[daily-use guide](../../docs/wiki/getting-the-most.md) are the operational
references. The work applies to the inspected i7-8650U T480 and its existing
configuration; it does not establish tuning values for other machines.

## Inspection and deployed change

Review of the shipped controller, its source, the live fan interface, and the
AC/battery configuration found a concrete recovery problem: manual fan control
disabled the kernel watchdog. A stopped daemon could therefore leave its last
fan level indefinitely. The controller also crossed only one fan bracket per
sample and did not relinquish manual control when CPU temperature reads failed.

The revised [`thermald.c`](../../src/thermald-t480/thermald.c) was built and
deployed on the laptop. It now:

- Arms `watchdog 120` before manual level writes and renews it every 30 seconds
  with a monotonic clock; the poll interval cannot exceed the renewal period.
- Returns its fan control to EC automatic mode on invalid/missing temperature
  data and graceful exit. The watchdog is disabled only after returning to
  automatic mode succeeds.
- Detects failed and short writes, attempts fallback, and retries rather than
  recording an unapplied fan setting.
- Crosses every applicable fan bracket after a sudden temperature change while
  retaining ordinary hysteresis.
- Rejects invalid fan levels/ranges and retains the previous/default curve when every
  supplied entry is invalid.
- Defaults to zero undervolt when configuration is missing. The explicit
  machine-specific settings were retained.

This is a verified recovery improvement, not a new cooling-capacity or
sustained-performance claim. The live configuration remained unchanged.

## Validation and recovery

The [emulated fan tests](../../src/thermald-t480/tests/fan-control.c) passed
watchdog ordering/refresh, heating/cooling/hysteresis, invalid curves, release,
failed/short writes, retries, dry-run behavior, and conservative defaults.
The replacement was dry-run checked against the existing config; stopping the
s6 service returned the fan to `auto`, and restarting restored curve control.

The live failure test paused the daemon for **125 seconds**, taking observations
every 25 seconds. The fan returned from level 4 to `auto` while the daemon was
still stopped and returned to level 4 after it resumed. The highest sampled CPU
temperature was **49°C**. This directly verifies the kernel fallback for a
userspace pause; it does not simulate a kernel lockup. The existing TCO and
soft/NMI recovery mechanisms were retained.

The deployed binary SHA256 is
`c64159a5f21100e3f55fd8ca8c228c5aaee8d15bc615c76f451b52da0f463f61`.
Original programs/configuration and the checksum-checked
[rollback script](../../tools/re-audit/rollback.sh) were preserved. A full
restoration roundtrip was tested before reapplying the improvements. Follow
the [rollback instructions](../../docs/wiki/rollback.md); restoring the old
controller also restores its old watchdog behavior.

## Power measurements that preserve their meaning

The new repository utility
[`measure-power.py`](../../tools/re-audit/measure-power.py) reads status and
energy counters without changing policies or opening NVIDIA devices. It uses
monotonic intervals and `max_energy_range_uj` to handle a RAPL wrap, avoids
double-counting package backends/subdomains, and marks missing/changing counters
as incomplete instead of inventing zero power. It records package residency,
GPU sysfs state, display brightness, CPU policy, and source/status transitions.

Battery discharge is reported only when external supplies explicitly report
offline and present-battery states are consistent. An inactive `Full` or
`Not charging` pack contributes zero, even if its cached rate is stale. AC,
charging, unknown source/status, or missing active-pack readings suppress the
whole-system result. CPU-package energy remains a separate measurement.
The [21 fixture tests](../../tools/re-audit/tests/test_measure_power.py) cover
wraps, missing/range-changing counters, monotonic timing, discovery, dual-pack
validity, transitions, and PMC units.

The actual 20-second AC observation used 21 snapshots over **19.995 seconds**:

| Observation | Result |
|---|---|
| CPU package energy / average | 63.678 J / **3.185 W** |
| Batteries | BAT0 `Not charging`; BAT1 `Full`; AC connected |
| Whole-system discharge | Unavailable |
| NVIDIA | D0 / active throughout |
| Display and CPU policy | Brightness 38/100; performance governor/EPP |
| Package residency | C2 24.8%, C3 53.0%; deeper-state deltas zero |

This was ordinary desktop/SSH activity, with deliberate renderer/storage tests
held during capture. It was not an isolated idle benchmark. **3.185 W is not
total T480 power**, and the observation does not quantify GPU savings or battery
runtime. Counter resets can resemble wraps, sequential reads are not atomic,
and fuel-gauge rates need longer repeatable measurements.

## Daily plans and retained stability margins

The source and matching live/repository
[`thermald.conf`](../../system/etc/thermald.conf) and
[`tlp.conf`](../../system/etc/tlp.conf) establish these choices:

| Plan | CPU PL1/PL2 | Behavior |
|---|---|---|
| `auto`, AC | 64/90 W | TLP performance policy; Intel GPU maximum 1150 MHz |
| `auto`, battery | 15/25 W | TLP powersave/balance_power policy; turbo allowed; Intel maximum 900 MHz |
| `balanced` | 25/44 W on either source | Turbo allowed; Intel maximum 1150 MHz |
| `powersave` | 10/20 W on either source | Turbo disabled; Intel maximum 700 MHz |

Explicit plans persist across unplugging and reboot; `balanced` raises battery
limits above `auto`. `performance` retains 64/90 W on either source. Returning
to `auto` restores source-dependent thermal selection and invokes TLP, although
TLP's own smart/manual profile behavior remains separate. These are configured
allowances, not guaranteed sustained clocks or temperatures.

Existing AC core/cache undervolt remains −115 mV; battery remains −100 mV.
Earlier sweeps found an AC error at −124 mV and a battery freeze at −121 mV.
MX150 AC offsets remain +200/+1250 MHz, with zero battery offsets: earlier
correctness tests found silent CUDA errors at +250 MHz core. This audit did not
raise those values. See the [retained configs](../../system/etc/gpu-power.conf)
and [NVIDIA results](../../docs/wiki/nvidia.md).

Open work is repeatable battery/source-transition testing, sustained mixed
CPU/GPU thermals, and the cross-stack review's still-unfixed AC-detection
failure fallback and numeric configuration bounds. Those limits are documented
in the [cross-stack review](../../docs/wiki/cross-stack-review.md). Measure one
change at a time and retain the original policy; keep ordinary applications on
[Intel](../../docs/wiki/gpu-routing.md) and use PRIME deliberately.
