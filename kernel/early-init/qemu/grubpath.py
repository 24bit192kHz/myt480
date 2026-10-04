#!/usr/bin/env python3
# After run.py passed: boot the converted replica disk through the coreboot + GRUB test ROM
# (no TPM there). Proves that GRUB finds the signed kernel on the new /boot partition under
# the old root UUID, and that the passphrase fallback of the built-in early init works.
import os, subprocess, sys, time, select, re
H=os.path.dirname(os.path.abspath(__file__)); W=H+"/work"
ROM=os.path.expanduser("~/t480-build/src/coreboot/build-qemu/coreboot.rom")
p=subprocess.Popen(["qemu-system-x86_64","-enable-kvm","-machine","q35","-cpu","host","-m","768","-bios",ROM,
    "-drive",f"file={W}/disk.img,if=none,id=d0,format=raw,snapshot=on","-device","nvme,drive=d0,serial=t480test",
    "-display","none","-monitor","none","-serial","stdio","-no-reboot"],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
os.set_blocking(p.stdout.fileno(),False); buf=""; typed=False; t0=time.time(); log=open(W+"/serial-grubpath.log","w")
while p.poll() is None and time.time()-t0<60:
    r,_,_=select.select([p.stdout],[],[],0.5)
    d=p.stdout.read() if r else None
    if d: d=d.decode(errors="replace"); buf+=d; log.write(d); log.flush()
    if not typed and buf.rstrip(" ").endswith("LUKS passphrase:"):
        typed=True; time.sleep(0.3); p.stdin.write(b"test-pass-phrase-1\n"); p.stdin.flush()
    if "t480-early: switching to" in buf and time.time()-t0>3 and typed: time.sleep(2); break
p.kill(); p.wait()
checks={"GRUB loaded the kernel (signature accepted)": "Linux version" in buf or "t480-early" in buf,
        "no signature error": "bad signature" not in buf and "not found" not in buf.split("Linux version")[0][-3000:],
        "early init asked for the passphrase": typed,
        "root LOCKED without a TPM, then opened": "root LOCKED, swap LOCKED, TPM key not released" in buf,
        "switch to the real root on dm-0": bool(re.search(r"t480-early: switching to \S+ on /dev/dm-0",buf))}
for k,v in checks.items(): print(("PASS  " if v else "FAIL  ")+k)
sys.exit(0 if all(checks.values()) else 1)
