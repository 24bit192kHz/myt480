# MX150 series for upstream coreboot

The dGPU support as four patches on coreboot main (e1207bdb84, 2026-09-27), meant for
https://review.coreboot.org. coreboot takes patches through Gerrit, not pull requests.

| Patch | What it does |
|---|---|
| 0001 | `_ROM` gets the whole option ROM, not only what the one-byte size field of its header describes |
| 0002 | power the dGPU in the bootblock, so FSP-S finds a link on root port 1; full reset on the first boot after enabling it; power off before sleep |
| 0003 | ACPI power resource: the OS can turn the GPU off and on at runtime |
| 0004 | keep the setup option visible after the dGPU was disabled |

The difference to what is flashed: this series reads the option `dgpu_enable`, the
flashed build reads CMOS byte 0x6f (it has no option backend).

## State

| | |
|---|---|
| Builds | T480, T480s, T580, X280, T470s, X380 Yoga |
| Tested on hardware | the same code in the flashed build, on a T480 only |
| Not tested | this series as built from main; patch 0004; T480s and T580 |

## Before sending

1. `make gitconfig` in the coreboot tree (commit hook for the Change-Id).
2. Sign off with your real name: `git rebase --signoff origin/main`.
3. Correct the TEST= lines if you test more.
4. `git push origin HEAD:refs/for/main`
