#!/usr/bin/env python3
# Read-only sampler: package/core C-state residency and RAPL power over N seconds.
# usage (root): pkgc.py [seconds] [label]
import os, struct, sys, time
def rd(msr, cpu=0):
    fd = os.open(f"/dev/cpu/{cpu}/msr", os.O_RDONLY)
    try: return struct.unpack("<Q", os.pread(fd, 8, msr))[0]
    finally: os.close(fd)
PK = {"pc2":0x60D,"pc3":0x3F8,"pc6":0x3F9,"pc7":0x3FA,"pc8":0x630,"pc9":0x631,"pc10":0x632}
CC = {"c3":0x3FC,"c6":0x3FD,"c7":0x3FE}
EN = {"pkg":0x611,"core":0x639,"gfx":0x641,"ram":0x619}
unit = 1.0 / (1 << ((rd(0x606) >> 8) & 0x1f))
def bat():
    w = 0
    for b in ("BAT0","BAT1"):
        try:
            if open(f"/sys/class/power_supply/{b}/status").read().strip() == "Discharging":
                w += int(open(f"/sys/class/power_supply/{b}/power_now").read())/1e6
        except OSError: pass
    return w
def snap():
    s = {"tsc": rd(0x10), "t": time.monotonic()}
    for k,m in PK.items(): s[k] = rd(m)
    for k,m in CC.items(): s[k] = sum(rd(m,c) for c in range(0,4))/4
    for k,m in EN.items(): s[k] = rd(m) & 0xffffffff
    s["irq"] = sum(int(x) for x in open("/proc/stat").read().split("intr ")[1].split()[:1])
    return s
n = float(sys.argv[1]) if len(sys.argv) > 1 else 20
lab = sys.argv[2] if len(sys.argv) > 2 else ""
a = snap(); bw = []
end = time.monotonic() + n
while time.monotonic() < end:
    time.sleep(min(2, max(0.1, end - time.monotonic()))); bw.append(bat())
b = snap(); dt = b["t"] - a["t"]; dtsc = b["tsc"] - a["tsc"]
out = [f"{lab:<28}"]
out += [f"{k}={100*(b[k]-a[k])/dtsc:5.1f}%" for k in PK]
out += [f"{k}={100*(b[k]-a[k])/dtsc:5.1f}%" for k in CC]
out += [f"{k}={((b[k]-a[k]) & 0xffffffff)*unit/dt:5.2f}W" for k in EN]
out += [f"irq/s={(b['irq']-a['irq'])/dt:6.0f}", f"bat={sum(bw)/max(1,len(bw)):5.2f}W"]
try: out.append("T=" + open("/proc/acpi/ibm/thermal").read().split()[1])
except Exception: pass
print(" ".join(out), flush=True)
