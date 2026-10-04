#!/bin/sh
# Runs inside the t480-early container (root, unprivileged): builds the replica disk
#   p1 ext4 root (odd sector count like the T480), p2 swap; kernel in /boot.
# usage: mkdisk.sh /w (the early-init directory) /k/vmlinuz [/k/vmlinuz.sig]
set -eu
W=$1; K=$2; SIG=${3:-}; O=$W/qemu/work; T=/tmp/root
rm -rf $T; mkdir -p $T $O
# ROOT_TAR: another root file system (out/archroot.tar.gz = GNU userland as on the T480)
tar xzf ${ROOT_TAR:-$W/out/toolbox.tar.gz} -C $T
mkdir -p $T/var/lib/t480-convert/toolbox $T/usr/local/sbin $T/boot $T/proc $T/sys $T/run $T/dev
tar xzf $W/out/toolbox.tar.gz -C $T/var/lib/t480-convert/toolbox
install -m 755 $W/convert/convert.sh $T/var/lib/t480-convert/convert.sh
install -m 755 $W/convert/t480-init.convert $T/usr/local/sbin/t480-init
install -m 755 $W/qemu/test-init $T/usr/local/sbin/t480-init.real
install -m 755 $W/t480-reseal $T/usr/local/sbin/t480-reseal
install -m 644 $K $T/boot/vmlinuz-linux-t480
[ -z "$SIG" ] || install -m 644 $SIG $T/boot/vmlinuz-linux-t480.sig
head -c 20000000 /dev/urandom > $T/boot/initramfs-linux.img	# stand-in for the other /boot files
printf 'UUID=11111111-2222-3333-4444-555555555555 /              ext4    defaults,noatime   0 1\nUUID=22222222-3333-4444-5555-666666666666 swap           swap    defaults,pri=100   0 0\n' > $T/etc/fstab
S1=2048; N1=3145731; S2=$((S1+N1)); TOTAL=5242880; N2=$((TOTAL-S2))
rm -f $O/p1.img $O/p2.img $O/disk.img
truncate -s $((N1*512)) $O/p1.img
mke2fs -q -t ext4 -U 11111111-2222-3333-4444-555555555555 -d $T $O/p1.img
truncate -s $((N2*512)) $O/p2.img
mkswap -q -U 22222222-3333-4444-5555-666666666666 $O/p2.img
truncate -s $((TOTAL*512)) $O/disk.img
printf 'label: dos\nlabel-id: 0x12345678\nunit: sectors\n\n1 : start=%s, size=%s, type=83, bootable\n2 : start=%s, size=%s, type=82\n' $S1 $N1 $S2 $N2 | sfdisk -q $O/disk.img
dd if=$O/p1.img of=$O/disk.img bs=512 seek=$S1 conv=notrunc status=none
dd if=$O/p2.img of=$O/disk.img bs=512 seek=$S2 conv=notrunc status=none
rm -f $O/p1.img $O/p2.img
# updated early init as an external initramfs (overrides the one built into the kernel)
rm -rf /tmp/ird; mkdir -p /tmp/ird/bin; cp $W/out/init /tmp/ird/init; cp $W/out/cryptsetup /tmp/ird/bin/cryptsetup
(cd /tmp/ird && find . | cpio -o -H newc 2>/dev/null) > $O/early.cpio
chown -R 1000:1000 $O
sfdisk -d $O/disk.img | tail -3
