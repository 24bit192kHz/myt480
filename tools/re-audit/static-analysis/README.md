# Reusable static readers and an actual-source fixture

The two Python workstation readers preserve selected analysis steps as source. They
read artifact copies and produce derived JSON metadata. They use Python 3's
standard library, contact no target or network, load no kernel module and
execute no code from the input. They do not extract an updater, communicate
with the EC or modify firmware. Input size and SHA256 must match the audited
revision before parsing begins; neither reader has a bypass option.

Inputs and saved Ghidra projects remain private. These scripts make selected
tables reproducible from the same inputs; they do not replace the complete
private evidence collection or recover everything in either firmware/driver.
The [provenance manifest](provenance.json) records the original authored scripts,
their hashes, adaptations and comparisons with retained historical results.
The C fixture below is separate: it executes locally compiled early-init control
flow with all device/file/process/privileged calls mocked.

## NVIDIA physical chip and legacy tables

Use the proprietary package's `kernel/nvidia/nv-kernel.o_binary`, not the
installed `nvidia.ko`, an open module or a different driver release. The reader
accepts only the exact 580.178.04, 610.57.04 and 615.78.08 objects identified in
the [published upgrade manifest](../../../research/2026-10-09-t480/12-mx150/08-upgrade-evidence.json).
It recovers every row of the selected physical chip table and the reviewed
legacy table, including relocation-backed label strings. A physical chip table
row is 12 bytes; the legacy record is 24 bytes. Version-specific symbols and
section-relative legacy offsets are retained explicitly in the source.

```sh
python3 tools/re-audit/static-analysis/inspect-closed-elf.py \
  --version 615.78.08 --input /path/to/private/615.78.08/nv-kernel.o_binary \
  --output /path/to/private/new-615-tables.json
```

The output contains SHA256, symbol/section-relative metadata, physical table
rows and legacy classifications. It excludes the original legacy-record raw
hex and absolute input path. It does not recover HAL object registration,
constructor call chains, all incoming relocations, userspace ABI or a working
Pascal patch. Those independently reviewed conclusions and exact selected
addresses are in the [upgrade report](../../../research/2026-10-09-t480/12-mx150/07-driver-upgrade-and-pascal-support.md).

## N24HT37W EC INI3 initialization

Use the exact 286,720-byte N24HT37W payload after removing the 32-byte FL2
wrapper. Extraction, architecture/import settings and input hashes are in the
[EC research](../../../research/2026-10-09-t480/10-ec-firmware-and-lid-wake.md#exact-extraction-and-hashes).
The reader starts at the reviewed INI3 record stream `0x2e574`. It implements
literal data, zero-fill records and the recovered LSB-first compression grammar,
checks SRAM bounds/overlap, and reports the six signed halfwords at `0x800518`.

```sh
python3 tools/re-audit/static-analysis/decode-ec-ini.py \
  --input /path/to/private/n24ht37w-payload.bin \
  --output /path/to/private/new-ec-ini.json
```

The output contains record destinations/lengths, decoded hashes and the derived
map. It writes no reconstructed SRAM binary. Static initialized values do not
show current retained state or establish host-command eligibility, sleep/lid
behavior, EC write safety or complete EC coverage.

## Output and verification

Without `--output`, either reader prints JSON to stdout. An explicit output
must be a new file in an existing directory; the tool refuses to overwrite an
existing file. Artifact identity failure happens before any output file is
opened. Run on workstation copies and keep generated results in a private
analysis directory. Review any output before publishing it; the input basename
is retained as metadata.

The adapted readers were compared locally with historical JSON on October 10,
2026. All 113/116/116 physical chip rows and 15/19/19 legacy records matched
for the three revisions. EC record addresses, destinations, lengths, decoded
hashes, terminal address and the six-halfword map matched. Mismatched sizes,
same-size invalid hashes and existing-output refusal were checked. These are
reader-reproduction checks, with no new hardware or firmware behavior test.

## Actual-source plain-root ordering fixture

[`plain-root-harness.c`](plain-root-harness.c) includes the exact reviewed
[`kernel/early-init/init.c`](../../../kernel/early-init/init.c) through a relative
path. It supplies plain root/swap headers and a synthetic sealed-key blob, then
assumes a successful matching-policy TPM unseal. It runs the source twice: with
the production command line the master key must stay sealed while the plain root
is still mounted and its init executed (the fix of 2026-10-10); with
`T480_FIXTURE_PROVISION` set the command line carries `t480.provision` and the
master load precedes the plain-root mount, as the conversion's check boot needs.
It does not test TPM cryptography, extract any key, mount a partition, execute a
root init or demonstrate a physical exploit.

Run only through the guarded runner:

```sh
sh tools/re-audit/static-analysis/run-plain-root-fixture.sh
```

The runner requires a C compiler with GNU-compatible `--wrap`, Linux development
headers, `nm`, `sha256sum` and standard shell tools. It copies the source/fixture
into a private temporary directory and rejects a source hash other than
`59840d7b9a8a8485be5803ee7ef48c522511ea81ffcff2694e5c5c6ad8894628` (the fixed
source; the audited original was `2f57dddb…8b95d`) before compiling. All source-facing device/file/process/privileged operations use an
explicit linker wrapper list, including `dup2` and the alternative ioctl,
passphrase, child-process, resume and failure paths. Alternative paths not part
of this fixture stop locally. Formatting/memory helpers and fixture verdict
output use ordinary libc. Source-object external symbols are checked before
linking; remaining real operation imports are checked before execution. No
environment `CFLAGS` or `CC` overrides are used.

The temporary binary is removed on exit. Never compile or run the C file without
its runner's wrappers. A compile-time guard rejects ordinary compilation, but
that guard alone cannot replace the linker wrapper list.

On October 10, 2026 the guarded fixture printed, for the audited original source:

```text
Confirmed source path: matched-policy master load -> plain root mount -> root init execution.
```

and, later the same day, for the fixed source (two runs):

```text
Confirmed production path: plain root -> master key left sealed -> plain root mount -> root init execution.
Confirmed provisioning path: t480.provision -> matched-policy master load -> plain root mount -> root init execution.
```

Copies with a changed `init.c` and an added unreviewed external operation were
rejected before execution. The fixture checks control flow in the source; the
live boot of the fixed kernel is a separate, attended step.
