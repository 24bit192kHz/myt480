# MX150 series for upstream coreboot

The dGPU support as five patches on coreboot main (e1207bdb84, 2026-09-27), meant for
https://review.coreboot.org. coreboot takes patches through Gerrit, not pull requests.

| Patch | What it does |
|---|---|
| 0001 | `_ROM` gets the whole option ROM, not only what the one-byte size field of its header describes; never more than the CBFS file holds |
| 0002 | power the dGPU in the bootblock and wait for its link, so FSP-S finds it on root port 1; full reset on the first boot after enabling it; power off before sleep |
| 0003 | ACPI power resource: the OS can turn the GPU off and on at runtime |
| 0004 | keep the setup option visible after the dGPU was disabled |
| 0005 | documentation of the board |

The difference to the flashed build: this series reads the option `dgpu_enable`, the
flashed build reads CMOS byte 0x6f (it has no option backend).

## State

| | |
|---|---|
| Builds | T480, T480s, T580, X280, T470s, X380 Yoga; every patch on its own for the T480; with the setup menu (CFR) for T480 and X280 |
| `checkpatch` | clean, apart from the sign-off that you add |
| Tested on hardware | the code of 0001 to 0003 in builds for this T480 (C35, and C36 with the option read from CBFS in the bootblock): warm reset, first boot after enabling, S3 resume with a program on the GPU, 20 on/off cycles; the link wait reported 0 ms on a warm boot and 2 ms on resume |
| Not tested | this series as built from coreboot main (only on the branch this laptop runs, base 61483663b2); a cold boot with the final code; patch 0004; T480s and T580 |

## What reviewers will ask

- **The full reset in 0002** rests on what this one machine does: root port 1 stays
  disabled across warm resets and comes back after a reset with a power cycle, and
  `MEM_SR` tells the two apart. The numbers are in the commit message. There is no
  Intel document behind it.
- **T480s and T580** share the code and the pins. Somebody with one has to test.
- **`drivers/lenovo/hybrid_graphics`** is for the older ThinkPads with a display
  switch and does not fit this design, but it is the precedent for powering a GPU
  before the ramstage.

## Before sending

1. Boot the series as built from main, with the external programmer at hand.
2. `make gitconfig` in the coreboot tree (commit hook for the Change-Id).
3. Commit under your real name and sign off:
   `git rebase origin/main --exec 'git commit --amend --no-edit --reset-author --signoff'`
4. Make the TEST= lines say what you tested.
5. `git push origin HEAD:refs/for/main%topic=t480-dgpu`

Send 0001 first: it stands on its own.
