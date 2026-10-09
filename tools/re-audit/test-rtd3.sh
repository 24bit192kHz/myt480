#!/bin/sh
# Root-only, temporary RTD3 test. Always restores the original config and mode.
# Run with no GPU clients; tests CUDA correctness, cold wake, and offset retention.
set -eu
[ "$(id -u)" = 0 ]
[ "$(cat /sys/class/power_supply/AC/online)" = 1 ]
backup=$(mktemp -d /root/myt480-rtd3-test.XXXXXX)
chmod 700 "$backup"
cp -p /etc/gpu-power.conf "$backup/config"
if [ -f /run/gpu-power.manual ]; then
    cp -p /run/gpu-power.manual "$backup/manual"
fi
restore() {
    cp -p "$backup/config" /etc/gpu-power.conf
    gpu-power off || return 1
    if [ -f "$backup/manual" ]; then
        case $(cat "$backup/manual") in
            on) gpu-power on ;;
            off) gpu-power off ;;
        esac
    else
        gpu-power auto
    fi
}
if fuser /dev/nvidia* 2>/dev/null; then
    echo 'GPU clients are running; test cancelled.' >&2
    exit 1
fi
trap restore 0
trap 'exit 130' INT
trap 'exit 143' TERM HUP
echo "Restoration snapshot: $backup"
gpu-power status
sed -i 's/^rtd3 .*/rtd3 1/' /etc/gpu-power.conf
gpu-power off
gpu-power auto
for round in 1 2 3 4 5; do
    sleep 3
    echo "Round $round before wake"
    gpu-power status
    [ "$(cat /sys/bus/pci/devices/0000:01:00.0/power_state)" = D3cold ]
    python3 - <<'PY'
import subprocess
import time
start = time.monotonic()
subprocess.run(['prime-run', '/home/btw/t480-build/work/gpu/bench-sm61/vectorAdd'], check=True)
print('Cold vectorAdd including wrapper: %.3f seconds' % (time.monotonic() - start))
PY
    prime-run /home/btw/t480-build/work/gpu/bench-sm61/scan
    prime-run python3 - <<'PY'
import ctypes
from pathlib import Path
n = ctypes.CDLL('libnvidia-ml.so.1')
assert n.nvmlInit_v2() == 0
h = ctypes.c_void_p()
assert n.nvmlDeviceGetHandleByIndex_v2(0, ctypes.byref(h)) == 0
core, mem = ctypes.c_int(), ctypes.c_int()
assert n.nvmlDeviceGetGpcClkVfOffset(h, ctypes.byref(core)) == 0
assert n.nvmlDeviceGetMemClkVfOffset(h, ctypes.byref(mem)) == 0
values = dict(line.split()[:2] for line in Path('/etc/gpu-power.conf').read_text().splitlines()
              if line.strip() and not line.lstrip().startswith('#'))
prefix = 'ac_' if Path('/sys/class/power_supply/AC/online').read_text().strip() == '1' else 'batt_'
expected = int(values.get(prefix+'gpc_offset', 0)), int(values.get(prefix+'mem_offset', 0))
print('Active offsets:', core.value, mem.value, 'expected:', expected)
assert (core.value, mem.value) == expected
assert Path('/sys/bus/pci/devices/0000:01:00.0/power/runtime_status').read_text().strip() == 'active'
assert n.nvmlShutdown() == 0
PY
done
sleep 3
gpu-power status
echo 'Five cold wakes, CUDA checks, and clock restoration checks passed.'
