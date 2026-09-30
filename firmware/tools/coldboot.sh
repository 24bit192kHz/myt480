#!/bin/sh
# coldboot.sh [on|off] [seconds]: set the dGPU request, arm an RTC alarm, power off (root). No warm reboot.
S=${2:-20}; [ -n "$1" ] && /usr/local/bin/dgpu $1 | tail -1
echo "$(date +%T) coldboot: rtc alarm in $S s, poweroff" >> /home/btw/t480-build/work/coldboot.log
rtcwake -m no -s $S 2>&1 | tail -1 >> /home/btw/t480-build/work/coldboot.log
cat /sys/class/rtc/rtc0/wakealarm >> /home/btw/t480-build/work/coldboot.log; date +%s >> /home/btw/t480-build/work/coldboot.log
sync; sleep 1; poweroff
