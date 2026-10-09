"""Compile actual coreboot CBFS loader/allocator code with ASAN and UBSAN.

The exact GPL baseline is cbfs.c from c57exp 84126e3. Controlled stubs provide
storage, allocation, hashes and decoder results; the real loader/allocator
decision logic is extracted from baseline/patched source. No actual decoder,
firmware, CBFS image, TPM, PCI, target machine or other device is accessed.
T480_COREBOOT_ROOT optionally checks the existing baseline byte-for-byte.
"""

import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest


REPO = Path(__file__).resolve().parents[3]
SOURCE_PATH = Path("src/lib/cbfs.c")
BASELINE = Path(__file__).with_name("fixtures") / "coreboot_cbfs_84126e3.c"
HARNESS = Path(__file__).with_name("cbfs_size_fixture.c")
PATCH = REPO / "firmware/coreboot/patches/0031-local-cbfs-report-actual-loaded-size.patch"
BASELINE_SHA256 = "214706ef48ab64cffc21338ab3e5f9029bed2ded72c66983526da176957c36d3"


def function(source, name):
    match = re.search(rf"(?m)^(?:static )?[^\n;]*\b{name}\s*\(", source)
    if match is None:
        raise AssertionError(f"Missing actual source definition: {name}")
    start = match.start()
    opening = source.index("{", match.end())
    depth = 0
    for end in range(opening, len(source)):
        if source[end] == "{":
            depth += 1
        elif source[end] == "}":
            depth -= 1
            if not depth:
                return source[start:end + 1]
    raise AssertionError(f"Unclosed actual source definition: {name}")


def run(command, cwd=None, env=None):
    result = subprocess.run(command, cwd=cwd, env=env, capture_output=True, text=True, timeout=20)
    if result.returncode:
        raise AssertionError(f"Command failed: {command!r}\n{result.stdout}\n{result.stderr}")
    return result


class CbfsSizeContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        compiler = os.environ.get("CC") or shutil.which("clang") or shutil.which("cc")
        if not compiler or not shutil.which("patch"):
            raise unittest.SkipTest("A host C compiler and patch are required")
        if hashlib.sha256(BASELINE.read_bytes()).hexdigest() != BASELINE_SHA256:
            raise AssertionError("Audited GPL baseline fixture changed")
        cls.baseline = BASELINE.read_text()
        cls.directory = tempfile.TemporaryDirectory(prefix="t480-cbfs-size-")
        cls.work = Path(cls.directory.name)
        source = cls.work / SOURCE_PATH
        source.parent.mkdir(parents=True)
        source.write_text(cls.baseline)
        run(["patch", "--batch", "--fuzz=0", "-p1", "-i", str(PATCH)], cls.work)
        cls.patched = source.read_text()
        cls.sanitizer_env = os.environ.copy()
        cls.sanitizer_env["ASAN_OPTIONS"] = "detect_leaks=1:halt_on_error=1"
        cls.sanitizer_env["UBSAN_OPTIONS"] = "halt_on_error=1:print_stacktrace=1"
        cls.executables = {}
        names = ["cbfs_load_and_decompress", "do_alloc", "check_or_query_type", "_cbfs_alloc"]
        for label, actual in (("baseline", cls.baseline), ("patched", cls.patched)):
            definitions = "\n\n".join(function(actual, name) for name in names)
            host_source = HARNESS.read_text().replace("/* SOURCE_FUNCTIONS */", definitions)
            path = cls.work / f"{label}.c"
            path.write_text(host_source)
            executable = cls.work / label
            run([compiler, "-std=gnu11", "-O1", "-g", "-Wall", "-Wextra", "-Werror",
                 "-fsanitize=address,undefined", "-fno-sanitize-recover=all",
                 "-fno-omit-frame-pointer", str(path), "-o", str(executable)])
            cls.executables[label] = executable

    @classmethod
    def tearDownClass(cls):
        cls.directory.cleanup()

    def scenario(self, name, label="patched"):
        result = run([str(self.executables[label]), name], env=self.sanitizer_env)
        self.assertNotIn("runtime error:", result.stderr)
        self.assertNotIn("AddressSanitizer", result.stderr)
        return json.loads(result.stdout)

    def failed(self, *names):
        for name in names:
            with self.subTest(name=name):
                result = self.scenario(name)
                self.assertFalse(result["ok"])
                self.assertEqual(result["reported"], 0)

    def test_patch_applies_and_reverses_exact_baseline(self):
        reverse = self.work / "reverse"
        source = reverse / SOURCE_PATH
        source.parent.mkdir(parents=True)
        source.write_text(self.patched)
        run(["patch", "--batch", "--fuzz=0", "-R", "-p1", "-i", str(PATCH)], reverse)
        self.assertEqual(source.read_text(), self.baseline)

    def test_baseline_reports_capacity_instead_of_produced_bytes(self):
        result = self.scenario("compressed-short-cache", "baseline")
        self.assertTrue(result["ok"])
        self.assertEqual((result["reported"], result["produced"]), (1024, 512))

    def test_baseline_none_attribute_overreports_raw_mapping_and_hash_extent(self):
        result = self.scenario("none-inflated", "baseline")
        self.assertTrue(result["ok"])
        self.assertEqual((result["reported"], result["hash_size"], result["raw_size"]),
                         (1024, 1024, 512))

    def test_baseline_accepts_nonzero_size_t_decoder_error(self):
        result = self.scenario("zstd-size-error", "baseline")
        self.assertTrue(result["ok"])
        self.assertGreater(result["produced"], result["allocation_size"])
        self.assertEqual(result["reported"], 1024)

    def test_direct_map_uses_raw_extent_with_or_without_none_attribute(self):
        for name in ("direct-valid", "none-inflated", "none-undersized", "none-zero-attribute"):
            with self.subTest(name=name):
                result = self.scenario(name)
                self.assertTrue(result["ok"])
                self.assertEqual((result["reported"], result["hash_size"]), (512, 512))
                self.assertEqual((result["allocation_calls"], result["decode_calls"]), (0, 0))

    def test_none_custom_allocator_receives_raw_extent(self):
        for name in ("custom-none-inflated", "custom-none-undersized"):
            with self.subTest(name=name):
                result = self.scenario(name)
                self.assertTrue(result["ok"])
                self.assertEqual((result["reported"], result["allocation_size"]), (512, 512))
                self.assertEqual((result["custom_calls"], result["read_calls"]), (1, 1))

    def test_compressed_cache_and_custom_keep_capacity_but_publish_short_output(self):
        for name in ("compressed-short-cache", "compressed-short-custom", "lzma-short"):
            with self.subTest(name=name):
                result = self.scenario(name)
                self.assertTrue(result["ok"])
                self.assertEqual((result["allocation_size"], result["decoder_capacity"]), (1024, 1024))
                self.assertEqual(result["reported"], 512)
                self.assertEqual((result["maps"], result["unmaps"]), (1, 1))
        self.assertEqual(self.scenario("compressed-short-custom")["custom_calls"], 1)

    def test_compressed_exact_output_retains_normal_behavior(self):
        result = self.scenario("compressed-exact")
        self.assertTrue(result["ok"])
        self.assertEqual((result["reported"], result["allocation_size"]), (1024, 1024))

    def test_zero_and_overcapacity_decoder_results_do_not_publish_success(self):
        self.failed("compressed-zero", "compressed-over-capacity", "zstd-size-error")
        for name in ("compressed-zero", "compressed-over-capacity", "zstd-size-error"):
            with self.subTest(name=name):
                result = self.scenario(name)
                self.assertEqual((result["decode_calls"], result["unmaps"]), (1, 1))

    def test_lookup_and_type_failures_reset_stale_output_before_allocating(self):
        self.failed("lookup-failure", "type-failure")
        for name in ("lookup-failure", "type-failure"):
            with self.subTest(name=name):
                result = self.scenario(name)
                self.assertEqual((result["maps"], result["allocation_calls"]), (0, 0))

    def test_map_and_hash_failures_preserve_mapping_cleanup(self):
        self.failed("direct-map-failure", "compressed-map-failure",
                    "direct-hash-failure", "compressed-hash-failure", "custom-hash-failure")
        self.assertEqual(self.scenario("direct-map-failure")["unmaps"], 0)
        self.assertEqual(self.scenario("direct-hash-failure")["unmaps"], 1)
        result = self.scenario("compressed-hash-failure")
        self.assertEqual((result["unmaps"], result["decode_calls"]), (1, 0))
        self.assertEqual(self.scenario("custom-hash-failure")["unmaps"], 0)

    def test_unavailable_cache_and_allocator_failures_report_zero(self):
        self.failed("cache-unavailable", "cache-allocation-failure", "custom-allocation-failure")
        self.assertEqual(self.scenario("cache-unavailable")["allocation_calls"], 0)
        self.assertEqual(self.scenario("custom-allocation-failure")["custom_calls"], 1)

    def test_raw_read_failure_reports_zero(self):
        self.failed("raw-read-failure")
        result = self.scenario("raw-read-failure")
        self.assertEqual((result["read_calls"], result["hash_calls"]), (1, 0))

    def test_zero_length_behavior_is_preserved(self):
        result = self.scenario("direct-empty")
        self.assertTrue(result["ok"])
        self.assertEqual(result["reported"], 0)
        self.failed("custom-empty", "compressed-empty-capacity")

    def test_null_optional_output_and_type_query_are_supported(self):
        result = self.scenario("null-output")
        self.assertTrue(result["ok"])
        self.assertEqual(result["reported"], 777)
        result = self.scenario("query-type")
        self.assertTrue(result["ok"])
        self.assertEqual((result["type"], result["reported"]), (0x50, 512))

    def test_preload_cleanup_and_force_ro_dispatch_are_unchanged(self):
        result = self.scenario("preload-short")
        self.assertTrue(result["ok"])
        self.assertEqual((result["reported"], result["maps"], result["unmaps"]), (512, 2, 2))
        self.failed("preload-decode-failure")
        result = self.scenario("preload-decode-failure")
        self.assertEqual((result["maps"], result["unmaps"]), (2, 2))
        result = self.scenario("force-ro")
        self.assertTrue(result["ok"])
        self.assertEqual(result["preload_calls"], 0)

    def test_optional_existing_coreboot_baseline_matches(self):
        root = os.environ.get("T480_COREBOOT_ROOT")
        if not root:
            self.skipTest("Set T480_COREBOOT_ROOT to validate the existing actual source")
        self.assertEqual((Path(root) / SOURCE_PATH).read_bytes(), BASELINE.read_bytes())


if __name__ == "__main__":
    unittest.main()
