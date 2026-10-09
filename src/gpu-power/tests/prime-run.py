#!/usr/bin/env python3
"""Exercise the shipped wrapper with fake GPU endpoints, never real hardware."""
import os
from pathlib import Path
import signal
import subprocess
import tempfile
import time
import unittest


class PrimeRun(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.root = Path(self.temp.name)
        self.log = self.root / "calls"
        self.users = self.root / "users"
        self.users.touch()
        self.node = self.root / "nvidia0"
        self.node.touch()
        source = Path(__file__).resolve().parents[1] / "prime-run"
        self.wrapper = self.root / "prime-run"
        self.wrapper.write_text(source.read_text().replace("/run/gpu-power.users", str(self.users))
                                .replace("/dev/nvidia0", str(self.node)))
        self.wrapper.chmod(0o755)
        fake = self.root / "gpu-power"
        fake.write_text('#!/bin/sh\necho "$1" >> "$TEST_LOG"\n'
                        '[ "$1" != use ] || exit "${USE_RC:-0}"\n')
        fake.chmod(0o755)
        self.env = dict(os.environ, PATH=str(self.root) + ":" + os.environ["PATH"],
                        TEST_LOG=str(self.log))

    def tearDown(self):
        self.temp.cleanup()

    def run_job(self, *args, **kwargs):
        return subprocess.run([str(self.wrapper), *args], env=self.env,
                              capture_output=True, text=True, timeout=10, **kwargs)

    def test_stdin_and_exit_status(self):
        result = self.run_job("sh", "-c", 'read value; echo "$value"; exit 7', input="hello\n")
        self.assertEqual(result.returncode, 7)
        self.assertEqual(result.stdout, "hello\n")
        self.assertEqual(self.log.read_text().splitlines(), ["status", "use", "tune", "release"])

    def test_descriptors_not_inherited(self):
        code = "import os; assert not os.path.exists('/proc/self/fd/8'); assert not os.path.exists('/proc/self/fd/9')"
        self.assertEqual(self.run_job("python3", "-c", code).returncode, 0)

    def test_igpu_fallback(self):
        self.env["USE_RC"] = "2"
        result = self.run_job("sh", "-c", 'echo "${__NV_PRIME_RENDER_OFFLOAD:-unset}"')
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, "unset\n")
        self.assertNotIn("tune", self.log.read_text().splitlines())

    def test_term_releases_lock(self):
        self.check_signal(signal.SIGTERM, 143)

    def test_interrupt_releases_lock(self):
        self.check_signal(signal.SIGINT, 130)

    def check_signal(self, sig, expected):
        ready = self.root / "ready"
        job = subprocess.Popen([str(self.wrapper), "sh", "-c",
                                'echo $$ > "$1"; exec sleep 30', "sh", str(ready)], env=self.env)
        try:
            deadline = time.monotonic() + 5
            while not ready.exists() and time.monotonic() < deadline:
                time.sleep(0.01)
            self.assertTrue(ready.exists())
            child = int(ready.read_text())
            job.send_signal(sig)
            self.assertEqual(job.wait(timeout=5), expected)
            self.assertFalse(Path(f"/proc/{child}").exists())
            self.assertEqual(self.log.read_text().splitlines()[-1], "release")
            check = subprocess.run(["flock", "-n", "-x", str(self.users), "true"])
            self.assertEqual(check.returncode, 0)
        finally:
            if job.poll() is None:
                job.kill()
                job.wait()


if __name__ == "__main__":
    unittest.main()
