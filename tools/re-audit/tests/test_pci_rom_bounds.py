"""Compile the actual patched coreboot probe/walker/SSDT code with ASAN/UBSAN.

Run: PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover \
  -s tools/re-audit/tests -p test_pci_rom_bounds.py -v

The GPL baseline fixture is byte-for-byte src/device/pci_rom.c from local
coreboot c57exp 84126e3fc1e4129f5be9af44d633736cea68d169. Patch 0029 changes
another source file. Test stubs supply allocated CBFS buffers and collect
CBMEM/_ROM copies; no host PCI or firmware access exists. Optional
T480_COREBOOT_ROOT validates the existing source/header baseline too.
T480_MX150_VBIOS can select another full ROM; by default the kit's own
firmware/coreboot/site-local/data/mx150-vbios.rom is used. The synthetic
fixtures below do not embed it.
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
BASELINE = Path(__file__).with_name("fixtures") / "coreboot_pci_rom_84126e3.c"
HARNESS = Path(__file__).with_name("pci_rom_fixture.c")
PATCH = REPO / "firmware/coreboot/patches/0030-local-pci-rom-bound-cbfs-probe-and-acpi-copy.patch"
SOURCE_PATH = Path("src/device/pci_rom.c")
BASELINE_SHA256 = "9770b6345ab1d15e74cb43e06691a0dbc5080ed6c874c7dbcb4fd92ebc025b24"
MX150_SHA256 = "92936e91fbb591086473366fa6a73c40ec2450e58a90489fba83ce4b58803d82"


def function(source, name):
    """Extract an actual definition, not a separately maintained test model."""
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
    raise AssertionError(f"Unclosed actual definition: {name}")


def run(command, cwd=None, env=None):
    result = subprocess.run(command, cwd=cwd, env=env, capture_output=True, text=True, timeout=20)
    if result.returncode:
        raise AssertionError(f"Command failed: {command!r}\n{result.stdout}\n{result.stderr}")
    return result


class PciRomBoundsTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        compiler = os.environ.get("CC") or shutil.which("clang") or shutil.which("cc")
        if not compiler or not shutil.which("patch"):
            raise unittest.SkipTest("A host C compiler and patch are required")
        baseline = BASELINE.read_text()
        if hashlib.sha256(BASELINE.read_bytes()).hexdigest() != BASELINE_SHA256:
            raise AssertionError("Audited GPL baseline fixture changed")
        cls.directory = tempfile.TemporaryDirectory(prefix="t480-pci-rom-")
        cls.work = Path(cls.directory.name)
        source = cls.work / SOURCE_PATH
        source.parent.mkdir(parents=True)
        source.write_text(baseline)
        run(["patch", "--batch", "--fuzz=0", "-p1", "-i", str(PATCH)], cls.work)
        cls.patched = source.read_text()
        cls.baseline = baseline
        cls.sanitizer_env = os.environ.copy()
        cls.sanitizer_env["ASAN_OPTIONS"] = "detect_leaks=1:halt_on_error=1"
        cls.sanitizer_env["UBSAN_OPTIONS"] = "halt_on_error=1:print_stacktrace=1"
        cls.executables = {}
        for label, actual in (("baseline", baseline), ("patched", cls.patched)):
            names = ["cbfs_boot_map_optionrom"]
            if label == "patched":
                names += ["pci_rom_cbfs_image_header", "pci_rom_cbfs_images_size", "pci_rom_probe_with_size"]
            else:
                names += ["cbfs_boot_optionrom_size"]
            names += ["pci_rom_probe", "pci_rom_images_size", "pci_rom_ssdt"]
            definitions = "\n\n".join(function(actual, name) for name in names)
            host_source = HARNESS.read_text().replace("/* SOURCE_FUNCTIONS */", definitions)
            path = cls.work / f"{label}.c"
            path.write_text(host_source)
            executable = cls.work / label
            run([compiler, "-std=gnu11", "-O1", "-g", "-Wall", "-Wextra", "-Werror",
                 "-Wno-sign-compare",  # existing baseline class-code comparison
                 "-fsanitize=address,undefined", "-fno-sanitize-recover=all",
                 "-fno-omit-frame-pointer", str(path), "-o", str(executable)])
            cls.executables[label] = executable

    @classmethod
    def tearDownClass(cls):
        cls.directory.cleanup()

    def scenario(self, name, *args):
        result = run([str(self.executables["patched"]), name, *map(str, args)], env=self.sanitizer_env)
        self.assertNotIn("runtime error:", result.stderr)
        self.assertNotIn("AddressSanitizer", result.stderr)
        return json.loads(result.stdout)

    def rejected(self, *names):
        for name in names:
            with self.subTest(name=name):
                result = self.scenario(name)
                self.assertEqual(result["copied"], 0)
                self.assertEqual(result["exposed"], 0)
                self.assertEqual(result["size_queries"], 0)

    def test_exact_patch_application_and_reversal(self):
        reverse = self.work / "reverse"
        source = reverse / SOURCE_PATH
        source.parent.mkdir(parents=True)
        source.write_text(self.patched)
        run(["patch", "--batch", "--fuzz=0", "-R", "-p1", "-i", str(PATCH)], reverse)
        self.assertEqual(source.read_text(), self.baseline)

    def test_baseline_unchecked_init_copy_is_reproduced_under_asan(self):
        result = subprocess.run([str(self.executables["baseline"]), "init-over-file"],
                                env=self.sanitizer_env, capture_output=True, text=True, timeout=20)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("AddressSanitizer: heap-buffer-overflow", result.stderr)

    def test_empty_and_short_header_are_rejected_before_probe_fields(self):
        self.rejected("empty", "short-header")
        self.assertFalse(self.scenario("short-header")["probe_ok"])

    def test_pcir_header_offset_and_signature_bounds(self):
        self.rejected("truncated-pcir", "bad-offset", "overlapping-offset", "bad-pcir-signature")
        for name in ("truncated-pcir", "bad-offset", "overlapping-offset", "bad-pcir-signature"):
            with self.subTest(name=name):
                self.assertFalse(self.scenario(name)["probe_ok"])

    def test_pcir_declared_structure_must_fit_mapping_and_its_image(self):
        self.rejected("short-dlen", "long-dlen", "pcir-crosses-image")

    def test_zero_and_overlength_images_are_rejected(self):
        self.rejected("zero-ilen", "overlength")

    def test_init_length_cannot_enlarge_mapping_or_declared_image(self):
        self.rejected("init-over-file", "init-over-image")
        self.assertFalse(self.scenario("init-over-file")["probe_ok"])

    def test_complete_multi_image_chain_at_exact_boundary(self):
        result = self.scenario("multi-valid")
        self.assertEqual((result["copied"], result["exposed"]), (1024, 1024))

    def test_incomplete_chain_does_not_expose_a_valid_prefix(self):
        self.rejected("missing-last", "multi-truncated-next", "multi-bad-second")

    def test_id_fallback_uses_the_successful_mapping_size(self):
        result = self.scenario("fallback-valid")
        self.assertEqual((result["copied"], result["exposed"]), (512, 512))
        self.assertEqual(result["maps"], 4)  # public probe + SSDT, each tries mapped/original ID
        self.assertEqual(result["size_queries"], 0)
        self.rejected("fallback-overlength")

    def test_compressed_mapping_uses_decompressed_not_stored_size(self):
        result = self.scenario("compressed")
        self.assertEqual((result["copied"], result["exposed"]), (1024, 1024))
        self.assertEqual(result["size_queries"], 0)

    def test_unaligned_buffers_and_pcir_are_read_without_typed_ub(self):
        for name in ("unaligned-map", "unaligned-pcir"):
            with self.subTest(name=name):
                result = self.scenario(name)
                self.assertEqual((result["copied"], result["exposed"]), (512, 512))

    def test_allocation_failure_exposes_nothing_and_releases_mapping(self):
        result = self.scenario("allocation-failure")
        self.assertEqual((result["copied"], result["exposed"]), (0, 0))
        self.assertEqual(result["unmaps"], 1)

    def test_existing_full_mx150_keeps_all_nonpadding_bytes(self):
        path = Path(os.environ.get("T480_MX150_VBIOS", str(REPO / "firmware/coreboot/site-local/data/mx150-vbios.rom")))
        if not path.is_file():
            self.skipTest("Full MX150 VBIOS (site-local/data/mx150-vbios.rom) is absent")
        self.assertEqual(hashlib.sha256(path.read_bytes()).hexdigest(), MX150_SHA256)
        result = self.scenario("full-mx150", path)
        self.assertEqual(result["mapped_size"], 184320)
        self.assertEqual((result["copied"], result["exposed"]), (182272, 182272))

    def test_optional_actual_source_and_header_abi_match(self):
        root = os.environ.get("T480_COREBOOT_ROOT")
        if not root:
            self.skipTest("Set T480_COREBOOT_ROOT to validate existing actual source/header")
        root = Path(root)
        self.assertEqual((root / SOURCE_PATH).read_text(), self.baseline)
        header = (root / "src/include/device/pci_rom.h").read_text()
        harness = HARNESS.read_text()
        for name in ("rom_header", "pci_data"):
            actual = re.search(rf"struct\s+{name}\s*\{{.*?\}};", header, re.S).group(0)
            fixture = re.search(rf"struct\s+{name}\s*\{{.*?\}};", harness, re.S).group(0)
            self.assertEqual(re.sub(r"\s+", "", actual), re.sub(r"\s+", "", fixture))


if __name__ == "__main__":
    unittest.main()
