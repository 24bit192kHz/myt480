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

The kernel command line has `panic=10`: after a kernel panic the machine restarts. A hang
that is not a panic is caught by the PCH TCO watchdog, which
`system/etc/local.d/watchdog.start` arms at boot: 30 s without a keepalive and the machine
resets. A firmware hang before Linux runs is not covered: do not test firmware when nobody
can reach the power button.

For a hang that leaves no trace, see "Debug build" in `coreboot/README.md`.

## Boot time

Measured on a warm reboot with `cbmem -t`, GRUB's `boottime` and the kernel log
(`tools/tscmono.c` gives the offset between the reset and the kernel's clock).

| Stage | Before (September 2026, C35) | Now (C42 and later, same boot path up to C51) |
|---|---|---|
| coreboot, reset to payload | 1,026 ms | 650-710 ms with the GPU, 450 ms without |
| GRUB, start to kernel jump | 680 ms | 50 ms |
| kernel, start to init | 570 ms | 410 ms |
| Xorg starts | 1.06 s after kernel start | 0.9 s |
| window manager up | 1.74 s after kernel start | 1.5 s |
| kernel clock zero after the reset | 1.73 s | 0.96 s |
| reset to desktop | about 3.5 s | about 2.6 s |

What made the difference, in order of size:

- the panel gets its power request before FSP-S (coreboot patch 0016): its 210 ms
  power-up delay used to be waited for in the graphics init, now it overlaps FSP-S
- GRUB no longer scans 256 PCI buses per driver and no longer walks CBFS for the
  config (GRUB patch 0015): module init 201 -> 12 ms, config 139 -> 0 ms
- the HDMI DDC pads have pull-ups (0016): the kernel and Xorg probed an empty HDMI
  port with a 65 ms I2C timeout, twice at boot and on every `xrandr`
- after a reset without power loss the DIMM serial numbers are not read again
  over SMBus (0015): 130 ms
- GRUB reads a file in runs of consecutive blocks and hands them to the NVMe as 1 MiB
  transfers with proper PRP lists instead of a 4 KiB bounce buffer (GRUB patches 0016,
  0017): the 18 MB kernel loads in 21 ms instead of 120; ramstage is LZ4 compressed

What is left, and why it stays: FSP-S takes 330-480 ms with the MX150 enabled and about
180 ms without it; the difference is FSP's own init of the GPU's root port (Gen3 link
equalization), and the only settings that would shorten it limit the link speed or drop
a device, so it stays.
libgfxinit's remaining 13 ms and the 50 ms of FSP-M are at the floor. The kernel's
18 MB image loads in about 100 ms from the NVMe. A Linux payload in the flash was
considered and dropped: the kernel image does not fit next to the firmware in 16 MB,
and a small kexec kernel would add a second kernel start (about 400 ms) to save
GRUB's 140 ms.
