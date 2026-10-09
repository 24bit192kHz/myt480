# Early init: encrypted root and swap without a prompt

`init.c` is a small static program built into the kernel as its only initramfs
(`CONFIG_INITRAMFS_SOURCE`, see `../PKGBUILD`). It opens the LUKS2 root and swap
partitions with a key the TPM releases, resumes from hibernation or mounts root, and
hands over to `init=` (`t480-init`). It never starts a shell. If the TPM refuses the
key it asks for the LUKS passphrase on the console.

## How the key is protected

- coreboot measures itself, GRUB and `grub.cfg` into TPM PCR 2 (measured boot). GRUB on
  coreboot cannot measure what it starts, so GRUB checks the kernel's signature instead
  and asks a password for every entry that could start something else.
- The volume keys are kernel `encrypted` keys (`t480-root`, `t480-swap`), wrapped by a
  kernel `trusted` key (`t480-kmk`). The kernel unseals that one itself over an
  encrypted TPM session. There is no LUKS keyslot for the TPM; slot 0 is the passphrase.
- The TPM policy is: PCR 2 has the sealed value and PCR 8 is still zero. The early
  init extends PCR 8 right after the attempt, so the key can be released once per boot
  and nothing that runs later can get it again.
- Each partition is looked at on its own (LUKS2 header or not), so the same kernel boots
  the machine before and after the conversion.
- The master key is released only to open a LUKS2 root (since 2026-10-10, kernel
  7.2.8-8). With a plain root the next code to run would be that root's init, which
  nothing has measured or signed, and the key would sit in its keyring: a plain disk
  swapped in next to the real `/boot` would have got it (found by the 2026-10-09 audit,
  [cross-stack review](../../docs/wiki/cross-stack-review.md)). The one exception is
  the check boot of the conversion, which needs the word `t480.provision` on the kernel
  command line. The command line comes from the GRUB configuration that coreboot
  measures into PCR 2, so only the GRUB shell or an entry behind the GRUB password
  can add a word the default entry does not have. A plain root next to an encrypted
  swap asks the passphrase for the swap.

The `kmk.next` migration blob still relies on file deletion rather than TPM-enforced
single use; it exists only between `t480-reseal next-boot` and the next boot.

## Files

| File | Use |
|---|---|
| `init.c` | the early init; log in `/dev/t480-early.log` |
| `Dockerfile` | build and test image (Alpine): musl toolchain, a static cryptsetup for the passphrase fallback, swtpm and tpm2-tools for the tests |
| `Dockerfile.toolbox` | the tool set the conversion runs from RAM; also the root of the first test replica |
| `Dockerfile.archroot` | root of the second test replica: bash, util-linux and coreutils as on Artix |
| `t480-reseal` | TPM side at runtime, installed as `/usr/local/sbin/t480-reseal` (see below) |
| `convert/t480-init.convert`, `convert/convert.sh` | the in-place conversion: a one-shot stand-in for `t480-init` copies the toolbox into RAM, leaves the root file system and encrypts it |
| `fallback/` | mkinitcpio hook `t480crypt` for the distro kernels: one passphrase opens root and swap |
| `qemu/` | the rehearsal: `go.sh` builds a replica disk with the same odd partition layout and walks through sealing, the plain-root refusal without `t480.provision`, the check boot with it, conversion, TPM boot, hibernate, one-shot blob, changed firmware, passphrase, re-key; `grubpath.py` and `stocktest.py` cover GRUB and the distro kernel |

Blobs in `/boot/t480/`: `kmk.blob` (trusted key, policy PCR 2 + 8), `kmk.next` (one-shot,
no policy), `root.key`, `swap.key` (encrypted keys), `root.dm`, `swap.dm` (data offset and
sector size).

## Build

```sh
docker build -t t480-early .
docker run --rm -v "$PWD:/w" -w /w t480-early sh -c \
  'mkdir -p out && gcc -Os -static -o out/init init.c && strip out/init && cp /usr/local/bin/cryptsetup.static out/cryptsetup'
mkdir -p ../early/root/bin ../early/root/{dev,proc,sys,boot,newroot,run}
cp out/init ../early/root/init && cp out/cryptsetup ../early/root/bin/cryptsetup
```

Then build the kernel. `qemu/go.sh` needs the built `vmlinuz` in `qemu/work/` and runs
the whole procedure with a software TPM; run it before touching a real disk.

## Converting a machine

1. Kernel with the early init installed and booted (it boots a plain disk as before).
2. A backup you have verified.
3. As root: two 64-byte volume keys and a passphrase file in `/var/lib/t480-convert/`,
   then `t480-reseal init`, `wrap root`, `wrap swap` into `/var/lib/t480-convert/t480/`,
   copy them to `/boot/t480/` and reboot once with `t480.provision` added to the kernel
   line (GRUB shell, password): the log must say "TPM key released". Without the word a
   plain root leaves the key sealed ("plain root: TPM key left sealed").
4. Unpack the toolbox image to `/var/lib/t480-convert/toolbox`, copy `convert.sh` there,
   install `t480-init.convert` as `t480-init` (keep the original as `t480-init.real`),
   `swapoff -a`, `touch /var/lib/t480-convert/armed`, reboot.
5. The machine converts itself (about 20 minutes for 200 GiB) and reboots. Up to the
   partition table rewrite every failure boots the unchanged system.

`/boot` becomes a third partition that takes over the old root UUID, with a symlink
`boot -> .`, so the GRUB configuration in the firmware finds the kernel where it did.

## Day to day

| Command | When |
|---|---|
| `t480-reseal next-boot` | before every firmware flash; `firmware/tools/flashrom.sh` does it. The first boot of the new firmware then re-seals by itself (`local.d/t480-reseal.start`) |
| `t480-reseal` | after a firmware change when the machine is up and the key is loaded |
| `t480-reseal rekey` | after a boot with the passphrase: new master key, keys wrapped again |
| `t480-reseal status` | PCRs, loaded keys, blobs, dm tables |

Anything that changes what coreboot measures into PCR 2 makes the next boot ask for
the passphrase unless `next-boot` ran first. On this board that included the MX150
option ROM, which is only loaded while the GPU is on; `t480.defconfig` therefore lists
it as runtime data (PCR 3).
