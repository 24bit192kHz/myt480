#!/usr/bin/env python3
"""Recover selected chip/legacy tables from three exact audited NVIDIA ELFs.

Reads an artifact copy only. Does not load the module, execute NVIDIA code,
connect to a target or implement a driver patch. See README.md for scope.
"""

import argparse
import hashlib
import json
from pathlib import Path
import struct
import sys


AUDITED = {
    "580.178.04": {
        "bytes": 112677456,
        "sha256": "5269a23ffba19a17fe16f076463e7ad4151254659441d1711bee6c6221ff6f4b",
        "chiptable": "_nv019377rm", "legacy_offset": 0x4E085A0, "legacy_count": 15,
    },
    "610.57.04": {
        "bytes": 120980872,
        "sha256": "c90f58d59e8fef44fa07d057bd9ffb1e1b5ee38d3c51b35df71e05e0ad268cbf",
        "chiptable": "_nv019282rm", "legacy_offset": 0x5725040, "legacy_count": 19,
    },
    "615.78.08": {
        "bytes": 115570992,
        "sha256": "8fed359e27d938fd90f0a027fbfe98f30737f97c9055f6f31b20d245b6b4ca2d",
        "chiptable": "_nv020004rm", "legacy_offset": 0x514E840, "legacy_count": 19,
    },
}


class ELF:
    """Bounded ELF64 little-endian ET_REL reader for the audited object files."""

    def __init__(self, data):
        self.b = data
        if data[:6] != b"\x7fELF\x02\x01":
            raise ValueError("expected ELF64 little-endian")
        header = struct.unpack_from("<16sHHIQQQIHHHHHH", data, 0)
        if header[1] != 1 or header[2] != 62 or header[11] != 64:
            raise ValueError("expected x86-64 ET_REL with 64-byte section headers")
        self.slice(header[6], header[11] * header[12])
        fields = ("nameoff", "type", "flags", "addr", "offset", "size",
                  "link", "info", "align", "entsize")
        self.sh = [dict(zip(fields, struct.unpack_from(
            "<IIQQQQIIQQ", data, header[6] + i * header[11])))
            for i in range(header[12])]
        names = self.sectiondata_by_index(header[13])
        for i, section in enumerate(self.sh):
            section.update(index=i, name=self.zstr(names, section["nameoff"]))
        self.sections = {s["name"]: s for s in self.sh}
        self.symbols = []
        self.named = {}
        self.symbol_tables = {}
        for section in self.sh:
            if section["type"] != 2:
                continue
            if section["entsize"] != 24 or section["size"] % 24:
                raise ValueError("unexpected symbol entry size")
            strings = self.sectiondata_by_index(section["link"])
            symbols = []
            raw = self.sectiondata_by_index(section["index"])
            for offset in range(0, len(raw), 24):
                name, info, other, index, value, size = struct.unpack_from(
                    "<IBBHQQ", raw, offset)
                symbol = dict(index=len(symbols), name=self.zstr(strings, name),
                              info=info, section=index, value=value, size=size)
                symbols.append(symbol)
                if symbol["name"]:
                    self.named[symbol["name"]] = symbol
            self.symbol_tables[section["index"]] = symbols
            self.symbols.extend(symbols)
        self.relocs = []
        for section in self.sh:
            if section["type"] != 4:
                continue
            if section["entsize"] != 24 or section["size"] % 24:
                raise ValueError("unexpected RELA entry size")
            symbols = self.symbol_tables[section["link"]]
            raw = self.sectiondata_by_index(section["index"])
            for offset in range(0, len(raw), 24):
                target, info, addend = struct.unpack_from("<QQq", raw, offset)
                self.relocs.append(dict(section=section["info"], offset=target,
                                        type=info & 0xFFFFFFFF,
                                        symbol=symbols[info >> 32], addend=addend))

    def slice(self, offset, size):
        if offset < 0 or size < 0 or offset + size > len(self.b):
            raise ValueError("ELF range exceeds input")
        return self.b[offset:offset + size]

    @staticmethod
    def zstr(data, offset):
        if not 0 <= offset < len(data):
            raise ValueError("string offset exceeds table")
        end = data.find(b"\0", offset)
        if end < 0:
            raise ValueError("unterminated ELF string")
        return data[offset:end].decode(errors="replace")

    def sectiondata_by_index(self, index):
        section = self.sh[index]
        return self.slice(section["offset"], section["size"])

    def sectiondata(self, name):
        return self.sectiondata_by_index(self.sections[name]["index"])

    def symdata(self, name):
        symbol = self.named[name]
        section = self.sectiondata_by_index(symbol["section"])
        end = symbol["value"] + symbol["size"]
        if end > len(section):
            raise ValueError("symbol exceeds its section")
        return section[symbol["value"]:end]

    def symrelocs(self, name):
        symbol = self.named[name]
        return [r for r in self.relocs if r["section"] == symbol["section"]
                and symbol["value"] <= r["offset"] < symbol["value"] + symbol["size"]]

    def strptr(self, section, offset):
        relocation = next((r for r in self.relocs
                           if r["section"] == section and r["offset"] == offset), None)
        if relocation is None:
            return None
        symbol = relocation["symbol"]
        data = self.sectiondata_by_index(symbol["section"])
        return self.zstr(data, symbol["value"] + relocation["addend"])


def inspect(path, version):
    identity = AUDITED[version]
    if path.stat().st_size != identity["bytes"]:
        raise ValueError("input length does not match the selected audited artifact")
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    if digest != identity["sha256"]:
        raise ValueError("input SHA256 does not match the selected audited artifact")
    elf = ELF(data)
    table = identity["chiptable"]
    raw = elf.symdata(table)
    if len(raw) % 12:
        raise ValueError("chip table is not a sequence of 12-byte rows")
    rows = [dict(index=i // 12, arch=a, impl=b, hidrev=c)
            for i, (a, b, c) in zip(range(0, len(raw), 12), struct.iter_unpack("<III", raw))]
    rodata = elf.sectiondata(".rodata")
    legacy = []
    for index in range(identity["legacy_count"]):
        offset = identity["legacy_offset"] + index * 24
        a, b, pointer, branch, pad = struct.unpack_from("<IIQII", rodata, offset)
        legacy.append(dict(index=index, boot0arch=a, boot42arch=b,
                           label=elf.strptr(elf.sections[".rodata"]["index"], offset + 8),
                           branch=branch))
    return dict(version=version, input_name=path.name, input_bytes=len(data), sha256=digest,
                chiptable_symbol=elf.named[table], chiptable=rows,
                legacytable_offset=hex(identity["legacy_offset"]), legacy=legacy,
                scope="Exact static chip and legacy table recovery; does not establish runtime support, complete backend recovery or a working patch.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True, choices=tuple(AUDITED))
    parser.add_argument("--input", required=True, type=Path, help="audited nv-kernel.o_binary copy")
    parser.add_argument("--output", type=Path, help="new JSON file; default is stdout")
    args = parser.parse_args()
    try:
        result = inspect(args.input, args.version)
        encoded = json.dumps(result, indent=2) + "\n"
        if args.output:
            with args.output.open("x", encoding="utf-8") as stream:
                stream.write(encoded)
        else:
            sys.stdout.write(encoded)
    except (OSError, ValueError, KeyError, IndexError, struct.error) as error:
        parser.exit(1, f"error: {error}\n")


if __name__ == "__main__":
    main()
