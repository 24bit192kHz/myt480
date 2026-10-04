#!/bin/sh
# flashrom.sh NAME: flash roms/NAME.rom (FMAP + COREBOOT), cold boot if VERIFIED. Root, detached.
# Leaves a one-shot TPM blob first, so the new firmware boots without the passphrase.
N=${1:?rom name}; L=/home/btw/t480-build/work/flash-$N.log
cd /home/btw/t480-build/roms || exit 1
echo "$(date +%T) flash $N start" > $L
sh /home/btw/t480-build/work/romcheck.sh $N.rom >> $L 2>&1 || exit 1
/usr/local/sbin/t480-reseal next-boot >> $L 2>&1 || { echo "$(date +%T) next-boot failed - nothing written" >> $L; exit 1; }
flashprog -p internal --fmap-file $N.rom -i FMAP -i COREBOOT -w $N.rom >> $L 2>&1
rc=$?
echo "$(date +%T) flashprog rc=$rc" >> $L
if [ $rc -eq 0 ] && grep -q "VERIFIED" $L; then
  echo "$(date +%T) VERIFIED -> coldboot in 5 s" >> $L
  sleep 5
  sh /home/btw/t480-build/work/coldboot.sh "" 25
else
  echo "$(date +%T) NOT VERIFIED - no power-off" >> $L
fi
