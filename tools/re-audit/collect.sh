#!/bin/sh
# Read-only, identity-free subsystem inventory; run locally on the T480.
set -u
section() { printf '\n[%s]\n' "$1"; }
section kernel
uname -r
if [ -r /proc/config.gz ]; then
    zcat /proc/config.gz | grep -E 'CONFIG_(TYPEC|UCSI|THUNDERBOLT|USB_ROLE|USB_ROLES|INTEL_IDLE|X86_INTEL_PSTATE|PSTORE)'
fi
section power
cat /sys/power/mem_sleep
for file in /sys/class/power_supply/*/online /sys/class/power_supply/BAT*/status /sys/class/power_supply/BAT*/capacity; do
    [ -r "$file" ] && printf '%s: %s\n' "$file" "$(cat "$file")"
done
section cpu
for key in scaling_driver scaling_governor energy_performance_preference; do
    cat "/sys/devices/system/cpu/cpu0/cpufreq/$key" 2>/dev/null || true
done
section thermal
cat /proc/acpi/ibm/fan 2>/dev/null || true
for file in /sys/class/hwmon/hwmon*/name /sys/class/hwmon/hwmon*/temp*_input; do
    [ -r "$file" ] && printf '%s: %s\n' "$file" "$(cat "$file")"
done
section gpu
gpu-power status || true
section usb
lsusb
section pci
lspci -nn | grep -E 'VGA|3D|Network|Ethernet|Non-Volatile|Thunderbolt|USB'
section policies
for file in /sys/module/pcie_aspm/parameters/policy \
    /sys/module/nvme_core/parameters/default_ps_max_latency_us \
    /sys/module/snd_hda_intel/parameters/power_save \
    /sys/module/iwlwifi/parameters/power_save; do
    [ -r "$file" ] && printf '%s: %s\n' "$file" "$(cat "$file")"
done
section fingerprints
fprint-backend || true
section recent_errors
dmesg | grep -Ei 'NVRM: Xid|AER:.*error|nvme.*timeout|iwlwifi.*error|thermal.*critical' | tail -20
exit 0
