#!/usr/bin/env python3
# EXPERIMENT 2026-10-08: EC indexed register space at ports 0x15EC (index) / 0x15EE (data).
# usage: ecidx.tmp read IDX | setbit IDX BIT | clearbit IDX BIT
import os, sys
fd = os.open("/dev/port", os.O_RDWR)
def rd(i): os.pwrite(fd, bytes([i]), 0x15ec); return os.pread(fd, 1, 0x15ee)[0]
def wr(i, v): os.pwrite(fd, bytes([i]), 0x15ec); os.pwrite(fd, bytes([v]), 0x15ee)
cmd, idx = sys.argv[1], int(sys.argv[2], 0)
if cmd == "read": print("%02x" % rd(idx))
else:
    bit = int(sys.argv[3]); v = rd(idx)
    nv = v | (1 << bit) if cmd == "setbit" else v & ~(1 << bit)
    wr(idx, nv); print("%02x -> %02x" % (v, rd(idx)))
