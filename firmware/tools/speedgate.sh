#!/bin/sh
# Speed gate (roadmap 3.1): raw swap partition vs the same partition under LUKS2/dm-crypt. Root.
# Swap is off meanwhile (no hibernate); the partition is restored as swap with its old UUID.
# fio: static build from the workstation in /run/fio (not packaged for Artix).
P=/dev/nvme0n1p2; U=<UUID of the swap partition>; L=/home/btw/t480-build/work/speedgate.log; K=/run/speedgate.key
say(){ echo "$(date +%T) $*" >> $L; }
one(){ # label device jobname fio-args...
  a=$1; d=$2; n=$3; shift 3
  r=$(/run/fio --name=$n --filename=$d --direct=1 --ioengine=libaio --minimal "$@" 2>&1 | tail -1)
  # terse v3: read bw KiB/s = field 7, read iops 8, write bw 48, write iops 49, cpu usr 88 sys 89
  say "$a $n: $(echo "$r" | awk -F';' '{printf "read %.0f MiB/s %d iops | write %.0f MiB/s %d iops | cpu usr %s sys %s", $7/1024, $8, $48/1024, $49, $88, $89}')"
}
t(){ # label device
  one $1 $2 seqwrite --rw=write --bs=1M --size=4G
  one $1 $2 seqread --rw=read --bs=1M --size=4G
  one $1 $2 rand4k-write-qd1 --rw=randwrite --bs=4k --iodepth=1 --runtime=12 --time_based --size=8G
  one $1 $2 rand4k-read-qd1 --rw=randread --bs=4k --iodepth=1 --runtime=12 --time_based --size=8G
  one $1 $2 rand4k-write-qd32 --rw=randwrite --bs=4k --iodepth=32 --runtime=12 --time_based --size=8G
  one $1 $2 rand4k-read-qd32 --rw=randread --bs=4k --iodepth=32 --runtime=12 --time_based --size=8G
  one $1 $2 rand4k-mixed-4jobs --rw=randrw --rwmixread=70 --bs=4k --iodepth=16 --numjobs=4 --group_reporting --runtime=12 --time_based --size=8G
}
say "=== speed gate 2 (aligned window, discard + 40 s rest before each round), kernel $(uname -r), AC=$(cat /sys/class/power_supply/AC/online)"
swapoff $P || { say "swapoff failed"; exit 1; }
# p2 starts on an odd sector; test inside a 4K-aligned window of it (the final layout is aligned)
dmsetup create sgwin --table "0 72099320 linear $P 5" || { say "dm-linear failed"; swapon -p 100 $P; exit 1; }
W=/dev/mapper/sgwin
head -c 32 /dev/urandom > $K
rest(){ blkdiscard -f $W >/dev/null 2>&1; sleep 40; }
luks(){ # label, extra open flags
  cryptsetup luksFormat --batch-mode --type luks2 --cipher aes-xts-plain64 --key-size 512 --sector-size 4096 --pbkdf pbkdf2 --pbkdf-force-iterations 1000 --key-file $K $W >> $L 2>&1
  cryptsetup open --key-file $K --allow-discards $2 $W speedgate >> $L 2>&1 || { say "open failed"; return; }
  [ "$1" = luks ] && [ -z "$TABLE" ] && { TABLE=1; say "table: $(dmsetup table speedgate)"; ls -la /dev/mapper >> $L; }
  t $1 /dev/mapper/speedgate
  cryptsetup close speedgate
}
NOWQ="--perf-no_read_workqueue --perf-no_write_workqueue"
rest; t raw $W
rest; luks luks "$NOWQ"
rest; t raw $W
rest; luks luks "$NOWQ"
rest; luks luks-with-workqueues ""
rm -f $K
dmsetup remove sgwin
blkdiscard -f $P >> $L 2>&1
mkswap -U $U $P >> $L 2>&1 && swapon -p 100 $P && say "swap restored: $(swapon --show --noheadings)"
say "=== done"
