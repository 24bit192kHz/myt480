# Kernel and security — 2026-10-09

The running kernel is **7.2.8-7-t480**, and its captured configuration matches
[config-7.2.8-7-t480](../../kernel/config-7.2.8-7-t480), SHA256
`bad21a17f884dcad8726dea817cba49f3384840cace55459b44296210cce3b1e`.
[kernel/config](../../kernel/config) is the base before tailoring, so its enabled
features do not describe the laptop. This audit did not install a new kernel or
change its command line, hardening settings or sleep policies.

## What the actual build supports

NVMe, ext4 and i915 are built in. The only initramfs is the embedded early init;
dm-crypt, AES-NI and TPM trusted/encrypted key support are built in for disk
unlock. The NVIDIA module is built through DKMS against the matching headers.
GVT-g and VFIO are intentional VM features: `VFIO_PCI=m` and
`INTEL_IOMMU_DEFAULT_ON=y` are enabled. Their presence was previously obscured
by descriptions of the base configuration; the
[kernel README](../../kernel/README.md) now identifies the final configuration
as authoritative.

The package description's AutoFDO/Propeller wording is also insufficient:
both options are disabled in the final snapshot. Inspect the resulting config
and measured behavior when making a performance claim.

## Performance settings are security choices

| Current setting or observation | Meaning and useful next comparison |
|---|---|
| Live `mitigations=off`; applicable vulnerability files report vulnerable states | Deliberate existing policy. Mitigations are compiled in; compare a separate mitigated boot profile under the same workloads |
| Module signatures, lockdown, audit and major MAC stacks disabled by tailoring | A smaller tuned kernel with fewer enforcement/diagnostic options. Build a separate audit/security profile when those capabilities are needed |
| Active LSMs `capability,landlock,yama` | This identifies active mechanisms, not proof that every application has a confinement policy |
| FTRACE/BTF and allocation initialization disabled | Some tracing and hardening facilities are unavailable in this build. Enable them in a diagnostic comparison kernel, preserving the tuned kernel for recovery |
| `iomem=relaxed` retained | Existing internal-flashing maintenance requirement; keep that requirement explicit when evaluating a hardened profile |

No speed, battery or stability improvement from reversing these choices was
measured. Treat each comparison as an experiment, with matched workloads and
boot recovery, rather than calling the tuned settings broken drivers.
[Linux parameter documentation](https://docs.kernel.org/admin-guide/kernel-parameters.html)
and the [cross-stack review](../../docs/wiki/cross-stack-review.md#4-separate-the-tuned-kernel-from-an-auditsecurity-profile)
provide the detailed policy context.

VT-d is enabled and 18 IOMMU groups were observed, but host devices default to
passthrough. The Thunderbolt NHI `05:00.0` shares group 13 with `04:00.0`, and that
group's observed type is `identity`. `07:00.0` is xHCI, not the NHI. The existence
of IOMMU groups therefore does not establish translated host-DMA isolation.
A translated/strict host profile needs dock and VM compatibility checks before
changing the default. See [USB-C and Thunderbolt](05-usb-c-and-thunderbolt.md).

## Sleep is a cross-driver operation

The original review found incorrect NVIDIA hibernate mapping/asynchronous resume,
unverified locker readiness, watchdog error gaps and a raw `rtc-hibernate` entry
that bypassed hooks. That review did not induce an unlocked resume, failed disarm
or hardware sleep transition. The separate [evening record](../../docs/notes/2026-10-09-evening.md)
reports subsequent changes and idle-GPU RTC S3/S4 tests:

- The package NVIDIA hook was masking the site override. Removing it and adding
  `NoExtract` makes the site's hibernate mapping and bounded synchronous resume
  effective. The [later approved file observation](12-mx150/04-sleep-and-firmware.md)
  confirmed those changes. The separate October 10 record subsequently reports
  deploying truthful helper/timeout status, the file diagnostic and the lid-hook
  opt-out cleanup correction, followed by one successful idle-GPU RTC S3 cycle.
- The watchdog hook now has a forced disarm fallback and checks keepalive startup
  before claiming rearming. Failure/cancellation sequencing and the final timer
  state still require broader validation.
- `rtc-hibernate` now arms the RTC and enters through `loginctl hibernate`, so the
  ordinary sleep hooks run.
- The locker still accepts an existing `slock` process or launches one in the
  background without proving that the intended session has acquired its grabs.

Locker readiness, live GPU-allocation preservation and coherent failure cleanup
remain open. `AllowSuspendInterrupts` was deliberately left off: enabling it can
cancel sleep on a failed pre-hook without running post hooks, leaving earlier
preparations undone. A nonzero hook exit alone therefore does not cancel current
elogind sleep. Review transaction/unwind behavior with mocked failures before a
separately approved hardware trial. The
[sleep-path review](../../docs/wiki/cross-stack-review.md#3-make-sleep-one-coherent-checked-operation)
links exact paths and the vendor/elogind contracts.

## Priorities and evidence limits

Fix the [early-init trust boundary](01-firmware-and-boot-trust.md) first, then
make sleep sequencing coherent. Keep the known working kernel and an independently
usable passphrase recovery path while preparing replacement signed builds.
Create a separate hardening/tracing profile and measure its cost before choosing
defaults. TYPEC is disabled in the running kernel, but enabling it alone cannot
restore the missing firmware UCSI transport.

Microcode 0xf6, DMC 1.4, active Intel FBC, matching NVIDIA 580.178.04 components and
working watchdog recovery were observed. GVT's missing `golden_hw_state` message
uses a supported fallback; it was not proof of failed virtualization. Retained
logs showed no matching current NVIDIA Xid, NVMe timeout/reset or AER failure,
which is limited evidence rather than complete workload validation. TSC-adjust
and DP-adapter messages still need event correlation. The
[evidence page](../../docs/wiki/evidence.md) records the tools and captures;
[storage/display findings](../../docs/wiki/storage-display-thunderbolt.md) cover
the measured device state.
