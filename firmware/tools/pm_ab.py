import time,glob,os,subprocess,sys
P="/sys/kernel/debug/pmc_core/package_cstate_show"
def pc():
    d={}
    for l in open(P):
        k,v=l.split(":"); d[k.strip()]=int(v)
    return d
def rapl(n): return int(open(f"/sys/class/powercap/intel-rapl:0{n}/energy_uj").read())
def core_c():
    t={}
    for s in glob.glob("/sys/devices/system/cpu/cpu*/cpuidle/state*/"):
        n=open(s+"name").read().strip(); t[n]=t.get(n,0)+int(open(s+"time").read())
    return t
def fr(): 
    v=[int(open(f).read()) for f in glob.glob("/sys/devices/system/cpu/cpu*/cpufreq/scaling_cur_freq")]; return sum(v)/len(v)/1000
def measure(label,secs=20):
    a=pc(); e0=rapl(""); g0=rapl(":1") if os.path.exists("/sys/class/powercap/intel-rapl:0:1") else 0; c0=rapl(":0"); t0=time.time(); cc0=core_c()
    fs=[]
    for i in range(secs): time.sleep(1); fs.append(fr())
    b=pc(); e1=rapl(""); c1=rapl(":0"); g1=rapl(":1") if g0 else 0; t1=time.time(); cc1=core_c()
    dt=t1-t0; tot=dt*1e6  # usec
    res={k:(b[k]-a[k])/tot*100 for k in a}
    ctot=sum(cc1[k]-cc0[k] for k in cc1)
    print(f"{label:38s} pkg {(e1-e0)/dt/1e6:5.2f} W  cores {(c1-c0)/dt/1e6:4.2f} W  gfx {(g1-g0)/dt/1e6:4.2f} W | "+" ".join(f"{k.replace("Package ","P")} {v:4.1f}%" for k,v in res.items() if v>0.05)+f" | avg cur freq {sum(fs)/len(fs):4.0f} MHz")
def w(p,v):
    try: open(p,"w").write(v)
    except Exception as ex: print("write fail",p,ex)
measure("A: current AC (perf gov, rpm on, aspm def)")
w("/sys/module/pcie_aspm/parameters/policy","powersupersave")
for d in glob.glob("/sys/bus/pci/devices/*/power/control"): w(d,"auto")
time.sleep(3)
measure("B: + PCI runtime PM auto + ASPM pss")
for c in glob.glob("/sys/devices/system/cpu/cpu*/cpufreq/"):
    w(c+"scaling_governor","powersave"); w(c+"energy_performance_preference","performance")
time.sleep(3)
measure("C: + powersave gov, EPP performance")
