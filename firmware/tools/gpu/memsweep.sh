#!/bin/sh
# memsweep.sh: memory offset sweep (+250 steps). Each step: gputest (90 s load, Xid/hang check) and the
# bandwidth shader (memcheck). Stops at a FAIL or when the bandwidth fps gains < 1 % over the step before
# (+250 = +125 MHz = +4.2 %; GDDR5 error retries eat the gain before anything crashes).
W=$(cd "$(dirname "$0")" && pwd); LOG=$W/gputest.log
say(){ echo "$(date +%T) $*" >> $LOG; }
bw(){ sh $W/memcheck.sh $1 | grep -o "mem+$1: rc=0 [0-9.]*" | awk '{print $3}'; }
say "=== MEMSWEEP start"
prev=$(bw 0); say "MEMSWEEP mem+0 bandwidth fps $prev"; best=0
for m in 250 500 750 1000 1250 1500 1750 2000; do
  sh $W/gputest.sh mem$m 90 0 $m
  grep -E "=== (PASS|FAIL) mem$m " $LOG | tail -1 | grep -q "=== PASS" || { say "MEMSWEEP: failure at +$m"; break; }
  f=$(bw $m)
  [ -n "$f" ] || { say "MEMSWEEP: bandwidth run failed at +$m"; break; }
  say "MEMSWEEP mem+$m bandwidth fps $f (before $prev)"
  [ "$(echo "$f >= $prev * 1.01" | bc)" = 1 ] || { say "MEMSWEEP: gain under 1 % at +$m"; break; }
  best=$m; prev=$f
done
say "=== MEMSWEEP done: best mem=+$best"
