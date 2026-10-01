#!/bin/sh
# memcheck.sh OFFSET...: fps of the memory-bandwidth shader at each memory offset (graphics offset 0)
W=$(cd "$(dirname "$0")" && pwd); R="ssh -n -o BatchMode=yes root@localhost"
DISPLAY=:0 python3 - > /tmp/gt-wid <<'PY' &
from Xlib import display
import time
d = display.Display(); s = d.screen()
w = s.root.create_window(s.width_in_pixels + 200, 0, 3840, 2160, 0, s.root_depth, override_redirect=1)
w.map(); d.sync(); print(w.id, flush=True); time.sleep(600)
PY
HP=$!; sleep 0.6
for m in "$@"; do
  $R "python3 $W/nvoff.py 0 $m" > /dev/null || { echo "offset $m not set"; continue; }
  ( sleep 6; nvidia-smi --query-gpu=clocks.mem,utilization.gpu,utilization.memory --format=csv,noheader > /tmp/gt-smi ) & SP=$!
  a=$(date +%s.%N)
  DISPLAY=:0 timeout -k 3 60 prime-run mpv --no-config --gpu-context=x11 --wid=$(cat /tmp/gt-wid) --fs=no --ao=null --untimed --opengl-swapinterval=0 --frames=40 \
     --vo=gpu --gpu-api=opengl --glsl-shader=$W/membw.glsl --really-quiet av://lavfi:color=c=gray:size=3840x2160:rate=1000 > /dev/null 2>&1
  rc=$?; e=$(date +%s.%N); wait $SP
  echo "mem+$m: rc=$rc $(echo "40/($e-$a)" | bc -l | cut -c1-6) fps  [$(cat /tmp/gt-smi)]"
done
$R "python3 $W/nvoff.py 0 0"; kill $HP
