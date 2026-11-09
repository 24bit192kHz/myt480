#!/usr/bin/env python3
# Read-only: per-source interrupt and softirq rates over N seconds, plus top CPU users.
import sys, time, os
def snap(p):
    d = {}
    for l in open(p).read().splitlines()[1:]:
        f = l.split()
        if not f: continue
        n = 0; i = 1
        while i < len(f) and f[i].isdigit(): n += int(f[i]); i += 1
        d[f[0] + " " + " ".join(f[i:])[:40]] = n
    return d
def procs():
    d = {}
    for p in os.listdir("/proc"):
        if p.isdigit():
            try:
                s = open(f"/proc/{p}/stat").read(); c = s[s.index("(")+1:s.rindex(")")]; f = s[s.rindex(")")+2:].split()
                d[(p, c)] = int(f[11]) + int(f[12])
            except Exception: pass
    return d
n = float(sys.argv[1]) if len(sys.argv) > 1 else 5
a, sa, pa = snap("/proc/interrupts"), snap("/proc/softirqs"), procs(); time.sleep(n)
b, sb, pb = snap("/proc/interrupts"), snap("/proc/softirqs"), procs()
for name, x, y in (("irq", a, b), ("softirq", sa, sb)):
    r = sorted(((y[k] - x.get(k, 0)) / n, k) for k in y)
    print(name + ": " + "; ".join(f"{k.strip()}={v:.0f}/s" for v, k in r[::-1][:9] if v >= 1))
r = sorted(((pb[k] - pa.get(k, 0)) / n, k) for k in pb)
print("cpu%: " + "; ".join(f"{k[1]}[{k[0]}]={v:.1f}" for v, k in r[::-1][:10] if v >= 0.3))
