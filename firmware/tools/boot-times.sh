#!/bin/sh
# Boot times of the current boot on the T480, one line. Run as a user who can
# `ssh root@localhost` (cbmem and dmesg need root).
R="ssh -o BatchMode=yes root@localhost"
ts=$($R cbmem -t); con=$($R cbmem -c)
fsps=$(echo "$ts" | awk '/returning from FspSiliconInit/{gsub(/[(),]/,"",$NF); print $NF}')
jump=$(echo "$ts" | awk '/selfboot jump/{gsub(/,/,"",$(NF-1)); print $(NF-1)}')
gma=$(echo "$con" | grep "00:00:02.0 init finished" | tail -1 | awk '{print $(NF-1)}')
grub=$(echo "$con" | grep "Elapsed" | tail -1 | awk '{print $3}')
fbcon=$($R "dmesg | grep -m1 'Console: switching' | cut -c2-13")
init=$($R "dmesg | grep -m1 'Freeing unused kernel image (initmem' | cut -c2-13")
xorg=$(for p in $(pgrep -x Xorg); do awk '{print $22/100}' /proc/$p/stat; done | head -1)
echo "FSP-S ${fsps} us, reset->payload ${jump} us, graphics init ${gma} ms, GRUB kernel load ${grub} s, fbcon at ${fbcon} s, init at ${init} s, Xorg at ${xorg} s"
