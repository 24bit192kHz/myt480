# 2026-11-02: hibernation wake, measured from power-on

The wake is timed with the TSC: it restarts at power-on (S4 is a cold start) and
keeps counting into the resumed kernel, so a 20 ms sampler that reads it right
after the wall clock jumps gives the exact time from power-on to the thaw; coreboot's
own timestamps use the same counter. The RTC alarm only has 1 s resolution.

## Where the time went (C65, kernel 7.2.8-8, 2 GB image)

| Stage | Time |
|---|---|
| power-on latency (RTC alarm to reset, Deep Sx exit) | ~1 s (±1 s, alarm resolution) |
| coreboot | 1.04 s (Thunderbolt firmware start 0.29 s, FSP-S 0.20 s) |
| GRUB (kernel load, signature) | ~0.47 s |
| kernel to `/init` | 0.96 s (tpm_tis 0.46 s: the TPM2 HMAC null key; i915 0.13 s; DEBUG_WX 0.1 s) |
| early init (TPM unseal, dm-crypt) | 0.11-0.19 s |
| image read + restore | ~2.4 s (about 0.5 s per GB of image) |
| device restore in the resumed kernel | 1.23 s |
| power-on to thaw | 6.36 s (mean of 4) |

## Changes

- Kernel 7.2.8-9: i915 as a module that the early init loads only when no image is
  restored. With i915 in the restore kernel the panel was lit and switched off again
  before the jump, and the resumed kernel had to power-cycle it (graphics restore
  0.96 s). Now 0.015-0.038 s. Power-on to thaw 5.93 / 6.19 / 5.99 / 6.20 s, mean 6.08 s.
  DEBUG_WX off. Cold boot unchanged in behaviour: i915 loads in a child parallel to the
  root mount (`Run /init` 0.96 -> 0.86 s).

## Measured and kept as is

- Image cap 1 GiB instead of 2: wake 0.4-0.5 s faster, but entry 4.5-7 s (anonymous
  memory pushed to swap) and a sluggish session after the wake.
- TPM null key (0.46 s): needed for the encrypted session that unseals the disk key.
- Thunderbolt Set_Security_Level (0.29 s): the controller's own firmware start; the
  USB-C port depends on it.
- The disk unlock does not use Argon2 on the TPM path (the volume keys are kernel
  encrypted keys; 0.11 s).

## Promising, not shipped

USB `power/persist=0` on the internal devices (Bluetooth, camera, fingerprint, touch
screen, card reader) during hibernation: they re-enumerate after the thaw instead of
being reset one by one inside it (device restore 1.23 -> 0.39 s, wake 0.95 s faster with
the i915 change). One run (C65, persist=0, i915 still in the restore kernel) hung with
the fan on and no screen, stage unknown; not deployed. A camera in use or a mounted SD
card would need to be excluded.
