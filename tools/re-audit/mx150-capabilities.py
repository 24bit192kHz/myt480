#!/usr/bin/env python3
"""Query MX150 NVML capabilities without setters or policy changes.

NVML queries can wake an idle GPU. This is an active diagnostic, not a passive
power measurement. Do not run on the T480 without the user's approval.
"""
import argparse
import ctypes as C
import json
from pathlib import Path
import re
import sys


UINT = C.c_uint
INT = C.c_int
HANDLE = C.c_void_p
STATUS = {0: "ok", 2: "invalid_argument", 3: "not_supported", 4: "no_permission",
          15: "gpu_lost", 25: "argument_version_mismatch"}


class ClockOffset(C.Structure):
    # nvmlClockOffset_v1_t from NVIDIA's contemporaneous NVML header.
    _fields_ = [("version", UINT), ("type", UINT), ("pstate", UINT),
                ("clockOffsetMHz", INT), ("minClockOffsetMHz", INT),
                ("maxClockOffsetMHz", INT)]


def call(library, name, types, *args):
    if not (name.startswith("nvmlDeviceGet") or name.startswith("nvmlSystemGet")
            or name in {"nvmlInit_v2", "nvmlShutdown"}):
        raise ValueError("Only getter/lifetime APIs are permitted")
    try:
        function = getattr(library, name)
    except AttributeError:
        return {"status": "missing_symbol", "api": name}
    function.argtypes = types
    function.restype = INT
    code = int(function(*args))
    return {"status": STATUS.get(code, "nvml_error"), "code": code, "api": name}


def scalar(library, handle, name, kind=UINT):
    value = kind()
    result = call(library, name, [HANDLE, C.POINTER(kind)], handle, C.byref(value))
    if result["status"] == "ok":
        result["value"] = value.value
    return result


def pair(library, handle, name, kind=INT):
    lower, upper = kind(), kind()
    result = call(library, name, [HANDLE, C.POINTER(kind), C.POINTER(kind)],
                  handle, C.byref(lower), C.byref(upper))
    if result["status"] == "ok":
        result.update(min=lower.value, max=upper.value)
    return result


def text_query(library, name, handle=None):
    buffer = C.create_string_buffer(256)
    types, args = [C.POINTER(C.c_char), UINT], [buffer, UINT(len(buffer))]
    if handle is not None:
        types.insert(0, HANDLE)
        args.insert(0, handle)
    result = call(library, name, types, *args)
    if result["status"] == "ok":
        result["value"] = buffer.value.decode("utf-8", errors="replace")
    return result


def collect(library, bus_id="0000:01:00.0"):
    if not re.fullmatch(r"[0-9a-fA-F]{4}:[0-9a-fA-F]{2}:[0-9a-fA-F]{2}\.[0-7]", bus_id):
        raise ValueError("Use a complete PCI bus ID such as 0000:01:00.0")
    report = {"schema": 1, "pci_bus_id": bus_id, "settings_changed": False,
              "may_wake_gpu": True,
              "power_note": "Limits are driver-reported metadata, not proof of an adjustable cap or measured draw.",
              "init": call(library, "nvmlInit_v2", [])}
    if report["init"]["status"] != "ok":
        report["status"] = "init_failed"
        return report
    try:
        handle = HANDLE()
        report["device"] = call(library, "nvmlDeviceGetHandleByPciBusId_v2",
                                [C.c_char_p, C.POINTER(HANDLE)],
                                bus_id.encode("ascii"), C.byref(handle))
        if report["device"]["status"] != "ok":
            report["status"] = "device_lookup_failed"
            return report
        report["name"] = text_query(library, "nvmlDeviceGetName", handle)
        if report["name"]["status"] != "ok":
            report["status"] = "identity_unverified"
            return report
        if not re.search(r"\bMX150\b", report["name"]["value"]):
            report["status"] = "wrong_gpu"
            return report
        report["driver"] = text_query(library, "nvmlSystemGetDriverVersion")
        report["nvml"] = text_query(library, "nvmlSystemGetNVMLVersion")
        report["architecture"] = scalar(library, handle, "nvmlDeviceGetArchitecture")
        report["performance_state"] = scalar(library, handle, "nvmlDeviceGetPerformanceState")
        report["offsets_mhz"] = {
            "graphics": scalar(library, handle, "nvmlDeviceGetGpcClkVfOffset", INT),
            "memory": scalar(library, handle, "nvmlDeviceGetMemClkVfOffset", INT),
            "graphics_range": pair(library, handle, "nvmlDeviceGetGpcClkMinMaxVfOffset"),
            "memory_range": pair(library, handle, "nvmlDeviceGetMemClkMinMaxVfOffset"),
        }
        report["power_mw"] = {
            "usage": scalar(library, handle, "nvmlDeviceGetPowerUsage"),
            "limit": scalar(library, handle, "nvmlDeviceGetPowerManagementLimit"),
            "default_limit": scalar(library, handle, "nvmlDeviceGetPowerManagementDefaultLimit"),
            "limit_range": pair(library, handle, "nvmlDeviceGetPowerManagementLimitConstraints", UINT),
        }
        report["clocks_mhz"] = {}
        for label, clock_type in (("graphics", 0), ("sm", 1), ("memory", 2), ("video", 3)):
            value = UINT()
            result = call(library, "nvmlDeviceGetClockInfo", [HANDLE, UINT, C.POINTER(UINT)],
                          handle, UINT(clock_type), C.byref(value))
            if result["status"] == "ok":
                result["value"] = value.value
            report["clocks_mhz"][label] = result
        report["per_state_offsets_mhz"] = {}
        for label, clock_type in (("graphics", 0), ("memory", 2)):
            states = {}
            for pstate in range(16):
                info = ClockOffset()
                info.version = C.sizeof(ClockOffset) | (1 << 24)
                info.type, info.pstate = clock_type, pstate
                result = call(library, "nvmlDeviceGetClockOffsets", [HANDLE, C.POINTER(ClockOffset)],
                              handle, C.byref(info))
                if result["status"] == "ok":
                    result.update(value=info.clockOffsetMHz,
                                  min=info.minClockOffsetMHz, max=info.maxClockOffsetMHz)
                states[f"P{pstate}"] = result
            report["per_state_offsets_mhz"][label] = states
        report["voltage"] = {
            "status": "unresolved",
            "reason": "NVML does not establish MX150 millivolt control; NV-CONTROL support/range is separate.",
        }
        report["status"] = "queried"
        return report
    finally:
        report["shutdown"] = call(library, "nvmlShutdown", [])
        if report.get("status") == "queried" and report["shutdown"]["status"] != "ok":
            report["status"] = "shutdown_failed"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bus-id", default="0000:01:00.0")
    parser.add_argument("--library", default="/usr/lib/libnvidia-ml.so.1")
    args = parser.parse_args()
    # Do not initialize the driver or trigger helpers when the module is absent.
    if not Path("/sys/module/nvidia").is_dir():
        print(json.dumps({"status": "driver_not_loaded", "settings_changed": False}))
        return 77
    try:
        report = collect(C.CDLL(args.library), args.bus_id)
    except (OSError, ValueError) as error:
        print(json.dumps({"status": "probe_failed", "error": str(error)}, indent=2))
        return 1
    print(json.dumps(report, indent=2))
    return 0 if report["status"] == "queried" else 1


if __name__ == "__main__":
    sys.exit(main())
