# Reference: Lenovo's firmware

Dumped from this T480 while it ran the stock BIOS. Not used by the build.

| File | Content |
|---|---|
| `DSDT.aml`, `DSDT.dsl` | main ACPI table, binary and decompiled |
| `SSDT*.aml` | the other ACPI tables |
| `SSDT15.dsl` | the dGPU table ("SgPch"): `HGON`/`HGOF` power sequence, power resource `PC01`, GC6, Optimus `_DSM` |
| `mx150-vbios-10de-1d10.rom` | VBIOS as read from the ROM of the running GPU: 119808 bytes, cut off at the size in its header. The complete image (182272 bytes) is `../coreboot/site-local/data/mx150-vbios.rom` |

Decompile another table with `iasl -d SSDTn.aml`.
