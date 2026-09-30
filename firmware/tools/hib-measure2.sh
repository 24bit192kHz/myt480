#!/bin/sh
# hib-measure2.sh LABEL ALARM_S : hibernate; a 0.2 s wall-clock loop finds the freeze and thaw instants.
# freeze->thaw with a long alarm = resume time (from the alarm); with a short alarm = the whole cycle.
L=${1:-x}; A=${2:-75}; W=/home/btw/t480-build/work; LOG=$W/hibernate.log
echo 1 > /sys/power/pm_print_times
echo "$(date +%T) HIB2 $L alarm=$A image_size=$(cat /sys/power/image_size) threads=$(cat /sys/power/hibernate_compression_threads) mem_used=$(free -m | awk '/Mem/{print $3}')M kernel=$(uname -r) nvidia=$([ -e /sys/module/nvidia ] && echo loaded || echo no)" >> $LOG; sync
rtcwake -m no -s $A > /dev/null 2>&1; TA=$(( $(date +%s) + A )); T0=$(date +%s.%N)
loginctl hibernate &
prev=$(date +%s.%N); while :; do now=$(date +%s.%N); d=$(echo "$now - $prev" | bc); if [ "$(echo "$d > 3" | bc)" = 1 ]; then break; fi; prev=$now; sleep 0.2; done
F=$prev; T=$now; sleep 2
E=$(dmesg | grep -o -E "Allocated [0-9]+ kbytes in [0-9.]+ seconds|Need to copy [0-9]+ pages" | tail -2 | tr '\n' ' ')
echo "$(date +%T) HIB2 $L RESULT: loginctl->freeze $(echo "$F - $T0" | bc | cut -c1-5) s | freeze->thaw $(echo "$T - $F" | bc | cut -c1-6) s | alarm->thaw $(echo "$T - $TA" | bc | cut -c1-6) s (resume, valid if alarm long) | $E" >> $LOG; sync
