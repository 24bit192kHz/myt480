#!/usr/bin/env python3
"""Read-only, identity-free power observations from Linux sysfs/debugfs.

No NVML, PCI config-space, EC, policy writes, or implicit log files are used.
CPU package RAPL energy and battery discharge are deliberately separate.
"""

import argparse
import datetime
import json
import math
from pathlib import Path
import re
import statistics
import time


def read_text(path):
    try:
        return path.read_text().strip()
    except (OSError, UnicodeError):
        return None


def read_int(path):
    value = read_text(path)
    try:
        return int(value) if value is not None else None
    except ValueError:
        return None


def counter_delta(before, after, maximum=None):
    """Return (delta, reason), allowing one wrap of a documented energy counter.

    Missing/negative counters and out-of-range values never become zero energy.
    Without a range, a decreasing counter could be a reset; reject that interval.
    Multiple wraps between reads cannot be detected by this interface.
    """
    if before is None or after is None:
        return None, "counter_missing"
    if before < 0 or after < 0:
        return None, "counter_negative"
    if maximum is not None:
        if maximum <= 0 or before > maximum or after > maximum:
            return None, "counter_range_invalid"
    if after >= before:
        return after - before, None
    if maximum is None:
        return None, "counter_decreased_without_range"
    return maximum - before + after, "wrapped"


def discover_packages(root):
    """Find package zones in both Linux powercap layouts; exclude subdomains.

    Class symlinks and nested entries can expose the same zone more than once.
    Prefer intel-rapl over intel-rapl-mmio for a duplicate package name, so an
    alternate backend does not double-count the same physical CPU package.
    """
    candidates = []
    for relative in ("sys/class/powercap", "sys/class/power_cap"):
        base = root / relative
        candidates.extend(base.glob("*"))
        candidates.extend(base.glob("*/*"))
    packages = {}
    for path in sorted(candidates):
        name = read_text(path / "name")
        if not name or not re.fullmatch(r"package[-_]?\d+", name):
            continue
        if not (path / "energy_uj").exists():
            continue
        priority = 0 if re.fullmatch(r"intel-rapl:\d+", path.name) else 1
        old = packages.get(name)
        if old is None or priority < old[0]:
            packages[name] = (priority, path)
    return {name: value[1] for name, value in sorted(packages.items())}


def read_packages(root):
    return {
        name: {
            "energy_uj": read_int(path / "energy_uj"),
            "max_energy_range_uj": read_int(path / "max_energy_range_uj"),
        }
        for name, path in discover_packages(root).items()
    }


def read_pmc(root):
    raw = read_text(root / "sys/kernel/debug/pmc_core/package_cstate_show")
    states = {}
    if raw is not None:
        for line in raw.splitlines():
            match = re.fullmatch(r"\s*([^:]+):\s*(\d+)\s*", line)
            if match:
                states[match[1].strip()] = int(match[2])
    return {
        "package_residency_us": states,
        "slp_s0_residency_us": read_int(
            root / "sys/kernel/debug/pmc_core/slp_s0_residency_usec"
        ),
    }


def read_supplies(root):
    external, batteries = {}, {}
    for path in sorted((root / "sys/class/power_supply").glob("*")):
        supply_type = read_text(path / "type")
        if supply_type == "Battery":
            batteries[path.name] = {
                "present": read_int(path / "present"),
                "status": read_text(path / "status"),
                "power_now_uw": read_int(path / "power_now"),
                "current_now_ua": read_int(path / "current_now"),
                "voltage_now_uv": read_int(path / "voltage_now"),
                "energy_now_uwh": read_int(path / "energy_now"),
                "capacity_percent": read_int(path / "capacity"),
            }
        elif supply_type in {"Mains", "USB", "USB_C", "USB_PD", "USB_DCP",
                             "USB_CDP", "USB_ACA", "Wireless"} or (path / "online").exists():
            external[path.name] = {"type": supply_type, "online": read_int(path / "online")}
    return external, batteries


def battery_discharge(external, batteries):
    """Only call a reading system discharge when the source/status support it.

    On a dual-battery T480, an inactive Full/Not charging pack contributes zero;
    its stale power_now is not included. Any unknown/charging pack invalidates
    the estimate. At least one external source must explicitly report offline.
    """
    online = [item.get("online") for item in external.values()]
    if 1 in online:
        ac_state = "present"
    elif not online or any(value != 0 for value in online):
        ac_state = "unknown"
    else:
        ac_state = "absent"
    result = {"external_power": ac_state, "system_discharge_w": None,
              "reason": None, "power_methods": {}}
    present = {name: pack for name, pack in batteries.items() if pack.get("present") != 0}
    statuses = [pack.get("status") for pack in present.values()]
    if ac_state == "present":
        result["reason"] = "external_power_charging" if "Charging" in statuses else "external_power_connected"
        return result
    if ac_state == "unknown":
        result["reason"] = "external_power_unknown"
        return result
    if not present:
        result["reason"] = "no_present_battery"
        return result
    total_uw, discharging = 0, False
    for name, pack in present.items():
        if pack.get("present") != 1:
            result["reason"] = "battery_presence_unknown"
            return result
        status = pack.get("status")
        if status in {"Not charging", "Full"}:
            result["power_methods"][name] = "inactive_pack_ignored"
            continue
        if status != "Discharging":
            result["reason"] = "battery_status_inconsistent" if status == "Charging" else "battery_status_unknown"
            return result
        discharging = True
        power = pack.get("power_now_uw")
        if power is not None and power >= 0:
            result["power_methods"][name] = "power_now"
        elif power is not None:
            result["reason"] = "battery_power_negative"
            return result
        else:
            current, voltage = pack.get("current_now_ua"), pack.get("voltage_now_uv")
            if current is None or voltage is None or voltage <= 0:
                result["reason"] = "battery_power_missing"
                return result
            # Status defines direction; current_now can be signed by a driver.
            power = abs(current) * voltage / 1e6
            result["power_methods"][name] = "abs_current_times_voltage"
        total_uw += power
    if not discharging:
        result["reason"] = "no_discharging_battery"
        return result
    result["system_discharge_w"] = total_uw / 1e6
    result["reason"] = "valid_battery_discharge"
    return result


def read_gpu(root):
    devices = {}
    for path in sorted((root / "sys/bus/pci/devices").glob("*")):
        if read_text(path / "vendor") != "0x10de":
            continue
        device_class = read_text(path / "class")
        if device_class is None or not device_class.startswith("0x03"):
            continue
        devices[path.name] = {
            "power_state": read_text(path / "power_state"),
            "runtime_status": read_text(path / "power/runtime_status"),
            "runtime_control": read_text(path / "power/control"),
            "runtime_suspended_time_ms": read_int(path / "power/runtime_suspended_time"),
            "d3cold_allowed": read_int(path / "d3cold_allowed"),
        }
    return devices


def read_brightness(root):
    result = {}
    for path in sorted((root / "sys/class/backlight").glob("*")):
        result[path.name] = {name: read_int(path / name)
                             for name in ("brightness", "actual_brightness", "max_brightness")}
    return result


def read_cpu_policy(root):
    result = {}
    base = root / "sys/devices/system/cpu/cpufreq"
    for path in sorted(base.glob("policy*")):
        result[path.name] = {name: read_text(path / name) for name in (
            "scaling_driver", "scaling_governor", "energy_performance_preference"
        )}
    return result


def snapshot(root):
    external, batteries = read_supplies(root)
    return {
        "rapl_cpu_packages": read_packages(root),
        "pmc": read_pmc(root),
        "external_supplies": external,
        "batteries": batteries,
        "battery_interpretation": battery_discharge(external, batteries),
        "nvidia_sysfs": read_gpu(root),
        "backlight": read_brightness(root),
        "cpu_policy": read_cpu_policy(root),
    }


def summarize(samples):
    elapsed = samples[-1]["elapsed_s"] - samples[0]["elapsed_s"]
    packages = {}
    names = sorted({name for sample in samples for name in sample["rapl_cpu_packages"]})
    for name in names:
        energy, coverage, wraps, failures = 0, 0.0, 0, []
        for before, after in zip(samples, samples[1:]):
            dt = after["elapsed_s"] - before["elapsed_s"]
            a = before["rapl_cpu_packages"].get(name, {})
            b = after["rapl_cpu_packages"].get(name, {})
            maximum = a.get("max_energy_range_uj")
            if maximum != b.get("max_energy_range_uj"):
                delta, reason = None, "counter_range_changed"
            else:
                delta, reason = counter_delta(a.get("energy_uj"), b.get("energy_uj"), maximum)
            if dt <= 0:
                delta, reason = None, "nonpositive_interval"
            if delta is None:
                failures.append({"at_elapsed_s": after["elapsed_s"], "reason": reason})
                continue
            energy += delta
            coverage += dt
            wraps += int(reason == "wrapped")
        complete = not failures and coverage > 0
        packages[name] = {
            "energy_j": energy / 1e6 if coverage > 0 else None,
            "average_w": energy / 1e6 / coverage if complete else None,
            "valid_coverage_s": coverage,
            "complete_interval_coverage": complete,
            "wraps_detected": wraps,
            "invalid_intervals": failures,
        }
    pmc_states = {}
    for name in sorted(set(samples[0]["pmc"]["package_residency_us"]) |
                       set(samples[-1]["pmc"]["package_residency_us"])):
        a = samples[0]["pmc"]["package_residency_us"].get(name)
        b = samples[-1]["pmc"]["package_residency_us"].get(name)
        delta, reason = counter_delta(a, b)
        percent = delta / elapsed / 1e6 * 100 if delta is not None and elapsed > 0 else None
        pmc_states[name] = {"delta_us": delta, "residency_percent": percent, "reason": reason}
    s0_delta, s0_reason = counter_delta(samples[0]["pmc"]["slp_s0_residency_us"],
                                       samples[-1]["pmc"]["slp_s0_residency_us"])
    discharge = [sample["battery_interpretation"]["system_discharge_w"] for sample in samples]
    valid = [value for value in discharge if value is not None]
    continuous = len(samples) >= 2 and elapsed > 0 and len(valid) == len(samples)
    transitions = []
    for before, after in zip(samples, samples[1:]):
        fields = {
            "external_power": lambda s: s["battery_interpretation"]["external_power"],
            "battery_status": lambda s: {name: (p["present"], p["status"]) for name, p in s["batteries"].items()},
            "nvidia_power": lambda s: {name: (p["power_state"], p["runtime_status"]) for name, p in s["nvidia_sysfs"].items()},
            "backlight": lambda s: s["backlight"],
            "cpu_policy": lambda s: s["cpu_policy"],
        }
        for field, getter in fields.items():
            a, b = getter(before), getter(after)
            if a != b:
                transitions.append({"at_elapsed_s": after["elapsed_s"], "field": field, "before": a, "after": b})
    return {
        "elapsed_s": elapsed,
        "max_snapshot_read_span_s": max((sample.get("read_span_s", 0) for sample in samples), default=0),
        "cpu_package_energy": packages,
        "battery_system_discharge": {
            "continuous_valid_discharge": continuous,
            "valid_sample_count": len(valid), "sample_count": len(samples),
            "median_w": statistics.median(valid) if continuous else None,
            "min_w": min(valid) if continuous else None,
            "max_w": max(valid) if continuous else None,
            "reasons": sorted({s["battery_interpretation"]["reason"] for s in samples}),
        },
        "pmc_package_cstates": pmc_states,
        "pmc_slp_s0": {"delta_us": s0_delta, "reason": s0_reason},
        "observed_transitions": transitions,
    }


def measure(root, seconds, interval):
    started = time.monotonic()
    deadline = started + seconds
    samples = []
    target = started
    while True:
        remaining = target - time.monotonic()
        if remaining > 0:
            time.sleep(remaining)
        before = time.monotonic()
        sample = snapshot(root)
        after = time.monotonic()
        sample["elapsed_s"] = (before + after) / 2 - started
        sample["read_span_s"] = after - before
        samples.append(sample)
        if after >= deadline:
            break
        target = min(target + interval, deadline)
        if target < after:
            target = min(after + interval, deadline)
    return {
        "schema_version": 1,
        "captured_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "requested_duration_s": seconds, "sampling_interval_s": interval,
        "measurement_scope": "observational; CPU package is not whole-system power",
        "summary": summarize(samples), "samples": samples,
    }


def human_report(result):
    summary, last = result["summary"], result["samples"][-1]
    lines = [f"Observed {summary['elapsed_s']:.2f} s; {len(result['samples'])} samples (read-only)."]
    if not summary["cpu_package_energy"]:
        lines.append("CPU package RAPL: unavailable; not a whole-board measurement.")
    for name, value in summary["cpu_package_energy"].items():
        power = f"{value['average_w']:.3f} W" if value["average_w"] is not None else "unavailable (incomplete counters)"
        lines.append(f"CPU {name}: {power}; wraps {value['wraps_detected']}; not whole-board power.")
    battery = summary["battery_system_discharge"]
    if battery["continuous_valid_discharge"]:
        lines.append(f"Battery system discharge: median {battery['median_w']:.3f} W, range {battery['min_w']:.3f}–{battery['max_w']:.3f} W.")
    else:
        lines.append("Battery system discharge: unavailable (" + ", ".join(battery["reasons"]) + ").")
    statuses = ", ".join(f"{name}={p['status'] or 'unknown'}" for name, p in last["batteries"].items()) or "unavailable"
    lines.append(f"External power: {last['battery_interpretation']['external_power']}; {statuses}.")
    pmc = ", ".join(f"{name} {p['residency_percent']:.1f}%" for name, p in summary["pmc_package_cstates"].items() if p["residency_percent"] is not None)
    lines.append("PMC package residency: " + (pmc or "unavailable") + ".")
    gpu = ", ".join(f"{name} {p['power_state'] or '?'} / {p['runtime_status'] or '?'}" for name, p in last["nvidia_sysfs"].items())
    lines.append("NVIDIA sysfs: " + (gpu or "unavailable") + ".")
    brightness = ", ".join(f"{name} {p['actual_brightness'] if p['actual_brightness'] is not None else p['brightness']}/{p['max_brightness']}" for name, p in last["backlight"].items())
    lines.append("Backlight: " + (brightness or "unavailable") + ".")
    policy = sorted({(p["scaling_governor"], p["energy_performance_preference"]) for p in last["cpu_policy"].values()}, key=str)
    lines.append("CPU governor/EPP: " + (", ".join(f"{a or '?'}/{b or '?'}" for a, b in policy) or "unavailable") + ".")
    lines.append(f"Observed source/status/policy/GPU/backlight transitions: {len(summary['observed_transitions'])}.")
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seconds", type=float, default=30, help="duration (default: 30 seconds)")
    parser.add_argument("--interval", type=float, default=1, help="sample period, 0.1–60 seconds (default: 1)")
    parser.add_argument("--json", action="store_true", help="print JSON instead of a concise report")
    parser.add_argument("--root", type=Path, default=Path("/"), help="filesystem root (for fixtures/offline mounts)")
    parser.add_argument("--from-json", type=Path, help="report an existing capture without reading hardware")
    args = parser.parse_args()
    if not math.isfinite(args.seconds) or not 0 < args.seconds <= 86400:
        parser.error("--seconds must be finite and between 0 and 86400")
    if not math.isfinite(args.interval) or not 0.1 <= args.interval <= 60:
        parser.error("--interval must be finite and between 0.1 and 60")
    if args.from_json:
        try:
            result = json.loads(args.from_json.read_text())
        except (OSError, ValueError) as error:
            parser.error(f"cannot read capture: {error}")
    else:
        result = measure(args.root, args.seconds, args.interval)
    print(json.dumps(result, indent=2, allow_nan=False) if args.json else human_report(result))


if __name__ == "__main__":
    main()
