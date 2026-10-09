#!/usr/bin/env python3
"""Check the firmware scenario runner's verdict/exit contract without QEMU."""
import contextlib
import importlib.util
import io
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch


SOURCE = Path(__file__).resolve().parents[1] / "qtest52.py"
SPEC = importlib.util.spec_from_file_location("qtest52", SOURCE)
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class VerdictTests(unittest.TestCase):
    def verdict(self, selectors, results):
        output, errors = io.StringIO(), io.StringIO()
        with patch.object(MODULE, "run", side_effect=results) as run:
            with contextlib.redirect_stdout(output), contextlib.redirect_stderr(errors):
                status = MODULE.main(selectors)
        return status, output.getvalue(), errors.getvalue(), run

    def test_all_scenarios_pass(self):
        status, output, errors, run = self.verdict([], [True] * len(MODULE.T))
        self.assertEqual(status, 0)
        self.assertEqual(output, "ALL PASS\n")
        self.assertEqual(errors, "")
        self.assertEqual(run.call_count, 13)

    def test_selected_failure_has_nonzero_verdict(self):
        status, output, _, run = self.verdict(["S1"], [False])
        self.assertEqual(status, 1)
        self.assertIn("FAILURES: S1 corrupted sig no key", output)
        self.assertEqual(run.call_count, 1)

    def test_mixed_results_report_every_failure(self):
        status, output, _, run = self.verdict(["S1", "S2", "S3"], [False, True, False])
        self.assertEqual(status, 1)
        self.assertIn("S1 corrupted sig no key", output)
        self.assertIn("S3 tampered kernel no key", output)
        self.assertNotIn("S2 missing sig no key", output)
        self.assertEqual(run.call_count, 3)

    def test_unknown_selector_cannot_report_an_empty_success(self):
        status, output, errors, run = self.verdict(["S100"], [])
        self.assertEqual(status, 2)
        self.assertEqual(output, "")
        self.assertIn("S100", errors)
        run.assert_not_called()

    def test_duplicate_selector_runs_once(self):
        status, output, _, run = self.verdict(["S2", "S2"], [True])
        self.assertEqual(status, 0)
        self.assertEqual(output, "ALL PASS\n")
        self.assertEqual(run.call_count, 1)

    def test_program_entrypoint_propagates_failure(self):
        # Execute the real __main__ block. Inject the scenario result rather
        # than starting a VM, then require the surrounding process to fail.
        code = """
from pathlib import Path
import sys
source = Path(sys.argv[1]).read_text()
source = source.replace('sys.exit(main())', 'run = lambda *args: False; sys.exit(main())')
sys.argv = [sys.argv[1], 'S1']
exec(compile(source, sys.argv[0], 'exec'), {'__name__': '__main__'})
"""
        result = subprocess.run([sys.executable, "-c", code, str(SOURCE)],
                                capture_output=True, text=True, timeout=5)
        self.assertEqual(result.returncode, 1)
        self.assertIn("FAILURES: S1", result.stdout)


if __name__ == "__main__":
    unittest.main()
