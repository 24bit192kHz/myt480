#!/usr/bin/env python3
"""Exercise real GPU policy functions with fake sysfs, modprobe and users locks.

No root privileges, NVIDIA libraries, device nodes or kernel modules are used.
"""
import fcntl
from pathlib import Path
import shlex
import subprocess
import tempfile
import unittest


class PolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.directory = tempfile.TemporaryDirectory(prefix="gpu-policy-")
        cls.root = Path(cls.directory.name)
        root = cls.root
        source = Path(__file__).resolve().parents[1] / "gpu-power.c"
        fake = root / "modprobe"
        fake.write_text(f"""#!/bin/sh
printf '%s\\n' "$*" >> {shlex.quote(str(root / 'calls'))}
rc=$(cat {shlex.quote(str(root / 'returncode'))})
if [ "$rc" -eq 0 ] && [ "$1" = '-r' ]; then
    rm -f {shlex.quote(str(root / 'module'))}
fi
exit "$rc"
""")
        fake.chmod(0o755)
        harness = root / "policy.c"
        harness.write_text(f"""
#define main production_main
#include "{source}"
#undef main
int main(int argc, char **argv) {{
    int users, rc;
    if (argc != 2) return 64;
    users = open(USERS, O_RDONLY | O_CREAT, 0644);
    if (users < 0) return 1;
    if (!strcmp(argv[1], "on")) rc = on(0);
    else if (!strcmp(argv[1], "off")) {{
        set_manual("off");
        rc = manual_off(users);
    }} else if (!strcmp(argv[1], "apply")) rc = apply(users, 1);
    else if (!strcmp(argv[1], "bad-off-lock")) rc = manual_off(-1);
    else if (!strcmp(argv[1], "bad-apply-lock")) rc = apply(-1, 1);
    else rc = 64;
    close(users);
    return rc;
}}
""")
        paths = {"GPU": root, "AC": root / "ac", "MODPROBE": fake,
                 "USERS": root / "users", "MANUAL": root / "manual",
                 "CONF": root / "config", "NVIDIA_DEVICE": root / "node",
                 "NVIDIA_MODULE": root / "module", "NVML": root / "missing.so"}
        cls.executable = root / "policy"
        subprocess.run(["cc", "-O2", "-Wall", "-Wextra", "-Werror",
                        *[f'-D{key}="{value}"' for key, value in paths.items()],
                        str(harness), "-ldl", "-o", str(cls.executable)], check=True)

    @classmethod
    def tearDownClass(cls):
        cls.directory.cleanup()

    def setUp(self):
        root = self.root
        (root / "vendor").write_text("0x10de\n")
        (root / "ac").write_text("0\n")
        (root / "config").write_text("rtd3 0\n")
        (root / "returncode").write_text("0\n")
        (root / "calls").write_text("")
        (root / "node").touch()
        (root / "module").touch()
        (root / "manual").unlink(missing_ok=True)
        (root / "power").mkdir(exist_ok=True)
        (root / "power/control").write_text("on\n")

    def run_policy(self, command):
        return subprocess.run([str(self.executable), command], capture_output=True,
                              text=True, timeout=8)

    def calls(self):
        return (self.root / "calls").read_text().splitlines()

    def test_automatic_unload_failure_is_reported(self):
        (self.root / "returncode").write_text("1\n")
        self.assertEqual(self.run_policy("apply").returncode, 3)
        self.assertEqual(len(self.calls()), 1)
        self.assertTrue((self.root / "module").exists())

    def test_manual_unload_failure_is_reported(self):
        (self.root / "returncode").write_text("1\n")
        self.assertEqual(self.run_policy("off").returncode, 3)
        self.assertEqual(len(self.calls()), 1)

    def test_manual_off_protects_launcher_before_device_open(self):
        with (self.root / "users").open("a+") as launcher:
            fcntl.flock(launcher, fcntl.LOCK_SH)
            result = self.run_policy("off")
            self.assertEqual(result.returncode, 3)
            self.assertIn("using or starting", result.stderr)
            self.assertEqual(self.calls(), [])
            self.assertEqual((self.root / "manual").read_text(), "off")
        # The retained off request can be applied once the launcher has ended.
        self.assertEqual(self.run_policy("apply").returncode, 0)
        self.assertEqual(len(self.calls()), 1)
        self.assertFalse((self.root / "module").exists())

    def test_automatic_off_defers_while_launcher_holds_lock(self):
        with (self.root / "users").open("a+") as launcher:
            fcntl.flock(launcher, fcntl.LOCK_SH)
            self.assertEqual(self.run_policy("apply").returncode, 0)
            self.assertEqual(self.calls(), [])

    def test_manual_users_lock_failure_stops_unload(self):
        self.assertEqual(self.run_policy("bad-off-lock").returncode, 1)
        self.assertEqual(self.calls(), [])

    def test_automatic_users_lock_failure_stops_unload(self):
        self.assertEqual(self.run_policy("bad-apply-lock").returncode, 1)
        self.assertEqual(self.calls(), [])

    def test_successful_manual_off_releases_lock(self):
        self.assertEqual(self.run_policy("off").returncode, 0)
        with (self.root / "users").open("r") as users:
            fcntl.flock(users, fcntl.LOCK_EX | fcntl.LOCK_NB)
        self.assertFalse((self.root / "module").exists())

    def test_absent_module_needs_no_unload(self):
        (self.root / "module").unlink()
        self.assertEqual(self.run_policy("apply").returncode, 0)
        self.assertEqual(self.calls(), [])

    def test_missing_node_after_load_is_failure(self):
        (self.root / "node").unlink()
        result = self.run_policy("on")
        self.assertEqual(result.returncode, 1)
        self.assertIn("did not appear", result.stderr)
        self.assertEqual(len(self.calls()), 4)

    def test_successful_module_load_with_node(self):
        self.assertEqual(self.run_policy("on").returncode, 0)
        self.assertEqual(len(self.calls()), 4)

    def test_module_load_failure_is_reported(self):
        (self.root / "returncode").write_text("1\n")
        self.assertEqual(self.run_policy("on").returncode, 1)
        self.assertEqual(len(self.calls()), 1)

    def test_firmware_absence_stops_module_load(self):
        (self.root / "vendor").write_text("0x8086\n")
        self.assertEqual(self.run_policy("on").returncode, 2)
        self.assertEqual(self.calls(), [])


if __name__ == "__main__":
    unittest.main()
