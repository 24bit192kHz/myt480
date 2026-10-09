# NVIDIA and Intel graphics

The subsequent [MX150 subject folder](12-mx150/README.md) extends this record
with voltage controls, signed NV-CONTROL dispatch, selected closed RM, VBIOS,
EC protections, CUDA/media limits and power/sleep corrections with deployment
status recorded separately from the original source pass.
It does not add new live undervolt, sleep, energy or performance results.

## Inspected baseline and recovered driver behavior

The MX150 is GP108M/Pascal (`10de:1d10`), running proprietary NVIDIA `580.178.04`.
The desktop uses Intel UHD620. Driver mode `0x03` left NVIDIA loaded and active/D0
on AC; the existing tuning used +200 MHz core/+1250 MHz memory on AC and stock
offsets on battery.

REA's complete analysis of the 143 MB relocatable kernel module timed out after
330 seconds. Ghidra MCP imported it without initial auto-analysis, exposed 61,109
symbol-backed functions, and decompiled selected RM dynamic-power initialization,
enable and transition functions. Full auto-analysis completed later. Those paths
distinguished explicit coarse/fine modes from default platform policy and led to
a live coarse-mode experiment. Unresolved kernel externals and relocation
warnings limit type/address interpretation; no loaded module was patched.

The [detailed NVIDIA report](../../docs/wiki/nvidia.md) contains function names,
primary-source links, driver compatibility and experimental limits. R580 is the
Pascal branch to retain; NVIDIA's open kernel modules require newer hardware.
The observed coarse RTD3 result is specific to this firmware/driver/GPU, rather
than proof of general Pascal support.

## Implemented and deployed changes

Runtime suspend clears the clock offsets. The updated
[`prime-run`](../../src/gpu-power/prime-run) obtains the users lock, brings up the
driver, holds `/dev/nvidia0` open, restores offsets, launches the application,
then releases the device and applies the idle policy. The child does not inherit
the wrapper's device/lock descriptors. Immediate-child HUP/INT/TERM forwarding,
stdin and exit status were checked with fixtures.

[`gpu-power.c`](../../src/gpu-power/gpu-power.c) now attempts core and memory
setters independently, avoids an unnecessary NVML wake during coarse-mode idle
release, and reports actual runtime/module state. Configured `rtd3 1` loads
`NVreg_DynamicPowerManagement=0x01`; NVIDIA DRM modeset/fbdev are off because Intel
owns the desktop. The existing battery idle policy unloads NVIDIA when unused.
The existing offsets were retained.

Five cold-wake CUDA rounds passed self-checking `vectorAdd` and `scan`; offsets
were restored within held-open wrapper jobs and idle returned to D3cold. The
complete wrapper-plus-vectorAdd durations ranged 1.249–1.277 seconds, not an
isolated hardware wake measurement. PRIME OpenGL rendered on the MX150. No new
Xid was found in the selected retained log. AC charging and unavailable NVIDIA
power telemetry prevent a demonstrated wattage or battery-runtime claim.

## Intel routing and application holders

Bitwarden held NVIDIA device/render-node handles and kept the coarse-mode GPU
awake. Added [`igpu-run`](../../src/gpu-power/igpu-run), selecting the explicit
Intel PCI device, Mesa GLX/EGL vendor and Intel Vulkan ICD. It clears inherited
NVIDIA offload/layer/filter and Mesa device overrides before direct execution.
Missing required manifests cause exit 69. It makes no policy writes or NVML calls.

Installed the helper and a
[Bitwarden user desktop override](../../home/local-share/applications/bitwarden.desktop).
Accelerated Intel GLX, targeted X11 EGL and Intel Vulkan passed, along with four
fixture tests. Full removal/reinstallation restored original absence then
installed checksums. The running password-manager instance was left intact:
the override takes effect on its next normal full restart. A second launch of
a single-instance application does not replace the original environment.

Use `igpu-run application` for ordinary graphics and `prime-run application`
for intentional MX150 work. Inspect `gpu-power status` and device holders before
idle measurements; NVML queries can themselves wake hardware. Details and the
separate routing rollback are in [GPU routing](../../docs/wiki/gpu-routing.md).

## Remaining defects and next checks

The cross-stack review found installed sleep hooks requesting `suspend` for all
pre-events, including hibernate, and backgrounding resume. Video-memory
preservation is enabled, making ordering significant. Correct action/phase
mapping and synchronous resume need mocked coverage before real sleep with GPU
allocations. The vendor VT helper can lose a failed `chvt` status; this was
reproduced offline. See [sleep findings](../../docs/wiki/cross-stack-review.md#3-make-sleep-one-coherent-checked-operation).

The later approved file-only pass found a site hook with corrected hibernate
mapping and synchronous resume, already present before this continuation's
queries. That observed revision hid error status; a correction and fourteen
mock lifecycle fixtures were added without deployment in that continuation.
The separate October 10 record subsequently reports installing the corrected
hook and diagnostic and passing one idle-GPU RTC S3 cycle.
See [the current sleep record](12-mx150/04-sleep-and-firmware.md).

The later workstation continuation fixes false success after node-wait or
automatic unload failure, and guards manual `off` with the users lock before
unloading. Twelve fake-device/modprobe/lock tests passed, including a held launch
lock before device open. These began source-only; the separate evening record
reports deployment through `syswork` with updated hashes. Exact whitelisted commands, fixed arguments/
environment and root-controlled paths were inspected without finding an
injection exploit. See the [NVIDIA update](../../docs/wiki/nvidia.md#later-source-only-policy-fixes).

The evening record adds idle-GPU S3/S4 and two owner-operated lid wakes; live
GPU allocations, battery transitions, MX150 Vulkan and external-display coverage
remain open. Measure application/device state and battery
energy before attributing savings to Intel routing. Keep +200/+1250 offsets:
earlier +250 MHz core tuning produced silent CUDA errors.
