#!/bin/sh
# sweep-planes.sh CORE : with core=cache=CORE fixed, gpu plane in 10 mV steps under an Intel-GPU render load, then uncore
W=$HOME/t480-build/work/uv; C=$1
echo "$(date +%T) PLANE SWEEP AC with core/cache $C" >> $W/uvtest.log
best_gpu=0; for g in -50 -60 -70 -80 -90 -100; do RENDER=env sh $W/uvtest.sh $C $C $g 0 0 200; grep -q "PASS core=$C gpu=$g " $W/uvtest.log || break; best_gpu=$g; done
echo "$(date +%T) gpu plane: last pass $best_gpu" >> $W/uvtest.log
best_unc=0; for u in -50 -60 -70 -80 -90 -100; do sh $W/uvtest.sh $C $C $best_gpu $u 0 200; grep -q "PASS core=$C gpu=$best_gpu cache=$C uncore=$u " $W/uvtest.log || break; best_unc=$u; done
echo "$(date +%T) uncore plane: last pass $best_unc" >> $W/uvtest.log
python3 $W/uvset.py 0 0 0 0 0 >/dev/null; echo "$(date +%T) PLANE SWEEP DONE core=$C gpu=$best_gpu uncore=$best_unc (offsets back to 0)" >> $W/uvtest.log
