"""Exercise the actual hook against temporary files and mocked NVIDIA/VT tools.

No device, service, NVIDIA helper or real sleep operation is used. One test uses
GNU timeout against a busy mock process to check signal/return-code lifecycle.
"""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


HOOK = Path(__file__).resolve().parents[3] / "system/etc/elogind/system-sleep/nvidia"


class NvidiaSleepHookTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="nvidia-hook-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.events = self.root / "events"
        self.proc = self.root / "proc-suspend"
        self.proc.touch()
        self.vtf = self.root / "Xorg.vt_number"
        self.vtf.write_text("7\n")
        self.helper = self.bin / "nvidia-sleep.sh"
        self.write_tool("nvidia-sleep.sh", """#!/bin/sh
printf 'helper:start:%s\n' "$1" >> "$EVENTS"
if [ "$MOCK_BUSY_RESUME" = 1 ]; then
    trap 'printf "helper:terminated\\n" >> "$EVENTS"; exit 143' TERM
    while :; do :; done
fi
printf 'helper:end:%s\n' "$1" >> "$EVENTS"
exit "${MOCK_HELPER_RC:-0}"
""")
        self.write_tool("logger", """#!/bin/sh
printf 'logger:%s\n' "$*" >> "$EVENTS"
exit "${MOCK_LOGGER_RC:-0}"
""")
        self.write_tool("chvt", """#!/bin/sh
printf 'chvt:%s\n' "$*" >> "$EVENTS"
exit "${MOCK_CHVT_RC:-0}"
""")
        self.write_tool("timeout", """#!/bin/sh
printf 'timeout:limit:%s\n' "$1" >> "$EVENTS"
if [ -n "$MOCK_TIMEOUT_RC" ]; then
    exit "$MOCK_TIMEOUT_RC"
fi
shift
"$@"
rc=$?
printf 'timeout:end:%s\n' "$rc" >> "$EVENTS"
exit "$rc"
""")

    def write_tool(self, name, source):
        path = self.bin / name
        path.write_text(source)
        path.chmod(0o755)

    def invoke(self, phase, action, *, sleep_phase=None, loaded=True,
               real_timeout=False, **values):
        # Rewrite only the fixture's fixed paths, never the production source.
        source = HOOK.read_text().replace("/proc/driver/nvidia/suspend", str(self.proc))
        source = source.replace("/var/run/nvidia-sleep/Xorg.vt_number", str(self.vtf))
        source = source.replace("/usr/bin/nvidia-sleep.sh", str(self.helper))
        if real_timeout:
            timeout = shutil.which("timeout", path=os.defpath)
            if not timeout:
                self.skipTest("GNU timeout is unavailable")
            source = source.replace("timeout 30 ", f"{timeout} 0.2 ")
        fixture = self.root / "hook"
        fixture.write_text(source)
        if not loaded:
            self.proc.unlink(missing_ok=True)
        self.events.write_text("")
        environment = dict(os.environ)
        environment.pop("SYSTEMD_SLEEP_ACTION", None)
        environment.update(PATH=f"{self.bin}:{os.defpath}", EVENTS=str(self.events),
                           MOCK_HELPER_RC="0", MOCK_LOGGER_RC="0", MOCK_CHVT_RC="0",
                           MOCK_TIMEOUT_RC="", MOCK_BUSY_RESUME="0")
        environment.update({key: str(value) for key, value in values.items()})
        if sleep_phase is not None:
            environment["SYSTEMD_SLEEP_ACTION"] = sleep_phase
        result = subprocess.run(["/bin/sh", str(fixture), phase, action],
                                env=environment, capture_output=True, text=True, timeout=5)
        return result, self.events.read_text().splitlines()

    def test_pre_dispatch_includes_both_suspend_then_hibernate_phases(self):
        cases = [("suspend", None, "suspend"), ("hibernate", None, "hibernate"),
                 ("hybrid-sleep", None, "hibernate"),
                 ("suspend-then-hibernate", "suspend", "suspend"),
                 ("suspend-then-hibernate", "hibernate", "hibernate"),
                 ("suspend-then-hibernate", None, "suspend")]
        for action, phase, expected in cases:
            with self.subTest(action=action, phase=phase):
                result, events = self.invoke("pre", action, sleep_phase=phase)
                self.assertEqual(result.returncode, 0)
                self.assertIn(f"helper:start:{expected}", events)
                self.assertNotIn("timeout:limit:30", events)
                self.assertTrue(self.vtf.exists())

    def test_pre_failure_returns_actual_status_and_logs_it(self):
        result, events = self.invoke("pre", "hibernate", MOCK_HELPER_RC=37)
        self.assertEqual(result.returncode, 37)
        self.assertIn("logger:-t nvidia-sleep pre hibernate: hibernate failed (rc 37)", events)
        self.assertTrue(self.vtf.exists())

    def test_logger_failure_cannot_mask_pre_helper_success_or_failure(self):
        for status in (0, 37):
            with self.subTest(status=status):
                result, events = self.invoke("pre", "suspend", MOCK_HELPER_RC=status,
                                             MOCK_LOGGER_RC=23)
                self.assertEqual(result.returncode, status)
                self.assertIn("helper:end:suspend", events)

    def test_unloaded_driver_skips_pre_and_post_without_cleanup(self):
        for phase in ("pre", "post"):
            with self.subTest(phase=phase):
                result, events = self.invoke(phase, "hibernate", loaded=False)
                self.assertEqual(result.returncode, 0)
                self.assertEqual(events, [])
                self.assertEqual(self.vtf.read_text(), "7\n")

    def test_post_waits_for_resume_and_preserves_helper_owned_success_cleanup(self):
        result, events = self.invoke("post", "suspend-then-hibernate", sleep_phase="hibernate")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(events, ["timeout:limit:30", "helper:start:resume",
                                  "helper:end:resume", "timeout:end:0"])
        self.assertTrue(self.vtf.exists())

    def test_post_failure_restores_vt_and_preserves_actual_helper_code(self):
        result, events = self.invoke("post", "suspend", MOCK_HELPER_RC=73)
        self.assertEqual(result.returncode, 73)
        self.assertIn("logger:-t nvidia-sleep post suspend: resume failed (rc 73)", events)
        self.assertLess(events.index("helper:end:resume"), events.index("chvt:7"))
        self.assertFalse(self.vtf.exists())

    def test_timeout_reports_124_and_runs_vt_cleanup(self):
        result, events = self.invoke("post", "hibernate", MOCK_TIMEOUT_RC=124)
        self.assertEqual(result.returncode, 124)
        self.assertIn("logger:-t nvidia-sleep post hibernate: resume failed or timed out (30 s limit; rc 124)", events)
        self.assertNotIn("helper:start:resume", events)
        self.assertIn("chvt:7", events)
        self.assertFalse(self.vtf.exists())

    def test_other_timeout_error_is_not_reported_as_elapsed_30_seconds(self):
        result, events = self.invoke("post", "suspend", MOCK_TIMEOUT_RC=137)
        self.assertEqual(result.returncode, 137)
        self.assertIn("logger:-t nvidia-sleep post suspend: resume failed (rc 137)", events)
        self.assertFalse(any("timed out" in event for event in events))

    def test_helper_124_does_not_fabricate_elapsed_timeout_duration(self):
        result, events = self.invoke("post", "suspend", MOCK_HELPER_RC=124)
        self.assertEqual(result.returncode, 124)
        self.assertIn("helper:end:resume", events)
        self.assertIn("logger:-t nvidia-sleep post suspend: resume failed or timed out (30 s limit; rc 124)", events)
        self.assertFalse(any("after 30 s" in event for event in events))

    def test_cleanup_and_logger_failures_do_not_replace_resume_failure(self):
        result, events = self.invoke("post", "suspend", MOCK_HELPER_RC=73,
                                     MOCK_CHVT_RC=4, MOCK_LOGGER_RC=23)
        self.assertEqual(result.returncode, 73)
        self.assertIn("logger:-t nvidia-sleep post suspend: VT restore failed (rc 4); resume rc 73", events)
        self.assertFalse(self.vtf.exists())

    def test_failed_resume_without_vt_record_has_no_vt_command(self):
        self.vtf.unlink()
        result, events = self.invoke("post", "hibernate", MOCK_HELPER_RC=73)
        self.assertEqual(result.returncode, 73)
        self.assertFalse(any(event.startswith("chvt:") for event in events))

    def test_missing_helper_error_is_preserved(self):
        self.helper.unlink()
        result, events = self.invoke("pre", "suspend")
        self.assertEqual(result.returncode, 127)
        self.assertIn("logger:-t nvidia-sleep pre suspend: suspend failed (rc 127)", events)

    def test_unknown_phase_remains_an_inert_success(self):
        result, events = self.invoke("unrecognized", "suspend")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(events, [])

    def test_real_timeout_stops_mock_resume_before_vt_cleanup(self):
        result, events = self.invoke("post", "suspend", real_timeout=True,
                                     MOCK_BUSY_RESUME=1)
        self.assertEqual(result.returncode, 124)
        self.assertIn("helper:terminated", events)
        self.assertIn("chvt:7", events)
        self.assertLess(events.index("helper:terminated"), events.index("chvt:7"))
        self.assertFalse(self.vtf.exists())


if __name__ == "__main__":
    unittest.main()
