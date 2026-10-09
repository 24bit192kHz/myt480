#!/usr/bin/env python3
"""Meaningful fixture tests; never access host hardware or NVIDIA APIs."""

import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).resolve().parents[1] / "measure-power.py"
spec = importlib.util.spec_from_file_location("measure_power", SCRIPT)
power = importlib.util.module_from_spec(spec)
spec.loader.exec_module(power)


def pack(status="Discharging", power_uw=7_000_000, present=1):
    return {"present": present, "status": status, "power_now_uw": power_uw,
            "current_now_ua": None, "voltage_now_uv": None}


def sample(elapsed, energy=1_000_000, maximum=10_000_000, external=0,
           status="Discharging", pmc=0, gpu="D3cold"):
    snapshot = {
        "elapsed_s": elapsed,
        "rapl_cpu_packages": {"package-0": {"energy_uj": energy, "max_energy_range_uj": maximum}},
        "pmc": {"package_residency_us": {"Package C8": pmc}, "slp_s0_residency_us": None},
        "external_supplies": {"AC": {"type": "Mains", "online": external}},
        "batteries": {"BAT0": pack(status)},
        "nvidia_sysfs": {"0000:01:00.0": {"power_state": gpu, "runtime_status": "suspended" if gpu == "D3cold" else "active"}},
        "backlight": {}, "cpu_policy": {},
    }
    snapshot["battery_interpretation"] = power.battery_discharge(snapshot["external_supplies"], snapshot["batteries"])
    return snapshot


class CounterTests(unittest.TestCase):
    def test_documented_wrap(self):
        self.assertEqual(power.counter_delta(90, 10, 100), (20, "wrapped"))

    def test_decrease_without_range_is_not_negative_energy(self):
        self.assertEqual(power.counter_delta(90, 10), (None, "counter_decreased_without_range"))

    def test_missing_negative_and_out_of_range(self):
        for a, b, maximum, reason in (
            (None, 1, 100, "counter_missing"), (1, None, 100, "counter_missing"),
            (-1, 10, 100, "counter_negative"), (1, 101, 100, "counter_range_invalid"),
            (1, 10, 0, "counter_range_invalid"),
        ):
            with self.subTest(a=a, b=b, maximum=maximum):
                self.assertEqual(power.counter_delta(a, b, maximum), (None, reason))

    def test_package_watts_with_wrap(self):
        result = power.summarize([sample(0, 8_000_000), sample(2, 2_000_000)])
        package = result["cpu_package_energy"]["package-0"]
        self.assertEqual(package["energy_j"], 4)
        self.assertEqual(package["average_w"], 2)
        self.assertEqual(package["wraps_detected"], 1)

    def test_partial_counters_cannot_report_complete_window_average(self):
        result = power.summarize([sample(0), sample(1, 2_000_000), sample(2, None)])
        package = result["cpu_package_energy"]["package-0"]
        self.assertEqual(package["energy_j"], 1)
        self.assertEqual(package["valid_coverage_s"], 1)
        self.assertIsNone(package["average_w"])
        self.assertFalse(package["complete_interval_coverage"])

    def test_range_change_and_disappearing_zone(self):
        a, b = sample(0), sample(2, 4_000_000, 20_000_000)
        package = power.summarize([a, b])["cpu_package_energy"]["package-0"]
        self.assertIsNone(package["average_w"])
        self.assertEqual(package["invalid_intervals"][0]["reason"], "counter_range_changed")
        b["rapl_cpu_packages"] = {}
        self.assertIsNone(power.summarize([a, b])["cpu_package_energy"]["package-0"]["average_w"])

    def test_zero_interval_and_pmc_decrease_are_invalid(self):
        result = power.summarize([sample(0, pmc=10), sample(0, pmc=5)])
        self.assertIsNone(result["cpu_package_energy"]["package-0"]["average_w"])
        self.assertIsNone(result["pmc_package_cstates"]["Package C8"]["residency_percent"])


class BatteryTests(unittest.TestCase):
    def evaluate(self, batteries, external=None):
        return power.battery_discharge({"AC": {"online": 0}} if external is None else external, batteries)

    def test_dual_pack_inactive_rate_is_ignored(self):
        result = self.evaluate({"BAT0": pack(), "BAT1": pack("Not charging", 20_000_000)})
        self.assertEqual(result["system_discharge_w"], 7)
        self.assertEqual(result["power_methods"]["BAT1"], "inactive_pack_ignored")

    def test_two_active_packs_sum_and_absent_pack_is_ignored(self):
        result = self.evaluate({"BAT0": pack(), "BAT1": pack(power_uw=3_000_000), "BAT2": pack(present=0)})
        self.assertEqual(result["system_discharge_w"], 10)

    def test_ac_charging_and_not_charging_do_not_become_discharge(self):
        for status, reason in (("Charging", "external_power_charging"), ("Not charging", "external_power_connected")):
            with self.subTest(status=status):
                result = self.evaluate({"BAT0": pack(status)}, {"AC": {"online": 1}})
                self.assertIsNone(result["system_discharge_w"])
                self.assertEqual(result["reason"], reason)

    def test_unknown_source_cannot_assert_ac_absent(self):
        for external in ({}, {"AC": {"online": None}}, {"AC": {"online": 0}, "USB": {"online": None}}):
            with self.subTest(external=external):
                result = self.evaluate({"BAT0": pack()}, external)
                self.assertIsNone(result["system_discharge_w"])
                self.assertEqual(result["external_power"], "unknown")

    def test_connected_usb_counts_as_external(self):
        result = self.evaluate({"BAT0": pack()}, {"AC": {"online": 0}, "USB-C": {"online": 1}})
        self.assertEqual(result["external_power"], "present")
        self.assertIsNone(result["system_discharge_w"])

    def test_missing_status_presence_and_power_are_invalid(self):
        cases = ((pack(None), "battery_status_unknown"),
                 (pack("Charging"), "battery_status_inconsistent"),
                 (pack(present=None), "battery_presence_unknown"),
                 (pack(power_uw=None), "battery_power_missing"),
                 (pack(power_uw=-1), "battery_power_negative"))
        for battery, reason in cases:
            with self.subTest(reason=reason):
                result = self.evaluate({"BAT0": battery})
                self.assertIsNone(result["system_discharge_w"])
                self.assertEqual(result["reason"], reason)

    def test_current_times_voltage_fallback_uses_correct_units(self):
        battery = pack(power_uw=None)
        battery.update(current_now_ua=-500_000, voltage_now_uv=12_000_000)
        result = self.evaluate({"BAT0": battery})
        self.assertEqual(result["system_discharge_w"], 6)
        self.assertEqual(result["power_methods"]["BAT0"], "abs_current_times_voltage")

    def test_source_transition_invalidates_entire_discharge_summary(self):
        samples = [sample(0), sample(1, 2_000_000), sample(2, 3_000_000, external=1, status="Charging", gpu="D0")]
        result = power.summarize(samples)
        battery = result["battery_system_discharge"]
        self.assertEqual(battery["valid_sample_count"], 2)
        self.assertIsNone(battery["median_w"])
        self.assertFalse(battery["continuous_valid_discharge"])
        fields = {transition["field"] for transition in result["observed_transitions"]}
        self.assertEqual(fields, {"external_power", "battery_status", "nvidia_power"})


class DiscoveryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)

    def put(self, relative, value):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(str(value) + "\n")

    def zone(self, relative, name, energy):
        self.put(relative + "/name", name)
        self.put(relative + "/energy_uj", energy)
        self.put(relative + "/max_energy_range_uj", 10_000_000)

    def test_flat_nested_symlink_and_duplicate_backend_discovery(self):
        package = "sys/class/powercap/intel-rapl/intel-rapl:0"
        self.zone(package, "package-0", 111)
        self.zone(package + "/intel-rapl:0:0", "core", 222)
        self.zone("sys/class/powercap/intel-rapl-mmio:0", "package-0", 333)
        self.zone("sys/class/power_cap/intel-rapl:1", "package-1", 444)
        self.zone("sys/class/powercap/intel-rapl:2", "psys", 555)
        link = self.root / "sys/class/powercap/intel-rapl:0"
        link.symlink_to("intel-rapl/intel-rapl:0", target_is_directory=True)
        result = power.read_packages(self.root)
        self.assertEqual(set(result), {"package-0", "package-1"})
        self.assertEqual(result["package-0"]["energy_uj"], 111)
        self.assertEqual(result["package-1"]["energy_uj"], 444)

    def test_nonstandard_supply_names_and_nvidia_only_sysfs(self):
        self.put("sys/class/power_supply/packA/type", "Battery")
        self.put("sys/class/power_supply/packA/present", 1)
        self.put("sys/class/power_supply/packA/status", "Discharging")
        self.put("sys/class/power_supply/packA/power_now", 8_000_000)
        self.put("sys/class/power_supply/pd-port/type", "USB_C")
        self.put("sys/class/power_supply/pd-port/online", 0)
        for bdf, vendor, device_class in (("0000:01:00.0", "0x10de", "0x030200"),
                                         ("0000:00:02.0", "0x8086", "0x030000"),
                                         ("0000:01:00.1", "0x10de", "0x040300")):
            self.put(f"sys/bus/pci/devices/{bdf}/vendor", vendor)
            self.put(f"sys/bus/pci/devices/{bdf}/class", device_class)
        self.put("sys/bus/pci/devices/0000:01:00.0/power_state", "D3cold")
        self.put("sys/bus/pci/devices/0000:01:00.0/power/runtime_status", "suspended")
        observed = power.snapshot(self.root)
        self.assertEqual(observed["battery_interpretation"]["system_discharge_w"], 8)
        self.assertEqual(set(observed["nvidia_sysfs"]), {"0000:01:00.0"})
        self.assertEqual(observed["nvidia_sysfs"]["0000:01:00.0"]["power_state"], "D3cold")

    def test_missing_files_and_bad_values_do_not_crash_or_become_zero(self):
        self.put("sys/class/backlight/panel/brightness", "invalid")
        observed = power.snapshot(self.root)
        self.assertEqual(observed["rapl_cpu_packages"], {})
        self.assertEqual(observed["pmc"]["package_residency_us"], {})
        self.assertIsNone(observed["backlight"]["panel"]["brightness"])
        self.assertEqual(observed["battery_interpretation"]["external_power"], "unknown")

    def test_pmc_parser_and_residency_conversion(self):
        self.put("sys/kernel/debug/pmc_core/package_cstate_show", "Package C2 : 123\nPackage C8: 4500\nignored line")
        result = power.read_pmc(self.root)
        self.assertEqual(result["package_residency_us"], {"Package C2": 123, "Package C8": 4500})
        summary = power.summarize([sample(0), sample(2, pmc=1_500_000)])
        self.assertEqual(summary["pmc_package_cstates"]["Package C8"]["residency_percent"], 75)


class SamplingTests(unittest.TestCase):
    def test_monotonic_deadlines_and_nonatomic_read_span(self):
        clock = [0.0]
        readings = [0]

        def sleep(duration):
            self.assertGreaterEqual(duration, 0)
            clock[0] += duration

        def capture(_root):
            readings[0] += 1
            clock[0] += 0.01
            return sample(0, readings[0] * 1_000_000)

        with patch.object(power.time, "monotonic", side_effect=lambda: clock[0]), \
             patch.object(power.time, "sleep", side_effect=sleep), \
             patch.object(power.time, "time", side_effect=AssertionError("wall clock used for interval")), \
             patch.object(power, "snapshot", side_effect=capture):
            result = power.measure(Path("/unused-fixture"), 2, 1)
        self.assertEqual(len(result["samples"]), 3)
        self.assertAlmostEqual(result["summary"]["elapsed_s"], 2)
        self.assertAlmostEqual(result["summary"]["cpu_package_energy"]["package-0"]["average_w"], 1)
        self.assertAlmostEqual(result["summary"]["max_snapshot_read_span_s"], 0.01)

    def test_one_observation_has_no_discharge_window(self):
        result = power.summarize([sample(0)])
        self.assertFalse(result["battery_system_discharge"]["continuous_valid_discharge"])
        self.assertIsNone(result["battery_system_discharge"]["median_w"])


if __name__ == "__main__":
    unittest.main()
