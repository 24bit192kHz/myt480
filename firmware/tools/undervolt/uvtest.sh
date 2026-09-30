#!/bin/sh
# uvtest.sh core cache gpu uncore analogio [seconds]: set the offsets, stress with verification, log PASS/FAIL. root.
W=$HOME/t480-build/work/uv; L=$W/uvtest.log; B=/home/btw/.cache/mpv-bench; T=${6:-240}
log(){ echo "$(date +%T) $*" >> $L; sync -d $L; }
snap(){ [ -s $W/sample.txt ] && awk '{m+=$1; if($2>t)t=$2; p+=$3; n++} END{if(n)printf "mhz=%d maxpkg=%dC rapl=%.1fW samples=%d",m/n,t,p/n,n}' $W/sample.txt; :; }
SET=$(python3 $W/uvset.py $1 $2 $3 $4 $5); log "=== START $SET AC=$(cat /sys/class/power_supply/AC/online) ${T}s"
MCE0=$(dmesg | grep -c -i "machine check\|mce:")
# phase A: all-core AVX with verification + dGPU renders
su btw -c "cd $B && DISPLAY=:0 RENDER=${RENDER:-prime-run} setsid sh -c 'end=\$((\$(date +%s)+$((T/2)))); while [ \$(date +%s) -lt \$end ]; do python3 hidwin.py 70 > wid.uv & sleep 0.3; timeout 40 \${RENDER:-prime-run} mpv --wid=\$(cat wid.uv) --fs=no --ao=null --untimed --opengl-swapinterval=0 --frames=600 h264_1080.mkv >/dev/null 2>&1; kill %1 2>/dev/null; done' > /dev/null 2>&1 < /dev/null" &
python3 $W/sample.py > $W/sample.txt 2>/dev/null & SMP=$!
stress-ng --cpu 8 --cpu-method matrixprod --verify --timeout $((T/2)) --metrics-brief > $W/stress-A.txt 2>&1; RA=$?
kill $SMP 2>/dev/null; log "A all-core+GPU rc=$RA $(grep -c -E "failed: [1-9]|error" $W/stress-A.txt) err $(snap) | $(python3 $W/uvset.py | cut -c1-40)"
# phase B: load/idle bursts and a single-core turbo burst, verified
i=0; while [ $i -lt 6 ]; do stress-ng --cpu 8 --cpu-method fft --verify --timeout 4 > /dev/null 2>&1 || log "B burst $i FAILED"; sleep 4; i=$((i+1)); done
stress-ng --cpu 1 --cpu-method prime --verify --timeout 30 > $W/stress-B.txt 2>&1; RB=$?
log "B bursts+1core rc=$RB $(grep -c -E "failed: [1-9]|error" $W/stress-B.txt) err $(snap)"
# phase C: memory and cache pressure, verified
stress-ng --vm 4 --vm-bytes 512M --vm-method all --verify --cache 2 --timeout $((T/4)) > $W/stress-C.txt 2>&1; RC=$?
log "C vm+cache rc=$RC $(grep -c -E "failed: [1-9]|error" $W/stress-C.txt) err $(snap)"
sleep 20
MCE1=$(dmesg | grep -c -i "machine check\|mce:")
END=$(python3 $W/uvset.py)
if [ $RA = 0 ] && [ $RB = 0 ] && [ $RC = 0 ] && [ $MCE0 = $MCE1 ] && [ "$END" = "$SET" ]; then log "=== PASS $SET"; else log "=== FAIL $SET (rc $RA/$RB/$RC mce $MCE0->$MCE1 end=$END)"; fi
