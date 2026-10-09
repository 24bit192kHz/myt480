#!/usr/bin/env python3
"""Offline T480 stock UCSI packet model. Never opens devices or executes I/O.

Recovered from UsbCTabl AML and SmmAslSmi. This is a protocol fixture, not a
driver: EC status bits 0..6, transport arbitration, and real timing are unknown.
Run --self-test, packet <operation> [--data HEX], or cci <integer>.
"""

import argparse
import itertools
import json
import struct
import unittest
from dataclasses import dataclass


OPERATIONS = {
    "write-message-out": (0x0A, 0x06, 16),
    "write-control": (0x0A, 0x04, 8),
    "read-message-in": (0x0B, 0x05, 16),
    "read-cci": (0x0B, 0x03, 4),
}
CCI_FLAGS = {
    25: "not-supported",
    26: "cancel-complete",
    27: "reset-complete",
    28: "busy",
    29: "ack-complete",
    30: "error",
    31: "command-complete",
}


@dataclass(frozen=True)
class Request:
    operation: str
    data: bytes = b""

    def __post_init__(self):
        if self.operation not in OPERATIONS:
            raise ValueError("unsupported stock UCSI operation")
        if not isinstance(self.data, bytes):
            raise ValueError("payload must be bytes")
        command, _, length = OPERATIONS[self.operation]
        expected = length if command == 0x0A else 0
        if len(self.data) != expected:
            raise ValueError(f"{self.operation} needs exactly {expected} payload bytes")

    def packet(self):
        """37-byte MHPF request; unspecified/padding bytes are zero."""
        command, selector, length = OPERATIONS[self.operation]
        packet = bytearray(37)
        packet[:4] = bytes((command, 0, 0x02, selector))
        packet[4:4 + len(self.data)] = self.data
        packet[36] = length
        return bytes(packet)

    def submission_writes(self, command_register):
        """EC register/value plan only. A busy EC must receive no writes."""
        if not 0 <= command_register <= 255:
            raise ValueError("command register must be a byte")
        if command_register:
            raise BlockingIOError("EC mailbox busy; status is not a response for this request")
        packet = self.packet()
        writes = [(0x52, packet[2]), (0x53, packet[3]), (0x74, packet[36])]
        if not packet[0] & 1:
            writes += [(0x54 + i, value) for i, value in enumerate(self.data)]
        writes.append((0x50, packet[0]))  # Submit only after metadata/payload.
        return writes


def write_sequence(control, message_out):
    """ECWR sends MESSAGE_OUT before CONTROL, under one outer lock."""
    return [Request("write-message-out", message_out), Request("write-control", control)]


def read_sequence():
    """ECRD refreshes MESSAGE_IN before CCI; NTFY publishes afterward."""
    return [Request("read-message-in"), Request("read-cci")]


def stock_completion(command_samples, status):
    """Model CHKS's finite poll count, not wall-clock or a port transaction.

    Samples are EC RAM 0x50 values. Stock sleeps 1 ms after each busy sample,
    at most 1000 times. 0x51 bit 7 is HMDN. Low status bits remain opaque.
    Insufficient fixture samples are rejected instead of fabricating completion.
    """
    if not 0 <= status <= 255:
        raise ValueError("status must be a byte")
    for index, command in enumerate(itertools.islice(command_samples, 1000)):
        if not 0 <= command <= 255:
            raise ValueError("command samples must be bytes")
        if command == 0:
            return {"aml_result": 0 if status & 0x80 else 0x8081,
                    "busy_polls": index, "raw_status": status,
                    "opaque_status_bits": status & 0x7F}
        if index == 999:
            return {"aml_result": 0x8080, "busy_polls": 1000,
                    "raw_status": status, "opaque_status_bits": status & 0x7F}
    raise ValueError("fixture ended before completion or timeout")


def decode_cci(value):
    if not 0 <= value <= 0xFFFFFFFF:
        raise ValueError("CCI must be an unsigned 32-bit integer")
    return {"raw": f"0x{value:08x}", "connector": (value >> 1) & 0x7F,
            "data_length": (value >> 8) & 0xFF,
            "flags": [name for bit, name in CCI_FLAGS.items() if value & (1 << bit)],
            "length_exceeds_stock_window": ((value >> 8) & 0xFF) > 16}


def initial_mailbox():
    """48 bytes explicitly initialized by stock DXE; its allocation is 4 KiB."""
    return struct.pack("<H", 0x0100) + bytes(46)


class ModelTests(unittest.TestCase):
    def test_golden_requests(self):
        vectors = {
            "write-message-out": ("0a000206", bytes(range(16)), 16, 16),
            "write-control": ("0a000204", bytes(range(8)), 24, 8),
            "read-message-in": ("0b000205", b"", 32, 16),
            "read-cci": ("0b000203", b"", 32, 4),
        }
        for operation, (header, data, padding, length) in vectors.items():
            packet = Request(operation, data).packet()
            self.assertEqual(len(packet), 37)
            self.assertEqual(packet, bytes.fromhex(header) + data + bytes(padding) + bytes((length,)))

    def test_exact_lengths_and_supported_selectors(self):
        for operation in OPERATIONS:
            for bad in [b"\x00", bytes(33)]:
                with self.assertRaises(ValueError):
                    Request(operation, bad)
        with self.assertRaises(ValueError):
            Request("read-version")  # DXE supplies version, not an AML selector.

    def test_submission_and_busy(self):
        request = Request("write-control", bytes(range(8)))
        writes = request.submission_writes(0)
        self.assertEqual(writes[:3], [(0x52, 2), (0x53, 4), (0x74, 8)])
        self.assertEqual(writes[3:-1], list(zip(range(0x54, 0x5C), range(8))))
        self.assertEqual(writes[-1], (0x50, 0x0A))
        self.assertEqual(Request("read-cci").submission_writes(0),
                         [(0x52, 2), (0x53, 3), (0x74, 4), (0x50, 0x0B)])
        for command in (1, 0x0A, 0xFF):
            with self.assertRaises(BlockingIOError):
                request.submission_writes(command)

    def test_pair_order(self):
        writes = write_sequence(bytes(8), bytes(16))
        self.assertEqual([r.operation for r in writes], ["write-message-out", "write-control"])
        self.assertEqual([r.operation for r in read_sequence()], ["read-message-in", "read-cci"])

    def test_completion_boundaries_and_opaque_bits(self):
        self.assertEqual(stock_completion([0], 0x80)["aml_result"], 0)
        self.assertEqual(stock_completion([1, 1, 0], 0x80)["busy_polls"], 2)
        self.assertEqual(stock_completion([0], 0)["aml_result"], 0x8081)
        result = stock_completion([0], 0x87)
        self.assertEqual(result["aml_result"], 0)
        self.assertEqual(result["opaque_status_bits"], 7)  # Not interpreted as success/error.
        self.assertEqual(stock_completion([1] * 999 + [0], 0x80)["aml_result"], 0)
        self.assertEqual(stock_completion([1] * 1000 + [0], 0x80)["aml_result"], 0x8080)
        with self.assertRaises(ValueError):
            stock_completion([1] * 999, 0x80)

    def test_cci_and_initialization(self):
        cci = decode_cci(0x80001004)
        self.assertEqual(cci["connector"], 2)
        self.assertEqual(cci["data_length"], 16)
        self.assertEqual(cci["flags"], ["command-complete"])
        self.assertFalse(cci["length_exceeds_stock_window"])
        self.assertTrue(decode_cci(17 << 8)["length_exceeds_stock_window"])
        self.assertEqual(initial_mailbox(), b"\x00\x01" + bytes(46))
        for bad in (-1, 1 << 32):
            with self.assertRaises(ValueError):
                decode_cci(bad)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    subparsers = parser.add_subparsers(dest="mode")
    packet = subparsers.add_parser("packet", help="print packet bytes and offline EC write plan")
    packet.add_argument("operation", choices=OPERATIONS)
    packet.add_argument("--data", default="", help="exact payload bytes as hex")
    cci = subparsers.add_parser("cci", help="decode a UCSI CCI value")
    cci.add_argument("value", type=lambda value: int(value, 0))
    args = parser.parse_args()
    if args.self_test:
        result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(ModelTests))
        raise SystemExit(not result.wasSuccessful())
    try:
        if args.mode == "packet":
            request = Request(args.operation, bytes.fromhex(args.data))
            print(json.dumps({"packet_hex": request.packet().hex(),
                              "writes_if_idle": request.submission_writes(0)}, indent=2))
        elif args.mode == "cci":
            print(json.dumps(decode_cci(args.value), indent=2))
        else:
            parser.error("choose packet, cci, or --self-test")
    except ValueError as error:
        parser.error(str(error))


if __name__ == "__main__":
    main()
