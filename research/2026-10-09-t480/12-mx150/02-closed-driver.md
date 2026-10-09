# Closed NVIDIA 580 control paths

## Artifact and method

Analyzed retained `nvidia.ko` from `580.178.04`: ELF64 little-endian x86-64,
relocatable object, **143,576,728 bytes**, SHA-256
`99946c338065a775ce13fdb49179440d2ae9732767545e77524c3078fb2a0ce1`.
Used Ghidra 12.1.4/JDK 21 with the requested bethington MCP checkout
`9cc29c0f1efb6c63a7d6898c9a23aff39397f992` (v7.0.0).
Saved a persistent `mx150-static` project; automatic analysis completed in
285 seconds with 61,679 functions. Forty-nine selected entry points were
inventoried and forty-four independent GNU disassembly files saved.
These counts measure tool output and selected evidence, not fully understood
functions or complete proprietary-driver recovery.

ELF relocations and same-version Linux glue checked selected call targets.
Ghidra relocates `.text`; its addresses differ from ELF section-relative offsets.
Unsupported `R_X86_64_PC64` relocations, unresolved kernel externals and thunk
injection can mislead decompilation. In particular, a displayed
`nv_pci_tegra_pm_init` → return-thunk substitution was checked against its
original relocation and open implementation; it is not a real recovered
initializer call chain.

## NVML clock transport and corrected RM dispatch

The exact NVML library SHA-256 is
`451ad740fe549ed7023570fda15ba2762f6fb29ea50afe468b0eb5e74246a4c1`.
Legacy GPC/memory setters submit control **`0x2080d031`**, payload size `0x608`.
The exact NVIDIA NVOC runtime record layout puts the function pointer 16 bytes
before the method ID. Using that layout, its kernel record selects
`_nv057516rm`; the earlier interpretation incorrectly read the next record's
function and is discarded. The correct handler obtains a clock controller,
checks its mode gate and calls a resolved interface's method `+0x28`.
Missing controller returns`0x56`, invalid interface`0x40`, and the additional
mode gate`0x6c`; its final instantiated method implementation remains unresolved.

```text
legacy NVML frequency-offset setter
  → RM 0x2080d031 / 0x608
  → _nv057516rm → clock-controller interface +0x28 (final implementation open)
```

The separately analyzed `_nv057593rm` clock-controller/board-object route is
not proved to be this command's handler and is excluded from that claim.
The modern userspace setter checks
effective root, record version `0x01000018`, nonzero limits and signed range.
It selects newer `0x2080e06b`/`0x1a0` or legacy routes according to interface
version. These newer control IDs also carry voltage descriptors in the X driver;
they are multipurpose performance transport, not uniquely clock commands.
Final instantiated clock-descriptor methods and independent per-state behavior
remain open; the approved modern getters accept all 32 state/domain inputs.
These identifiers document analysis; they are not instructions to issue raw RM controls.

## NV-CONTROL voltage reaches RM transport

In the exact `nvidia_drv.so`, the attribute 412 callback table at ELF `0x65aa20`
binds setter `0x52d00`, getter `0x52cb0` and range getter `0x52c60`.
Attribute 413 binds a read-only getter. The support check requires Coolbits bit 4
(mask 16) and a backend-derived support flag. Its range comes from signed backend
min/max fields, rather than a fixed positive-only X clamp.

The setter forwards the unchanged 32-bit µV offset in a descriptor through
`0x2080e06b`/`0x1a0`. Both getters use `0x2080a06a`/`0x1a0`, but attribute 412
returns the signed offset from descriptor `+0x14`, while attribute 413 returns the
separate unsigned current-voltage field at `+0x20`. Capability initialization requests voltage
descriptor id 1, combines its returned support bit with Coolbits 16, and retains
returned min/max values. That initializer runs on a performance-interface
version `0x20` branch; the `0x30` branch does not run this initialization.

The correct NVOC layout now also establishes the lower route:

```text
set 0x2080e06b → _nv012995rm serializer → expanded 0x2080e06c
  → _nv058537rm → _nv015047rm (global descriptor id 1) → _nv060791rm
get 0x2080a06a → _nv012987rm serializer → expanded 0x2080a06b
  → _nv058419rm → _nv015047rm → _nv060792rm
```

The setter's common accepted path tests the signed request, uses zero for every
negative value, then clamps the nonnegative value to its maximum. Independent
GNU checks at ELF offsets `0x9c9b3c`/`0x9c9b43`/`0x9c9b4f` confirm the signed
test, conditional selection and unsigned maximum clamp. The getter explicitly
writes minimum offset 0 at descriptor `+0x18`, supplies the internal maximum at
`+0x1c`, current offset at `+0x14` and the separate current-voltage result at `+0x20`.

Thus the recovered normal attribute 412/global-voltage path is **overvoltage-only**.
Signed bits forwarded by the X client cannot produce a negative undervolt here.
This does not prove every possible V/F/firmware approach impossible. The active
GP108 interface, support bit, maximum and useful voltage telemetry still require
observation. No binary clamp patch or raw assignment was attempted; bypassing
the frontend does not bypass the lower range policy.

The global dispatcher can treat the setter's unsupported status (`0x56`) as a
non-error for that descriptor. A raw successful return alone therefore would
not prove a voltage change; feature/range and actual returned offset/voltage
would have to agree. No raw RM query or assignment was performed.

## Internal voltage programming

`_nv060941rm` → `_nv000275rm` parses voltage-device objects. It accepts version
high nibble `0x10`, header ≥4 and entries ≥24 bytes. A table entry type **2**
is normalized to internal type **3**; `_nv060933rm` → `_nv060666rm` →
`_nv060662rm` constructs the object and binds its setter to `_nv060677rm`.
This explains an apparent table/internal-type mismatch.

That setter rejects zero, invalid objects and empty allowed-point lists. It
selects the first allowed point at least as large as the unsigned request, or
the final point if the request exceeds the list. It obtains a PWM tuple through
`_nv051636rm`, writes a changed tuple through `_nv051640rm`, stores the selected
point and waits for settling after success. Controller-class gates return
`0x56` for unsupported classes or `0x40` for absent required state; transient
programming conditions have timeout handling.

This is a table-bounded positive voltage/PWM path, not a recovered signed user
undervolt API. The selected GP108 object, point generation/calibration and live
limits remain unknown. BIOS-object field `+0x444` supplies a locator, but its
exact binding to this ROM's BIT P+`0x6c` has not been established. A similarly
numbered hardware-snapshot field in `_nv024568rm` was investigated and discarded
as a false lead.

`EnableCoreVoltage` toggles an initialization feature for values 0/1.
`RmNapllCalVoltOffset` affects NAPLL calibration under a packed gate.
Neither is demonstrated to be a generic user voltage setting. Initialization,
firmware-control and voltage settling-delay strings do not establish useful
undervolting knobs.

## Runtime power gates

`_nv054158rm` calls `nv_get_screen_info` and returns whether framebuffer size is
nonzero. The matching source identifies a console framebuffer on the GPU;
this is a console-use guard, not a Pascal fine-DPM capability detector.

`rm_init_dynamic_power_management` handles explicit modes 1 and 2 separately
from default mode 3's chip/platform/chassis/battery policy. Linux glue checks
runtime-PM/sysfs availability and `_PR3`; allocation failure disables selection.
`rm_enable_dynamic_power_management` requires a nonzero selected mode and no
console/primary-VGA guard in wrapper `+0x2cd`.

`_nv049866rm` decrements a protected idle reference and changes internal PM state
at zero; its paired increment can wake the GPU and propagate errors. This proves
reference bookkeeping, rather than validating fine DPM on Pascal. The earlier
five-round coarse-mode experiment remains the hardware evidence for this
particular machine. [NVIDIA's documented RTD3 platform requirements](https://download.nvidia.com/XFree86/Linux-x86_64/580.178.04/README/dynamicpowermanagement.html)
are narrower than every behavior obtainable by forcing a parameter.

## Resume and the missing offset-reset instruction

`_nv000816rm` selects `_nv031151rm` or `_nv031152rm` from saved state, restores
a small enum through `_nv037755rm`, and on one path obtains ACPI power source
and submits **`0x2080205b`**, named `PERF_SET_POWERSTATE` in NVIDIA's
[exact public control header](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/580.178.04/src/common/sdk/nvidia/inc/ctrl/ctrl2080/ctrl2080perf.h).
An alternative `_nv030380rm` resume path has explicit entry, in-progress,
failure and successful cleanup states. `_nv031152rm` restores subsystems through
partly resolved class methods and records failure stage/status.

The post-resume path `_nv000841rm` invokes `_nv059551rm` with a timeout value
`5000000000`; its timer units and relationship to clock offsets were not proved.
No selected routine reveals the exact instruction clearing MHz offsets.
The earlier observed offset loss and wrapper restoration remain valid, but this
pass has not turned that observation into an exact patchable reset site.

No binary patch, undocumented ioctl, forced PWM or registry-policy change was
performed. Approved supported getter observations are recorded in
[the interface report](01-voltage-and-controls.md#approved-getter-observations).
Next static dependencies are active voltage-object construction/support/max,
voltage table/allowed-point binding, final clock methods, offset storage/lifetime
and GP108 ROM authentication. Each needs independent evidence before a patch.
