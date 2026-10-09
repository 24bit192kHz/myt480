# C52 boot policy scenarios: signature enforcement and GRUB password.
# Each step: (text to wait for, monitor commands sent 0.3 s after it appears).
import subprocess, socket, time, os, re, sys
Q=os.path.expanduser("~/t480-build/tools/qemu"); ROM=os.path.expanduser("~/t480-build/src/coreboot/build-qemu/coreboot.rom")
MENU="Artix Linux (linux-t480)"; BOOT="T480-KERNEL-SMOKE"; ASK="Enter username"
def run(name, disk, steps, want, forbid=(), timeout=40):
    log=f"{Q}/serial-{name}.log"; mon=f"{Q}/mon-{name}.sock"
    for f in (log,mon):
        try: os.unlink(f)
        except FileNotFoundError: pass
    p=subprocess.Popen(["qemu-system-x86_64","-enable-kvm","-machine","q35","-cpu","host","-m","1024","-bios",ROM,
        "-drive",f"file={Q}/{disk},if=none,id=d0,format=raw,snapshot=on","-device","nvme,drive=d0,serial=t480test",
        "-serial",f"file:{log}","-monitor",f"unix:{mon},server,nowait","-display","none","-no-reboot"],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    t0=time.time()
    while not os.path.exists(mon) and time.time()-t0<5: time.sleep(0.01)
    s=socket.socket(socket.AF_UNIX); s.connect(mon); s.setblocking(False)
    steps=list(steps); pos=0; txt=""; done=0
    while time.time()-t0<timeout:
        try: s.recv(65536)
        except Exception: pass
        txt=open(log,errors="replace").read() if os.path.exists(log) else ""
        if steps:
            i=txt.find(steps[0][0],pos)
            if i>=0:
                pos=i+len(steps[0][0]); time.sleep(0.3)
                for c in steps[0][1]: s.sendall((c+"\n").encode()); time.sleep(0.08)
                steps.pop(0)
        elif all(w in txt for w in want):
            # leave time for something forbidden to show up
            done=done or time.time()
            if not forbid or time.time()-done>3: break
        time.sleep(0.01)
    p.kill(); p.wait()
    if os.path.exists(mon): os.unlink(mon)
    ok=not steps and all(w in txt for w in want) and not any(f in txt for f in forbid)
    print(f"{'PASS' if ok else 'FAIL'}  {name}" + ("" if ok else f"   steps left {len(steps)}, missing {[w for w in want if w not in txt]}, forbidden seen {[f for f in forbid if f in txt]}"))
    return ok
def keys(s): return [f"sendkey {'ret' if c=='\n' else c}" for c in s]
J="Jumping to boot code"; esc=(J,["sendkey esc"])
T=[("S1 corrupted sig no key","disk-badsig.img",[],["Failed to boot both default and fallback",MENU],[BOOT]),
   ("S2 missing sig no key","disk-nosig.img",[],["Failed to boot both default and fallback",MENU],[BOOT]),
   ("S3 tampered kernel no key","disk-tamper.img",[],["Failed to boot both default and fallback",MENU],[BOOT]),
   ("S4 menu default entry t","disk.img",[esc,(MENU,["sendkey t"])],[BOOT],[ASK]),
   ("S5 menu Enter on default","disk.img",[esc,(MENU,["sendkey ret"])],[BOOT],[ASK]),
   ("S6 menu stock kernel l","disk.img",[esc,(MENU,["sendkey l"])],[ASK],[BOOT]),
   ("S7 menu lts kernel L","disk.img",[esc,(MENU,["sendkey shift-l"])],[ASK],[BOOT]),
   ("S8 menu SeaBIOS s","disk.img",[esc,(MENU,["sendkey s"])],[ASK],[BOOT,"SeaBIOS (version"]),
   ("S9 menu edit e","disk.img",[esc,(MENU,["sendkey e"])],[ASK],[BOOT]),
   ("S10 menu shell c","disk.img",[esc,(MENU,["sendkey c"])],[ASK],[BOOT,"grub>"]),
   ("S11 menu shell c wrong password","disk.img",[esc,(MENU,["sendkey c"]),(ASK,keys("btw\n")),("Enter password",keys("wrong\n"))],[ASK],[BOOT,"grub>"]),
   ("S12 menu corrupted sig t","disk-badsig.img",[esc,(MENU,["sendkey t"])],["signature."],[BOOT]),
   ("S13 menu tampered kernel t","disk-tamper.img",[esc,(MENU,["sendkey t"])],["signature."],[BOOT])]
def main(argv=None):
    sel = sys.argv[1:] if argv is None else argv
    unknown = sorted(set(sel) - {t[0].split()[0] for t in T})
    if unknown:
        print("Unknown scenario(s): " + ", ".join(unknown), file=sys.stderr)
        return 2
    fails = [t[0] for t in T if (not sel or t[0].split()[0] in sel) and not run(*t)]
    print("ALL PASS" if not fails else "FAILURES: " + ", ".join(fails))
    return 1 if fails else 0


if __name__ == "__main__":
    sys.exit(main())
