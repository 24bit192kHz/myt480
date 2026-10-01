#!/usr/bin/python3
# nvoff.py [GPC_MHZ MEM_MHZ]: set (root) or read the MX150 clock offsets through NVML; prints "gpc=N mem=N".
import ctypes as C, sys
n = C.CDLL("libnvidia-ml.so.1")
assert n.nvmlInit_v2() == 0
h = C.c_void_p(); assert n.nvmlDeviceGetHandleByIndex_v2(0, C.byref(h)) == 0
if len(sys.argv) == 3:
    g, m = int(sys.argv[1]), int(sys.argv[2])
    r1 = n.nvmlDeviceSetGpcClkVfOffset(h, g); r2 = n.nvmlDeviceSetMemClkVfOffset(h, m)
    if r1 or r2: print(f"set failed: gpc rc={r1} mem rc={r2}"); sys.exit(1)
a = C.c_int(); b = C.c_int()
n.nvmlDeviceGetGpcClkVfOffset(h, C.byref(a)); n.nvmlDeviceGetMemClkVfOffset(h, C.byref(b))
print(f"gpc={a.value} mem={b.value}")
n.nvmlShutdown()
