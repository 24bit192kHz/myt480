# 2026-11-10: idle power, the lost package C8, GVT-g back

Firmware untouched (C65). Kernel 7.2.8-9 rebuilt in place (same release name): new built-in
early init, one GVT-g patch. Everything else is configuration.

## What was wrong

Idle on AC, screen locked: package 5.67 W (RAPL), package C-states C2/C3 only,
5600 interrupts/s. On 2026-10-01 the same machine idled 42 % in package C8.

| Cause | Effect | Fix |
|---|---|---|
| DeepFilterNet LADSPA plugin inside PulseAudio (`module-ladspa-sink`): its worker thread polls ~1600 times a second for as long as the plugin exists, also with every sink suspended. `unload-module` does not stop it; every reload leaked one more such thread. | No package C8 since the mic chain was set up (10-13); `PC8 0 %` in the 10-26 notes. | `dfn-mic` runs the filter in an ffmpeg process (`-af ladspa=...`, mic -> `dfn_out`) only while something records from `mic_denoised`, and kills it afterwards. PulseAudio: 1629 -> 2 wakeups/s. |
| With the NVIDIA driver loaded (policy on AC), any program that only enumerates GPUs opens the MX150: the Vulkan loader reads `nvidia_icd.json`, glvnd tries `10_nvidia.json` first. Wine does both. The driver's coarse runtime D3 then never suspends the GPU, `gpu-power off` cannot unload the driver ("the GPU is in use"), also after unplugging. Found with a Dev-C++ console left open: `explorer.exe` held the GPU. | GPU in D0, link in L0 (no ASPM on that port, as on the vendor firmware): package capped at C3. | `~/.xinitrc` exports the `igpu-run` variables for the whole session (`__GLX_VENDOR_LIBRARY_NAME=mesa`, `__EGL_VENDOR_LIBRARY_FILENAMES`, `VK_DRIVER_FILES`, `VK_ICD_FILENAMES`); `prime-run` unsets the three file variables. Only `prime-run` reaches the MX150 now. |
| `performance` governor + EPP on AC with 5000 wakeups/s | cores at 4.2 GHz for every wakeup: core power 2.9 W at idle | gone with the wakeups (now 0.3-0.9 W); policy unchanged |

Not a cause, although it looked like one: the LTR values of the Wi-Fi card (88 us) and of
the dGPU port (102 us). Ignoring them in `pmc_core/ltr_ignore` changed nothing.

## Measured (AC, locked, package power from RAPL, 15-25 s windows)

| State | pkg W | PC8+PC9 |
|---|---|---|
| as found (GPU held by Wine, DFN thread) | 5.67 | 0 % |
| now, panel on, globe at 15 fps, performance governor | 2.36 | 36 % |
| now, panel off (120 s into the lock) | 0.91 | 51 % |
| battery profile (`tlp bat`), globe at 10 fps, before the reboot | 1.39 | 30 % |
| battery profile, globe stopped | 0.58-0.60 | 68-73 % |

The globe wallpaper costs about 0.8 W of package power on battery (1.39 vs 0.58 W).
Left as it is.

Tools: `tools/idle/` (`pkgc.py` = package/core C-state residency and RAPL power from the
MSRs, `wakeups.py` = context switches per thread, `irqdiff.py` = interrupt rates,
`idle-matrix3.sh` = step-by-step matrix that restores every setting). The kernel has no
ftrace, turbostat and powertop are not installed.

## GVT-g

Gone since 7.2.8-9 (11-02): i915 became a module loaded by the built-in early init with
`finit_module(fd, "", 0)`. `i915.enable_gvt=1` on the command line reaches a built-in
i915 or one loaded by modprobe (which reads `/proc/cmdline` itself), not this one. No
mdev types, `docker start WinApps` failed on `/dev/vfio/17`.

`kernel/early-init/init.c` now passes the `i915.*` words of the command line to i915.ko;
if the load fails with parameters it is repeated without them. Tested in QEMU (plain-root
replica): old image `enable_gvt=N`, new image `Y`, bogus value -> logged, loaded without
parameters. On the T480: `i915-GVTg_V5_4` and `V5_8` are back, the Windows VM boots with
the vfio-pci mdev device. The image was rebuilt in the existing tree (`makepkg -e`), the
release name stays `7.2.8-9-t480` (build #6; superseded the same night by build #8 with
the scheduler patch below). A real hibernation with RTC wake passed on build #6 (resume
8.3 s from the alarm).

The static init is built with `gcc -Os -static` in Alpine 3.22 (`build-base
linux-headers`), as `kernel/early-init/README.md` says.

### The scheduler tick (kernel patch 0002)

With GVT-g back, a second idle regression showed up, and it explains the `PC8 0 %,
PC6 33 %` of the 10-26 notes: from the first start of a vGPU on, `gvt_service_thread` woke
1000 times a second, for ever, also with the guest paused or shut down. The time-based
scheduler's 1 ms hrtimer is started with the first vGPU and cancelled only when the last
vGPU is destroyed, and the mdev is created at boot and never destroyed. Cost at idle after
one use of the VM: package 0.6 -> 1.1 W on battery settings (no PC8), 0.9 -> 3.8 W on AC.

`kernel/0002-t480-gvt-stop-the-scheduler-tick-while-idle.patch` (kvmgt.ko,
`gvt/sched_policy.c`): `intel_gvt_schedule()` stops the timer when no vGPU in the run queue
has a pending workload and nothing but the idle vGPU is scheduled, and starts it again on
the next pass with work. A queued workload always causes such a pass
(`intel_vgpu_queue_workload()` kicks the scheduler). The PKGBUILD applies it as a local
source; tonight it was applied to the existing tree and built with `makepkg -e`
(`/proc/version` build #8, 2026-11-09 21:43 UTC).

Tested on the T480 (`up2/gvt-tick-test.sh`, log `work/gvt-tick-test.log`): Windows boots,
two headless RDP sessions of 45 and 30 s stay connected, pause, unpause, clean shutdown,
`hibtest.sh`; the service thread stays below 1 wakeup/s in every state. The guest reports
`Intel(R) UHD Graphics 620 / OK / CM_PROB_NONE`, driver 31.0.101.2145. Idle after the VM
ran and stopped: package 1.02 W on AC with the panel off, PC8+PC9 74 %. Not tested: a
graphics load inside the guest for longer than a minute, two vGPUs at once.

Fallbacks on the T480: `~/t480-build/kernel/prev-9c/` (parameters fix only) and
`prev-9b/` (the 11-02 packages); `/boot/vmlinuz-linux-t480-7.2.8-9.bak` is the 11-02 image.

## Smaller changes

- `thermald.conf`: fan off below about 48 C (`fan_levels 0:0:48,2:40:52,...`; it never
  stopped before and ran 3200 rpm at a 39 C idle). AC undervolt core/cache -115 -> -105:
  the sweep hit an MCE at -124 and the -115 soak logged one after a suspend.
- The mic filter now needs about 2 s to start when a recording begins (model load in the
  new process); before, the plugin was always loaded.

## Looked at, not changed

- Thunderbolt in coreboot: the first mailbox command waits about 265 ms for the
  controller's firmware on a cold boot. Sending the commands after enumeration would hide
  that wait behind the payload load, but then devices attached at boot are no longer
  connected by the firmware (`Connect_Topology` before the bus scan). Not testable
  without a Thunderbolt device, so not done.
- ASPM on the dGPU root port: the vendor firmware has it off too.
- Wi-Fi power save while unlocked on battery (0.4 W, measured 10-26): still off, latency.
- `mitigations=auto`: unchanged. `retbleed=off` would avoid IBRS on every kernel entry;
  a security decision, and any change there must pass `work/hibtest.sh`.
