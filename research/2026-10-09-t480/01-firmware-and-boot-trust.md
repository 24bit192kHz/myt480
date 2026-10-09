# Firmware and boot trust — 2026-10-09

The T480 runs coreboot C55 with GRUB and a signed custom kernel. This research
found two priorities before further firmware experiments: tighten the early-init
boundary around TPM unlock, and repair GRUB's NVMe failure handling. Neither fix
is deployed. This audit did not flash firmware, dump live SPI, change TPM policy,
or perform a physical key-release attack.

## TPM unlock crosses the plain-root compatibility path

The built-in [early init](../../kernel/early-init/init.c) classifies the root and
swap headers, then attempts to load the trusted master `t480-kmk` before deciding
whether to mount an encrypted or plain root. A plain root can subsequently supply
the privileged init program. The retained master can authorize kernel consumers
of wrapped volume keys; its non-exportability does not close that boundary.

The offline harness included the actual source and mocked devices, key operations,
mounts and execution. **Assuming a successful unseal against matching TPM policy**,
it reproduced master load → plain-root mount → root init execution. It did not
test TPM cryptography, extract a key, or exploit the laptop. A valid hibernation
image can resume the legitimate system first; the clean-boot path still needs a
stricter boundary. Extending PCR 8 prevents another matching-policy unseal but
does not revoke the master already loaded.

Daily boot should require the expected encrypted root before releasing production
secrets. Preserve plain-disk provisioning in an explicit recovery path that does
not automatically unseal them. The separate `kmk.next` migration blob has no PCR
policy and relies on deleting its file for one-shot use; a retained copy has no
TPM-enforced single-use guarantee. No such file remained on the laptop at review.
The [early-init README](../../kernel/early-init/README.md) and
[cross-stack review](../../docs/wiki/cross-stack-review.md#1-the-tpm-boot-path-needs-a-stricter-root-boundary)
explain the current design and the required encrypted/plain-root, passphrase,
missing-disk and hibernation fixtures.

## GRUB defects are tied to the archived C55 build

The final GRUB source revision was `fb40995250874853982cb5cc8416cad4fce11706`.
Its reviewed `nvme.mod` occurs byte-for-byte in the archived C55 candidate's
extracted payload. This proves the build contains the reviewed module; it is
**not** a comparison against a fresh live firmware dump.

| Finding | Concrete impact | Evidence and next validation |
|---|---|---|
| Timeout leaves a submission outstanding | The configured retry can obtain `NULL` and dereference it; late completions also need correct ownership | [Timeout patch](../../firmware/grub/patches/0015-nvme-real-timeouts-pci-scan-only-existing-buses-memd.patch), lines 134–136; [NVMe patch](../../firmware/grub/patches/0001-Add-native-NVMe-driver-based-on-SeaBIOS-out-of-tree-.patch), line 694. Inject timeout, queue-full and late-completion cases; recover the controller and buffer lifetime before retrying |
| Success predicate ignores status-code type | Raw completion status `0x201` passes despite decoded command-specific error `0x100` | NVMe patch, line 364. Check SC and SCT with offline vectors |

These are latent defects, not observed live NVMe timeouts. A software queue reset
alone cannot establish safety while the controller can still complete DMA.
The [full firmware review](../../docs/wiki/cross-stack-review.md#2-repair-the-bootloader-failure-paths-before-more-firmware-experiments)
records the cumulative source and recovery requirements.

## Make the build and flash gates truthful

| Current weakness | Reversible next step before use |
|---|---|
| [Debug defconfig](../../firmware/coreboot/site-local/t480-debug.defconfig) selects older GRUB branch `t480`, omits production PGP modules and measured boot, although described as the same image with logging | Derive it from [production](../../firmware/coreboot/site-local/t480.defconfig) and compare boot policy offline |
| [romcheck.sh](../../firmware/tools/romcheck.sh) accepts matching UUID text without verifying actual config/key/auth contents | Compare explicit extracted artifacts; a deliberately stale fixture already returned `OK` |
| [Option-ROM patch](../../firmware/coreboot/patches/0013-local-dGPU-wait-for-the-link-before-FSP-S-bound-the-.patch), lines 80–81, can choose a header length beyond its CBFS bound | Reject oversized copies and test truncated ROMs; the current VBIOS fits and did not trigger this defect |
| [qtest52.py](../../firmware/tools/qtest52.py) reports failures but exits 0 | Return nonzero; the failing-stub fixture reproduced the problem |
| [Cold](../../firmware/tools/flashrom.sh) and [warm](../../firmware/tools/flashrom-warm.sh) flash wrappers finish failed-verification branches successfully | Propagate failure and resolve any pending migration blob; no flash was run |

The first publication-ready deliverable is the evidence and a repair plan, not a
new ROM. Preserve the known working image and passphrase recovery while testing
changes offline, then validate a candidate through the ordinary recovery workflow.
Archived ROM/module hashes, harness assumptions and private evidence locations
are in [evidence](../../docs/wiki/evidence.md#cross-stack-assessment). No proprietary
raw firmware or key material is included in this report.
