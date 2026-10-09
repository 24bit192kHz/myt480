# Getting the most out of this T480

Keep ordinary applications on Intel, use the MX150 for work that benefits from
it, and choose a CPU plan for the task rather than raising voltage/clock limits.
Start with `auto`: it follows the existing AC/battery profiles. The live plan
was `auto` when checked on 2026-10-09. Live `/etc/thermald.conf` and `/etc/tlp.conf`
matched their repository copies; this guide describes those settings.

## Choose the CPU plan deliberately

The [`thermald` source](../../src/thermald-t480/thermald.c) implements the plans,
while [`thermald.conf`](../../system/etc/thermald.conf) supplies the machine's
AC/battery limits. In `auto`, [`tlp.conf`](../../system/etc/tlp.conf) owns the CPU
governor/EPP, turbo allowance, and Intel GPU limits.

| Plan | CPU PL1 / PL2 | Thermal trip | Governor / EPP | Turbo | Intel GPU maximum |
|---|---|---|---|---|---|
| `auto`, AC profile | 64 / 90 W | 95°C | performance / performance | Allowed | 1150 MHz |
| `auto`, battery profile | 15 / 25 W | 85°C | powersave / balance_power | Allowed | 900 MHz |
| `balanced`, either source | 25 / 44 W | 90°C | powersave / balance_performance | Allowed | 1150 MHz |
| `powersave`, either source | 10 / 20 W | 80°C | powersave / power | Disabled | 700 MHz |
| `performance`, either source | 64 / 90 W | 95°C | performance / performance | Allowed | 1150 MHz |

These are configured limits and allowances, not promised sustained watts,
temperature, or clock speed. The T480's cooling and shared CPU/iGPU power budget
still constrain a long job. The Intel `powersave` governor used here can scale
up under load; it is not a fixed minimum-frequency mode. See the
[Intel driver documentation](https://www.kernel.org/doc/html/latest/admin-guide/pm/intel_pstate.html).

Use `balanced` for an AC comparison when `auto` produces more heat/noise than
the task needs. Try `powersave` for light mobile work when reduced responsiveness
is acceptable, then measure it. **An explicit plan persists across unplugging
and reboot.** In particular, `balanced` raises the battery CPU limits above
`auto`'s 15/25 W, and `performance` retains 64/90 W on battery. Return to `auto`
after an experiment rather than assuming unplugging cancels it.

On the T480, inspect the current word and choose one of the following commands:

```sh
cat /var/lib/thermald-t480/plan
# Select a plan; applied at the daemon's next poll:
printf 'balanced\n' | sudo tee /var/lib/thermald-t480/plan >/dev/null
# Or use the light-work plan:
printf 'powersave\n' | sudo tee /var/lib/thermald-t480/plan >/dev/null
# Restore source-dependent operation:
printf 'auto\n' | sudo tee /var/lib/thermald-t480/plan >/dev/null
```

Explicit plans override PL1/PL2/trip and CPU/iGPU knobs; undervolt, current
limits, time windows, and fan control remain source-dependent. The daemon
reasserts explicit knobs if TLP changes them. Returning to `auto` invokes
`tlp start` once. Thermald's `balanced` is separate from TLP's profile of the
same name: it does not select a new peripheral profile. TLP 1.10's default
[smart switching](https://linrunner.de/tlp/settings/operation.html) can preserve
a manually selected TLP profile. If you have used TLP's manual profile commands,
inspect `sudo tlp-stat -s -c -p` and the sampler's actual governor/EPP rather
than assuming `auto` reset every setting.

The same TLP configuration already requests PCI runtime PM on both sources,
Wi-Fi power saving on battery, and a one-second HDA audio idle timeout on
battery. It keeps the existing PCIe workaround and watchdog policy. Both packs
have configured 85% start / 90% stop charge thresholds; check `sudo tlp-stat -b`
for their applied state. A configured threshold is not proof that a currently
full pack has already discharged to it. Avoid applying a blanket tuning tool
over these settings before comparing a specific change.

## Put applications on the appropriate GPU

Use the audited [`igpu-run`](../../src/gpu-power/igpu-run) for ordinary programs
that discover NVIDIA unnecessarily, and [`prime-run`](../../src/gpu-power/prime-run)
for deliberate rendering or CUDA work:

```sh
igpu-run bitwarden
igpu-run your-desktop-application
prime-run your-gpu-application
gpu-power status
```

The Bitwarden desktop override uses Intel. The separate evening deployment also
corrected `bw-screen boot`, which actually starts it from the session and after
hibernate; a desktop override alone had missed that route. A second single-instance
launch does not replace the first instance's driver choices. The
[GPU-routing page](gpu-routing.md) records the checks and deployment/rollback
boundaries; the evening notes report D3cold with Bitwarden open.

With no GPU clients, audited coarse RTD3 can let loaded hardware enter D3cold
on AC. On battery, automatic GPU policy unloads the driver when no wrapper job
needs it. `prime-run` holds the device while its job runs and restores the
existing offsets after a cold wake. Another application's open GPU handles
can keep it in D0. Inspect sysfs through `gpu-power status`; clock/utilization
queries through NVML or `nvidia-smi` can wake the GPU during an idle measurement.
See [NVIDIA results and remaining limits](nvidia.md), including the uncompleted
real suspend/hibernate tests.

## Keep the tested margin

Retain the configured undervolt: AC core/cache −115 mV, iGPU −80 mV, uncore
−70 mV; battery core/cache −100 mV, iGPU −60 mV, uncore −50 mV. These are values
for this CPU, with margins recorded in
[`thermald.conf`](../../system/etc/thermald.conf), not settings to copy onto
another T480. The battery sweep froze at −121 mV; the AC sweep first reported
an error at core −124 mV. Further undervolting is not a daily-use improvement
without fresh error-checked validation.

Keep MX150 AC core/memory offsets at +200/+1250 MHz and battery offsets at zero,
as configured in [`gpu-power.conf`](../../system/etc/gpu-power.conf). The earlier
tests found silent CUDA errors at +250 MHz core. A rendered image or a benchmark
score alone would miss those failures. Higher offsets are not recommended here.

Those MX150 settings are frequency tuning, not a demonstrated GPU undervolt.
The [voltage research](../../research/2026-10-09-t480/12-mx150/01-voltage-and-controls.md)
traces the PWM regulator and signed overvoltage frontend, then proves that the
exact 580 kernel handler clamps negative requests to zero and reports minimum 0.
That control cannot undervolt the MX150. Alternative V/F-policy control,
calibration and usable voltage telemetry remain unresolved. Approved getters
confirmed frequency ranges, not safe settings or a voltage control. The
[MX150 use guide](../../research/2026-10-09-t480/12-mx150/05-use-and-validation.md)
also covers Pascal-compatible CUDA builds, media limits, sleep and measurement.

## Measure the result on battery

Keep brightness, displays/docks, radios, workload, and application routing the
same between runs. Let background work and the fuel gauge settle. Disconnect
external power, check that the sampler reports a present discharging pack, and
record a repeatable workload or several minutes of comparable quiet activity:

```sh
sudo python3 tools/re-audit/measure-power.py --seconds 180 --json > battery-auto.json
python3 tools/re-audit/measure-power.py --from-json battery-auto.json
```

Repeat after changing one plan or application route, then restore the original
choice. Compare completion time and CPU joules for a fixed job as well as the
valid battery-discharge rate. A lower instantaneous rate can accompany a slower
job that consumes more energy. Discard runs with unexpected source, brightness,
GPU, or policy transitions.

The recorded AC observation of **3.185 W** is CPU-package power, not total T480
power or a runtime prediction. The sampler suppresses whole-system discharge
when AC is present or battery/source states are uncertain. The
[measurement page](power-measurement.md) explains counter wrap, dual-battery
handling, snapshot timing, and the observational nature of the baseline.

## Recover before experimenting further

Return the plan to `auto` first if a manual plan causes an unwanted tradeoff.
Keep the audited fan watchdog and the existing TCO recovery service running;
disable neither to chase a small idle-power reading. Use the
[read-only collector](../../tools/re-audit/collect.sh) to record policy state
before a change.

For program/config restoration, follow [the verified rollback procedure](rollback.md).
Close GPU applications before a driver reload. Intel application routing has
its [own checksum-checked rollback](gpu-routing.md#revert). USB-C reconstruction,
fingerprint protocol work, and firmware changes remain separate experiments;
daily performance tuning does not require flashing an experimental image.
