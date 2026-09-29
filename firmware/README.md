# Firmware

coreboot with a GRUB 2 payload, built from upstream sources plus the patches here.

| Directory | Content |
|---|---|
| `coreboot/` | base commit, patches, configuration and `site-local/` (the board's own files and blobs) |
| `libgfxinit/` | patches for coreboot's graphics init submodule; apply before the coreboot patches |
| `grub/` | patches for the payload |
| `upstream-dgpu-series/` | the MX150 work again, as a series for coreboot review |
| `stock-reference/` | what Lenovo's firmware does, for comparison |
| `tools/` | helpers that run on the laptop |
| `MX150-notes.md` | history of the dGPU work |

Build order: libgfxinit patches, coreboot patches, GRUB tree, `site-local/`, toolchain,
`make`. Each directory's README has its part; the top-level README has the commands.

## Flashing

The first flash needs an external SPI programmer (Lenovo's firmware write-protects the
chip). After that, from Linux:

```sh
flashprog -p internal -r backup.rom                     # always first, and keep it
flashprog -p internal --fmap-file new.rom -i FMAP -i RW_SPD_CACHE -i COREBOOT -w new.rom
```

Do not write the whole BIOS region: that replaces `RW_MRC_CACHE` with an empty one and
the next boot trains the memory again for about 14 s.

## When a boot hangs

Hold the power button for 4 s, then power on. coreboot sees the forced power-off,
clears the dGPU request and asks GRUB to show its menu. If that does not help, write
your last good image, or the dump of the stock firmware, with the external programmer.

The kernel command line has `panic=10`: after a kernel panic the machine restarts.
There is no watchdog (`nowatchdog`, no `iTCO_wdt`), so a hang that is not a panic stays
a hang. Do not test firmware when nobody can reach the power button.

For a hang that leaves no trace, see "Debug build" in `coreboot/README.md`.
