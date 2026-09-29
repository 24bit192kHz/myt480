#!/bin/sh
# Run as root with no other sensor user (stop python3-validity and any
# validity-rs daemon first). Usage: N=20 cancel-matrix.sh USER
R=/home/btw/Projects/software/validity-rs/target/release/validity-rs
S=/home/btw/Projects/software/validity-rs/tests/cancel-stress.py
N=${N:-20}
USER_=${1:-btw}
if [ -f /root/vrs.pid ] && kill -TERM "$(cat /root/vrs.pid)" 2>/dev/null; then
	i=0; while kill -0 "$(cat /root/vrs.pid)" 2>/dev/null && [ $i -lt 150 ]; do sleep 0.1; i=$((i+1)); done
fi
setsid "$R" -v daemon >/root/vrs-daemon.log 2>&1 </dev/null &
echo $! >/root/vrs.pid
i=0; while ! grep -q registered /root/vrs-daemon.log && [ $i -lt 100 ]; do sleep 0.1; i=$((i+1)); done
d0=$(dmesg | grep -c 'usb 1-9: USB disconnect')
python3 "$S" "$N" 1.0 "$USER_"
echo "$(( $(dmesg | grep -c 'usb 1-9: USB disconnect') - d0 ))/$N sensor disconnects"
echo "captures that saw a finger: $(grep -c -E 'verify: |retry' /root/vrs-daemon.log)"
