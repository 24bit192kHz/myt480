# Kernel: linux-t480

Linux 7.2.8 from the CachyOS PKGBUILD, BORE scheduler, built only with the modules this
laptop uses.

| File | Meaning |
|---|---|
| `PKGBUILD` | the package; `makepkg -si` builds `linux-t480` and `linux-t480-headers` |
| `config` | kernel configuration |
| `config-7.2.8-1-t480` | configuration of the first build, before the MX150 work |
| `config-7.2.8-2-t480` | adds GVT-g (mediated iGPU for a Windows VM), `i915.enable_gvt=1` built into the command line |
| `config-7.2.8-3-t480` | the running build: adds the TPM 2.0 driver (`tpm_tis`, the Infineon chip coreboot declares as MSFT0101) and the PCH TCO watchdog (`iTCO_wdt`) |
| `modprobed.db` | module list for `localmodconfig`, made from the two files below |
| `lsmod.t480` | modules seen loaded |
| `extra-modules.txt` | modules that were not loaded at that time but are needed |
| `fw/` | firmware built into the kernel (i915 DMC) |

NVMe, ext4 and i915 are built in, so the kernel boots without an initramfs; see
`firmware/tools/t480-init`. The headers package is needed for the NVIDIA DKMS module.

Not enabled, in case you look for them: `VFIO_PCI` (GPU passthrough) and
`INTEL_IOMMU_DEFAULT_ON`.

A module that is missing: add its name to `extra-modules.txt`, rebuild `modprobed.db`,
rebuild the kernel.
