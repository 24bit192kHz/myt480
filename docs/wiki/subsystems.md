# Other subsystems and remaining experiments

The [collector](../../tools/re-audit/collect.sh) reads policy/status endpoints and
selected error messages. It does not benchmark devices, enumerate personal
files, probe GPU clocks through NVML, or access raw firmware/EC ports.

```sh
sudo sh tools/re-audit/collect.sh
```

## Inventory and decisions

| Subsystem | Observed on AC | Decision / next useful measurement |
|---|---|---|
| CPU | i7-8650U, `intel_pstate`, performance governor/EPP; intel_idle C1..C10 available | Retained existing AC policy. Compare energy and response time on battery before changing EPP or turbo |
| CPU voltage/limits | Explicit per-source undervolt; AC PL1 64 W / PL2 90 W, TCC 95°C | Retained previously tested values. Missing-config default is now zero undervolt |
| Intel graphics | UHD620; live FBC compressing, DMC 1.4 loaded; PSR status unavailable | [Actual display state](storage-display-thunderbolt.md) checked; no forced feature enablement |
| NVIDIA | MX150; initial idle tests reached D3cold, while the initial final check found Bitwarden holding handles; the evening run subsequently records D3cold with Bitwarden open | Wrapper restoration, coarse RTD3 and actual `bw-screen` Intel restart routing deployed; see [GPU routing](gpu-routing.md). Live GPU allocation preservation remains open |
| NVMe | Toshiba XG6, APST budget 25,000 µs; SMART no critical/media errors, 26% endurance used | [Read-only health reader](storage-display-thunderbolt.md) added; no justification for tightening APST |
| PCIe | Global ASPM `powersupersave`; GPU port deliberately excludes ASPM | Keep known port workaround; previous freezes make blind ASPM changes inappropriate |
| Wi-Fi | Intel 8265/8275, `iwlwifi` module power_save=N on AC | Compare throughput, latency, and discharge under the battery profile first |
| Ethernet | I219-LM; existing AC-only wake policy | Retained; monitor wake sources and overnight discharge |
| Audio | `snd_hda_intel power_save=0` on AC | Avoid changing active-audio behavior without pop/click and wake-latency tests |
| Thunderbolt | JHL6240 bound; NVM 23.0; domain `none`, advertised DMA protection 0 | [NVM/domain findings](storage-display-thunderbolt.md) recorded; test an actual dock; no firmware update |
| USB-C | TYPEC disabled and UCSI ACPI transport absent | Kernel option alone is insufficient; [native EC bridge route](usb-c.md) documented |
| USB / fingerprint | Two readers, libfprint selected; alternate services down intentionally | Retain backend selection and isolate cancellation/resume tests |
| Sleep | `deep` selected; existing GPU saved-VRAM and sleep-guard hooks; evening record reports RTC-woken idle-GPU S3/S4 and two owner-operated S4 lid wakes | Watchdog/RTC and lid-hook deployment are recorded; live GPU allocations, cancellation/failure cleanup and retained EC baseline/rollback remain open; see [evening results](../notes/2026-10-09-evening.md) |
| Crash recovery | TCO watchdog and NMI/soft-lockup policy already configured | Retained. Do not kill the TCO keepalive as an idle-power experiment |
| Firmware / boot trust | Coreboot C55; TPM-bound disk-key and signing workflow already documented | No flash, key export, reseal, or boot-chain changes during this audit |

No targeted Xid, NVMe timeout, iwlwifi error, AER error, or critical-thermal match
appeared in the inspected current kernel log. This is an observation of the
retained log, not proof that all previous boots or all hardware workloads are clean.

The two battery capacities were approximately 87% and 85% early in the audit,
with AC connected and charging activity. Capacity percentages and AC charging
cannot establish system idle watts. A repeatable energy experiment should log
power source, charging state, battery energy/rate, CPU package residency,
GPU sysfs state, screen brightness, radios, and workload together. The new
[power sampler](power-measurement.md) implements that observation without NVML;
the continuation's AC capture reports CPU-package energy only.

## Firmware issues already investigated

The repository's [2026-10-08 note](../notes/2026-10-08-port-fixes.md) records the
stock SMM sleep handlers, C55/C56/C57 experiments, and hibernation/TCO fix. This
audit inspected that history and extended the SMM analysis to the USB-C mailbox.
It did not repeat the failed lid-wake experiments or change EC indexed bits.
The later [EC static analysis](../../research/2026-10-09-t480/10-ec-firmware-and-lid-wake.md)
recovered a distinct retained lid gate through ordinary EC byte `0x01` bit 6;
the separate [evening record](../notes/2026-10-09-evening.md) then reports the
installed hook and two successful owner-operated S4 lid wakes. Those results
do not prove every retained-state, cancellation or S5 path.

Do not interpret the absence of EFI in this custom legacy-boot kernel as an
accidental omission. Enabling it would not reconstruct the vendor UCSI service.
Likewise, enabling every module from a reverse-engineering catalog has no
demonstrated performance benefit. Changes need a hardware use case and a
measurement against this system's baseline.

Useful follow-up work is: native USB-C transport; GPU resume with live allocations; battery energy
measurement; fingerprint trace comparison; dock/DP-alt-mode coverage; then
NVMe, Wi-Fi, and iGPU policy experiments one variable at a time. Each should keep
its own original policy and reproduce failures before making it permanent.
