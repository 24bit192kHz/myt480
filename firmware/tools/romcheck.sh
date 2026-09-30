#!/bin/sh
# Refuse a ROM whose GRUB payload does not carry site-local/grub.cfg.
# The payload is one file shared by every build directory; a ROM built after a
# QEMU harness run once carried the harness config and did not find the kernel.
# usage: romcheck.sh coreboot.rom     (run from the coreboot tree; exit 0 = flash it)
ROM=${1:?usage: romcheck.sh coreboot.rom}
CB=${CB:-.}
UUID=$(sed -n 's/^set rootuuid="\(.*\)"/\1/p' "$CB/site-local/grub.cfg")
T=$(mktemp)
"$CB/build/cbfstool" "$ROM" extract -n fallback/payload -f "$T" -m x86 >/dev/null 2>&1 || { echo "romcheck: cannot extract the payload from $ROM"; rm -f "$T"; exit 1; }
REAL=$(grep -a -c "$UUID" "$T"); OTHER=$(grep -a -c -E "11111111-2222|<UUID of" "$T")
rm -f "$T"
echo "romcheck $ROM: grub.cfg of this tree: $REAL, harness/placeholder config: $OTHER"
[ -n "$UUID" ] && [ "$REAL" -ge 1 ] && [ "$OTHER" -eq 0 ] && { echo "romcheck: OK"; exit 0; }
echo "romcheck: REFUSED, do not flash this image"; exit 1
