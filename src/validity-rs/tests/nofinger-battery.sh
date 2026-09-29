#!/bin/bash
# No-finger hardware battery for validity-rs. Run as root with python3-validity
# stopped; nobody may touch the sensor while it runs. Uses user $U (needs prints).
R=/home/btw/Projects/software/validity-rs/target/release/validity-rs
T=/home/btw/Projects/software/validity-rs/tests
U=${U:-btw}
LOG=/root/vrs-daemon.log
fail=0
disc() { dmesg | grep -c 'usb 1-9: USB disconnect'; }
check() { if eval "$2"; then echo "PASS  $1"; else echo "FAIL  $1"; fail=$((fail+1)); fi; }
start() {
	setsid "$R" -v daemon >>"$LOG" 2>&1 </dev/null & echo $! >/root/vrs.pid
	for _ in $(seq 100); do [ "$(grep -c registered "$LOG")" -gt "${1:-0}" ] && return 0; sleep 0.1; done; return 1
}
stop() { kill -TERM "$(cat /root/vrs.pid)" 2>/dev/null; for _ in $(seq 50); do kill -0 "$(cat /root/vrs.pid)" 2>/dev/null || return 0; sleep 0.1; done; return 1; }
fingers() { fprintd-list "$U" 2>&1 | grep -c ' - #'; }

: >"$LOG"; d0=$(disc)
start 0; check "daemon starts and registers" "grep -q registered $LOG"
n0=$(fingers); check "fprintd-list $U works ($n0 finger(s))" "[ $n0 -ge 1 ]"
check "no-prints user gets NoEnrolledPrints" "timeout 10 fprintd-verify nobody 2>&1 | grep -q 'No fingers enrolled'"

out=$(python3 $T/cancel-stress.py 100 0.5 "$U" | tail -1)
check "100 verify/cancel cycles: $out" "[ \"$out\" = 'cycles with errors: 0/100' ]"

ok=0
for i in $(seq 10); do python3 $T/suspend-abort.py "$U" 2>&1 | grep -q "\[('verify-disconnected', True)\]" && ok=$((ok+1)); done
check "10 suspend mid-verify -> verify-disconnected + resume ($ok/10)" "[ $ok -eq 10 ]"

timeout 3 fprintd-enroll -f left-little-finger "$U" >/dev/null 2>&1
sleep 1
check "cancelled enroll leaves the template DB unchanged" "[ $(fingers) -eq $n0 ]"
check "cancelled enroll logged, no records created" "grep -q 'enroll cancelled' $LOG && ! grep -q 'enrolled left-little' $LOG"

r0=$(grep -c registered "$LOG")
s6-svc -r /run/service/open-fprintd; sleep 4
check "re-registers after open-fprintd restart" "[ $(grep -c registered $LOG) -gt $r0 ]"
check "fprintd-list works after open-fprintd restart" "[ $(fingers) -eq $n0 ]"

ok=0
for i in $(seq 10); do
	stop || break; r0=$(grep -c registered "$LOG"); start "$r0" || break
	python3 $T/cancel-stress.py 1 0.3 "$U" | grep -q '0/1' && ok=$((ok+1))
done
check "10 daemon restarts, each followed by a verify/cancel ($ok/10)" "[ $ok -eq 10 ]"
check "no wedged-sensor recovery needed on clean restarts" "! grep -q 'does not answer' $LOG"

python3 - "$U" <<'PY' &
import dbus, sys, time
bus = dbus.SystemBus()
mgr = dbus.Interface(bus.get_object('net.reactivated.Fprint', '/net/reactivated/Fprint/Manager'), 'net.reactivated.Fprint.Manager')
dev = dbus.Interface(bus.get_object('net.reactivated.Fprint', mgr.GetDefaultDevice()), 'net.reactivated.Fprint.Device')
dev.Claim(sys.argv[1]); dev.VerifyStart('any'); time.sleep(6)
PY
cl=$!; sleep 1.5
kill -9 "$(cat /root/vrs.pid)"; sleep 0.5; kill "$cl" 2>/dev/null; wait "$cl" 2>/dev/null
r0=$(grep -c registered "$LOG"); t=$(date +%s%N); start "$r0"
check "recovers after SIGKILL mid-capture ($(( ($(date +%s%N)-t)/1000000 )) ms to registered)" "[ $(grep -c registered $LOG) -gt $r0 ]"
sleep 1; check "fprintd-list works after SIGKILL recovery" "[ $(fingers) -eq $n0 ]"

sleep 10; p=$(cat /root/vrs.pid)
cpu=$(awk '{print $14+$15}' /proc/$p/stat); sleep 10; cpu2=$(awk '{print $14+$15}' /proc/$p/stat)
rss=$(awk '/VmRSS/{print $2}' /proc/$p/status); thr=$(ls /proc/$p/task | wc -l)
check "idle: $((cpu2-cpu)) ticks CPU in 10 s, RSS ${rss} kB, $thr threads" "[ $((cpu2-cpu)) -le 2 ] && [ $rss -lt 20000 ]"

d=$(( $(disc) - d0 ))
check "sensor USB disconnects during the whole battery: $d" "[ $d -eq 0 ]"
check "no ERROR lines in the daemon log" "! grep -q ERROR $LOG"
stop
echo "failures: $fail"
