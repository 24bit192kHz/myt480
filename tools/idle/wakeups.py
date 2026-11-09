#!/usr/bin/env python3
# Read-only poor man's powertop: per-thread context switches/s over N seconds (root sees all).
import os, sys, time
def snap():
    d = {}
    for p in os.listdir("/proc"):
        if not p.isdigit(): continue
        try: tids = os.listdir(f"/proc/{p}/task")
        except OSError: continue
        for t in tids:
            try:
                s = open(f"/proc/{p}/task/{t}/status").read()
                name = s.split("\n",1)[0].split("\t",1)[1]
                v = int(s.split("voluntary_ctxt_switches:\t")[1].split("\n")[0])
                nv = int(s.split("nonvoluntary_ctxt_switches:\t")[1].split("\n")[0])
                d[(p, t, name)] = v + nv
            except Exception: pass
    return d
n = float(sys.argv[1]) if len(sys.argv) > 1 else 5
a = snap(); time.sleep(n); b = snap()
r = sorted(((b[k] - a.get(k, b[k])) / n, k) for k in b)[::-1]
tot = sum(v for v, k in r)
print(f"total {tot:.0f} switches/s")
agg = {}
for v, k in r:
    key = k[2] if not k[2].startswith("kworker") else "kworker/*"
    agg[key] = agg.get(key, 0) + v
print("; ".join(f"{k}={v:.0f}" for k, v in sorted(agg.items(), key=lambda x: -x[1])[:22] if v >= 1))
print("top threads: " + "; ".join(f"{k[2]}[{k[0]}/{k[1]}]={v:.0f}" for v, k in r[:12]))
