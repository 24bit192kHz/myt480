# MX150 driver, voltage and EC research

Continuation date: **2026-10-09**. Device anchor: Pascal GP108M MX150,
`10de:1d10`, previously inspected with coreboot C55, Artix/s6,
Linux `7.2.8-7-t480` and proprietary NVIDIA `580.178.04`.
Static analysis uses retained artifacts on the workstation. After explicit user
approval, supported getter and file/X-session queries were also run on the T480.
No tuning setting, module policy, EC state or firmware was changed.

**CPU undervolting already works on this machine. NVIDIA 580's recovered voltage
offset control cannot undervolt the MX150:** its RM backend clamps negative
requests to zero and returns a minimum offset of zero. The GPU physically
controls its regulator's PWM input and has internal voltage machinery, so this
does not prove every possible V/F-policy approach impossible. An alternative
usable control/policy, GP108 calibration, reliable readings and stability at a
lower voltage remain unproved. Existing positive clock offsets are not a
measured undervolt.

| Subject | Report |
|---|---|
| Undervolting, clock controls and their units | [Voltage and supported interfaces](01-voltage-and-controls.md) |
| Closed NVIDIA 580 internals | [RM clock, voltage, power and resume paths](02-closed-driver.md) |
| Full VBIOS, regulator wiring and EC throttle | [Firmware and board control](03-vbios-and-ec.md) |
| Runtime power-on and system sleep correctness | [Sleep and power failure handling](04-sleep-and-firmware.md) |
| CUDA, applications, media, measurement and recovery | [Getting the most out of the MX150](05-use-and-validation.md) |
| What is covered, exact evidence and remaining dependencies | [Coverage and evidence](06-coverage-and-evidence.md) |
| Newer NVIDIA branches and possible patches | [610/615 support, recovered GP108 initialization blocks and expected gains](07-driver-upgrade-and-pascal-support.md) |

The requested [Ghidra MCP](https://github.com/bethington/ghidra-mcp) was used
again. Full automatic analysis created a 61,679-function inventory; selected
control paths were reviewed, rather than treating that count as complete
reverse engineering. Independent ELF relocations/GNU disassembly and the exact
driver's open interfaces were used to check important decompiler conclusions.
Raw driver disassembly, firmware captures and private projects remain outside
the publication.

The useful improvements prepared here are a getter-only capability inventory,
a corrected sleep configuration diagnostic, truthful source-only hook status,
and firmware power-good/CBFS ROM bounds corrections. Their offline checks do
not prove a successful hardware transition.
The [coverage ledger](06-coverage-and-evidence.md) explicitly records unresolved
areas, including alternative undervolt policy, VBIOS authentication, offset lifetime,
EC policy meanings, MX150 Vulkan and allocation-preserving sleep. The approved
queries confirmed NVML frequency getters and unavailable existing-session
NV-CONTROL, rather than establishing a new voltage setter. No claim of
100% recovery of the proprietary stack is made.

For the whole laptop, continue with the [whole-machine ledger](../11-coverage-and-feature-roadmap.md),
[EC lid investigation](../10-ec-firmware-and-lid-wake.md) and
[daily guide](../../../docs/wiki/getting-the-most.md).

The later workstation-only [upgrade investigation](07-driver-upgrade-and-pascal-support.md)
verifies 615.78.08 as the newest public Linux display release and 580.178.04 as
the latest compatible Pascal release found. Proprietary 610/615 retain shared
Pascal fragments but clear GP108's physical HAL entry and omit its object
registration. A PCI-ID/legacy-check patch alone cannot initialize this GPU.
No new driver was installed, no target command ran, and no performance benefit
or usable new-branch patch is claimed.
