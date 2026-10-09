"""Fake NVML tests: no GPU, driver or real NVML initialization occurs."""
import importlib.util
from pathlib import Path
import unittest


SOURCE = Path(__file__).resolve().parents[1] / "mx150-capabilities.py"
SPEC = importlib.util.spec_from_file_location("mx150_capabilities", SOURCE)
PROBE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROBE)


class Function:
    def __init__(self, library, name):
        self.library, self.name = library, name

    def __call__(self, *args):
        self.library.calls.append(self.name)
        code = self.library.codes.get(self.name, 0)
        if code:
            return code
        if self.name == "nvmlDeviceGetHandleByPciBusId_v2":
            self.library.bus_id = args[0]
            args[1]._obj.value = 42
        elif self.name == "nvmlDeviceGetName":
            args[1].value = self.library.name
        elif self.name in {"nvmlSystemGetDriverVersion", "nvmlSystemGetNVMLVersion"}:
            args[0].value = b"580.178.04"
        elif self.name == "nvmlDeviceGetClockOffsets":
            info = args[1]._obj
            self.library.struct_inputs.append((info.version, info.type, info.pstate))
            info.clockOffsetMHz = 200 if info.type == 0 else 1250
            info.minClockOffsetMHz, info.maxClockOffsetMHz = -200, 300
        elif self.name not in {"nvmlInit_v2", "nvmlShutdown"}:
            for arg in args:
                if hasattr(arg, "_obj"):
                    arg._obj.value = 200
        return 0


class Library:
    def __init__(self, codes=None, missing=(), name=b"NVIDIA GeForce MX150"):
        self.codes = codes or {}
        self.missing, self.name = set(missing), name
        self.calls, self.struct_inputs = [], []

    def __getattr__(self, name):
        if name in self.missing:
            raise AttributeError(name)
        if "Set" in name or "Reset" in name:
            raise AssertionError("A capability query attempted a setter")
        return Function(self, name)


class CapabilityTests(unittest.TestCase):
    def test_success_preserves_units_offsets_and_selected_bus(self):
        library = Library()
        report = PROBE.collect(library)
        self.assertEqual(library.bus_id, b"0000:01:00.0")
        self.assertEqual(report["status"], "queried")
        self.assertEqual(report["offsets_mhz"]["graphics"]["value"], 200)
        self.assertEqual(report["power_mw"]["limit"]["value"], 200)
        self.assertFalse(report["settings_changed"])
        self.assertTrue(report["may_wake_gpu"])
        self.assertEqual(report["voltage"]["status"], "unresolved")
        self.assertEqual(library.calls[-1], "nvmlShutdown")

    def test_not_supported_is_distinct_from_missing_export(self):
        library = Library(codes={"nvmlDeviceGetPowerUsage": 3},
                          missing={"nvmlDeviceGetClockOffsets"})
        report = PROBE.collect(library)
        self.assertEqual(report["power_mw"]["usage"]["status"], "not_supported")
        entry = report["per_state_offsets_mhz"]["graphics"]["P0"]
        self.assertEqual(entry["status"], "missing_symbol")
        self.assertNotIn("value", report["power_mw"]["usage"])

    def test_permission_failure_does_not_become_unsupported_or_zero(self):
        report = PROBE.collect(Library(codes={"nvmlDeviceGetGpcClkVfOffset": 4}))
        entry = report["offsets_mhz"]["graphics"]
        self.assertEqual(entry["status"], "no_permission")
        self.assertNotIn("value", entry)

    def test_init_failure_stops_without_lookup_or_shutdown(self):
        library = Library(codes={"nvmlInit_v2": 18})
        report = PROBE.collect(library)
        self.assertEqual(report["status"], "init_failed")
        self.assertEqual(library.calls, ["nvmlInit_v2"])

    def test_lookup_failure_still_shuts_down(self):
        library = Library(codes={"nvmlDeviceGetHandleByPciBusId_v2": 6})
        report = PROBE.collect(library)
        self.assertEqual(report["status"], "device_lookup_failed")
        self.assertEqual(library.calls[-1], "nvmlShutdown")
        self.assertNotIn("offsets_mhz", report)

    def test_wrong_gpu_stops_before_clock_and_power_queries(self):
        library = Library(name=b"NVIDIA GeForce RTX 4090")
        report = PROBE.collect(library)
        self.assertEqual(report["status"], "wrong_gpu")
        self.assertNotIn("nvmlDeviceGetPowerUsage", library.calls)
        self.assertEqual(library.calls[-1], "nvmlShutdown")

    def test_unknown_identity_stops_before_clock_queries(self):
        library = Library(codes={"nvmlDeviceGetName": 15})
        report = PROBE.collect(library)
        self.assertEqual(report["status"], "identity_unverified")
        self.assertNotIn("nvmlDeviceGetClockOffsets", library.calls)

    def test_exact_versioned_structure_and_all_states(self):
        library = Library()
        PROBE.collect(library)
        self.assertEqual(PROBE.C.sizeof(PROBE.ClockOffset), 24)
        self.assertEqual(set(library.struct_inputs),
                         {(0x1000018, domain, pstate)
                          for domain in (0, 2) for pstate in range(16)})

    def test_unrecognized_return_code_remains_an_error(self):
        report = PROBE.collect(Library(codes={"nvmlDeviceGetClockInfo": 999}))
        self.assertEqual(report["clocks_mhz"]["memory"]["status"], "nvml_error")
        self.assertEqual(report["clocks_mhz"]["memory"]["code"], 999)
        self.assertNotIn("value", report["clocks_mhz"]["memory"])

    def test_argument_and_version_errors_remain_distinct(self):
        report = PROBE.collect(Library(codes={"nvmlDeviceGetClockOffsets": 25,
                                             "nvmlDeviceGetClockInfo": 2}))
        self.assertEqual(report["per_state_offsets_mhz"]["graphics"]["P0"]["status"],
                         "argument_version_mismatch")
        self.assertEqual(report["clocks_mhz"]["graphics"]["status"], "invalid_argument")

    def test_query_layer_rejects_every_setter(self):
        library = Library()
        with self.assertRaises(ValueError):
            PROBE.call(library, "nvmlDeviceSetGpcClkVfOffset", [], 200)
        self.assertEqual(library.calls, [])

    def test_invalid_bus_id_fails_before_initialization(self):
        library = Library()
        with self.assertRaises(ValueError):
            PROBE.collect(library, "0")
        self.assertEqual(library.calls, [])

    def test_shutdown_failure_is_not_reported_as_success(self):
        report = PROBE.collect(Library(codes={"nvmlShutdown": 999}))
        self.assertEqual(report["status"], "shutdown_failed")
        self.assertEqual(report["shutdown"]["code"], 999)


if __name__ == "__main__":
    unittest.main()
