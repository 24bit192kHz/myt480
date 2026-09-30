#!/bin/sh
# bench-all.sh LABEL : CPU, memory, GPU (Intel and MX150), NVMe, sustained clocks; results in work/bench-LABEL.txt (root)
L=${1:?label}; W=/home/btw/t480-build/work; O=$W/bench-$L.txt; B=/home/btw/.cache/mpv-bench; : > $O
say(){ echo "$*" >> $O; }
say "# bench $L $(date +%F_%T) AC=$(cat /sys/class/power_supply/AC/online) offsets: $(python3 $W/uv/uvset.py)"
say "## sysbench cpu (8 threads, 30 s, events/s)"; sysbench cpu --threads=8 --time=30 run 2>/dev/null | grep -E "events per second" >> $O
say "## sysbench cpu (1 thread, 15 s)"; sysbench cpu --threads=1 --time=15 run 2>/dev/null | grep -E "events per second" >> $O
say "## 7z b (MIPS, 8 threads)"; 7z b -mmt8 2>/dev/null | grep -E "^Tot:|^Avr:" >> $O
say "## openssl speed"; openssl speed -seconds 3 -evp sha256 2>/dev/null | tail -1 >> $O; openssl speed -seconds 3 -evp aes-256-gcm 2>/dev/null | tail -1 >> $O
say "## sysbench memory (8 threads, 10 s)"; sysbench memory --threads=8 --time=10 --memory-block-size=1M --memory-total-size=100G run 2>/dev/null | grep -E "MiB/sec|transferred" | head -2 >> $O
say "## sustained all-core 60 s (matrixprod): MHz / package temp / package W, every 10 s"; rm -f $W/uv/sample.txt; python3 $W/uv/sample.py > $W/uv/sample.txt 2>/dev/null & SMP=$!; stress-ng --cpu 8 --cpu-method matrixprod --metrics-brief --timeout 60 2>&1 | grep -E "matrixprod" | tail -1 >> $O; kill $SMP; awk '{printf "%d MHz %d C %.1f W\n",$1,$2,$3}' $W/uv/sample.txt >> $O
gpu_bench(){ su btw -c "cd $B && DISPLAY=:0 sh -c 'python3 hidwin.py 70 > wid.b & sleep 0.3; $1 mpv --wid=\$(cat wid.b) --fs=no --ao=null --untimed --opengl-swapinterval=0 --frames=1500 --vo=gpu --gpu-api=opengl --hwdec=no --scale=spline36 --cscale=spline36 -v h264_1080.mkv 2>&1 | grep -o -E \"drawn [0-9]+ frames|[0-9.]+ fps\" | tail -1; T0=; kill %1 2>/dev/null'"; }
say "## GPU: 1500 frames 1080p software decode, spline36 scaling, hidden window (seconds via time)"
for g in "" "prime-run"; do s=$(date +%s.%N); r=$(gpu_bench "$g"); e=$(date +%s.%N); say "$( [ -z "$g" ] && echo Intel || echo MX150 ): $(echo "$e - $s" | bc | cut -c1-5) s for 1500 frames = $(echo "1500 / ($e - $s)" | bc) fps ($r)"; done
say "## NVMe: hdparm -t and 2 GiB direct read"; hdparm -t /dev/nvme0n1 2>/dev/null | tail -1 >> $O; dd if=/dev/nvme0n1 of=/dev/null bs=1M count=2048 iflag=direct 2>&1 | tail -1 >> $O
say "## idle 30 s: package W"; rm -f $W/uv/sample.txt; python3 $W/uv/sample.py > $W/uv/sample.txt 2>/dev/null & SMP=$!; sleep 31; kill $SMP; awk '{printf "%d MHz %d C %.1f W\n",$1,$2,$3}' $W/uv/sample.txt >> $O
say "# done $(date +%T)"; chown btw:btw $O; cat $O
