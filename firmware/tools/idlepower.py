#!/usr/bin/python3
# idlepower.py LABEL [SECONDS]: idle power of the current state, one line (root).
# Battery draw (only meaningful on battery), RAPL package power, package C-state residency,
# power state of the dGPU and its root port.
import sys, time, glob, os
label=sys.argv[1]; secs=int(sys.argv[2]) if len(sys.argv)>2 else 60
P="/sys/kernel/debug/pmc_core/package_cstate_show"
def pc(): return {l.split(":")[0].strip(): int(l.split(":")[1]) for l in open(P)}
def rapl(): return int(open("/sys/class/powercap/intel-rapl:0/energy_uj").read())
def bat():
    w=0
    for b in glob.glob("/sys/class/power_supply/BAT*"):
        try: w+=int(open(b+"/power_now").read())
        except OSError: pass
    return w/1e6
def rd(p):
    try: return open(p).read().strip()
    except OSError: return "-"
a=pc(); e0=rapl(); t0=time.time(); s0=int(rd("/sys/kernel/debug/pmc_core/slp_s0_residency_usec") or 0); bs=[]
for i in range(secs): time.sleep(1); bs.append(bat())
b=pc(); e1=rapl(); dt=time.time()-t0
res=" ".join(f"{k.replace('Package ','P')} {(b[k]-a[k])/(dt*1e6)*100:.0f}%" for k in a if (b[k]-a[k])/(dt*1e6)>0.005)
bs.sort(); med=bs[len(bs)//2]
g="/sys/bus/pci/devices/0000:01:00.0/power_state"; r="/sys/bus/pci/devices/0000:00:1c.0"
line=(f"{time.strftime('%T')} {label}: battery median {med:.2f} W (min {bs[0]:.2f}), AC={rd('/sys/class/power_supply/AC/online')}, "
      f"RAPL pkg {(e1-e0)/dt/1e6:.2f} W | {res} | gpu {rd(g)}, port {rd(r+'/power_state') if os.path.exists(r) else 'absent'} "
      f"rt {rd(r+'/power/runtime_status')}, backlight {rd('/sys/class/backlight/intel_backlight/brightness')}")
print(line); open("/home/btw/t480-build/work/idlepower.log","a").write(line+"\n")
