#!/bin/sh
# gvt-tick-test.sh (btw; root parts via ssh): GVT-g scheduler tick patch test, 2026-11-10.
# VM start -> headless RDP session -> pause -> unpause -> clean stop, tick rate at each stage,
# then the self-recovering hibernation test. Panel dimmed for the duration, restored at the end.
L=$HOME/t480-build/work/gvt-tick-test.log; R="ssh -n -o BatchMode=yes root@localhost"
C=/home/btw/.config/winapps/compose.yaml
say() { echo "$(date +%T) $*" >> "$L"; }
tick() { t=$($R "/home/btw/t480-build/up2/wakeups.py 5" | sed -n 2p | grep -o "gvt_service_thread=[0-9]*"); echo "${t:-gvt_service_thread<1/s}"; }
st() { $R "docker inspect -f {{.State.Status}} WinApps"; }
$R 'cp -p /var/lib/backlight/level /var/lib/backlight/level.gvttest; echo "acpi_video0 1 100" > /var/lib/backlight/level; echo 1 > /sys/class/backlight/acpi_video0/brightness'
say "=== start $(cut -c1-60 /proc/version | cut -d' ' -f3) $(grep -o '#[0-9]* SMP' /proc/version) VM=$(st) tick: $(tick)"
$R "docker compose -f $C start" >/dev/null 2>&1
t=0; calm=0
while [ $t -lt 300 ] && [ $calm -lt 2 ]; do
	cpu=$($R 'docker stats --no-stream --format "{{.CPUPerc}}" WinApps' | tr -d '%')
	if [ "${cpu%.*}" -lt 15 ] 2>/dev/null; then calm=$((calm+1)); else calm=0; fi
	sleep 8; t=$((t+10))
done
say "VM $(st), calm after ~${t}s (cpu $cpu%); tick while idle-running: $(tick)"
. "$HOME/.config/winapps/winapps.conf"
rdp() { timeout "$1" xvfb-run -a -s "-screen 0 1280x800x24" xfreerdp3 /v:127.0.0.1 /u:"$RDP_USER" /p:"$RDP_PASS" /cert:ignore /sec:tls /size:1280x800 > "$HOME/t480-build/work/gvt-rdp.out" 2>&1; echo $?; }
( sleep 25; echo "$(date +%T) tick during RDP session: $(tick)" >> "$L" ) &
rc=$(rdp 45); wait
say "RDP session 1 rc=$rc (124 = stayed connected 45 s) $(grep -ciE 'error|fail' "$HOME/t480-build/work/gvt-rdp.out") error lines: $(grep -iE 'ERRCONNECT|ERRINFO' "$HOME/t480-build/work/gvt-rdp.out" | tail -1 | sed -E 's/^.*(ERR[A-Z_]+).*$/\1/')"
$R "docker compose -f $C pause" >/dev/null 2>&1; sleep 3
say "VM $(st); tick while paused: $(tick)"
$R "docker compose -f $C unpause" >/dev/null 2>&1; sleep 3
( sleep 12; echo "$(date +%T) tick during RDP session 2: $(tick)" >> "$L" ) &
rc=$(rdp 30); wait
say "after unpause: VM $(st), RDP session 2 rc=$rc"
s=$(date +%s); $R "docker compose -f $C stop -t 120" >/dev/null 2>&1
say "VM $(st) after $(( $(date +%s) - s )) s stop; tick after stop: $(tick)"
sleep 20
say "idle after VM: $($R '/home/btw/t480-build/up2/pkgc.py 15 x' | cut -c30-230)"
say "dmesg: $($R 'dmesg' | grep -iE 'gvt|kvmgt|i915.*(error|hang|reset)|BUG|WARNING|Call Trace|hung' | grep -v 'golden_hw_state\|Kernel command line' | tail -5 | cut -c1-140 | tr '\n' ';')"
$R '/home/btw/t480-build/work/hibtest.sh k9d-gvt-tick' >/dev/null 2>&1
say "hibtest: $(tail -1 "$HOME/t480-build/work/hibtest.log" | cut -c1-200)"
sleep 5
say "after hibtest: gvt=$($R 'cat /sys/module/i915/parameters/enable_gvt') mdev=$(ls /sys/bus/mdev/devices | wc -l) tick: $(tick) wd=$($R 'cat /sys/class/watchdog/watchdog0/state')"
DISPLAY=:0 xset +dpms dpms force off
$R 'cp -p /var/lib/backlight/level.gvttest /var/lib/backlight/level; rm -f /var/lib/backlight/level.gvttest; echo 37 > /sys/class/backlight/acpi_video0/brightness'
rm -f "$HOME/t480-build/work/gvt-rdp.out"
say "=== done"
