#!/bin/sh
# sweep-batt.sh START : on battery, core=cache from START in 1 mV steps until the first failure (shorter runs)
W=$HOME/t480-build/work/uv; v=$1
[ "$(cat /sys/class/power_supply/AC/online)" = 0 ] || { echo "$(date +%T) battery sweep: AC is plugged, not starting" >> $W/uvtest.log; exit 1; }
echo "$(date +%T) FINE SWEEP BATTERY: 1 mV steps from $v" >> $W/uvtest.log
while [ $v -ge -140 ]; do
  cap=$(cat /sys/class/power_supply/BAT0/capacity); [ "$cap" -lt 20 ] && { echo "$(date +%T) battery sweep stopped: BAT0 at $cap%" >> $W/uvtest.log; break; }
  sh $W/uvtest.sh $v $v 0 0 0 180; grep -q "PASS core=$v " $W/uvtest.log || { echo "$(date +%T) fine sweep BATTERY stopped: first failure at $v" >> $W/uvtest.log; break; }; v=$((v-1)); done
python3 $W/uvset.py 0 0 0 0 0 >/dev/null; echo "$(date +%T) FINE SWEEP BATTERY DONE (offsets back to 0)" >> $W/uvtest.log
