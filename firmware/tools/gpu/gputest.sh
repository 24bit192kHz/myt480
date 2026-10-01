#!/bin/sh
# gputest.sh LABEL SECONDS GPC_OFFSET MEM_OFFSET : MX150 load test with clock offsets (run as the desktop user, detached).
# Load: mpv (no user config, GLX), a synthetic 720p source (lavfi testsrc2, nothing to decode) upscaled to
# 3840x2160 with ewa_lanczossharp (shader-bound), untimed, on the MX150 (prime-run),
# into a mapped, override-redirect window placed off-screen (nothing visible, no focus change;
# an unmapped window stalls NVIDIA PRIME presentation). fps per 20 s chunk; nvidia-smi log every second.
# After the timed part: 600 frames with the bilinear scaler (memory/copy-bound) as a second metric.
# PASS: every chunk ran, no new Xid in the kernel log, offsets read back as set. Offsets go back to 0.
L=$1; T=$2; G=$3; M=$4
W=$(cd "$(dirname "$0")" && pwd); R="ssh -n -o BatchMode=yes root@localhost"
LOG=$W/gputest.log; CSV=$W/$L.csv; : > $CSV
say(){ echo "$(date +%T) $*" >> $LOG; }
xid(){ $R dmesg | grep -c -E "NVRM: Xid"; }
say "=== START $L ${T}s gpc=$G mem=$M AC=$(cat /sys/class/power_supply/AC/online)"
x0=$(xid)
o=$($R python3 $W/nvoff.py $G $M) || { say "=== FAIL $L: offsets not accepted ($o)"; exit 1; }
say "offsets: $o"
nvidia-smi --query-gpu=timestamp,pstate,clocks.gr,clocks.mem,temperature.gpu,utilization.gpu,clocks_event_reasons.active --format=csv,noheader,nounits -l 1 > $W/$L.smi 2>&1 &
SMI=$!
( while :; do printf "%s %s %s\n" "$(date +%s)" "$(cat /sys/class/thermal/thermal_zone*/temp 2>/dev/null | sort -n | tail -1)" "$(awk '/speed/{print $2}' /proc/acpi/ibm/fan)"; sleep 1; done > $W/$L.sys ) &
SYS=$!
DISPLAY=:0 python3 - <<'PY' > $W/wid.$L &
from Xlib import display
import time
d = display.Display(); s = d.screen()
w = s.root.create_window(s.width_in_pixels + 200, 0, 3840, 2160, 0, s.root_depth, override_redirect=1)
w.map(); d.sync(); print(w.id, flush=True); time.sleep(100000)
PY
WINPID=$!; sleep 0.5; WID=$(cat $W/wid.$L)
SRC=av://lavfi:testsrc2=size=1280x720:rate=60
s0=$(date +%s); fails=0; n=0; CH=0
while [ $(( $(date +%s) - s0 )) -lt $T ]; do
  [ $CH -eq 0 ] && FR=400 || FR=$CH
  a=$(date +%s.%N)
  DISPLAY=:0 timeout -k 5 90 prime-run mpv --no-config --gpu-context=x11 --wid=$WID --fs=no --ao=null --untimed --opengl-swapinterval=0 --frames=$FR \
     --vo=gpu --gpu-api=opengl --scale=ewa_lanczossharp --cscale=ewa_lanczossharp --really-quiet $SRC > /dev/null 2>&1
  rc=$?; e=$(date +%s.%N)
  fps=$(echo "$FR / ($e - $a)" | bc -l | cut -c1-6)
  [ $rc -eq 0 ] || fails=$((fails+1))
  n=$((n+1)); echo "$n $(( $(date +%s) - s0 )) $fps $rc" >> $CSV
  [ $CH -eq 0 ] && CH=$(echo "$fps * 20 / 1" | bc)
done
a=$(date +%s.%N)
DISPLAY=:0 timeout -k 5 90 prime-run mpv --no-config --gpu-context=x11 --wid=$WID --fs=no --ao=null --untimed --opengl-swapinterval=0 --frames=600 \
   --vo=gpu --gpu-api=opengl --scale=bilinear --cscale=bilinear --really-quiet $SRC > /dev/null 2>&1 || fails=$((fails+1))
e=$(date +%s.%N); bil=$(echo "600 / ($e - $a)" | bc -l | cut -c1-6)
kill $WINPID $SMI $SYS 2>/dev/null
ob=$($R python3 $W/nvoff.py); $R python3 $W/nvoff.py 0 0 > /dev/null
x1=$(xid)
half=$(awk -v n=$n 'NR > n/2 {print $3}' $CSV | sort -n | awk '{a[NR]=$1} END {print a[int((NR+1)/2)]}')
gr=$(awk -F', ' 'NR>5 && $6>90 {s+=$3; c++} END {if(c) printf "%d", s/c}' $W/$L.smi)
mem=$(awk -F', ' 'NR>5 && $6>90 {s+=$4; c++} END {if(c) printf "%d", s/c}' $W/$L.smi)
tmax=$(awk -F', ' '{if($5>m)m=$5} END {print m}' $W/$L.smi)
rs=$(awk -F', ' 'NR>5 && $6>90 {print $7}' $W/$L.smi | sort | uniq -c | sort -rn | head -3 | awk '{printf "%s x%s ", $2, $1}')
cpu=$(awk '{if($2>m)m=$2} END {printf "%d", m/1000}' $W/$L.sys); fan=$(awk '$3 != "" {s+=$3; c++} END {if (c) printf "%d", s/c}' $W/$L.sys)
res="fps_bilinear=$bil chunks=$n fails=$fails xid=$((x1-x0)) fps_2nd_half=$half gpu_clk=$gr mem_clk=$mem gpu_tmax=$tmax cpu_tmax=$cpu fan_rpm=$fan reasons: $rs| offsets during: $ob"
if [ $fails -eq 0 ] && [ $x1 -eq $x0 ] && [ "$ob" = "gpc=$G mem=$M" ]; then say "=== PASS $L $res"; else say "=== FAIL $L $res"; fi
