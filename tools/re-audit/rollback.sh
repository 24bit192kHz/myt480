#!/bin/sh
# Restore the three programs and GPU policy changed in the 2026-10-09 audit.
# Usage: rollback.sh thermal|gpu|all [--check]
set -eu
[ "$(id -u)" = 0 ] || { echo 'Run as root on the T480.' >&2; exit 77; }
scope=${1:-}
case "$scope" in thermal|gpu|all) ;; *) echo 'usage: rollback.sh thermal|gpu|all [--check]' >&2; exit 64;; esac
[ $# -le 2 ]
[ $# -lt 2 ] || [ "$2" = --check ]
backup=/root/myt480-audit-20261009
check() {
    actual=$(sha256sum "$backup/$1" | cut -d ' ' -f1)
    [ "$actual" = "$2" ] || { echo "Backup checksum failed: $1" >&2; exit 1; }
}
if [ "$scope" = thermal ] || [ "$scope" = all ]; then
    check thermald-t480 bb79265098040ad54f588e742d46dbb23e55f06407e559db77296d80659e87f0
    check thermald.conf 960b36ec57354117c2449e67425420976714ac13595b25005bf9f1f326c0205a
    [ -d /run/service/thermald ]
fi
if [ "$scope" = gpu ] || [ "$scope" = all ]; then
    check gpu-power 359b63d520fadc2b99977a39b96d9ada323934d418801c00bfcd0a3aa8e710f4
    check prime-run c34ab783d381d6e4269650da1db4883bcd1a571ba163cd6d08ce43125da1f488
    check gpu-power.conf 4cacdfbfe7853a60033b2d2fff2b5a32eb7bb37d8fd9e7ff68426826034215d4
    if fuser /dev/nvidia* 2>/dev/null; then
        echo 'Close GPU jobs before restoring the driver policy.' >&2
        exit 1
    fi
fi
if [ "${2:-}" = --check ]; then
    echo "Original $scope backups and required endpoints verified; no changes made."
    exit 0
fi
if [ "$scope" = thermal ] || [ "$scope" = all ]; then
    s6-svc -wD -T 5000 -d /run/service/thermald
    install -m 755 "$backup/thermald-t480" /usr/local/bin/thermald-t480.audit-restore
    mv /usr/local/bin/thermald-t480.audit-restore /usr/local/bin/thermald-t480
    cp -p "$backup/thermald.conf" /etc/thermald.conf
    s6-svc -u /run/service/thermald
fi
if [ "$scope" = gpu ] || [ "$scope" = all ]; then
    gpu-power off
    install -m 4755 -o root -g root "$backup/gpu-power" /usr/local/bin/gpu-power.audit-restore
    install -m 755 -o root -g root "$backup/prime-run" /usr/local/bin/prime-run.audit-restore
    mv /usr/local/bin/gpu-power.audit-restore /usr/local/bin/gpu-power
    mv /usr/local/bin/prime-run.audit-restore /usr/local/bin/prime-run
    cp -p "$backup/gpu-power.conf" /etc/gpu-power.conf
    if [ -f "$backup/gpu-power.manual" ]; then
        case $(cat "$backup/gpu-power.manual") in
            on) gpu-power on ;;
            off) gpu-power off ;;
            *) echo 'Invalid saved manual mode.' >&2; exit 1 ;;
        esac
    else
        gpu-power auto
    fi
fi
echo "Restored original $scope programs and policy."
