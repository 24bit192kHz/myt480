#!/bin/sh
# Remove the Intel launcher and desktop override added in the continuation audit.
# Requires their recorded original-absence markers and exact installed checksums.
set -eu
[ "$(id -u)" = 0 ] || { echo 'Run as root on the audited T480.' >&2; exit 77; }
backup=/root/myt480-audit-20261009-routing
[ -f "$backup/igpu-run.was-absent" ]
[ -f "$backup/bitwarden.desktop.was-absent" ]
(cd / && sha256sum -c "$backup/installed.sha256")
case "${1:-}" in
    --check) echo 'Routing files match; no changes made.'; exit 0 ;;
    '') ;;
    *) echo 'usage: rollback-routing.sh [--check]' >&2; exit 64 ;;
esac
rm /home/btw/.local/share/applications/bitwarden.desktop /usr/local/bin/igpu-run
if command -v update-desktop-database >/dev/null; then
    runuser -u btw -- update-desktop-database /home/btw/.local/share/applications
fi
echo 'Restored the original launcher selection; running applications were left alone.'
