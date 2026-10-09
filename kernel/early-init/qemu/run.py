#!/usr/bin/env python3
# Rehearsal of the T480 disk encryption in QEMU: replica disk + swtpm (in a container),
# direct kernel boot. Restarts QEMU after each power-off (hibernate) until the test
# driver prints its verdict. usage: run.py KERNEL [--initrd early.cpio]
import os, subprocess, sys, time, select, re
H=os.path.dirname(os.path.abspath(__file__)); W=H+"/work"; TPM=W+"/tpm"
kernel=sys.argv[1]; initrd=sys.argv[sys.argv.index("--initrd")+1] if "--initrd" in sys.argv else None
os.makedirs(TPM, exist_ok=True)
for f in os.listdir(TPM): os.unlink(TPM+"/"+f)
def swtpm():
    # swtpm ends when QEMU disconnects; its state (NVRAM, persistent handles) stays in TPM/
    subprocess.run(["docker","rm","-f","t480-swtpm"],capture_output=True)
    if os.path.exists(TPM+"/swtpm.sock"): os.unlink(TPM+"/swtpm.sock")
    subprocess.run(["docker","run","-d","--name","t480-swtpm","--user",f"{os.getuid()}:{os.getgid()}","-v",TPM+":/tpm","t480-early",
        "swtpm","socket","--tpm2","--tpmstate","dir=/tpm","--ctrl","type=unixio,path=/tpm/swtpm.sock"],check=True,capture_output=True)
    while not os.path.exists(TPM+"/swtpm.sock"): time.sleep(0.05)
CMD="root=PARTUUID=12345678-01 rootfstype=ext4 rootwait resume=PARTUUID=12345678-02 resumewait init=/usr/local/sbin/t480-init rw console=ttyS0 panic=10 loglevel=4 printk.devkmsg=off"
log=open(W+"/serial.log","w"); extra=[]; verdict=None; boots=0; t_start=time.time()
while verdict is None and boots<12 and time.time()-t_start<1500:
    boots+=1; swtpm()
    a=["qemu-system-x86_64","-enable-kvm","-machine","q35","-cpu","host","-smp","4","-m","768","-kernel",kernel,"-append",CMD,
       "-drive",f"file={W}/disk.img,if=none,id=d0,format=raw","-device","nvme,drive=d0,serial=t480test",
       "-chardev",f"socket,id=chrtpm,path={TPM}/swtpm.sock","-tpmdev","emulator,id=tpm0,chardev=chrtpm","-device","tpm-tis,tpmdev=tpm0",
       "-display","none","-monitor","none","-serial","stdio"]+extra+(["-initrd",initrd] if initrd else [])
    p=subprocess.Popen(a,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    os.set_blocking(p.stdout.fileno(),False); buf=""; seen=0
    while p.poll() is None and time.time()-t_start<1500:
        r,_,_=select.select([p.stdout],[],[],0.5)
        if not r: continue
        d=p.stdout.read()
        if not d: continue
        d=d.decode(errors="replace"); log.write(d); log.flush(); buf+=d
        for line in re.findall(r"(TEST stage.*|T480-TEST-\S+|.*t480-early.*|\[[0-9.]+\] (?:T480|FAILED|done|disk|encrypting).*)",d): print(line.strip()[:230],flush=True)
        n=buf.count("passphrase")
        if re.search(r"(LUKS passphrase: |Enter passphrase for [^:]*: )$",buf[-80:]) and len(buf)!=seen:
            seen=len(buf); time.sleep(0.3); p.stdin.write(b"test-pass-phrase-1\n"); p.stdin.flush(); print("  (harness typed the passphrase)")
        if "T480-TEST-CHANGE-FIRMWARE" in buf and not extra:
            extra=["-device","virtio-net-pci"]   # another option ROM -> another PCR 2 from the next start on
        # the provisioning word of the early init: on for the check boot and the conversion, off again after it
        if "T480-TEST-PROVISION-ON" in buf and " t480.provision" not in CMD: CMD+=" t480.provision"
        if "T480-TEST-PROVISION-OFF" in buf and " t480.provision" in CMD: CMD=CMD.replace(" t480.provision","")
        if "T480-TEST-ALL-PASS" in buf: verdict="PASS"
        if "T480-TEST-FAILED" in buf or "Kernel panic" in buf: verdict="FAIL"
        if verdict: time.sleep(1); break
    if p.poll() is None: p.kill()
    p.wait(); print(f"--- qemu run {boots} ended ({time.time()-t_start:.0f} s)",flush=True)
subprocess.run(["docker","rm","-f","t480-swtpm"],capture_output=True)
print("RESULT:",verdict or "TIMEOUT"); sys.exit(0 if verdict=="PASS" else 1)
