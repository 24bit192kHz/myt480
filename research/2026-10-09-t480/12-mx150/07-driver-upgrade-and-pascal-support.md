# NVIDIA 610/615 on the MX150: support, patches and expected benefit

Research date: **2026-10-09**. Device: GP108M Pascal MX150, `10de:1d10`,
compute capability 6.1. The last approved device queries recorded proprietary
**580.178.04**. This continuation used workstation downloads, source inspection,
ELF tables/relocations, GNU disassembly and the requested Ghidra MCP. It ran
**no command on the T480** and installed no driver or firmware.

## Which update is actually available?

The [official Linux x86_64 archive](https://download.nvidia.com/XFree86/Linux-x86_64/)
and [NVIDIA's release search](https://www.nvidia.com/Download/processFind.aspx?ctk=0&lang=en-us&lid=1&osid=12)
were checked directly. The archive is more current than the marketing page that
still advertises 615.71.09.

| Branch | Latest public display release found | This MX150 |
|---|---|---|
| R580 | **580.178.04** | Supported; already the recorded installed version |
| R595 | 595.104.02 | MX150 listed under the legacy R580 section |
| R610 | 610.57.04 | MX150 listed under the legacy R580 section |
| R615 | **615.78.08**, released October 7, 2026 | MX150 listed under the legacy R580 section |

These are NVIDIA public Linux display downloads, not a survey of every distro
package rebuild. Recheck before a later package operation. R580 is
[NVIDIA's last Pascal Linux branch](https://nvidia.custhelp.com/app/answers/detail/a_id/3142);
the [GeForce maintenance plan](https://nvidia.custhelp.com/app/answers/detail/a_id/5676)
provides critical-security updates through October 2028. Maintain this branch
with matching libraries rather than freezing it permanently.

Searching a newer [supported-products appendix](https://download.nvidia.com/XFree86/Linux-x86_64/615.78.08/README/supportedchips.html)
for “MX150” produces a misleading hit unless its section is read: PCI `1D10`
belongs to the GPUs requiring **580.xx**, not the current-release list. This
was checked in 590.48.01, 595.104.02, 610.57.04 and 615.78.08, against the current
supported entry in 580.178.04.

## What the actual proprietary binaries show

Both 610 and 615 still ship **proprietary and open** kernel flavors. The
[615 kernel-flavor manual](https://download.nvidia.com/XFree86/Linux-x86_64/615.78.08/README/kernel_open.html)
limits the proprietary flavor to Turing/Ampere/Ada/Hopper. “New drivers are
open-only” is therefore not the explanation for this laptop's incompatibility.

Official 610.57.04 and 615.78.08 packages were downloaded and selectively
extracted with the workstation's `zstd` and a path-checked tar reader. Neither
the installer script, bundled decompressor nor any extracted NVIDIA program was
executed. Full package/member inventories and hashes were retained privately;
the [published evidence manifest](08-upgrade-evidence.json) identifies the inputs.
The 580 comparison uses the retained exact-version package.

There are at least **three separate blocks** in the proprietary RM path:

1. **Legacy rejection:** PCI probe calls `rm_is_supported_pci_device()` and then
   `rm_is_supported_device()`. The latter reads physical PMC registers and checks
   a retired-architecture table. Compared with 580, 610/615 add GM1xx, GM2xx,
   GP1xx and GV1xx retirement records directing users to 580. Removing a Linux
   PCI-ID check would leave this physical-chip check.
2. **Hardware selection:** after that gate, the HAL lookup requires a registered
   object and a matching physical architecture/implementation entry. The 580
   GP108 entry at index 34 is `{0x13, 8, 0}`. Its counterpart is `{0, 0, 0}` in
   **both** 610 and 615. Neither newer inspected chip table contains any Pascal
   architecture `0x13` entry. Consequently, bypassing legacy rejection still
   leaves the shipped lookup unable to select GP108.
3. **Object registration:** 580's GP108 stub registers HAL index `0x22` (34).
   The inspected 610/615 registration stubs begin at TU102 index 37; none
   registers GP108. Their HAL constructor clears all slots first. Thus the
   normal shipping initialization leaves slot 34 null: restoring the chip-table
   row alone also fails the lookup's registered-object requirement.

| Package | Physical chip table | HAL lookup function | GP108 row 34 |
|---|---|---|---|
| 580.178.04 proprietary | `_nv019377rm` | `_nv034616rm` | `{0x13, 8, 0}` |
| 610.57.04 proprietary | `_nv019282rm` | `_nv033277rm` | `{0, 0, 0}` |
| 615.78.08 proprietary | `_nv020004rm` | `_nv034192rm` | `{0, 0, 0}` |

These obfuscated names are exact-build anchors, not stable APIs. Independent
ELF parsing and instruction/relocation inspection established the tables;
Ghidra MCP independently recovered the 615 lookup, both support functions and
the slot-clear/create/register functions. ELF call relocations independently
bound the fixed-ID registration stubs to the common registration path.
The lookup checks `BOOT_42` architecture bits 29:24 and implementation bits
23:20, plus a non-null HAL slot. Its failure returns `0x56` (not supported).

Some Pascal-era strings and shared routines **remain** in the newer binaries.
For example, `kbusInitInstBlk_GP100` survives, and newer compiler libraries still
contain `sm_61` text. This prevents the stronger claim that every Pascal routine
was deleted. Conversely, names, diagnostic strings and instruction constants do
not restore the missing chip registration or establish a complete working
graphics/CUDA/power backend. This is a bounded analysis of initialization, not a
recovery of every function in the new drivers.

## Why common patch ideas are insufficient

| Proposed change | What still needs implementation or proof |
|---|---|
| Add `1d10` to the PCI list or spoof a Turing PCI ID | Physical PMC identification and the missing GP108 HAL entry remain. Turing register operations do not become Pascal operations. |
| Bypass the proprietary legacy check | GP108 is absent from the newer HAL selection tables and shipping object-registration path. Restoring a row alone still leaves its slot null. |
| Enable `OpenRmEnableUnsupportedGpus` | The option is ignored/deprecated; it was an opt-in for newer non-datacenter GPUs, not a Pascal implementation. |
| Set `NVreg_EnableGpuFirmware=0` | This does not add a GP108 backend. The open flavor forces GSP use and disables monolithic fallback for this physical GPU. |
| Keep the 580 kernel module with 610/615 libraries | The normal and relaxed version checks reject different major branches. Disabling that check does not validate private RM controls, object classes, payload layouts or compiler behavior. |
| Edit the VBIOS, coreboot or EC | Those changes do not create Turing's GSP hardware or restore omitted host-driver implementations. |

The [pinned open 615 source](https://github.com/NVIDIA/open-gpu-kernel-modules/tree/1a82fbfda8f321b056eb3568850f9316c5e465c5)
provides independent implementation evidence:

- [Physical identification](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/1a82fbfda8f321b056eb3568850f9316c5e465c5/src/nvidia/arch/nvalloc/unix/src/osapi.c#L3747)
  reads PMC registers and invokes the HAL lookup.
- [HAL matching](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/1a82fbfda8f321b056eb3568850f9316c5e465c5/src/nvidia/src/kernel/core/hal_mgr.c#L157)
  requires a registered object; [GP108's open build flag](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/1a82fbfda8f321b056eb3568850f9316c5e465c5/src/nvidia/generated/rmconfig.h#L139)
  is disabled. It was also disabled in **open 580**, so that source comparison
  alone cannot prove proprietary removal; the binary tables above are separate evidence.
- [GSP capability](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/1a82fbfda8f321b056eb3568850f9316c5e465c5/src/nvidia/src/kernel/gpu_mgr/gpu_mgr.c#L1074)
  requires Turing or newer. [Firmware selection](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/1a82fbfda8f321b056eb3568850f9316c5e465c5/src/nvidia/arch/nvalloc/unix/src/osapi.c#L5174)
  forces the open firmware-client route without a host-RM fallback here.
- [Version checking](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/1a82fbfda8f321b056eb3568850f9316c5e465c5/src/nvidia/arch/nvalloc/unix/src/osapi.c#L1200)
  rejects 580 versus 610/615 even in relaxed mode. NVIDIA also requires
  [matching source, firmware and userspace](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/1a82fbfda8f321b056eb3568850f9316c5e465c5/README.md#L18).

A genuine port would need to restore/register a non-GSP GP108 implementation,
reconcile its object/control interfaces with the newer stack, and verify memory
initialization, engines/microcode, interrupts/reset, clock/voltage/protection,
PRIME, CUDA/graphics, RTD3 and allocation-preserving system sleep. For the open
driver that means implementing a host-managed route it does not supply for
Pascal. For the proprietary driver, further binary work must establish which
constructors/methods survive and which need replacement. This is a maintained
driver-port project, not a working one-line patch found in this investigation.
No installable 610/615 Pascal patch was produced or tested.

## Would a successful port gain anything?

Selected software fixes or API features could have value **if** their Pascal
implementation were restored and an application used them. There is no measured
MX150 speed, latency or energy improvement from 610/615 here.

Several advertised 610 fixes already appear in the official **580.173.02**
[release record](https://www.nvidia.com/Download/processFind.aspx?ctk=0&lang=en-us&lid=1&osid=12):
Vulkan semaphore wake/stutter, X11 Present black screens, OpenGL storage residency
and DKMS handling. Retained 580.178.04 is newer, and its package changelog contains
those fixes. A larger branch number is unnecessary to obtain them.

The [615 announcement](https://forums.developer.nvidia.com/t/615-release-feedback-discussion/382815/1)
and latest release notes add low-latency APIs, memory accounting, suspend
notifiers and display/HDR fixes. Relevance differs: this T480 runs Artix/s6 with
elogind and an Intel-driven display. NVIDIA display and systemd-specific fixes
are not established bottlenecks. A suspend-notifier backport would need the
private RM contract and allocation preservation reviewed before replacing hooks.
[PR 1199](https://github.com/NVIDIA/open-gpu-kernel-modules/pull/1199) concerns
GSP/S0ix hibernate state, not EC lid wake; NVIDIA's maintainer reports using a
different fix from the proposed one-line change. It is not a ready T480 patch.

For CUDA, [NVIDIA's architecture matrix](https://docs.nvidia.com/datacenter/tesla/drivers/latest/cuda-toolkit-driver-and-architecture-matrix.html)
ends Pascal driver support at R580. Keep **CUDA 12.9** for `sm_61` builds:
[CUDA 13 drops offline compilation below 7.5](https://developer.nvidia.com/blog/navigating-gpu-architecture-support-a-guide-for-nvidia-cuda-developers).
An API version string cannot restore architecture support in a compiler or library.

For games, [current DXVK's baseline](https://github.com/doitsujin/dxvk/wiki/Driver-support)
lists proprietary NVIDIA **575.51.02**, so 580 clears the version floor;
actual Vulkan feature/limit checks and games remain untested on this device.
[DXVK 2.7](https://github.com/doitsujin/dxvk/releases/tag/v2.7) deliberately
disables descriptor buffers on Pascal because of severe regressions.
[DXVK 3.0](https://github.com/doitsujin/dxvk/releases/tag/v3.0) requires 595.84+
for its newer descriptor-heap path, but also contains other application fixes;
do not force optional descriptor paths or spoof an extension as a speed tweak.
Pascal is not inherently restricted below Vulkan 1.4:
[Khronos submission 859](https://www.khronos.org/conformance/adopters/conformant-products/vulkan#submission_859)
includes the GP108-class GTX1030 family on Linux. That is family evidence,
not an actual MX150 capability query.

A port cannot add Tensor/RT silicon, more VRAM or memory bandwidth, or a larger
physical power/cooling budget. It also establishes no new undervolt or codec
capability; the [voltage](01-voltage-and-controls.md) and
[media-profile uncertainty](05-use-and-validation.md#media-support-an-unresolved-documentation-conflict)
remain as previously recorded.

## Useful improvements and a reversible next step

Maintain a complete proprietary R580 stack and selectively backport an exposed
Linux-interface fix only after reproducing a relevant failure and checking that
580 lacks it. There is no current DKMS failure justifying an arbitrary patch:
the recorded stack already runs the custom kernel. Existing source corrections
for [power-good/ROM bounds and sleep errors](04-sleep-and-firmware.md) address
concrete problems independently of the driver version.

Compatible application updates, CUDA 12.9 builds, small-VRAM workload choices,
correct Intel/offload routing and validated idle power behavior offer practical
paths to measure. [Mesa NVK](https://docs.mesa3d.org/drivers/nvk.html) is an
independent source-accessible Vulkan implementation supporting Kepler and later;
that does not prove GP108 reclocking, performance parity or NVIDIA CUDA
replacement. Study it separately before proposing a driver switch.

Before any approved future package trial, prepare the exact distro package set,
matching 64/32-bit libraries where used, DKMS build for the candidate kernel,
cached restoration packages/configuration and a bootable known-good kernel.
Use an isolated installation/boot entry and keep the Intel display recovery path.
Validate stock clocks first: correct CUDA output, OpenGL/Vulkan rendering, PRIME,
errors and idle D3cold; allocation-preserving sleep requires separate approval.
Compare completed work and energy under the [matched measurement procedure](05-use-and-validation.md#measure-and-restore).
Rollback must restore the entire matched driver stack, not just a module.

That is a trial plan, not an action taken here. The earlier approval covered
only already-completed getter/file/existing-session queries. New target commands,
driver changes, tuning, EC access, module operations and sleep still require
their own approval. There is presently no working new-branch candidate to test.
