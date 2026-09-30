#!/usr/bin/env python3
"""every 10 s: mean core MHz, package temperature (coretemp), RAPL package watts -> stdout"""
import os, struct, time, glob
ct = [os.path.dirname(f) + "/temp1_input" for f in glob.glob("/sys/class/hwmon/hwmon*/name") if open(f).read().strip() == "coretemp"][0]
fd = os.open("/dev/cpu/0/msr", os.O_RDONLY); u = struct.unpack("<Q", os.pread(fd, 8, 0x606))[0]; eu = 1 / (1 << ((u >> 8) & 0x1f))
def energy(): return struct.unpack("<Q", os.pread(fd, 8, 0x611))[0]
while True:
    e0 = energy(); time.sleep(10); e1 = energy()
    mhz = [float(l.split(":")[1]) for l in open("/proc/cpuinfo") if l.startswith("cpu MHz")]
    print("%d %d %.1f" % (sum(mhz) / len(mhz), int(open(ct).read()) // 1000, ((e1 - e0) & 0xffffffff) * eu / 10), flush=True)
