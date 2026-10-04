#!/bin/sh
# T480 disk conversion, run as PID 1 from RAM by t480-init.convert (toolbox = Alpine).
#   before: p1 ext4 root (with /boot), p2 swap
#   after:  p1 LUKS2 -> ext4 root, p3 ext4 /boot (GRUB finds it by the old root UUID),
#           p2 LUKS2 -> swap. Volume keys come from /c/*.vk (already wrapped for the TPM),
#           the recovery passphrase from /c/pass.
# Up to the partition table rewrite every failure boots the old system again.
export PATH=/usr/sbin:/usr/bin:/sbin:/bin
export DM_DISABLE_UDEV=1		# no udevd here: libdevmapper makes the nodes itself
D=${T480_DISKDEV:-/dev/nvme0n1}; P=${D}p
LOG=/c/convert.log; STAGE=start
say() { echo "[$(cut -d' ' -f1 /proc/uptime)] $*" | tee -a $LOG; }
mount -t proc proc /proc; mount -t sysfs sys /sys
run() { say "+ $*"; "$@" >> $LOG 2>&1; }
finish() { # reboot; with QEMU -no-reboot this ends the run
	sync; say "rebooting"; sleep 2; reboot -f
}
old_system() { # the root partition is still the plain file system: put the log there and boot it
	say "FAILED at '$STAGE': $*  -> booting the unchanged system"
	mkdir -p /mnt && mount -t ext4 ${P}1 /mnt && { cp $LOG /mnt/var/lib/t480-convert/convert-failed.log; umount /mnt; }
	finish
}
stuck() { # past the point of no return
	say "FAILED at '$STAGE': $*"
	say "The disk is partly converted. Log: this screen. Recovery: backup on the workstation."
	mkdir -p /mnt; mount -t ext4 ${P}3 /mnt 2>/dev/null && { cp $LOG /mnt/convert-failed.log; umount /mnt; }
	sync; sleep 600; poweroff -f
}

say "T480 disk conversion. Do not power off. This takes about 15 minutes."
STAGE="leave the root file system"
umount /old || { sleep 1; umount /old; } || { say "cannot unmount the old root: $(cat /proc/mounts | tr '\n' ';')"; mount -o remount,rw /old; cp $LOG /old/var/lib/t480-convert/convert-failed.log; finish; }
[ "$(blkid -s TYPE -o value ${P}1)" = ext4 ] || old_system "${P}1 is not ext4"
grep -q "^${P}2 " /proc/swaps && old_system "swap is active"
OLDUUID=$(blkid -s UUID -o value ${P}1); SWAPUUID=$(blkid -s UUID -o value ${P}2)
[ -n "$OLDUUID" ] && [ -n "$SWAPUUID" ] || old_system "no UUIDs"
[ "$(wc -c < /c/root.vk)" = 64 ] && [ "$(wc -c < /c/swap.vk)" = 64 ] && [ -s /c/pass ] && [ -s /c/t480/kmk.blob ] && [ -s /c/t480/root.key ] && [ -s /c/t480/swap.key ] || old_system "key material incomplete"

# layout, in 512-byte sectors; everything 4K-aligned
TOTAL=$(blockdev --getsz $D); S1=$(sfdisk -d $D | sed -n "s|^${P}1 : start= *\([0-9]*\), size= *\([0-9]*\),.*|\1 \2|p")
START1=${S1% *}; SIZE1=${S1#* }
BOOTSZ=524288							# 256 MiB
NEW1=$(( (SIZE1 - START1 % 8) / 8 * 8 ))			# root partition, size rounded down
START3=$(( START1 + NEW1 )); START2=$(( START3 + BOOTSZ )); SIZE2=$(( (TOTAL - START2) / 8 * 8 ))
FSBLOCKS=$(( (NEW1 * 512 - 32 * 1048576) / 4096 ))		# file system: 32 MiB smaller, for the LUKS header
say "disk $TOTAL sectors; p1 $START1+$SIZE1 -> $START1+$NEW1, p3 $START3+$BOOTSZ, p2 $START2+$SIZE2; fs -> $FSBLOCKS blocks"
[ $START1 -ge 2048 ] && [ $(( START1 % 8 )) = 0 ] && [ $SIZE2 -gt 1048576 ] && [ $FSBLOCKS -gt 1000 ] || old_system "unexpected layout"

STAGE="check the file system"
e2fsck -fy ${P}1 >> $LOG 2>&1; rc=$?; say "e2fsck exit $rc"; [ $rc -le 1 ] || old_system "e2fsck"
USED=$(dumpe2fs -h ${P}1 2>/dev/null | awk -F: '/^Block count/{b=$2} /^Free blocks/{f=$2} END{print b-f}')
[ $(( USED + 65536 )) -lt $FSBLOCKS ] || old_system "not enough free space"
STAGE="shrink the file system by 32 MiB"
run resize2fs ${P}1 $FSBLOCKS || old_system "resize2fs"

STAGE="partition table"
sfdisk -d $D > /c/partitions.before
printf 'label: dos\nlabel-id: %s\nunit: sectors\n\n%s1 : start=%s, size=%s, type=83, bootable\n%s2 : start=%s, size=%s, type=82\n%s3 : start=%s, size=%s, type=83\n' \
	"$(sfdisk --disk-id $D)" $P $START1 $NEW1 $P $START2 $SIZE2 $P $START3 $BOOTSZ > /c/partitions.after
cat /c/partitions.after >> $LOG
sfdisk --no-reread -q $D < /c/partitions.after >> $LOG 2>&1 || { sfdisk --no-reread -q $D < /c/partitions.before; old_system "sfdisk"; }
blockdev --rereadpt $D >> $LOG 2>&1 || partx -u $D >> $LOG 2>&1; sleep 1
[ -b ${P}3 ] && [ "$(blockdev --getsz ${P}1)" = $NEW1 ] || { sfdisk --no-reread -q $D < /c/partitions.before; blockdev --rereadpt $D; old_system "kernel did not take the new table"; }

# ---- point of no return ----
STAGE="encrypt the root partition"
say "encrypting ${P}1 ($(( NEW1 / 2097152 )) GiB) in place ..."
cryptsetup reencrypt --encrypt --batch-mode --type luks2 --reduce-device-size 32M \
	--cipher aes-xts-plain64 --key-size 512 --sector-size 4096 --volume-key-file /c/root.vk \
	--key-file /c/pass --pbkdf argon2id --progress-frequency 30 --disable-locks ${P}1 2>&1 | tee -a $LOG | tr '\r' '\n' | grep -v '^$' > /dev/console
cryptsetup isLuks ${P}1 || stuck "reencrypt"
cryptsetup luksDump ${P}1 | grep -q "online-reencrypt\|Requirements" && stuck "reencryption did not finish"
run cryptsetup open --disable-locks --key-file /c/pass ${P}1 root || stuck "open root"
run e2fsck -fy /dev/mapper/root; [ $? -le 1 ] || stuck "e2fsck after encryption"
run resize2fs /dev/mapper/root || stuck "grow the file system"
NEWUUID=$(cat /proc/sys/kernel/random/uuid)
run tune2fs -U $NEWUUID /dev/mapper/root || stuck "tune2fs"

STAGE="boot partition"
run mke2fs -q -F -t ext4 -L boot -U $OLDUUID -O ^metadata_csum_seed,^orphan_file -m 0 ${P}3 || stuck "mke2fs"
mkdir -p /mnt /bootfs
run mount -t ext4 /dev/mapper/root /mnt || stuck "mount root"
run mount -t ext4 ${P}3 /bootfs || stuck "mount boot"
cp -a /mnt/boot/. /bootfs/ >> $LOG 2>&1 || stuck "copy /boot"
ln -s . /bootfs/boot				# GRUB keeps asking for /boot/vmlinuz-linux-t480
rm -rf /bootfs/t480; mkdir -m 700 /bootfs/t480; cp /c/t480/kmk.blob /c/t480/root.key /c/t480/swap.key /bootfs/t480/

STAGE="swap"
run cryptsetup luksFormat --batch-mode --type luks2 --cipher aes-xts-plain64 --key-size 512 --sector-size 4096 \
	--volume-key-file /c/swap.vk --key-file /c/pass --pbkdf argon2id --disable-locks ${P}2 || stuck "luksFormat swap"
run cryptsetup open --disable-locks --key-file /c/pass ${P}2 swap || stuck "open swap"
run mkswap -U $SWAPUUID /dev/mapper/swap || stuck "mkswap"

STAGE="dm parameters"
for n in root:1 swap:2; do
	cryptsetup luksDump ${P}${n#*:} | awk '/^Data segments:/{s=1} s&&/offset:/{o=$2} s&&/sector:/{z=$2} /^Keyslots:/{s=0} END{printf "%d %d\n", o/512, z}' > /bootfs/t480/${n%:*}.dm
	say "${n%:*}.dm: $(cat /bootfs/t480/${n%:*}.dm)"
done
chmod 600 /bootfs/t480/*

STAGE="system files"
find /mnt/boot -mindepth 1 -delete >> $LOG 2>&1		# /boot is a mount point now
sed -i.before-encryption "s|^UUID=$OLDUUID[[:space:]]\+/[[:space:]].*|UUID=$NEWUUID /              ext4    defaults,noatime   0 1\n${P}3                            /boot          ext4    defaults,noatime   0 2|" /mnt/etc/fstab
grep -q "^UUID=$NEWUUID" /mnt/etc/fstab || stuck "fstab"
cp /c/partitions.before /c/partitions.after /mnt/var/lib/t480-convert/
say "done: root UUID $NEWUUID, boot ${P}3 (UUID $OLDUUID), swap UUID $SWAPUUID"
cp $LOG /mnt/var/lib/t480-convert/convert.log
rm -f /mnt/var/lib/t480-convert/root.vk /mnt/var/lib/t480-convert/swap.vk
umount /bootfs; umount /mnt; swapoff -a 2>/dev/null
cryptsetup close swap; cryptsetup close root
finish
