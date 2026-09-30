#!/usr/bin/env python3
"""uvset.py core cache gpu uncore analogio  (mV, negative = undervolt) : write the OC mailbox offsets and read them back. No args: read only."""
import os, struct, sys
PLANES = (("core", 0), ("gpu", 1), ("cache", 2), ("uncore", 3), ("analogio", 4))
fd = os.open("/dev/cpu/0/msr", os.O_RDWR)
def rd(plane):
    os.pwrite(fd, struct.pack("<Q", 0x8000001000000000 | (plane << 40)), 0x150)
    v = struct.unpack("<Q", os.pread(fd, 8, 0x150))[0]; o = (v >> 21) & 0x7ff
    return round((o - 2048 if o > 1023 else o) / 1.024)
def wr(plane, mv):
    o = int(round(mv * 1.024)) & 0x7ff
    os.pwrite(fd, struct.pack("<Q", 0x8000001100000000 | (plane << 40) | (o << 21)), 0x150)
if len(sys.argv) == 6:
    want = dict(zip(("core", "cache", "gpu", "uncore", "analogio"), map(int, sys.argv[1:])))
    for name, plane in PLANES: wr(plane, want[name])
print(" ".join("%s=%d" % (name, rd(plane)) for name, plane in PLANES))
