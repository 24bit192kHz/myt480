#!/bin/sh
W=$HOME/t480-build/work/uv
sh $W/uvtest.sh 0 0 0 0 0 150
for v in -80 -100 -110 -120; do sh $W/uvtest.sh $v $v 0 0 0 240; grep -q "PASS core=$v" $W/uvtest.log || { echo "$(date +%T) sweep stopped at $v" >> $W/uvtest.log; break; }; done
python3 $W/uvset.py 0 0 0 0 0 >/dev/null; echo "$(date +%T) SWEEP DONE (offsets back to 0)" >> $W/uvtest.log
