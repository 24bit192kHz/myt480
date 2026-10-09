#!/bin/sh
# File observations only; no NVML, nvidia-smi, sudo, systemctl or device writes.
# --root DIR reads an offline fixture with matching proc/sys/etc paths.
exec python3 - "$@" <<'PY'
import argparse
import json
from pathlib import Path
import re
import sys

parser = argparse.ArgumentParser(prog="nvidia-suspend-test.sh",
    description="Inspect T480 NVIDIA configuration; this does not test sleep.")
parser.add_argument("--root", type=Path, default=Path("/"))
parser.add_argument("--json", action="store_true")
args = parser.parse_args()
root = args.root.resolve()
if not root.is_dir():
    parser.error("--root must be a directory")
rows = []

def note(status, message):
    rows.append({"status": status, "message": message})

def path(name):
    return root / name.lstrip("/")

def read(name):
    try:
        return path(name).read_text(errors="replace").strip()
    except FileNotFoundError:
        return None
    except OSError as error:
        note("UNKNOWN", f"Cannot read {name}: {error.strerror}")
        return None

gpu = "/sys/bus/pci/devices/0000:01:00.0"
vendor, device = read(gpu + "/vendor"), read(gpu + "/device")
loaded = path("/sys/module/nvidia").is_dir()
if vendor != "0x10de" or device != "0x1d10":
    note("SKIP", "MX150 is absent at 01:00.0; firmware-off or a different topology needs separate inspection.")
else:
    note("INFO", "MX150 PCI identity is present.")
    for label, suffix in (("PCI power", "/power_state"), ("Runtime PM", "/power/runtime_status")):
        value = read(gpu + suffix)
        note("INFO" if value is not None else "UNKNOWN", f"{label}: {value or 'unavailable'}")

if not loaded:
    note("SKIP", "NVIDIA driver is unloaded; valid for on-demand policy. Driver sleep readiness was not assessed.")
else:
    version = read("/proc/driver/nvidia/version")
    if version is None:
        note("UNKNOWN", "Loaded module version is unavailable.")
    else:
        match = re.search(r"Kernel Module\s+(\d+\.\d+\.\d+)", version)
        note("INFO", f"Driver version: {match.group(1) if match else 'unparsed'}")
        if "Open Kernel Module" in version and vendor == "0x10de" and device == "0x1d10":
            note("FAIL", "NVIDIA open kernel modules do not support the Pascal MX150.")
    raw = read("/proc/driver/nvidia/params")
    params = {key.strip(): value.strip() for line in (raw or "").splitlines()
              for key, sep, value in [line.partition(":")] if sep}
    if raw is None:
        note("UNKNOWN", "Loaded driver parameters are unavailable.")
    preserve = params.get("PreserveVideoMemoryAllocations")
    if preserve == "1":
        if path("/proc/driver/nvidia/suspend").exists():
            note("INFO", "VRAM preservation is enabled and the proc sleep interface exists.")
        else:
            note("FAIL", "VRAM preservation is enabled but its proc sleep interface is absent.")
    elif preserve == "0":
        note("WARN", "Full VRAM preservation is disabled; live GPU allocations need separate sleep validation.")
    else:
        note("UNKNOWN", "VRAM preservation state could not be determined.")
    mode = params.get("DynamicPowerManagement")
    note("INFO" if mode is not None else "UNKNOWN", f"Runtime D3 parameter: {mode or 'unavailable'}")
    for label, name in (("KMS", "/sys/module/nvidia_drm/parameters/modeset"),
                        ("fbdev", "/sys/module/nvidia_drm/parameters/fbdev")):
        note("INFO", f"NVIDIA {label}: {read(name) or 'module absent/unreadable'}; disabled is valid for Intel-driven offload.")

note("INFO", "Systemd units, NVIDIA persistence and early NVIDIA KMS are not prerequisites for this Artix/s6 offload configuration.")
owners = []
config_files = ["/etc/elogind/logind.conf", "/etc/elogind/sleep.conf"]
for directory in ("/etc/elogind/logind.conf.d", "/etc/elogind/sleep.conf.d"):
    config_files.extend("/" + str(p.relative_to(root)) for p in sorted(path(directory).glob("*.conf")))
for name in config_files:
    content = read(name)
    if content:
        owners.extend(re.findall(r"(?m)^\s*HandleNvidiaSleep\s*=\s*(\S+)", content))
note("INFO", "Observed HandleNvidiaSleep: " + (", ".join(owners) if owners else "unspecified; daemon defaults not inferred"))
hooks = []
for directory in ("/usr/lib/elogind/system-sleep", "/usr/libexec/elogind/system-sleep",
                  "/etc/elogind/system-sleep"):
    for candidate in sorted(path(directory).glob("*")):
        if candidate.is_file() and "nvidia" in candidate.name.lower():
            name = "/" + str(candidate.relative_to(root))
            content = read(name)
            if content is None:
                continue
            hooks.append(name)
            text = "\n".join(line for line in content.splitlines() if not line.lstrip().startswith("#"))
            if re.search(r"nvidia-sleep\.sh[\"']?\s+[\"']?resume[\"']?\s*&", text):
                note("WARN", f"{name} backgrounds NVIDIA resume; completion is not awaited.")
            pre = re.search(r"\bpre\s*\)(.*?);;", text, re.S)
            if pre and re.search(r"nvidia-sleep\.sh\s+suspend\b", pre.group(1)):
                if "$2" not in pre.group(1) and "SYSTEMD_SLEEP_ACTION" not in pre.group(1):
                    note("WARN", f"{name} has a fixed suspend pre-call; check hibernate dispatch.")
if hooks:
    note("INFO", "Observed NVIDIA hook files: " + ", ".join(hooks) + "; execution precedence/order was not tested.")
if any(v.lower() in {"yes", "true", "1"} for v in owners) and hooks:
    note("WARN", "Elogind NVIDIA handling and hook files coexist; verify one coherent owner before testing.")
note("INFO", "File observations and hook heuristics do not establish CUDA correctness, VRAM backing capacity, locker readiness or successful suspend/hibernate.")
failed = sum(r["status"] == "FAIL" for r in rows)
unknown = sum(r["status"] == "UNKNOWN" for r in rows)
if args.json:
    print(json.dumps({"checks": rows, "failures": failed, "unknown": unknown,
                      "hardware_tested": False, "settings_changed": False}, indent=2))
else:
    for row in rows:
        print(f"[{row['status']}] {row['message']}")
sys.exit(1 if failed else 2 if unknown else 0)
PY
