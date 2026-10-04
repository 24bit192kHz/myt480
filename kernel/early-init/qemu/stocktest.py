#!/usr/bin/env python3
# The distro kernel + its initramfs (hook t480crypt) on the converted replica disk:
# passphrase prompt, root and swap opened, switch to the replica's init.
import os, subprocess, sys, time, select
W=os.path.dirname(os.path.abspath(__file__))+"/work"
p=subprocess.Popen(["qemu-system-x86_64","-enable-kvm","-machine","q35","-cpu","host","-smp","2","-m","1024",
    "-kernel",W+"/stock/vmlinuz-linux","-initrd",W+"/stock/initramfs-linux.img",
    "-append","root=/dev/mapper/root rw resume=/dev/mapper/swap console=ttyS0 init=/usr/local/sbin/t480-init",
    "-drive",f"file={W}/disk.img,if=none,id=d0,format=raw,snapshot=on","-device","nvme,drive=d0,serial=t480test",
    "-display","none","-monitor","none","-serial","stdio","-no-reboot"],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
os.set_blocking(p.stdout.fileno(),False); buf=""; typed=0; t0=time.time(); log=open(W+"/serial-stock.log","w")
while p.poll() is None and time.time()-t0<90:
    r,_,_=select.select([p.stdout],[],[],0.5)
    d=p.stdout.read() if r else None
    if d: d=d.decode(errors="replace"); buf+=d; log.write(d); log.flush()
    if typed<2 and buf.rstrip(" ").endswith("LUKS passphrase:") and buf.count("LUKS passphrase:")>typed:
        typed+=1; time.sleep(0.3); p.stdin.write(b"wrong\n" if typed==1 else b"test-pass-phrase-1\n"); p.stdin.flush()
    if "TEST stage" in buf: time.sleep(1); break
p.kill(); p.wait()
checks={"prompt shown":typed>=1,"wrong passphrase refused":"That did not open the disk" in buf,
        "root opened and the replica's init reached":"TEST stage" in buf,
        "swap opened too":"swap not opened" not in buf}
for k,v in checks.items(): print(("PASS  " if v else "FAIL  ")+k)
sys.exit(0 if all(checks.values()) else 1)
