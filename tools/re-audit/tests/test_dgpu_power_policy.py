"""Offline AML control-flow tests; never load AML into the host or target.

Run: python3 -m unittest discover -s tools/re-audit/tests -p test_dgpu_power_policy.py -v
Optional actual-source/DSDT integration check:
  T480_COREBOOT_ROOT=/path/to/coreboot python3 -m unittest discover \
    -s tools/re-audit/tests -p test_dgpu_power_policy.py -v

The small fixture executes the actual patched methods using ACPICA acpiexec,
with fake GPIO methods and ordinary Names replacing the PCI OperationRegion.
It proves sequencing and cleanup, not electrical timing or OS recovery.
"""

import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest


REPO = Path(__file__).resolve().parents[3]
PATCHES = REPO / "firmware/coreboot/patches"
CREATION_PATCH = PATCHES / "0012-local-mb-lenovo-sklkbl_thinkpad-dGPU-support-as-flas.patch"
FIX_PATCH = PATCHES / "0029-local-dgpu-abort-runtime-power-on-without-power-good.patch"
SOURCE_PATH = Path("src/mainboard/lenovo/sklkbl_thinkpad/acpi/dgpu.asl")
BASELINE_SHA256 = "1cd672880aca951c7801e4d7076472fc96e76fbb7ce7d72f8af1c27094201afa"
# T480 variant/dgpu.h: GPU_RST=GPP_E22, power=GPP_E23, PWRGD=GPP_F3.
# Selected soc/gpio_soc_defs.h defines E22=118 (line153), E23=119 (154),
# F3=123 (162). Actual-source integration below checks these bindings;
# build/config.h selects the non-PCH-H header (SKYLAKE_SOC_PCH_H=0).
GPIO = {"GPIO_GPU_RST": 118, "GPIO_1R8VIDEO_AON_ON": 119, "GPIO_DGFX_PWRGD": 123}


def baseline_source():
    marker = f"diff --git a/{SOURCE_PATH} b/{SOURCE_PATH}\n"
    section = CREATION_PATCH.read_text().split(marker, 1)[1].split("\ndiff --git ", 1)[0]
    source = "\n".join(line[1:] for line in section.splitlines()
                       if line.startswith("+") and not line.startswith("+++")) + "\n"
    if hashlib.sha256(source.encode()).hexdigest() != BASELINE_SHA256:
        raise AssertionError("Creation patch no longer reconstructs the audited baseline")
    return source


def run(command, cwd=None):
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True, timeout=15)
    if result.returncode:
        raise AssertionError(f"Command failed: {command!r}\n{result.stdout}\n{result.stderr}")
    return result.stdout + result.stderr


def resolve_gpio(source):
    source = re.sub(r"^#include .*\n", "", source, flags=re.M)
    for symbol, number in GPIO.items():
        source = re.sub(rf"\b{symbol}\b", str(number), source)
    return source


def block_end(source, start):
    opening = source.index("{", start)
    depth = 0
    for end in range(opening, len(source)):
        if source[end] == "{":
            depth += 1
        elif source[end] == "}":
            depth -= 1
            if not depth:
                return end + 1
    raise AssertionError("Unclosed ASL block")


def method(source, name):
    start = re.search(rf"\bMethod\s*\(\s*{name}\s*,", source).start()
    end = block_end(source, start)
    return start, end, source[start:end]


def tokens(source):
    return re.sub(r"\s+", "", re.sub(r"/\*.*?\*/", "", source, flags=re.S))


class DgpuPowerPolicyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not shutil.which("patch"):
            raise unittest.SkipTest("POSIX patch is required for exact patch application")
        cls.directory = tempfile.TemporaryDirectory(prefix="t480-dgpu-policy-")
        cls.work = Path(cls.directory.name)
        cls.baseline = baseline_source()
        source = cls.work / SOURCE_PATH
        source.parent.mkdir(parents=True)
        source.write_text(cls.baseline)
        run(["patch", "--batch", "--fuzz=0", "-p1", "-i", str(FIX_PATCH)], cls.work)
        cls.patched = source.read_text()

    @classmethod
    def tearDownClass(cls):
        cls.directory.cleanup()

    def test_exact_application_and_reverse_preserve_baseline(self):
        reverse = self.work / "reverse"
        source = reverse / SOURCE_PATH
        source.parent.mkdir(parents=True)
        source.write_text(self.patched)
        run(["patch", "--batch", "--fuzz=0", "-R", "-p1", "-i", str(FIX_PATCH)], reverse)
        self.assertEqual(source.read_text(), self.baseline)
        run(["patch", "--batch", "--fuzz=0", "-p1", "-i", str(FIX_PATCH)], reverse)
        self.assertEqual(source.read_text(), self.patched)

    def execute(self, *, patched=True, initial_power=0, initial_reset=0,
                initial_link_disabled=1, ready_at=1, drop_after=0):
        if not shutil.which("iasl") or not shutil.which("acpiexec"):
            self.skipTest("iasl and acpiexec are required for actual AML execution")
        source = resolve_gpio(self.patched if patched else self.baseline)
        # Only the PCI field backing is replaced; all actual DGON/DGOF/DGLW
        # control flow, GPIO calls, Return and Sleep opcodes are retained.
        region = re.search(r"OperationRegion\s*\(DGPC,", source).start()
        field = re.search(r"Field\s*\(DGPC,", source).start()
        end = block_end(source, field)
        source = (source[:region] + f"Name (LNKD, {initial_link_disabled})\nName (DLLA, One)"
                  + source[end:])
        fixture = r'''DefinitionBlock ("", "SSDT", 2, "T480", "DGPUTEST", 1)
{
    Scope (\_SB)
    {
        Device (PCI0)
        {
            Name (_ADR, Zero)
            Name (POWR, INITIAL_POWER)
            Name (RSET, INITIAL_RESET)
            Name (RCNT, Zero)
            Name (ONCT, Zero)
            Name (OFCT, Zero)
            Name (RLCT, Zero)
            Name (RSCT, Zero)
            Method (GTXS, 1, Serialized)
            {
                If (Arg0 == 119) { Return (POWR) }
                Return (RSET)
            }
            Method (STXS, 1, Serialized)
            {
                If (Arg0 == 119) { POWR = One; ONCT++ }
                Else { RSET = One; RLCT++ }
            }
            Method (CTXS, 1, Serialized)
            {
                If (Arg0 == 119) { POWR = Zero; OFCT++ }
                Else { RSET = Zero; RSCT++ }
            }
            Method (GRXS, 1, Serialized)
            {
                RCNT++
                If (DROP_AFTER && RCNT > DROP_AFTER) { Return (Zero) }
                If (READY_AT && RCNT >= READY_AT) { Return (One) }
                Return (Zero)
            }
            Device (RP01) { Name (_ADR, 0x001C0000) }
        }
    }
    ACTUAL_METHODS
    Method (MAIN, 0, Serialized)
    {
        \_SB.PCI0.RP01.DGON ()
        // Pack observable final states and helper call counts, without
        // injecting a return value into the production void DGON method.
        Return (\_SB.PCI0.POWR | (\_SB.PCI0.RSET << 1) |
                (\_SB.PCI0.RP01.LNKD << 2) | (\_SB.PCI0.ONCT << 8) |
                (\_SB.PCI0.OFCT << 16) | (\_SB.PCI0.RLCT << 24) |
                (\_SB.PCI0.RSCT << 32) | (\_SB.PCI0.RCNT << 40))
    }
}
'''
        for symbol, value in {"INITIAL_POWER": initial_power, "INITIAL_RESET": initial_reset,
                              "READY_AT": ready_at, "DROP_AFTER": drop_after}.items():
            fixture = fixture.replace(symbol, str(value))
        fixture = fixture.replace("ACTUAL_METHODS", source)
        with tempfile.TemporaryDirectory(prefix="aml-", dir=self.work) as directory:
            path = Path(directory) / "fixture.asl"
            path.write_text(fixture)
            run(["iasl", "-p", str(path.with_suffix("")), str(path)])
            output = run(["acpiexec", "-di", "-b", "evaluate \\MAIN", str(path.with_suffix(".aml"))])
        self.assertNotIn("AE_ERROR", output)
        self.assertNotIn("AE_AML", output)
        returned = re.search(r"\[Integer\]\s*=\s*([0-9A-Fa-f]+)", output)
        self.assertIsNotNone(returned, output)
        value = int(returned.group(1), 16)
        return {"power": value & 1, "reset_released": (value >> 1) & 1,
                "link_disabled": (value >> 2) & 1, "power_on_calls": (value >> 8) & 0xff,
                "power_off_calls": (value >> 16) & 0xff,
                "reset_release_calls": (value >> 24) & 0xff,
                "reset_assert_calls": (value >> 32) & 0xff,
                "power_good_reads": (value >> 40) & 0xff}

    def test_baseline_exposes_the_failed_rail_bug(self):
        result = self.execute(patched=False, ready_at=0)
        self.assertEqual((result["power"], result["reset_released"], result["link_disabled"]), (1, 1, 0))

    def test_perpetual_low_powers_down_without_releasing_reset(self):
        result = self.execute(ready_at=0)
        self.assertEqual((result["power"], result["reset_released"], result["link_disabled"]), (0, 0, 1))
        self.assertEqual(result["reset_release_calls"], 0)
        self.assertEqual(result["reset_assert_calls"], 1)
        self.assertEqual(result["power_off_calls"], 1)
        self.assertGreaterEqual(result["power_good_reads"], 100)
        self.assertLessEqual(result["power_good_reads"], 102)

    def test_timeout_disables_link_and_asserts_an_initially_released_reset(self):
        result = self.execute(ready_at=0, initial_reset=1, initial_link_disabled=0)
        self.assertEqual((result["power"], result["reset_released"], result["link_disabled"]), (0, 0, 1))
        self.assertEqual(result["reset_release_calls"], 0)

    def test_immediate_power_good_preserves_success_path(self):
        result = self.execute(ready_at=1)
        self.assertEqual((result["power"], result["reset_released"], result["link_disabled"]), (1, 1, 0))
        self.assertEqual(result["power_off_calls"], 0)
        self.assertEqual(result["reset_release_calls"], 1)

    def test_ready_before_wait_boundary_preserves_success_path(self):
        result = self.execute(ready_at=100)
        self.assertEqual((result["power"], result["reset_released"], result["link_disabled"]), (1, 1, 0))
        self.assertEqual(result["power_off_calls"], 0)

    def test_ready_on_final_recheck_can_proceed(self):
        result = self.execute(ready_at=102)
        self.assertEqual((result["power"], result["reset_released"], result["link_disabled"]), (1, 1, 0))
        self.assertEqual(result["power_good_reads"], 102)

    def test_lost_power_good_on_final_recheck_aborts(self):
        result = self.execute(ready_at=1, drop_after=1)
        self.assertEqual((result["power"], result["reset_released"], result["link_disabled"]), (0, 0, 1))
        self.assertEqual(result["reset_release_calls"], 0)

    def test_already_powered_path_remains_no_op(self):
        result = self.execute(initial_power=1, initial_reset=1, initial_link_disabled=0, ready_at=0)
        self.assertEqual((result["power"], result["reset_released"], result["link_disabled"]), (1, 1, 0))
        for key in ("power_on_calls", "power_off_calls", "reset_release_calls", "reset_assert_calls", "power_good_reads"):
            self.assertEqual(result[key], 0)

    def test_actual_baseline_and_preprocessed_dsdt_compile(self):
        root = os.environ.get("T480_COREBOOT_ROOT")
        if not root:
            self.skipTest("Set T480_COREBOOT_ROOT for existing local DSDT integration")
        if not shutil.which("iasl"):
            self.skipTest("iasl is required for DSDT integration compilation")
        root = Path(root)
        self.assertEqual((root / SOURCE_PATH).read_text(), self.baseline)
        config = (root / "build/config.h").read_text()
        self.assertRegex(config, r"(?m)^#define CONFIG_SKYLAKE_SOC_PCH_H 0$")
        variant = (root / "src/mainboard/lenovo/sklkbl_thinkpad/variants/t480/include/variant/dgpu.h").read_text()
        pads = (root / "src/soc/intel/skylake/include/soc/gpio_soc_defs.h").read_text()
        for symbol, pad in (("GPIO_GPU_RST", "GPP_E22"), ("GPIO_1R8VIDEO_AON_ON", "GPP_E23"),
                            ("GPIO_DGFX_PWRGD", "GPP_F3")):
            self.assertRegex(variant, rf"(?m)^#define\s+{symbol}\s+{pad}\b")
            self.assertRegex(pads, rf"(?m)^#define\s+{pad}\s+{GPIO[symbol]}\b")
        actual = (root / "build/dsdt.asl").read_text()
        begin, end, old_method = method(actual, "DGON")
        _, _, expected = method(resolve_gpio(self.baseline), "DGON")
        self.assertEqual(tokens(old_method), tokens(expected))
        _, _, new_method = method(resolve_gpio(self.patched), "DGON")
        changed = actual[:begin] + new_method + actual[end:]
        # Compile both complete preprocessed T480 tables; no other source,
        # build tree or installed AML is modified or executed.
        diagnostics = []
        for name, source in (("actual-baseline", actual), ("actual-patched", changed)):
            path = self.work / f"{name}.asl"
            path.write_text(source)
            output = run(["iasl", "-p", str(path.with_suffix("")), str(path)])
            self.assertRegex(output, r"Compilation successful\. 0 Errors")
            summary = re.search(r"Compilation successful\. (\d+) Errors, (\d+) Warnings, (\d+) Remarks", output)
            self.assertIsNotNone(summary, output)
            diagnostics.append(summary.groups())
        self.assertEqual(diagnostics[0], diagnostics[1], "Patch introduced DSDT compiler diagnostics")


if __name__ == "__main__":
    unittest.main()
