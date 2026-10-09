#!/usr/bin/env python3
"""Check inherited routing, failure behavior, and ordinary process semantics."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


class IntelRun(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.mesa = self.root / "mesa.json"
        self.intel = self.root / "intel.json"
        self.mesa.write_text("{}")
        self.intel.write_text("{}")
        source = Path(__file__).resolve().parents[1] / "igpu-run"
        self.wrapper = self.root / "igpu-run"
        self.wrapper.write_text(source.read_text()
                                .replace("/usr/share/glvnd/egl_vendor.d/50_mesa.json", str(self.mesa))
                                .replace("/usr/share/vulkan/icd.d/intel_icd.json", str(self.intel)))
        self.wrapper.chmod(0o755)

    def tearDown(self):
        self.temp.cleanup()

    def run_job(self, *args, **kw):
        return subprocess.run([str(self.wrapper), *args], text=True,
                              capture_output=True, timeout=5, **kw)

    def test_overrides_nested_prime_and_filters(self):
        inherited = dict(os.environ, __NV_PRIME_RENDER_OFFLOAD="1",
                         __NV_PRIME_RENDER_OFFLOAD_PROVIDER="NVIDIA-G0",
                         __VK_LAYER_NV_optimus="NVIDIA_only", DRI_PRIME="1",
                         VK_DRIVER_FILES="nvidia.json", VK_ICD_FILENAMES="nvidia.json",
                         VK_LOADER_DRIVERS_SELECT="*nvidia*", VK_LOADER_DRIVERS_DISABLE="*intel*",
                         __EGL_VENDOR_LIBRARY_DIRS="/nvidia", MESA_VK_DEVICE_SELECT="10de:1d10",
                         VK_INSTANCE_LAYERS="VK_LAYER_NV_optimus", VK_LOADER_LAYERS_ENABLE="VK_LAYER_NV_*",
                         MESA_LOADER_DRIVER_OVERRIDE="nouveau")
        r = self.run_job("python3", "-c", "import os,json; print(json.dumps(dict(os.environ)))", env=inherited)
        self.assertEqual(r.returncode, 0, r.stderr)
        env = json.loads(r.stdout)
        for key in ["__NV_PRIME_RENDER_OFFLOAD", "__NV_PRIME_RENDER_OFFLOAD_PROVIDER",
                    "__VK_LAYER_NV_optimus", "VK_LOADER_DRIVERS_SELECT", "VK_LOADER_DRIVERS_DISABLE",
                    "__EGL_VENDOR_LIBRARY_DIRS", "MESA_VK_DEVICE_SELECT", "VK_INSTANCE_LAYERS",
                    "VK_LOADER_LAYERS_ENABLE", "MESA_LOADER_DRIVER_OVERRIDE"]:
            self.assertNotIn(key, env)
        self.assertEqual(env["DRI_PRIME"], "pci-0000_00_02_0")
        self.assertEqual(env["__GLX_VENDOR_LIBRARY_NAME"], "mesa")
        self.assertEqual(env["__EGL_VENDOR_LIBRARY_FILENAMES"], str(self.mesa))
        self.assertEqual(env["VK_DRIVER_FILES"], str(self.intel))
        self.assertEqual(env["VK_ICD_FILENAMES"], str(self.intel))

    def test_missing_driver_does_not_run_application(self):
        self.intel.unlink()
        marker = self.root / "ran"
        r = self.run_job("touch", str(marker))
        self.assertEqual(r.returncode, 69)
        self.assertFalse(marker.exists())

    def test_stdin_arguments_and_exit_status(self):
        r = self.run_job("sh", "-c", 'read value; printf "%s:%s\\n" "$value" "$1"; exit 7',
                         "sh", "two words", input="hello\n")
        self.assertEqual(r.returncode, 7)
        self.assertEqual(r.stdout, "hello:two words\n")

    def test_usage(self):
        self.assertEqual(self.run_job().returncode, 64)


if __name__ == "__main__":
    unittest.main()
