# Host-side matching for the 06cb:009a: measured, not shipped

Question (2026-10-01): can matching on the host instead of the sensor chip be both
**faster** and **as secure** as the chip's match-on-sensor?

## Speed

| step | chip (today) | host |
|---|---|---|
| touch -> image | 80 ms | ~140 ms (calibration-mode frame grab) |
| match | ~900 ms on the chip | 12 ms (correlation) / 1-10 ms (minutiae) |

Host matching is about 5x faster. Speed was never the problem.

## Security

The sensor is 112 x 112 px, about 5.6 x 5.6 mm at ~500 ppi. One touch carries only
0-10 minutiae. Measured with `validity-rs host-eval` on FVC2000/2002/2004 (B sets,
120 fingers), simulating the sensor with crops of the benchmark prints:

**NIST NBIS reference (MINDTCT + BOZORTH3):**

| probe area | real fingers rejected at FAR 1/1000 | real attempts that score 0 |
|---|---|---|
| 1 touch (112 px) | 98% | 96% |
| 2 touches (160 px) | 63% | 40% |
| 3.2 touches (200 px) | 39% | 9% |
| whole print | 14% | 0% |

The same NBIS matcher works on whole prints, so the failure on single touches is the
sensor area, not the code. Image correlation was worse for a different reason: it
accepted the user's own index finger (7/11) and synthetic stripe patterns (28/108).

Other facts that count against host matching on this sensor:

- Frames from `frame` / `frame-live` arrive on bulk endpoint 0x82 **outside the TLS
  channel**, so a host matcher would trust pixels that tampered hardware could replay.
  The chip's verdict arrives over the paired, encrypted channel.
- A stored host template is biometric data at rest.
- `fprint-mindtct` 0.1.0 panics (index out of bounds) on some valid 250x250 images.

**Conclusion:** the chip stays the matcher. Host matching would need several touches'
worth of rolled finger area to approach a safe false-accept rate, which takes longer
than the chip's ~1 s. Nothing here is wired into the daemon or the unlock path.

## What exists (src/host/, diagnostics only)

- `mosaic.rs`: dark-frame + flat-field calibration estimated from a moving-finger video,
  frame registration (cross-correlation), sigma-clipped stacking. `validity-rs host-mosaic`.
- `minutiae.rs`, `mcc.rs`: own minutiae extractor and Minutia Cylinder-Code matcher.
  Experimental: weaker than NBIS (whole-print FRR ~73% at FAR 1/1000).
- `eval.rs`: FAR/FRR harness, `--matcher mcc|nbis` (`nbis` needs `--features nbis`).
- `synth.rs`: synthetic prints with known minutiae (`host-synth`), `host-selfcheck`.

Reproduce (datasets: ~/datasets/fvc/fetch.sh + to-pgm.sh, research use only):

    cargo build --release --features nbis
    validity-rs host-eval ~/datasets/fvc-pgm/tune --matcher nbis --probe 112
    validity-rs host-eval ~/datasets/fvc-pgm/tune --matcher nbis --probe 0   # whole prints
