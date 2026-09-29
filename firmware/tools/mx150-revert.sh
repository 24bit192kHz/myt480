#!/bin/sh
# Undo the MX150 work on the T480 (2026-09-29): back to C18 + intel X +
# the pre-MX150 kernel. Run as root: sh ~btw/t480-build/tools/mx150-revert.sh
# Everything it undoes is kept on `mx150` branches, see ~btw/t480-build/MX150.md.
set -eu
B=/home/btw/t480-build

echo "== dGPU request off (CMOS 0x6f)"
/usr/local/bin/dgpu off || true

echo "== firmware: flash C18 (FMAP + COREBOOT, keeps MRC/SPD caches)"
flashprog -p internal -r /root/PRE-MX150-REVERT-backup.rom
flashprog -p internal --fmap-file $B/roms/C18.rom -i FMAP -i COREBOOT -w $B/roms/C18.rom

echo "== packages: NVIDIA driver and headers out, pre-MX150 kernel back"
pacman -Rns --noconfirm nvidia-580xx-dkms nvidia-580xx-utils egl-x11 egl-gbm \
	egl-wayland eglexternalplatform dkms linux-t480-headers
pacman -U --noconfirm $B/kernel/linux-t480-7.2.8-1-pre-mx150.pkg.tar.zst

echo "== /etc and /usr/local back to master (intel X config, no dgpu/prime-run)"
git -C /etc checkout master
git -C /usr/local checkout master

echo "== sources back to the pre-MX150 branches"
su btw -c "git -C $B/src/coreboot checkout -q t480 && git -C $B/src/coreboot/site-local checkout -q master && git -C $B/kernel checkout -q master"

echo "Done. Reboot to finish (firmware, kernel and X config take effect then)."
