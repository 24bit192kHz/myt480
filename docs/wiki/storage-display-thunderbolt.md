# Storage, Intel display and Thunderbolt findings

These checks were read-only. They identify useful follow-up experiments without
changing NVMe power states, forcing i915 features, or updating controller firmware.

## NVMe health and power policy

The installed drive is a Toshiba XG6 `KXG6AZNV256G`, firmware `5108AGLA`.
The kernel's APST latency budget remains **25,000 µs**. A latency budget permits
eligible power states; it does not establish how much time the drive spends in them.

The new [SMART reader](../../tools/re-audit/nvme-health.py) issues only Get Log Page
for the 512-byte SMART/health log, with a five-second command timeout. It opens
the controller read-only and sets Retain Asynchronous Events, preserving pending
event acknowledgement. It does not issue Set Features, firmware, format, sanitize,
or self-test commands, and does not output the drive's serial number.

Its ABI and field layout follow Linux's
[NVMe ioctl definition](https://github.com/torvalds/linux/blob/v6.18/include/uapi/linux/nvme_ioctl.h)
and [SMART structure](https://github.com/torvalds/linux/blob/v6.18/include/linux/nvme.h).
The ioctl encoding is intended for this x86-64 T480. Running a health query can
wake the drive, so run it separately from an idle-energy measurement.

```sh
sudo python3 tools/re-audit/nvme-health.py /dev/nvme0
# Offline decoding of an existing raw log:
python3 tools/re-audit/nvme-health.py --decode saved-smart.bin
```

| SMART observation on 2026-10-09 | Value |
|---|---|
| Critical warning | 0 |
| Composite temperature | Approximately 27°C |
| Available spare / threshold | 100% / 10% |
| Estimated endurance used | 26% |
| Media/data integrity errors | 0 |
| Error-log entries | 0 |
| Warning / critical temperature time | 0 / 0 minutes |
| Power-on hours | 10,651 |
| Historical unsafe shutdowns | 855 |

The endurance estimate is not a remaining-life countdown. Zero reported errors
does not predict future failures. The unsafe-shutdown count is historical; this
snapshot cannot attribute it to coreboot, a particular crash, or this audit.

Keep a dated health snapshot and an independently recoverable data backup. If
storage latency or suspend errors occur, compare the existing APST policy with
one reversible change while recording latency, kernel errors and energy. There
is no measurement here justifying a more aggressive APST budget or drive update.
Four fixture tests verify the request/ABI, large 128-bit counters, temperatures,
sparse sensor numbering and malformed log rejection; the live health read also
succeeded. The eight temperature-sensor slots retain their original numbering,
using `null` for unavailable readings.

## Intel display: inspect actual state before forcing features

The live i915 debug state reported:

- Framebuffer compression enabled, actively compressing, with the display plane
  eligible for FBC.
- DMC firmware initialized and loaded from `i915/kbl_dmc_ver1_04.bin`, version 1.4.
- Zero observed DC3→DC5 and DC5→DC6 counters at that snapshot.
- Intel GPU idle with the display active and the PCI device in D0.
- The PSR status read returned `ENODEV`.

An automatic module parameter (`-1`) is not proof that a feature is inactive.
FBC is already working. The PSR read failure establishes that this status endpoint
could not report a PSR context; it does not establish why or prove a panel fault.
Likewise, D0 on the graphics device driving a lit display is not the same problem
as the otherwise unused MX150 retaining device handles.

Use [Intel application routing](gpu-routing.md), a consistent screen brightness,
and the [power sampler](power-measurement.md) first. Compare display state and
package residency under the same workload before forcing PSR/GuC/DC options.
Any later display experiment needs flicker, video playback, external-monitor and
resume checks, with the original setting retained for recovery.

## Thunderbolt and USB-C are separate layers

The JHL6240 host is bound to Linux's Thunderbolt driver. Its readable NVM version
is **23.0**. Lenovo's
[critical-update bulletin](https://pcsupport.lenovo.com/gb/en/products/laptops-and-netbooks/thinkpad-t-series-laptops/thinkpad-t480-type-20l5-20l6/20l5/solutions/ht508988-critical-intel-thunderbolt-software-and-firmware-updates-thinkpad)
lists T480 firmware `N24TH08W` / minimum NVM **20**. This host is above that minimum;
that comparison does not certify it as the newest firmware or validate a dock.
No Thunderbolt firmware was written during this audit.

The domain reports security `none` and `iommu_dma_protection=0`. Under the
[Linux Thunderbolt interface](https://www.kernel.org/doc/html/latest/admin-guide/thunderbolt.html),
`none` permits automatic firmware connection, and a value of `1` would advertise
the platform's IOMMU DMA protection. The captured domain does not advertise that
protection. Review this policy when connecting untrusted PCIe-capable docks; do
not add blanket auto-authorization rules as a performance optimization.
No authorization or security policy was changed here.
The subsequent [cross-stack review](cross-stack-review.md) confirmed VT-d is
enabled but the NHI's current group uses an identity mapping; these are separate
observations from the domain's advertised protection flag.

The missing UCSI ACPI device is a separate firmware problem. A working
Thunderbolt host does not restore USB-C connector status, role control, or the
stock EC mailbox notification path. See [USB-C reconstruction](usb-c.md) for the
recovered transport, required synchronization and unresolved hardware behavior.

Useful dock tests are connection/disconnection, charging, USB devices, external
display, PCIe peripherals and suspend/resume. Record the device and topology for
each result; this pass had no attached dock to establish those outcomes.

## Evidence

Private logs are retained under
`/home/btw/test/rea/work/audit-20261009-routing`: `nvme-health.json`,
`display-storage-thunderbolt.txt`, and `roundtrip-routing.txt`.
They are distinct from the passive power capture. The additional domain
protection read and successful X11 EGL probe are included in the final validation
capture for this pass. No raw firmware or drive identity is published here.
