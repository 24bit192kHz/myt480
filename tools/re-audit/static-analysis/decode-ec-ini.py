#!/usr/bin/env python3
"""Decode INI3 initialized SRAM metadata in the exact N24HT37W EC payload.

Reads a firmware copy only; emits hashes and derived mapping, never SRAM bytes.
Does not access ports, execute firmware or write an EC. See README.md for scope.
"""

import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys


PAYLOAD_BYTES = 0x46000
PAYLOAD_SHA256 = "befcb425b9f2a83b959c0ef3ee490420b7c5e5ffa2a792fda821d0ff355e77cf"
RAM_BASE = 0x800000
RAM_END = 0x804000


def decode(path):
    if path.stat().st_size != PAYLOAD_BYTES:
        raise ValueError("expected the 286720-byte payload after the 32-byte FL2 wrapper")
    image = path.read_bytes()
    digest = hashlib.sha256(image).hexdigest()
    if digest != PAYLOAD_SHA256:
        raise ValueError("input SHA256 does not match the audited N24HT37W payload")
    pos = 0x2E574
    records = []
    ram = {}
    while True:
        dest, length = struct.unpack_from("<Ii", image, pos)
        record = pos
        pos += 8
        if dest == 0 and length == 0:
            break
        if length < 0:
            if -length > RAM_END - RAM_BASE:
                raise ValueError("zero-filled record exceeds SRAM")
            out = bytes(-length)
        elif length == 0:
            out = b""
        else:
            encoded = image[pos:pos + length]
            if len(encoded) != length:
                raise ValueError("encoded record exceeds payload")
            if encoded[0] == 1:
                out = encoded[1:]
            else:
                cursor = 1
                flags = remaining = 0
                out = bytearray()
                while cursor < len(encoded):
                    if remaining == 0:
                        flags = struct.unpack_from("<H", encoded, cursor)[0]
                        cursor += 2
                        remaining = 16
                    if flags & 1:
                        if cursor + 2 > len(encoded):
                            raise ValueError("truncated back reference")
                        a, b = encoded[cursor:cursor + 2]
                        cursor += 2
                        distance = b + ((a & 0xF0) << 4)
                        count = (a & 15) + 1
                        if not 0 < distance <= len(out):
                            raise ValueError("invalid back-reference distance")
                        for _ in range(count):
                            out.append(out[-distance])
                    else:
                        if cursor >= len(encoded):
                            raise ValueError("truncated literal")
                        out.append(encoded[cursor])
                        cursor += 1
                    if len(out) > RAM_END - RAM_BASE:
                        raise ValueError("decoded record exceeds SRAM")
                    flags >>= 1
                    remaining -= 1
                if cursor != len(encoded):
                    raise ValueError("record did not terminate at its encoded boundary")
                out = bytes(out)
            pos += (length + 3) & ~3
        if not RAM_BASE <= dest <= dest + len(out) <= RAM_END:
            raise ValueError("record destination exceeds audited SRAM")
        if any(dest + i in ram for i in range(len(out))):
            raise ValueError("overlapping SRAM records")
        ram.update({dest + i: byte for i, byte in enumerate(out)})
        records.append(dict(record_payload_address=hex(record), destination=hex(dest),
                            encoded_length=length, decoded_length=len(out),
                            decoded_sha256=hashlib.sha256(out).hexdigest()))
    mapping = bytes(ram[0x800518 + i] for i in range(12))
    values = struct.unpack("<6h", mapping)
    return dict(version="N24HT37W", input_name=path.name, input_bytes=len(image),
                source_sha256=digest, ini_address="0x2e56c", record_start="0x2e574",
                terminal_address=hex(pos - 8), callback="0x2defc",
                decoder="literal or 16-bit LSB-first flags; back-reference distance=(first&0xf0)*16+second, count=(first&0x0f)+1",
                records=records, map_sram_address="0x800518", six_signed_halfwords=values,
                map_bytes_sha256=hashlib.sha256(mapping).hexdigest(), id_0x20_group=4,
                id_0x20_cached_byte=values[4], id_0x20_mask=1,
                limitations="Static initialized values only; does not observe retained/runtime state, prove host eligibility, recover all EC code or authorize EC writes.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path, help="exact N24HT37W payload copy")
    parser.add_argument("--output", type=Path, help="new metadata JSON file; default is stdout")
    args = parser.parse_args()
    try:
        encoded = json.dumps(decode(args.input), indent=2) + "\n"
        if args.output:
            with args.output.open("x", encoding="utf-8") as stream:
                stream.write(encoded)
        else:
            sys.stdout.write(encoded)
    except (OSError, ValueError, KeyError, IndexError, struct.error) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
