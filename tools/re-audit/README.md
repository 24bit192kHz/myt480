# T480 audit tools

These tools have different effects; their names do not authorize running them.
The [research record](../../research/2026-10-09-t480/README.md) distinguishes
prior live checks from the current workstation-only continuation. The user has
required approval before any new T480 command.

| Tool | Purpose and effect |
|---|---|
| `mx150-capabilities.py` | Getter-only NVML support/range inventory. Initialization can wake an idle GPU; no voltage/clock/power setter. Approved queries ran over SSH without installation or tuning. |
| `measure-power.py` | Passive file/counter sampling, source/dual-battery validation and offline capture summaries. CPU RAPL is not GPU/board watts. |
| `nvme-health.py` | Fixed read-only NVMe SMART request; actual driver/device access needs approval. |
| `test-rtd3.sh` | Active module/policy/workload experiment with restoration; not authorized by a read-only query approval. |
| Rollback scripts | Restore earlier specific deployments from hash-checked backups; inspect scope before running. |

The corrected [`nvidia-suspend-test.sh`](../../system/usr-local/bin/nvidia-suspend-test.sh)
reads configuration/files only and offers `--root DIR --json` for offline fixtures.
It does not test sleep or certify successful CUDA/VRAM preservation. Fixture roots
are path prefixes, not symlink confinement.

The NVML probe selects full bus ID `0000:01:00.0` by default, refuses a non-MX150
product string, records unsupported/missing/error statuses separately and exits77
if the NVIDIA module is absent. It does not explicitly load that module. Returned
power caps are reported metadata, not automatically measured draw or usable
controls. P0–P15 queries can be individually unsupported. See the
[interface/approval report](../../research/2026-10-09-t480/12-mx150/01-voltage-and-controls.md).

Hardware-independent tests:

```sh
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tools/re-audit/tests -v
```

ACPICA tests require `patch`, `iasl` and `acpiexec`; missing optional integration
is reported as skipped. For the matching **workstation** coreboot tree, include
the complete preprocessed DSDT check:

```sh
T480_COREBOOT_ROOT=/path/to/matching/coreboot PYTHONDONTWRITEBYTECODE=1 \
  python3 -m unittest discover -s tools/re-audit/tests -v
```

The AML tests execute the actual patched methods in a userspace interpreter with
fake GPIO/link backing. They never install AML or access the real GPIOs. Source
patch reversal and fixture success are separate from hardware recovery.

The C fixtures compile actual coreboot ROM/CBFS functions from hashed GPL source
baselines with ASAN/UBSAN. They access temporary host buffers, not PCI/flash.
Use a sanitizer-capable host compiler (`CC` selects one) and `patch`. The optional
full-ROM check uses the kit's own `firmware/coreboot/site-local/data/mx150-vbios.rom`
(Lenovo's MX150 VBIOS, shipped with the kit as the top-level README says), or a file
selected by `T480_MX150_VBIOS`; the synthetic fixtures do not embed it. Missing tools or
optional integration files are reported as skips. Mock sleep-hook tests also
run GNU timeout against a mock process; they never call NVIDIA or real sleep.
