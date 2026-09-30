# GRUB payload

Patches on top of upstream GRUB at `BASE-COMMIT` (18 commits after the tag grub-2.14).

| Patches | What they do |
|---|---|
| 0001 | native NVMe driver (from SeaBIOS, as carried by Libreboot) |
| 0002, 0003 | build fixes for 32-bit `grub_size_t` |
| 0004, 0006 | timeouts and `sleep` in milliseconds |
| 0005 | modifier keys through `getkeystatus` |
| 0007, 0008 | which modules are preloaded on coreboot |
| 0009 - 0014 | PS/2 keyboard through the ThinkPad EC: scan code set, keys pressed before GRUB started, debug output |
| 0015 | NVMe: commands time out instead of hanging; PCI: scan only buses that exist (module init 201 -> 12 ms); `site.cfg` from the memdisk |
| 0016 | file reads merge consecutive blocks into one disk request, NVMe transfers of 1 MiB (kernel load 120 -> 105 ms) |

coreboot builds the payload from `payloads/external/GRUB2/grub2` and checks out the
revision named in the defconfig (`CONFIG_GRUB2_REVISION_ID="t480-fast"`). So that directory
has to be a GRUB clone with a branch `t480-fast`:

```sh
cd coreboot/payloads/external/GRUB2
git clone https://git.savannah.gnu.org/git/grub.git grub2
cd grub2
git checkout -b t480-fast "$(cat BASE-COMMIT)"
git am patches/*.patch
```

Test changes to GRUB or `grub.cfg` in QEMU before flashing. An untested change to the
keyboard handling once hung every warm reboot.
