"""Execute the diagnostic against fake files, never real proc/sysfs."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[3] / "system/usr-local/bin/nvidia-suspend-test.sh"


class ReadinessTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="nvidia-readiness-")
        self.root = Path(self.directory.name)
        self.gpu = "sys/bus/pci/devices/0000:01:00.0"
        self.write(self.gpu + "/vendor", "0x10de")
        self.write(self.gpu + "/device", "0x1d10")
        self.write(self.gpu + "/power_state", "D3cold")
        self.write(self.gpu + "/power/runtime_status", "suspended")

    def tearDown(self):
        self.directory.cleanup()

    def write(self, name, content):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)

    def loaded(self):
        (self.root / "sys/module/nvidia").mkdir(parents=True)
        self.write("proc/driver/nvidia/version", "NVIDIA UNIX x86_64 Kernel Module 580.178.04")
        self.write("proc/driver/nvidia/params", "PreserveVideoMemoryAllocations: 1\nDynamicPowerManagement: 1\n")
        self.write("proc/driver/nvidia/suspend", "")
        self.write("sys/module/nvidia_drm/parameters/modeset", "N")
        self.write("sys/module/nvidia_drm/parameters/fbdev", "N")

    def report(self):
        result = subprocess.run(["sh", str(SCRIPT), "--root", str(self.root), "--json"],
                                capture_output=True, text=True, timeout=5)
        return result.returncode, json.loads(result.stdout)

    def test_unloaded_on_demand_gpu_is_not_a_false_failure_or_sleep_result(self):
        code, report = self.report()
        self.assertEqual(code, 0)
        self.assertFalse(report["hardware_tested"])
        self.assertFalse(report["settings_changed"])
        self.assertTrue(any("was not assessed" in row["message"] for row in report["checks"]))

    def test_offload_does_not_require_kms_systemd_or_persistence(self):
        self.loaded()
        code, report = self.report()
        self.assertEqual(code, 0)
        self.assertEqual(report["failures"], 0)

    def test_preserve_without_proc_control_fails(self):
        self.loaded()
        (self.root / "proc/driver/nvidia/suspend").unlink()
        code, report = self.report()
        self.assertEqual(code, 1)
        self.assertEqual(report["failures"], 1)

    def test_unreadable_parameters_remain_unknown(self):
        self.loaded()
        (self.root / "proc/driver/nvidia/params").unlink()
        code, report = self.report()
        self.assertEqual(code, 2)
        self.assertGreater(report["unknown"], 0)

    def test_fixed_suspend_and_async_resume_are_reported(self):
        self.loaded()
        self.write("usr/lib/elogind/system-sleep/nvidia", '#!/bin/sh\ncase "$1" in\npre) /usr/bin/nvidia-sleep.sh suspend ;;\npost) /usr/bin/nvidia-sleep.sh resume & ;;\nesac\n')
        _, report = self.report()
        warnings = [row["message"] for row in report["checks"] if row["status"] == "WARN"]
        self.assertTrue(any("backgrounds" in message for message in warnings))
        self.assertTrue(any("fixed suspend" in message for message in warnings))

    def test_dual_sleep_ownership_is_observed_without_mutation(self):
        self.loaded()
        self.write("etc/elogind/sleep.conf", "[Sleep]\nHandleNvidiaSleep=yes\n")
        self.write("usr/lib/elogind/system-sleep/nvidia", "#!/bin/sh\nexit 0\n")
        _, report = self.report()
        self.assertTrue(any("one coherent owner" in row["message"] for row in report["checks"]))
        self.assertEqual((self.root / "etc/elogind/sleep.conf").read_text(), "[Sleep]\nHandleNvidiaSleep=yes\n")

    def test_open_module_is_wrong_for_pascal(self):
        self.loaded()
        self.write("proc/driver/nvidia/version", "NVIDIA UNIX Open Kernel Module 580.178.04")
        code, report = self.report()
        self.assertEqual(code, 1)
        self.assertTrue(any("do not support" in row["message"] for row in report["checks"]))

    def test_absent_gpu_does_not_require_a_driver_load(self):
        (self.root / self.gpu / "vendor").unlink()
        (self.root / self.gpu / "device").unlink()
        code, report = self.report()
        self.assertEqual(code, 0)
        self.assertFalse(report["hardware_tested"])


if __name__ == "__main__":
    unittest.main()
