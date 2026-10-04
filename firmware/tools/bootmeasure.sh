#!/bin/sh
# bootmeasure.sh LABEL: one line per boot, power-on -> chadwm (cold boots only). Run as btw.
L=${1:-x}; W=/home/btw/t480-build/work; R="ssh -o BatchMode=yes root@localhost"
i=0; while [ $i -lt 60 ] && ! pgrep -x chadwm >/dev/null; do sleep 1; i=$((i+1)); done; sleep 3
z=$($W/tscmono | sed -n 's/.*zero at \([0-9.]*\) s.*/\1/p')
st(){ p=$(pgrep -x "$1" | head -1); [ -n "$p" ] && awk '{print $22/100}' /proc/$p/stat; }
c=$(st chadwm); x=$(st Xorg); s=$(st slock)
fw=$($R "cbmem -c | grep -a -m1 -o 'coreboot-[^ ]*'")
pay=$($R cbmem -t | awk '/selfboot jump/{gsub(/,/,"",$(NF-1)); print $(NF-1)/1000000}')
tot=$($R cbmem -t | awk '/Total Time/{gsub(/,/,"",$NF); print $NF/1000000}')
g=$($R "cbmem -c" | grep -a Elapsed | tail -1 | awk '{print $3}')
pcr=$($R "tpm2_pcrread sha256:2" | awk '/ 2 :/{print substr($3,1,18)}')
echo "$(date '+%F %T') $L fw=$fw AC=$(cat /sys/class/power_supply/AC/online) payload=${pay}s cbmem_total=${tot}s grub_load=${g}s kernel_zero=${z}s Xorg=+${x} chadwm=+${c} slock=+${s} => power-on->chadwm $(echo "$z + $c" | bc)s pcr2=$pcr" | tee -a $W/bootmeasure.log
