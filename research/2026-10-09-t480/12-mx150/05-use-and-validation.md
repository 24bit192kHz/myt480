# Getting the most out of the MX150

## Retain compatible software

Keep the proprietary **580 branch** with matching kernel module, NVML,
OpenGL/Vulkan and CUDA driver libraries. [NVIDIA's Linux legacy schedule](https://nvidia.custhelp.com/app/answers/detail/a_id/3142)
names 580 as the last Maxwell/Pascal/Volta branch and promises kernel/X-server
compatibility and critical fixes within the relevant product lifetime.
[NVIDIA's GeForce plan](https://nvidia.custhelp.com/app/answers/detail/a_id/5676)
states critical-security support through October 2028. This does not promise new
RTX-era features. The exact 580.178.04 package was rechecked in NVIDIA's
[archive](https://download.nvidia.com/XFree86/Linux-x86_64/580.178.04/);
this pass did not replace packages. NVIDIA's [September 2026 bulletin](https://github.com/NVIDIA/product-security/blob/main/2026/5861/5861.md)
lists 580.178.04 as the fixed Linux R580 version for its GeForce rows. The recorded
driver matches that threshold. This is a version comparison for that bulletin,
not an exploit test or a certification of the entire system's security.
Keep taking appropriate 580 maintenance updates rather than freezing every
component indefinitely.

The [NVIDIA open kernel modules](https://github.com/NVIDIA/open-gpu-kernel-modules)
require Turing or newer. Their public control headers help interpret interfaces,
but do not provide a Pascal implementation to swap onto this GPU. A driver/kernel
update needs a checked DKMS build and matching userspace with a known recovery
kernel; no new kernel or driver has been deployed in this continuation.

## CUDA build compatibility

The recorded MX150 compute capability is 6.1. Use a **CUDA 12.9** toolkit/compiler
when building new Pascal code. CUDA 13 removed offline compilation below 7.5;
existing suitable binaries built with earlier toolkits can continue running on
a supporting driver. A `nvidia-smi` maximum CUDA version is not proof that the
installed `nvcc` can compile this architecture. [NVIDIA developer guidance](https://developer.nvidia.com/blog/navigating-gpu-architecture-support-a-guide-for-nvidia-cuda-developers)
separates driver and toolkit support.

A build targeting this recorded device can include native `sm_61` code and
`compute_61` PTX using the supported 12.9 compiler, for example:

```sh
nvcc -gencode arch=compute_61,code=sm_61 \
     -gencode arch=compute_61,code=compute_61 checked-workload.cu -o checked-workload
```

This is a build example, not a file compiled/run during this continuation.
Review host compiler and library architecture support as well; a newer framework
can omit Pascal or require features unavailable here even when the driver loads.
Prefer checked workloads whose data fits the actual VRAM capacity, avoid repeated
host/device transfer and allocation overhead, and benchmark completed useful
work rather than relying on a display-only score. Workload-specific gains remain
to be measured.

## Use the GPU deliberately

Use `igpu-run application` for ordinary desktop graphics and `prime-run application`
for intended MX150 CUDA/OpenGL work. The wrapper keeps the GPU awake while the
job runs and restores the retained offsets after wake. Persistent holders can
prevent coarse idle entry. The initial audit left Bitwarden untouched; the
separate evening run corrected its actual `bw-screen boot` route through Intel
and reports D3cold with it open. This query-only continuation stopped no app.
The [routing report](../../../docs/wiki/gpu-routing.md) includes validation and
rollback.

Retain AC +200/+1250 MHz offsets and battery 0/0. Prior core +250 produced silent
CUDA errors despite a render passing. This pass recommends neither a higher
offset nor a negative voltage value. FPS limits, smaller working sets and routing
can be compared reversibly in applications; no energy improvement is assumed
without a matched measurement. MX150 Vulkan and actual OpenCL workloads remain
unvalidated in this session, even though the matching installed packages are
inventoried.

## Media support: an unresolved documentation conflict

The current [NVIDIA codec matrix](https://developer.nvidia.com/video-encode-decode-support-matrix)
lists MX150–MX330 with zero NVENC and zero NVDEC engines. Conversely, the exact
[580 supported-products appendix](https://download.nvidia.com/XFree86/Linux-x86_64/580.178.04/README/supportedchips.html)
lists PCI 1d10 as VDPAU feature setH, whose
[decoder appendix](https://download.nvidia.com/XFree86/Linux-x86_64/580.178.04/README/vdpausupport.html)
describes supported decode profiles. The local exact package contains those
entries. Do not silently convert this conflict into a proven absence of all
decode hardware or copy GT1030 results onto the laptop.

No supported NVENC path is established. Actual VDPAU/NVDEC profile queries and a
checked decode workload on this MX150 remain open. Intel media acceleration is
the first practical route to evaluate for this Intel-driven desktop; actual
profiles, image correctness and energy need validation. Compiling an FFmpeg
binary with an encoder name does not prove the corresponding engine exists.
AV1, Tensor/RT acceleration and newer-generation features cannot be inferred
from a parser or API name.

## Alternative driver and kernel work

The inspected [Linux Nouveau GP108 device table](https://github.com/torvalds/linux/blob/master/drivers/gpu/drm/nouveau/nvkm/engine/device/base.c)
defines graphics, GPIO, thermal, PMU, SEC2 and NVDEC constructors; it has no GP108
clock/voltage or NVENC constructor entry there. This is open implementation
evidence, not proof of this laptop's enabled decode profiles or a performance
comparison. A constructor list cannot establish dynamic reclocking, firmware
authorization or a usable undervolter. Nouveau was not installed/switched or
benchmarked in this continuation.

Useful kernel/driver work should target a demonstrated failure: ACPI rail
cleanup, runtime references, allocation-preserving sleep, truthful errors and
bounded ROM parsing. Changing a chip support check does not implement missing
power sequencing or validate firmware. GPU firmware/microcode, memory training,
full error recovery and virtualization paths remain open. VFIO would additionally
need verified isolation, reset and display ownership; no vGPU/passthrough feature
is delivered here. The existing GPU ASPM workaround should remain until a
controlled failure/recovery experiment justifies changing it.

## Measure and restore

A future comparison should run stock 0/0 → retained tuning → stock 0/0 with the
same input, warm-up, power source, CPU plan/undervolt, brightness, peripherals,
ambient/starting temperature and fan policy. Check output and record completed
work, elapsed time, clock/P-state, temperatures, errors and energy.
Keep the held-open wrapper across any RTD3 cycle so offset lifetime is controlled.

Use the [dual-battery sampler](../../../tools/re-audit/measure-power.py) with
confirmed AC absent and valid discharge for whole-laptop energy. CPU RAPL is
package energy, not GPU/board power; prior charging-AC measurements cannot prove
battery savings. NVML queries can wake the GPU and must not contaminate a passive
idle comparison. Undervolt claims additionally need reliable voltage readings
at matched performance. Getter failures and placeholder caps remain explicit.

All new repository diagnostics/patches are source-only. Git can restore their
previous source; the
[existing hash-checked deployment rollback](../09-rollback-and-next-work.md)
applies to the earlier installed helper/thermal/routing changes. A later approved
deployment needs fresh old-file hashes/backups and restoration checks.
No EC/VBIOS flash, rail override, sleep or new tuning test has happened here.
