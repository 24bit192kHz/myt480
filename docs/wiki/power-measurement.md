# Measuring power without changing the result

Use [measure-power.py](../../tools/re-audit/measure-power.py) before choosing a
CPU, GPU, display, or peripheral power policy. It reads Linux counters and
status files, prints a concise report or JSON, and never writes a policy or
opens an NVIDIA device. It does not collect hostnames, serials, account names,
network addresses, SSIDs, fingerprint data, or personal files. It has no
implicit log location.

Run it on the T480, preferably as root so the kernel permits access to the RAPL
and PMC counters:

```sh
sudo python3 tools/re-audit/measure-power.py --seconds 30
sudo python3 tools/re-audit/measure-power.py --seconds 60 --interval 1 --json > power.json
python3 tools/re-audit/measure-power.py --from-json power.json
```

The last command only formats the saved capture; it does not sample hardware.
Redirect output to a location you choose. Missing or unreadable counters are
reported as unavailable, rather than as zero power. The utility does not mount
debugfs, relax file permissions, reload a driver, suspend the laptop, or change
the selected power source.

## What the counters mean

| Measurement | What it establishes | What it cannot establish |
|---|---|---|
| RAPL CPU package energy | Energy accumulated by the CPU package over the measured interval | Whole-board power, charger input, or NVIDIA-only power |
| Battery discharge rate | Approximate total draw from present batteries while all external sources explicitly report offline and battery states agree | AC system power, charger efficiency, or exact energy consumption from a short fuel-gauge sample |
| PMC package C-state deltas | Time reported in individual package sleep states during the interval | Why a deeper state was blocked, or suspend reliability |
| NVIDIA `power_state` / `runtime_status` | Cached PCI/runtime-PM state without an NVML query | GPU utilization, clock stability, or watts saved by D3cold |
| Brightness, governor/EPP, source/status changes | Conditions needed to compare runs | A controlled workload or a guarantee that unobserved applications stayed idle |

The [Linux powercap interface](https://www.kernel.org/doc/html/latest/power/powercap/powercap.html)
exposes energy counters and their ranges. The sampler identifies CPU package
zones by their names, excludes child domains, and avoids counting a duplicate
MMIO backend as another package. It computes joules and watts from monotonic
intervals, handling one counter wrap with `max_energy_range_uj`. Missing
counters, a changed range, and a decreasing counter without a range invalidate
that interval. If any interval is invalid, it withholds the complete-window
package average; JSON still shows valid coverage and the reason.

Counter sampling cannot distinguish a reset from a wrap when both fit the
reported range, or detect multiple wraps between reads. The default one-second
period keeps reads close together. No counter is reset by this tool. Individual
files are read sequentially, so the JSON includes each snapshot's read span
and uses its monotonic midpoint for timing. This is an observation tool rather
than a laboratory power meter.

Not every sysfs read is passive. The
[Linux PCI sysfs implementation](https://github.com/torvalds/linux/blob/master/drivers/pci/pci-sysfs.c)
returns `power_state`, vendor, and class from cached device fields, but reading
`current_link_speed` or `current_link_width` takes a runtime-PM reference to
access PCI configuration. The sampler uses the cached state and runtime-PM
status instead of polling those link attributes or `/dev/nvidia*`.

Battery units and operating statuses follow the
[Linux power-supply interface](https://www.kernel.org/doc/html/latest/power/power_supply_class.html).
On the T480, one pack can discharge while the other reports `Not charging` or
`Full`. The inactive pack contributes zero, even if its cached `power_now` is
nonzero. A `Charging`, unknown, or inconsistent pack prevents a system-discharge
estimate. If `power_now` is absent, a discharging pack can use explicitly labelled
current-times-voltage data. A connected AC/USB source always suppresses the
discharge result, including when charge thresholds keep both packs idle. Unknown
external-source status is not treated as proof that AC is absent.

The [Intel PMC driver's implementation](https://github.com/torvalds/linux/blob/master/drivers/platform/x86/intel/pmc/core.c)
converts package residency counters to microseconds for `package_cstate_show`.
The tool compares the endpoint values; unavailable/decreasing values remain
unavailable. Package C8 and S0ix are separate observations. A zero S0ix delta in
an awake, legacy/deep-sleep configuration does not by itself indicate a defect.

## Actual observation on 2026-10-09

A 20-second capture completed at approximately **13:30 UTC**, using 21 snapshots
through SSH. This was the normal desktop with background applications and the
sampler running. Deliberate renderer/storage checks were held during capture,
but this was not an isolated idle benchmark.

| Condition/result | Observed value |
|---|---|
| Measured interval | 19.995 s; maximum snapshot read span 15.1 ms |
| CPU package | 63.678 J; average **3.185 W**; no wrap or missing interval |
| External power | AC connected throughout |
| Batteries | BAT0 `Not charging` at 87%; BAT1 `Full` at 100% |
| System discharge | **Unavailable**, as required on AC |
| NVIDIA | `0000:01:00.0`, `D0` / `active` throughout |
| Display | `acpi_video0`, actual brightness 38/100 |
| CPU policy | `intel_pstate`, governor/EPP `performance` / `performance` |
| Package residency | C2 24.8%, C3 53.0%; C6–C10 zero deltas |
| S0ix residency | Zero delta |
| Source/status/GPU/brightness/CPU-policy transitions | None observed |

The D0 GPU and lack of deeper package residency make an Intel-only desktop a
useful comparison experiment. This single observation cannot assign causality
to NVIDIA or quantify its board-level cost. Do not advertise 3.185 W as T480
idle power: it is a CPU-package reading on AC.

Raw identity-free JSON is retained privately at
`/home/btw/test/rea/work/audit-20261009-power/observational-ac-20s.json`, with
`observational-ac-20s.txt`, `capture-stderr.txt`, and a capture manifest beside
it. No policies or services were modified to collect it.

## Getting useful comparisons

1. Keep brightness, connected displays/docks, radios, workload, and power source
   the same. Give the desktop and fuel gauge time to settle. Record any user
   activity; sampling itself has a small cost.
2. Compare Intel-routed ordinary applications against the existing desktop
   before attempting more clock/voltage changes. Use [PRIME](nvidia.md) for
   work that benefits from the MX150, and read its idle state through sysfs.
   `nvidia-smi`, NVML clock queries, and GPU renderer probes can wake it during
   the measurement.
3. For battery tests, capture only after all external sources are disconnected
   and a pack reports `Discharging`. Use longer, repeatable intervals and repeat
   them. Do not infer runtime from a charging reading or from the CPU package.
4. For CPU throughput, measure the same completed work and its completion time
   as well as joules. Lower instantaneous watts can mean a slower job that uses
   more energy; high AC limits do not automatically improve sustained speed.
5. Change one policy per comparison and keep its original value for restoration.
   The existing [thermal policy](thermal.md) and [rollback instructions](rollback.md)
   remain the basis for hardware changes. A source/GPU/brightness/CPU transition
   in the JSON makes that interval a poor direct comparison to a steady run.

## Earlier helper and validation

`firmware/tools/idlepower.py` remains an older, separate experiment helper. It
subtracts a single pair of RAPL values without wrap handling, times the interval
with the wall clock, hard-codes several device paths, and prints battery-rate
statistics while AC can be connected. Those battery fields must not be read as
system discharge. It also appends to a fixed personal log path. The new sampler
does not alter the older helper and avoids these measurement pitfalls.

Run its hardware-independent tests with:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s tools/re-audit/tests -p 'test_measure_power.py' -v
```

The 21 tests cover energy wrap/range changes/missing intervals, monotonic
deadlines, partial coverage, missing paths, flat/nested/class-symlink discovery,
duplicate package backends, battery validity and dual-pack switching rules,
AC/USB transitions, GPU-state changes, and PMC units. They do not prove battery
fuel-gauge accuracy, deep-sleep behavior, or savings from any particular policy.
