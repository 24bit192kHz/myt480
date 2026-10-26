#!/usr/bin/env python3
"""Pull the complete dGPU VBIOS out of a Lenovo firmware image.

The NVIDIA driver needs the whole PCI option ROM through ACPI _ROM (182272 bytes for the
MX150); a dump read from the running GPU is cut off at the size byte of its header and
fails with "RmInitAdapter failed! (0x24:0x40:1603)".

    extract-vbios.py IMAGE [-o pci10de,1d10.rom] [--vendor 10de] [--device 1d10] [--list] [--index N]

Candidates come from UEFIExtract ("uefiextract IMAGE all", the raw sections of the FFS
files) when it is installed, and from a raw scan of the image otherwise (that misses
compressed volumes). Every candidate is checked against its PCI data structures: the image
chain must be complete. The output file is exactly the length of the chain.
"""
import argparse, hashlib, os, re, shutil, struct, subprocess, sys, tempfile

GUID_RE = re.compile(r'[0-9A-Fa-f]{8}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{4}-[0-9A-Fa-f]{12}')


def parse_chain(buf, off=0):
    """Walk the PCI option ROM images at buf[off:]. Returns (total, vendor, device,
    header_size, complete, note)."""
    total = 0
    vendor = device = header_size = None
    while True:
        if off + total + 0x1a > len(buf):
            return total, vendor, device, header_size, False, 'header beyond the end'
        h = buf[off + total:]
        if h[0] != 0x55 or h[1] != 0xAA:
            return total, vendor, device, header_size, False, 'no 55 AA signature'
        pcir = struct.unpack_from('<H', h, 0x18)[0]
        if pcir < 0x1a or pcir + 0x18 > len(h):
            return total, vendor, device, header_size, False, 'PCIR pointer beyond the end'
        if h[pcir:pcir + 4] != b'PCIR':
            return total, vendor, device, header_size, False, 'no PCIR structure'
        v, d = struct.unpack_from('<HH', h, pcir + 4)
        ilen = struct.unpack_from('<H', h, pcir + 0x10)[0] * 512
        indicator = h[pcir + 0x15]
        if vendor is None:
            vendor, device, header_size = v, d, h[2] * 512
        if not ilen:
            return total, vendor, device, header_size, False, 'zero image length'
        total += ilen
        if off + total > len(buf):
            return total, vendor, device, header_size, False, \
                'cut: the image needs %d bytes, %d are there' % (total, len(buf) - off)
        if indicator & 0x80:
            return total, vendor, device, header_size, True, 'complete'


def candidates_raw(img):
    """Offsets in img that look like a PCI option ROM header with a PCIR structure."""
    out = []
    pos = 0
    while True:
        pos = img.find(b'\x55\xAA', pos)
        if pos < 0 or pos + 0x1a > len(img):
            break
        pcir = struct.unpack_from('<H', img, pos + 0x18)[0]
        if 0x1a <= pcir < 0x1000 and img[pos + pcir:pos + pcir + 4] == b'PCIR':
            out.append(('offset 0x%x' % pos, img, pos))
        pos += 2
    return out


def candidates_uefiextract(path):
    exe = shutil.which('uefiextract') or shutil.which('UEFIExtract')
    if not exe:
        return None
    tmp = tempfile.mkdtemp(prefix='vbios-')
    try:
        link = os.path.join(tmp, 'image.bin')
        shutil.copyfile(path, link)  # UEFIExtract follows symlinks and dumps next to the original
        r = subprocess.run([exe, link, 'all'], cwd=tmp, capture_output=True, text=True)
        dump = link + '.dump'
        if not os.path.isdir(dump):
            print('uefiextract failed (%s); falling back to a raw scan' % r.stdout.strip()[-80:],
                  file=sys.stderr)
            return None
        out = []
        for root, _dirs, files in os.walk(dump):
            for f in files:
                if f != 'body.bin':
                    continue
                p = os.path.join(root, f)
                with open(p, 'rb') as fh:
                    head = fh.read(0x1000)
                if len(head) < 0x1a or head[:2] != b'\x55\xAA':
                    continue
                pcir = struct.unpack_from('<H', head, 0x18)[0]
                if not (0x1a <= pcir < 0x1000) or head[pcir:pcir + 4] != b'PCIR':
                    continue
                with open(p, 'rb') as fh:
                    body = fh.read()
                guids = GUID_RE.findall(os.path.relpath(root, dump))
                where = 'FFS file %s' % guids[-1] if guids else os.path.relpath(root, dump)
                out.append((where, body, 0))
        return out
    finally:
        shutil.rmtree(tmp, ignore_errors=True)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('image')
    ap.add_argument('-o', '--output', help='output file (default pciVVVV,DDDD.rom)')
    ap.add_argument('--vendor', default='10de', help='PCI vendor ID (hex, default 10de)')
    ap.add_argument('--device', default='1d10', help='PCI device ID (hex, default 1d10)')
    ap.add_argument('--list', action='store_true', help='only list the candidates')
    ap.add_argument('--index', type=int, help='pick candidate N from the list')
    ap.add_argument('--raw', action='store_true', help='raw scan only, do not run uefiextract')
    a = ap.parse_args()
    want = (int(a.vendor, 16), int(a.device, 16))

    with open(a.image, 'rb') as f:
        img = f.read()
    cands = None if a.raw else candidates_uefiextract(a.image)
    if cands is None:
        cands = candidates_raw(img)

    rows = []
    for where, buf, off in cands:
        total, v, d, hs, ok, note = parse_chain(buf, off)
        if v is None:
            continue
        rows.append((where, buf, off, total, v, d, hs, ok, note))
    matches = [r for r in rows if (r[4], r[5]) == want]
    for i, r in enumerate(rows):
        where, buf, off, total, v, d, hs, ok, note = r
        mark = '*' if (v, d) == want else ' '
        print('%s[%d] %-45s %04x:%04x header %6d  image %6d  %s' % (mark, i, where, v, d, hs, total, note))
    if a.list:
        return 0
    if a.index is not None:
        pick = [rows[a.index]]
    else:
        pick = [r for r in matches if r[7]]
        if not pick:
            print('no complete %04x:%04x image found' % want, file=sys.stderr)
            return 1
        bodies = {hashlib.sha256(r[1][r[2]:r[2] + r[3]]).hexdigest() for r in pick}
        if len(bodies) > 1:
            print('%d different complete images for %04x:%04x: choose one with --index' % (len(bodies), *want),
                  file=sys.stderr)
            return 1
    where, buf, off, total, v, d, hs, ok, note = pick[0]
    if not ok:
        print('warning: writing an incomplete image (%s)' % note, file=sys.stderr)
    data = buf[off:off + total]
    out = a.output or 'pci%04x,%04x.rom' % (v, d)
    with open(out, 'wb') as f:
        f.write(data)
    print('wrote %s: %d bytes from %s, sha256 %s' % (out, len(data), where, hashlib.sha256(data).hexdigest()[:16]))
    return 0


if __name__ == '__main__':
    sys.exit(main())
