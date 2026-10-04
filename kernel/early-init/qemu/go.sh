#!/bin/sh
# rebuild toolbox + init + replica disk, run the rehearsal
set -e; cd "$(dirname "$0")/.."
docker build -q -t t480-toolbox -f Dockerfile.toolbox . >/dev/null
id=$(docker create t480-toolbox); docker export $id | gzip > out/toolbox.tar.gz; docker rm $id >/dev/null
docker run --rm -e ROOT_TAR=${ROOT_TAR:-} -v "$PWD:/w" -w /w t480-early sh -c 'gcc -Os -static -Wall -Wextra -o out/init init.c && strip out/init && rm -f qemu/work/*.img && sh qemu/mkdisk.sh /w /w/qemu/work/vmlinuz /w/qemu/work/vmlinuz.sig' | tail -2
# BUILTIN=1: use the early init built into the kernel instead of the freshly compiled one
cd qemu
if [ -n "${BUILTIN:-}" ]; then exec timeout 1700 python3 run.py work/vmlinuz; fi
exec timeout 1700 python3 run.py work/vmlinuz --initrd work/early.cpio
