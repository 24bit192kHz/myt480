# Reference: Lenovo's firmware

Dumped from this T480 while it ran the stock BIOS. Not used by the build.

| File | Content |
|---|---|
| `DSDT.aml`, `DSDT.dsl` | main ACPI table, binary and decompiled |
| `SSDT*.aml` | the other ACPI tables |
| `SSDT15.dsl` | the dGPU table ("SgPch"): `HGON`/`HGOF` power sequence, power resource `PC01`, GC6, Optimus `_DSM` |
| `mx150-vbios-10de-1d10.rom` | VBIOS as read from the ROM of the running GPU: 119808 bytes, cut off at the size in its header. Reference only: the NVIDIA driver rejects it (`RmInitAdapter failed! (0x24:0x40:1603)`). The complete image (182272 bytes) is `../coreboot/site-local/data/mx150-vbios.rom`, the raw section of FFS file `8B9BEF90-5FAF-46CF-A32A-7E1B7D33CC87` in the Lenovo image (`UEFIExtract <image> all`); on the T580 it is `17835F4C-5D9B-484D-ABBC-5FBDC11F891C` |

Decompile another table with `iasl -d SSDTn.aml`.
