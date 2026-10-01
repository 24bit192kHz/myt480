#!/bin/sh
# gpusweep.sh: MX150 clock offset sweep, 90 s per step (run as the desktop user, detached, after the baseline "base").
# Memory first (+250 MHz steps, judged by the bilinear fps), then graphics (+50 MHz steps, judged by the
# ewa fps). A plane stops at the first FAIL, or when its metric gains less than 0.3 % over the step before.
W=$(cd "$(dirname "$0")" && pwd); LOG=$W/gputest.log
say(){ echo "$(date +%T) $*" >> $LOG; }
last(){ grep -E "=== (PASS|FAIL) $1 " $LOG | tail -1; }
val(){ echo "$1" | grep -o "$2=[0-9.]*" | head -1 | cut -d= -f2; }
gain(){ [ "$(echo "$1 >= $2 * 1.003" | bc)" = 1 ]; }
say "=== SWEEP start"
best_m=0; prev=$(val "$(last base)" fps_bilinear)
for m in 250 500 750 1000 1250 1500 1750 2000; do
  sh $W/gputest.sh mem$m 90 0 $m; r=$(last mem$m)
  echo "$r" | grep -q "=== PASS" || { say "SWEEP mem: failure at +$m"; break; }
  f=$(val "$r" fps_bilinear)
  gain "$f" "$prev" || { say "SWEEP mem: no gain at +$m ($f vs $prev fps)"; break; }
  best_m=$m; prev=$f
done
say "SWEEP mem: best +$best_m"
best_g=0; prev=$(val "$(last base)" fps_2nd_half)
for g in 50 100 150 200 250 300 350 400; do
  sh $W/gputest.sh gpc$g 90 $g 0; r=$(last gpc$g)
  echo "$r" | grep -q "=== PASS" || { say "SWEEP gpc: failure at +$g"; break; }
  f=$(val "$r" fps_2nd_half)
  gain "$f" "$prev" || { say "SWEEP gpc: no gain at +$g ($f vs $prev fps)"; break; }
  best_g=$g; prev=$f
done
say "=== SWEEP done: best mem=+$best_m gpc=+$best_g"
