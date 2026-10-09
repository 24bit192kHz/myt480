#!/usr/bin/env python3
"""Read only NVMe SMART log 0x02; retain async events and omit drive identity."""
import argparse
import ctypes
import fcntl
import json
import os
from pathlib import Path


class AdminCommand(ctypes.Structure):
    _fields_ = [("opcode", ctypes.c_uint8), ("flags", ctypes.c_uint8),
                ("reserved", ctypes.c_uint16), ("nsid", ctypes.c_uint32),
                ("cdw2", ctypes.c_uint32), ("cdw3", ctypes.c_uint32),
                ("metadata", ctypes.c_uint64), ("addr", ctypes.c_uint64),
                ("metadata_len", ctypes.c_uint32), ("data_len", ctypes.c_uint32),
                *[(f"cdw{i}", ctypes.c_uint32) for i in range(10, 16)],
                ("timeout_ms", ctypes.c_uint32), ("result", ctypes.c_uint32)]


def smart_command(buffer):
    if ctypes.sizeof(AdminCommand) != 72 or ctypes.sizeof(buffer) != 512:
        raise ValueError("Unsupported admin ABI or SMART buffer size")
    return AdminCommand(opcode=0x02, nsid=0xffffffff, addr=ctypes.addressof(buffer),
                        data_len=512, cdw10=(127 << 16) | (1 << 15) | 0x02,
                        timeout_ms=5000)


def read_smart(device):
    buffer = ctypes.create_string_buffer(512)
    command = smart_command(buffer)
    request = (3 << 30) | (ctypes.sizeof(command) << 16) | (ord("N") << 8) | 0x41
    fd = os.open(device, os.O_RDONLY | os.O_CLOEXEC)
    try:
        result = fcntl.ioctl(fd, request, bytearray(bytes(command)), True)
        if result:
            raise RuntimeError(f"NVMe GET LOG PAGE returned status 0x{result:x}")
        return buffer.raw
    finally:
        os.close(fd)


def decode_smart(data):
    if len(data) != 512:
        raise ValueError("SMART log must contain exactly 512 bytes")
    def integer(offset, length):
        return int.from_bytes(data[offset:offset + length], "little")
    kelvin = integer(1, 2)
    result = {"critical_warning": data[0],
              "composite_temperature_c": round(kelvin - 273.15, 2) if kelvin else None,
              "available_spare_percent": data[3], "spare_threshold_percent": data[4],
              "percentage_used": data[5], "warning_temperature_minutes": integer(192, 4),
              "critical_temperature_minutes": integer(196, 4)}
    fields = ["data_units_read", "data_units_written", "host_read_commands", "host_write_commands",
              "controller_busy_minutes", "power_cycles", "power_on_hours", "unsafe_shutdowns",
              "media_data_integrity_errors", "error_log_entries"]
    result.update({field: integer(32 + i * 16, 16) for i, field in enumerate(fields)})
    # Keep the eight numbered slots: zero means this sensor is unavailable.
    result["temperature_sensors_c"] = [round(value - 273.15, 2) if value else None
                                       for i in range(8)
                                       for value in [integer(200 + i * 2, 2)]]
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("device", nargs="?", default="/dev/nvme0")
    parser.add_argument("--decode", type=Path, help="Decode a saved 512-byte SMART log without device access")
    args = parser.parse_args()
    try:
        data = args.decode.read_bytes() if args.decode else read_smart(args.device)
        print(json.dumps(decode_smart(data), indent=2))
    except (OSError, RuntimeError, ValueError) as error:
        parser.exit(1, f"nvme-health: {error}\n")


if __name__ == "__main__":
    main()
