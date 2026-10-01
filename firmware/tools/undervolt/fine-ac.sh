#!/bin/sh
W=$HOME/t480-build/work/uv
echo "$(date +%T) FINE SWEEP AC: 1 mV steps from -121" >> $W/uvtest.log
v=-121; while [ $v -ge -140 ]; do sh $W/uvtest.sh $v $v 0 0 0 240; grep -q "PASS core=$v " $W/uvtest.log || { echo "$(date +%T) fine sweep AC stopped: first failure at $v" >> $W/uvtest.log; break; }; v=$((v-1)); done
python3 $W/uvset.py 0 0 0 0 0 >/dev/null; echo "$(date +%T) FINE SWEEP AC DONE (offsets back to 0)" >> $W/uvtest.log
