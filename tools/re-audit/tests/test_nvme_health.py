import ctypes
import importlib.util
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("nvme_health", Path(__file__).parents[1] / "nvme-health.py")
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class SmartLog(unittest.TestCase):
    def test_read_only_command_and_abi(self):
        command = module.smart_command(ctypes.create_string_buffer(512))
        self.assertEqual(ctypes.sizeof(command), 72)
        self.assertEqual(module.AdminCommand.cdw10.offset, 40)
        self.assertEqual(command.opcode, 2)  # GET LOG PAGE, never a write/format
        self.assertEqual(command.data_len, 512)
        self.assertEqual(command.cdw10 & 0xff, 2)  # only SMART
        self.assertTrue(command.cdw10 & (1 << 15))  # retain async events
        self.assertEqual(command.cdw10 >> 16, 127)  # 128 dwords
        self.assertEqual(command.timeout_ms, 5000)

    def test_128_bit_counters_and_temperature(self):
        data = bytearray(512)
        data[1:3] = (310).to_bytes(2, "little")
        data[3:6] = bytes([100, 10, 7])
        data[32:48] = (2 ** 96 + 123).to_bytes(16, "little")
        data[160:176] = (13).to_bytes(16, "little")
        data[192:196] = (1024).to_bytes(4, "little")
        result = module.decode_smart(data)
        self.assertEqual(result["data_units_read"], 2 ** 96 + 123)
        self.assertEqual(result["media_data_integrity_errors"], 13)
        self.assertAlmostEqual(result["composite_temperature_c"], 36.85)
        self.assertEqual(result["warning_temperature_minutes"], 1024)

    def test_missing_temperature_and_short_log(self):
        self.assertIsNone(module.decode_smart(bytes(512))["composite_temperature_c"])
        self.assertEqual(module.decode_smart(bytes(512))["temperature_sensors_c"], [None] * 8)
        for size in [0, 511, 513]:
            with self.assertRaises(ValueError):
                module.decode_smart(bytes(size))

    def test_sparse_temperature_sensors_keep_their_number(self):
        data = bytearray(512)
        data[202:204] = (310).to_bytes(2, "little")  # Sensor 2, sensor 1 absent.
        sensors = module.decode_smart(data)["temperature_sensors_c"]
        self.assertEqual(len(sensors), 8)
        self.assertIsNone(sensors[0])
        self.assertAlmostEqual(sensors[1], 36.85)
        self.assertEqual(sensors[2:], [None] * 6)


if __name__ == "__main__":
    unittest.main()
