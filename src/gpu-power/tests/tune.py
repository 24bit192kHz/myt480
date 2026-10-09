#!/usr/bin/env python3
"""Verify NVML error handling without loading a driver or changing clocks."""
from pathlib import Path
import subprocess
import tempfile


with tempfile.TemporaryDirectory() as directory:
    root = Path(directory)
    source = Path(__file__).resolve().parents[1] / "gpu-power.c"
    (root / "config").write_text("ac_gpc_offset 200\nac_mem_offset 1250\nrtd3 1\n")
    (root / "ac").write_text("1\n")
    (root / "node").touch()
    (root / "fake.c").write_text('''
int gpc_calls, mem_calls;
int nvmlInit_v2(void) { return 0; }
int nvmlShutdown(void) { return 0; }
int nvmlDeviceGetHandleByIndex_v2(unsigned i, void **d) { *d = (void *)1; return 0; }
int nvmlDeviceSetGpcClkVfOffset(void *d, int v) { gpc_calls++; return 7; }
int nvmlDeviceSetMemClkVfOffset(void *d, int v) { mem_calls++; return v != 1250; }
''')
    subprocess.run(["cc", "-shared", "-fPIC", str(root / "fake.c"), "-o", str(root / "fake.so")], check=True)
    harness = root / "test.c"
    harness.write_text(f'''#define main production_main
#include "{source}"
#undef main
#include <assert.h>
int main(void) {{
    void *lib = dlopen(NVML, RTLD_NOW);
    assert(lib);
    int *gpc = dlsym(lib, "gpc_calls"), *mem = dlsym(lib, "mem_calls");
    assert(gpc && mem);
    assert(tune(1) == 1);
    assert(*gpc == 1 && *mem == 1); /* a core error must not skip memory */
    *gpc = *mem = 0;
    assert(apply(-1, 0) == 0); /* release must not wake an idle RTD3 GPU */
    assert(*gpc == 0 && *mem == 0);
    dlclose(lib);
    return 0;
}}
''')
    (root / "vendor").write_text("0x10de\n")
    overrides = {"NVML": root / "fake.so", "CONF": root / "config", "AC": root / "ac",
                 "NVIDIA_DEVICE": root / "node", "NVIDIA_MODULE": root,
                 "GPU": root, "MANUAL": root / "manual"}
    subprocess.run(["cc", "-O2", "-Wall", "-Wextra",
                    *[f'-D{key}="{value}"' for key, value in overrides.items()],
                    str(harness), "-ldl", "-o", str(root / "test")], check=True)
    subprocess.run([str(root / "test")], check=True)
    print("NVML failure and idle release checks passed")
