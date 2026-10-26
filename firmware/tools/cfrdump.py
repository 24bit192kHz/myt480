#!/usr/bin/env python3
"""cfrdump.py: list the CFR option forms in the coreboot table. Usage: cbmem -r 43425442 | python3 cfrdump.py"""
import struct, sys
buf = sys.stdin.buffer.read()
if buf[:4] != b'LBIO':
    sys.exit("no LBIO header (%r)" % buf[:4])
hb, _hc, tb, _tc, n = struct.unpack_from('<IIIII', buf, 4)
recs = []
off = hb
while off + 8 <= hb + tb:
    tag, size = struct.unpack_from('<II', buf, off)
    if size < 8:
        break
    recs.append((tag, off, size))
    off += size
print("coreboot table: %d records (header says %d), tags: %s" % (len(recs), n, ' '.join('%#x' % t for t, _, _ in recs)))
HDR = {1: 28, 2: 12, 3: 48, 4: 48, 5: 48, 6: 28, 11: 28}
NAMES = {1: 'form', 2: 'enum-value', 3: 'enum', 4: 'number', 5: 'bool', 6: 'varchar', 7: 'opt_name', 8: 'ui_name', 9: 'help', 10: 'default', 11: 'comment', 12: 'dep_values'}
FLAGS = {1: 'READONLY', 2: 'INACTIVE', 4: 'SUPPRESS', 8: 'VOLATILE', 16: 'RUNTIME', 32: 'LOCK_AT_BOOT'}

def flags(f):
    return ','.join(v for k, v in FLAGS.items() if f & k) or '-'

def walk(start, end, depth):
    off = start
    while off + 8 <= end:
        tag, size = struct.unpack_from('<II', buf, off)
        if size < 8 or off + size > end:
            print('  ' * depth + 'bad record tag %d size %d at %#x' % (tag, size, off)); return
        name = NAMES.get(tag, 'tag%d' % tag)
        if tag in (7, 8, 9, 10):
            dl = struct.unpack_from('<I', buf, off + 8)[0]
            print('  ' * depth + '%s: %r' % (name, buf[off + 12:off + 12 + dl].rstrip(b'\0').decode(errors='replace')))
        elif tag in (1, 3, 4, 5, 6, 11):
            oid, dep, fl = struct.unpack_from('<QQI', buf, off + 8)
            extra = ''
            if tag in (3, 4, 5):
                dv, mn, mx, stp, disp = struct.unpack_from('<IIIII', buf, off + 28)
                extra = ' default=%d' % dv + (' range %d..%d' % (mn, mx) if tag == 4 else '')
            print('  ' * depth + '%s id=%#x dep=%#x flags=%s%s' % (name, oid, dep, flags(fl), extra))
            walk(off + HDR[tag], off + size, depth + 1)
        elif tag == 2:
            val = struct.unpack_from('<I', buf, off + 8)[0]
            print('  ' * depth + 'enum-value %d' % val); walk(off + 12, off + size, depth + 1)
        else:
            print('  ' * depth + '%s size %d' % (name, size))
        off += size

cfr = [r for r in recs if r[0] == 0x47]
if not cfr:
    sys.exit("no CFR record (tag 0x47) in the table")
for _tag, off, size in cfr:
    ver, csum = struct.unpack_from('<II', buf, off + 8)
    print("CFR root: size %d version %d checksum %#x" % (size, ver, csum))
    walk(off + 16, off + size, 1)
