# Tools, evidence, and reproducibility

## Tool use

[REA](https://github.com/morluto/rea) was used through its CLI and a temporary
stdio MCP client. The installed 3.2.1 copy could not start this environment's
native workflow; `rea-agents@6.1.0` worked after explicitly setting
`GHIDRA_INSTALL_DIR` and `JAVA_HOME` to the existing Ghidra 12.1.4 and JDK 21
installations. Its doctor check passed. Fingerprint DLL, thermal executable,
SmmAslSmi, and EcIoSmm native analysis succeeded.

REA's NVIDIA native workflow timed out after 330 seconds. Following the requested
fallback, [bethington/ghidra-mcp](https://github.com/bethington/ghidra-mcp) dev,
version 7.0.0, was built with its Gradle wrapper against the same Ghidra/JDK.
The checkout was `9cc29c0f1efb6c63a7d6898c9a23aff39397f992`.
The build succeeded with deprecation warnings. Its headless server ran on
`127.0.0.1:8099`, with an 8 GiB heap; import and targeted `get_functions` calls
worked, and full auto-analysis subsequently completed. It was not exposed to
the LAN or registered as a permanent MCP service. Both temporary analysis servers
were stopped after saving their evidence.

The [awesome-reverse-engineering catalog](https://github.com/alphaSeclab/awesome-reverse-engineering)
was inspected as a tool index. Relevant tools were Ghidra, ELF symbols/objdump,
UEFI extraction, `iasl`, and `innoextract`. The catalog is not a guarantee that
every linked project is current, appropriate, or needed for this laptop.

The temporary Python MCP helper mistakenly looked up `isError` after saving the
SDK result; the SDK uses `is_error`. That produced a local reporting exception
after successful calls. Saved results and server status were checked directly;
this reporting exception was not counted as a failed native analysis.

## Evidence anchors

| Artifact | SHA256 | Role |
|---|---|---|
| Live `nvidia.ko` | `99946c338065a775ce13fdb49179440d2ae9732767545e77524c3078fb2a0ce1` | Shipped Linux NVIDIA code |
| `synaWudfBioUsb.dll` | `aed9f4246bbaca28776c05c169c734cae9763322b03dc00e4319320e1aa2294d` | Extracted Windows driver |
| Live `DSDT.aml` | `f2fd32df2cbd99def8d6a4af3743753879a08b31982d862307832642a5c5b750` | Actual coreboot ACPI interface |
| `mod-SmmAslSmi.pe` | `149a282a9ca05baa6ad659a09a9aeda2f6143612221cc2adb2828f7774312e10` | Stock SMI dispatch/mailbox bridge |
| `mod-EcIoSmm.pe` | `b44f9cb7fcc7a798b967ff5769bd9002981ec331cf7085f0a6fc62fc5feddf00` | Stock EC transport |

The Lenovo archive's expected SHA512, verified before extraction, was:

```text
a4a4e6058b1ea8ab721953d2cfd775a1e7bc589863d160e5ebbb90344858f147d695103677a8df0b2de0c95345df108bda97196245b067f45630038fb7c807cd
```

It is the archive from `https://download.lenovo.com/pccbbs/mobiles/nz3gf07w.exe`,
7,244,088 bytes, referenced by the python-validity manifest.

## Repeating selected analysis

Copy artifacts off the laptop for analysis; do not patch a loaded proprietary
module. Hash the copy, select the native provider, then use string xrefs to find
fingerprint lifecycle functions or symbol names for NVIDIA RM paths. For a large
ELF module, import without automatic analysis first and request a small function
bundle. Treat relocation/decompiler warnings as limitations. Function addresses
in the NVIDIA page are Ghidra addresses, not offsets to patch into the original file.

For ACPI, decompile stock SSDT10 with the stock DSDT supplied as an external table:

```sh
iasl -e firmware/stock-reference/DSDT.aml \
  -d firmware/stock-reference/SSDT10.aml
```

Run this on copies in an analysis directory to avoid overwriting repository
reference files. Compare the stock output against a copy of
`/sys/firmware/acpi/tables/DSDT` and the other live tables. The useful distinction
is what is actually exposed at runtime versus what exists only in the vendor BIOS.

Private evidence includes `fp-functions.json`, `fp-session-functions.json`,
`fp-follow.json`, `smm-bfw-dispatch.json`, `smm-mailbox-functions.json`,
`ecio-transport.json`, `nvidia-rm-init.json`, `nvidia-rm-transition.json`, and
`nvidia-rm-final.json`. Validation logs include thermal deployment, the fan pause
test, GPU deployment, five-round RTD3 checks, PRIME rendering, and final inventory.
These are retained under `/home/btw/test/rea/work/audit-20261009`.
`rollback-roundtrip.txt` records the verified original hashes and a reload refused
while an application held the GPU; `reapply-retry.txt` and `final-validation.txt`
record the final deployed state and successful post-restoration checks.

## Verification commands

```sh
cc -O2 -Wall -Wextra -Wno-missing-field-initializers \
  src/thermald-t480/tests/fan-control.c -o /tmp/test-fan
/tmp/test-fan
cc -O2 -Wall -Wextra -std=c99 src/gpu-power/gpu-power.c -o /tmp/test-gpu-power
python3 src/gpu-power/tests/prime-run.py
python3 src/gpu-power/tests/tune.py
for script in src/gpu-power/prime-run tools/re-audit/*.sh; do
  sh -n "$script" || exit
done
git diff --check
```

The hardware-independent tests use fixtures instead of MSRs, fan registers, or
NVIDIA hardware. They complement the live tests; they cannot establish dock,
recognition-accuracy, hibernation, or battery-runtime behavior.
