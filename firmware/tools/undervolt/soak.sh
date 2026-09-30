#!/bin/sh
# soak.sh core cache gpu uncore aio seconds: long combined run (dGPU renders), then Intel renders, then suspend/resume
W=$HOME/t480-build/work/uv
echo "$(date +%T) SOAK $*" >> $W/uvtest.log
sh $W/uvtest3.sh $1 $2 $3 $4 $5 $6
RENDER=env sh $W/uvtest3.sh $1 $2 $3 $4 $5 200
python3 $W/uvset.py $1 $2 $3 $4 $5 >/dev/null; rtcwake -m no -s 40 >/dev/null 2>&1; loginctl suspend; sleep 70
echo "$(date +%T) after suspend: offsets $(python3 $W/uvset.py) mce=$(dmesg | grep -c -i 'machine check\|mce:')" >> $W/uvtest.log
echo "$(date +%T) SOAK DONE" >> $W/uvtest.log
