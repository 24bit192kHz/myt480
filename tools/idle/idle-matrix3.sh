#!/bin/sh
# Root. Idle matrix on AC, dGPU off, battery-like settings; restores everything on exit.
P=/home/btw/t480-build/up2/pkgc.py; D=/sys/kernel/debug/pmc_core
X="su btw -c"
G=$(pgrep -x gxwc | head -1)
ltr() { grep -E "SOUTHPORT|XHCI|ME " $D/ltr_show | sed -E 's/[ \t]+/ /g' | awk '{print $2, $5, "ign=" $NF}' | tr '\n' ' '; echo; }
restore() {
  trap - EXIT HUP TERM INT
  for i in 0 1 4; do echo $i > $D/ltr_restore 2>/dev/null; done
  [ -n "$G" ] && kill -CONT $G 2>/dev/null
  $X 'DISPLAY=:0 xset dpms force on' 2>/dev/null
  iw dev wlan0 set power_save off
  tlp start >/dev/null 2>&1
  [ "$KEEP_GPU_OFF" = 1 ] || gpu-power auto >/dev/null 2>&1
  echo "restored: $(gpu-power status | tr '\n' ';') ps=$(iw dev wlan0 get power_save) gov=$(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor)"
}
trap restore EXIT HUP TERM INT
S=${1:-15}
gpu-power off || exit 1
sleep 3; echo "gpu: $(cat /sys/bus/pci/devices/0000:00:1c.0/firmware_node/real_power_state)"; ltr
$P $S 0-AC-profile-gpu-off
tlp bat >/dev/null 2>&1; sleep 5; $P $S 1+tlp-bat
[ -n "$G" ] && kill -STOP $G; sleep 2; $P $S 2+wallpaper-stopped
iw dev wlan0 set power_save on; sleep 3; $P $S 3+wifi-ps; ltr
$X 'DISPLAY=:0 xset dpms force off'; sleep 3; $P $S 4+panel-off
echo 1 > $D/ltr_ignore; $P $S 5+ignore-SPB
echo 0 > $D/ltr_ignore; $P $S 6+ignore-SPA; ltr
echo "dc: $(grep -hE 'DC[0-9]' /sys/kernel/debug/dri/0/i915_dmc_info | tr '\n' ' ')"
/home/btw/t480-build/up2/wakeups.py 4 | head -2
