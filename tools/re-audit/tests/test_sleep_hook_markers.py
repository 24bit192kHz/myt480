"""Marker handling of the lid-wake and btusb sleep hooks on their failure paths.

Both hooks remember in a marker file under /run that `pre` changed something
which `post` must undo. The marker may only go once the undo has succeeded;
otherwise a failed undo is never retried. The actual hooks run against
temporary files and mocked ectool/modprobe/logger; fixed paths are rewritten
in a copy, never in the production source. No EC, module or sleep is touched.
"""
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest


HOOKS = Path(__file__).resolve().parents[3] / "system/etc/elogind/system-sleep"


class HookCase(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix="sleep-hook-")
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.bin = self.root / "bin"
        self.bin.mkdir()
        self.events = self.root / "events"
        self.events.write_text("")
        self.tool("logger", '#!/bin/sh\nprintf "logger:%s\\n" "$*" >> "$EVENTS"\n')

    def tool(self, name, source):
        path = self.bin / name
        path.write_text(source)
        path.chmod(0o755)

    def run_hook(self, fixture, phase, action, **values):
        environment = {"PATH": f"{self.bin}:{os.defpath}", "EVENTS": str(self.events)}
        environment.update({key: str(value) for key, value in values.items()})
        return subprocess.run(["/bin/sh", str(fixture), phase, action], env=environment,
                              capture_output=True, text=True, timeout=10)

    def log(self):
        return self.events.read_text()

    def wait_for(self, text, seconds=5.0):
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            if text in self.log():
                return
            time.sleep(0.02)
        self.fail(f"{text!r} not logged; events:\n{self.log()}")


class LidWakeMarkerTests(HookCase):
    def setUp(self):
        super().setUp()
        self.ec = self.root / "ec01"
        self.ec.write_text("05")
        self.mark = self.root / "lid-wake-s4.was-clear"
        self.tool("ectool", f"""#!/bin/sh
state={self.ec}
case "$1" in
-d) [ "$MOCK_EC_READ_FAIL" = 1 ] && exit 1
    printf '00: a6 %s 00 00\\n' "$(cat "$state")" ;;
-w) [ "$MOCK_EC_WRITE_FAIL" = 1 ] && exit 1
    printf '%02x' "$4" > "$state" ;;
esac
""")
        source = (HOOKS / "03-lid-wake-s4").read_text()
        for old, new in (
                ("PATH=/usr/local/sbin:/usr/local/bin:/usr/bin:/bin", f"PATH={self.bin}:/usr/bin:/bin"),
                ("ECTOOL=/usr/local/sbin/ectool", f"ECTOOL={self.bin}/ectool"),
                ("MARK=/run/lid-wake-s4.was-clear", f"MARK={self.mark}"),
                ("/etc/elogind/no-lid-wake-s4", str(self.root / "no-lid-wake-s4"))):
            self.assertIn(old, source)
            source = source.replace(old, new)
        self.fixture = self.root / "hook"
        self.fixture.write_text(source)

    def hook(self, phase, **values):
        return self.run_hook(self.fixture, phase, "hibernate", **values)

    def test_pre_sets_the_bit_and_post_clears_it(self):
        self.hook("pre")
        self.assertEqual(self.ec.read_text(), "45")
        self.assertTrue(self.mark.exists())
        self.hook("post")
        self.assertEqual(self.ec.read_text(), "05")
        self.assertFalse(self.mark.exists())

    def test_bit_already_set_is_left_alone(self):
        self.ec.write_text("45")
        self.hook("pre")
        self.assertFalse(self.mark.exists())
        self.hook("post")
        self.assertEqual(self.ec.read_text(), "45")

    def test_unreadable_ec_in_post_keeps_the_marker_for_a_retry(self):
        self.hook("pre")
        self.hook("post", MOCK_EC_READ_FAIL=1)
        self.assertTrue(self.mark.exists(), "marker dropped although the bit is still set")
        self.assertEqual(self.ec.read_text(), "45")
        self.assertIn("unreadable", self.log())
        self.hook("post")
        self.assertEqual(self.ec.read_text(), "05")
        self.assertFalse(self.mark.exists())

    def test_failed_write_in_post_keeps_the_marker_for_a_retry(self):
        self.hook("pre")
        self.hook("post", MOCK_EC_WRITE_FAIL=1)
        self.assertTrue(self.mark.exists())
        self.assertEqual(self.ec.read_text(), "45")
        self.assertIn("EC write failed", self.log())
        self.hook("post")
        self.assertFalse(self.mark.exists())


class BtusbMarkerTests(HookCase):
    def setUp(self):
        super().setUp()
        self.module = self.root / "module-btusb"
        self.rfkill = self.root / "rfkill" / "rfkill3"
        self.unloaded = self.root / "btusb-hibernate.unloaded"
        self.tool("setsid", '#!/bin/sh\nexec "$@"\n')
        self.tool("sleep", "#!/bin/sh\nexit 0\n")
        self.tool("modprobe", f"""#!/bin/sh
printf 'modprobe:%s\\n' "$*" >> "$EVENTS"
if [ "$1" = -r ]; then
    rm -rf {self.module} {self.rfkill}
    exit 0
fi
[ "${{MOCK_MODPROBE_RC:-0}}" -eq 0 ] || exit "$MOCK_MODPROBE_RC"
mkdir -p {self.module} {self.rfkill}
echo hci0 > {self.rfkill}/name
echo 0 > {self.rfkill}/soft
""")
        self.module.mkdir()
        self.rfkill.mkdir(parents=True)
        (self.rfkill / "name").write_text("hci0\n")
        (self.rfkill / "soft").write_text("0\n")
        source = (HOOKS / "04-btusb-hibernate").read_text()
        for old, new in (("/run/btusb-hibernate.", f"{self.root}/btusb-hibernate."),
                         ("/sys/class/rfkill/rfkill*", f"{self.root}/rfkill/rfkill*"),
                         ("/sys/module/btusb", str(self.module))):
            self.assertIn(old, source)
            source = source.replace(old, new)
        self.fixture = self.root / "hook"
        self.fixture.write_text(source)

    def hook(self, phase, **values):
        return self.run_hook(self.fixture, phase, "hibernate", **values)

    def test_unload_and_reload(self):
        self.hook("pre")
        self.assertTrue(self.unloaded.exists())
        self.assertFalse(self.module.exists())
        self.hook("post")
        self.wait_for("btusb loaded again")
        self.assertTrue(self.module.exists())
        self.assertFalse(self.unloaded.exists())

    def test_failed_reload_is_logged_and_retried_by_the_next_post(self):
        self.hook("pre")
        self.hook("post", MOCK_MODPROBE_RC=1)
        self.wait_for("modprobe btusb failed")
        self.assertTrue(self.unloaded.exists(), "marker dropped although btusb is still unloaded")
        self.assertFalse(self.module.exists())
        self.hook("post")
        self.wait_for("btusb loaded again")
        self.assertFalse(self.unloaded.exists())


if __name__ == "__main__":
    unittest.main()
