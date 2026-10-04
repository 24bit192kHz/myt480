# Kernel: linux-t480

Linux 7.2.8 from the CachyOS PKGBUILD, BORE scheduler, built only with the modules this
laptop uses.

| File | Meaning |
|---|---|
| `PKGBUILD` | the package; `makepkg -si` builds `linux-t480` and `linux-t480-headers` |
| `config` | kernel configuration |
| `config-7.2.8-1-t480` | configuration of the first build, before the MX150 work |
| `config-7.2.8-2-t480` | adds GVT-g (mediated iGPU for a Windows VM), `i915.enable_gvt=1` built into the command line |
| `config-7.2.8-3-t480` | adds the TPM 2.0 driver (`tpm_tis`, the Infineon chip coreboot declares as MSFT0101) and the PCH TCO watchdog (`iTCO_wdt`) |
| `config-7.2.8-4-t480` | hibernation image compressed with LZ4 (resume 1.5 s faster than LZO) |
| `config-7.2.8-7-t480` | the running build: dm-crypt, AES-NI and the TPM trusted/encrypted key types built in, and `early/` embedded as the initramfs (`CONFIG_INITRAMFS_SOURCE`) |
| `early/nodes.list`, `early-init/` | the early init that opens the encrypted disk; `early-init/README.md` |
| `modprobed.db` | module list for `localmodconfig`, made from the two files below |
| `lsmod.t480` | modules seen loaded |
| `extra-modules.txt` | modules that were not loaded at that time but are needed |
| `fw/` | firmware built into the kernel (i915 DMC) |

NVMe, ext4 and i915 are built in, so the kernel needs no initramfs on disk; its only
initramfs is the built-in early init, which hands over to `firmware/tools/t480-init`.
Before `makepkg`, build `early/root/init` and `early/root/bin/cryptsetup` as
`early-init/README.md` describes. The headers package is needed for the NVIDIA DKMS module.

Not enabled, in case you look for them: `VFIO_PCI` (GPU passthrough) and
`INTEL_IOMMU_DEFAULT_ON`.

A module that is missing: add its name to `extra-modules.txt`, rebuild `modprobed.db`,
rebuild the kernel.
