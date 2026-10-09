# Keep ordinary applications on Intel

## Why runtime suspend was still blocked

At the continuation audit's baseline, `bitwarden-app` held `/dev/nvidiactl` and
the NVIDIA render node. The GPU remained active/D0 with coarse RTD3 enabled.
Coarse mode requires applications to close their GPU device handles. This is
a distinct problem from restoring offsets after wake: the wrapper fix cannot
make hardware sleep while another application owns it.

Application startup can discover more than one graphics driver. The existing
Vulkan wallpaper launcher already limits discovery to Intel, avoiding extra
drivers and libraries. The new [`igpu-run`](../../src/gpu-power/igpu-run) applies
that approach to ordinary GLX/EGL/Vulkan applications:

- Clears inherited NVIDIA PRIME settings, forced Vulkan layers and
  Mesa driver-selection overrides.
- Selects the Intel PCI device through Mesa and the Mesa GLX vendor.
- Limits GLVND EGL discovery to `50_mesa.json`.
- Limits Vulkan discovery to `intel_icd.json`, including the older loader variable.
- Executes the application directly, preserving arguments, stdin, signals, and exit code.

The loader override is documented by [LunarG](https://vulkan.lunarg.com/doc/view/latest/linux/LoaderDriverInterface.html),
Mesa's PCI selection by [Mesa](https://docs.mesa3d.org/envvars.html), and EGL vendor
selection in [GLVND's implementation](https://github.com/NVIDIA/libglvnd/blob/master/src/EGL/libeglvendor.c).
Forced layer settings can bypass an implicit layer's usual activation context;
the [Vulkan loader layer interface](https://vulkan.lunarg.com/doc/view/latest/linux/LoaderLayerInterface.html#layer-filtering)
documents those overrides. Clearing them prevents inherited NVIDIA layer forcing.
These are per-process settings. They are not a replacement for an application's
own renderer implementation and are not intended for privileged graphics programs.

## Use

```sh
igpu-run glxinfo -B
igpu-run vulkaninfo --summary
igpu-run bitwarden

# Use the MX150 when the workload benefits from it:
prime-run your-gpu-application
```

The helper is installed at `/usr/local/bin/igpu-run`; `make install` under
`src/gpu-power` installs it alongside `prime-run`. The manifests are specific to
this T480's Mesa installation; missing files cause a clear failure instead of
silently discovering another driver.

A user desktop override at `~/.local/share/applications/bitwarden.desktop` now
launches Bitwarden through the helper. The running Bitwarden instance was left
alone. Quit and relaunch it normally to adopt the policy; sending a new URI to
an already running single-instance application does not change its environment.
The override is reproduced under `home/local-share/applications/` in the repo.

## Validation and limitations

GLX reported accelerated Mesa Intel UHD620, with direct rendering. Vulkan
reported the Intel UHD620 device. A targeted `eglinfo -B -p x11` check succeeded
with Mesa EGL 1.5 and Intel OpenGL/OpenGL ES renderers. Four fixture tests passed for inherited PRIME
and filter removal, missing drivers, arguments/stdin/exit status, and usage.
The first live version used `DRI_PRIME=0`, which current Mesa warned was invalid;
it was corrected to the explicit Intel PCI selector and revalidated.

EGL's first broad platform probe initialized supported platforms but reported
failure for some unsupported/device contexts. That is not evidence that every
EGL platform works. No Bitwarden recognition/renderer state was inspected inside
the password manager, and no battery saving is claimed until it is relaunched
and device holders/power measurements confirm the result.

To inspect holders without waking the GPU through NVML:

```sh
sudo fuser -v /dev/nvidia* /dev/dri/*
gpu-power status
```

An application explicitly creating its own NVIDIA context can bypass these
loader choices. They do not affect CUDA. Use the MX150 deliberately for GPU
compute/rendering, and Intel for everyday applications when practical.

## Revert

The original helper and desktop override were absent. Their exact installed
checksums and absence markers are recorded in
`/root/myt480-audit-20261009-routing`. The full removal/reinstallation round trip
passed: both original absence states were restored, then installed checksums and
X11 EGL rendering passed again. Running applications were unaffected.

```sh
sudo sh /root/myt480-audit-20261009-routing/rollback-routing.sh --check
sudo sh /root/myt480-audit-20261009-routing/rollback-routing.sh
```

This removes only the two added files and refreshes the desktop database. It
does not stop an application or reload a driver. The script refuses to remove
files if their checksums no longer match, preserving later user edits.
