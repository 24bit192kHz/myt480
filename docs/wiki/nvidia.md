# MX150 / NVIDIA

## Baseline and supported software

This machine has a GP108M MX150 (`10de:1d10`, Pascal, CUDA capability 6.1) using the
proprietary `580.178.04` kernel module. R580 is the last Linux series supporting
Pascal according to [NVIDIA's legacy-driver schedule](https://nvidia.custhelp.com/app/answers/detail/a_id/3142/kw/ubuntu).
Keep its kernel module, NVML, OpenGL/Vulkan libraries, and CUDA driver compatible
when updating. The [open kernel modules require Turing or newer](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/main/README.md).

Before this audit, `NVreg_DynamicPowerManagement=0x03` selected the driver's default
policy, which disabled RTD3 on this GPU. The driver stayed loaded and the GPU
remained in D0 on AC. The existing AC offsets were core +200 MHz and memory
+1250 MHz; battery offsets were zero. Previous repository work had already found
silent CUDA errors at higher core offsets. This audit did not raise them.

## Binary analysis

REA's full native analysis of the 143,576,728-byte `nvidia.ko` timed out after
330 seconds. The Ghidra MCP fallback imported the module without automatic
analysis, recovered 61,109 symbol-backed functions, and decompiled selected RM
functions. A subsequent full auto-analysis also completed.

The module is an ELF relocatable object. Ghidra relocated the initializer to
`00e76870`; its original ELF symbol value differs. Unhandled `R_X86_64_PC64`
relocations and unresolved kernel externals appeared in the import log, so the
decompiler output is a guide to control flow, not a complete typed reconstruction.

| Function | Observed behavior |
|---|---|
| `nv_dynamic_power_available` | Linux wrapper checks the saved PCI config-file interface |
| `rm_init_dynamic_power_management` | Explicit coarse/fine settings are handled separately from mode 3's default policy and platform checks |
| `rm_enable_dynamic_power_management` | Enables a selected nonzero mode subject to additional state checks |
| `nv_pmops_runtime_suspend` | Delegates to `nvidia_transition_dynamic_power(..., true)` |
| `rm_transition_dynamic_power` | Uses RM locking and the internal transition routine; successful resume applies idle holdoff |

Those findings motivated a live test; they alone do not prove every Pascal GPU
supports runtime suspend. NVIDIA's [RTD3 documentation](https://download.nvidia.com/XFree86/Linux-x86_64/580.178.04/README/dynamicpowermanagement.html)
lists newer supported platforms. Forced coarse mode here is an experimentally
verified behavior of this particular firmware/driver/hardware combination.

## Implemented improvement

Runtime suspend clears the clock offsets. Simply enabling RTD3 loses the existing
performance tuning on the next wake. The new `prime-run` sequence is:

1. Take the existing shared policy lock and load the GPU driver.
2. Open `/dev/nvidia0` in the wrapper, keeping coarse-mode RTD3 awake.
3. Run the fixed `gpu-power tune` operation to restore offsets for the power source.
4. Run the application with PRIME variables; close the wrapper descriptors in the child.
5. Close the GPU descriptor, release the policy lock, and apply the idle policy.

The wrapper now forwards HUP/INT/TERM to its immediate child, waits for it, and
cleans up on exit. Stdin and the child's exit code are preserved. `gpu-power`
attempts both offset setters even if the first fails. `release` avoids an NVML
probe that would otherwise wake an idle RTD3 GPU just to set soon-to-be-lost offsets.
`status` reads sysfs and the active parameter instead of reporting configuration
intent as if it had already taken effect.

`rtd3 1` is now enabled in the live and repository configurations. Driver loads
use `NVreg_DynamicPowerManagement=0x01` and disable NVIDIA DRM KMS/fbdev, as the
Intel GPU drives the desktop. The modules remain loaded on AC while idle hardware
can enter D3cold. The battery policy still unloads them when no wrapper job uses
the GPU. Video-memory preservation and the existing suspend hooks were retained;
their purpose is documented by [NVIDIA's suspend guidance](https://download.nvidia.com/XFree86/Linux-x86_64/580.178.04/README/powermanagement.html).

## Results and limits

Five rounds verified D3cold before cold wake, self-checking CUDA `vectorAdd` and
`scan` results, and +200/+1250 offsets inside a held-open wrapper job. All passed.
Total cold `vectorAdd` time, including wrapper setup and program execution, was
1.277, 1.249, 1.270, 1.271, and 1.266 seconds. These are **not isolated hardware
wake latencies**. `scan` reported approximately 122.3 MElements/s. No new Xid was
found. PRIME `glxinfo -B` as user `btw` reported direct rendering on the MX150,
and idle state returned to suspended/D3cold afterward.

The laptop was charging on AC and NVIDIA's power reading was unavailable. No
wattage or battery-runtime gain is claimed. The demonstrated improvement is the
idle hardware power state while retaining offsets during wrapper jobs.

Applications launched without `prime-run` can wake the GPU without restoring
offsets. Keeping a GPU descriptor open also intentionally prevents coarse idle
suspend. At the final check, `bitwarden-app` held `/dev/nvidiactl` and the NVIDIA
render node, and the GPU was consequently active/D0 despite coarse mode being
enabled. It was left running. Use `fuser -v /dev/nvidia* /dev/dri/*` to identify
such holders before measuring idle behavior or reloading the driver. The
continuation installed [Intel routing](gpu-routing.md) and a Bitwarden desktop
override for its next normal launch. Intel GLX/EGL/Vulkan checks passed; the
existing Bitwarden instance was left running, so its post-restart device handles
and any battery improvement remain to be measured.

MX150 Vulkan, docks, real suspend/hibernate, and battery transitions were not
tested in this session. Retest after driver or firmware updates.

## Later source-only policy fixes

The workstation continuation fixes three further paths in `gpu-power.c`:
driver load fails if the NVIDIA device node never appears; automatic policy
returns the unload result; and manual `off` takes the users lock exclusively
without waiting before unloading. A held `prime-run` lock now refuses manual
unload with exit 3, including the interval before the launcher opens the device.
The manual off request is retained and can take effect when the program ends.
The nonblocking check avoids waiting for a launcher that needs the policy lock.

Twelve fake-device/modprobe/lock fixtures passed, along with the existing five
wrapper fixtures, NVML checks and a warning-free source build. **Deployed on
2026-10-09 at 19:01** (built on the laptop from this source, installed through
`syswork`, SHA256 `da0f739e…f00644`, see [rollback](rollback.md)). The audit's
original RTD3 results were taken with the preceding build; the reboot that
followed the deployment came up with the driver loaded, the GPU in D3cold and no
device holders.

The Bitwarden holder had a second cause: the laptop starts Bitwarden from
`bw-screen boot` (session script and the hibernate-later hook), never from the
desktop file, so the desktop override alone would have changed nothing after a
reboot. `bw-screen` now launches it through `igpu-run` as well.

Local checks and repeatable hardware test:

```sh
python3 src/gpu-power/tests/prime-run.py
python3 src/gpu-power/tests/tune.py
python3 src/gpu-power/tests/policy.py
# On this T480, on AC, with no GPU applications:
sudo sh tools/re-audit/test-rtd3.sh
```

The hardware script keeps a private config/mode snapshot and restores it on exit.
It depends on the existing SM61 samples under `/home/btw/t480-build/work/gpu/bench-sm61`.
For a quick policy rollback, set `rtd3 0` and reload with `gpu-power off;
gpu-power auto` after closing GPU applications. See [full restoration](rollback.md).
