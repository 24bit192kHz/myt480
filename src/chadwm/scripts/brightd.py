#!/usr/bin/env python3
"""brightd — adaptive, jitter-free Fn-key brightness ramping for chadwm.

Why this exists
---------------
The ThinkPad brightness keys are not normal keys:
  * they are absent from X's auto-repeat set, and
  * on this EC they repeat as *discrete press/release pulses* (~10 Hz, a few ms
    long), not as a continuous key-down.
A driver that steps once per detected press therefore advances at the pulse rate
— ~10 %/s, i.e. ~10 s for a full sweep ("very slow"), and one that polls X's key
state misses the short pulses and is jittery.

What this does
--------------
Reads the brightness keys straight from the kernel input devices
(`/dev/input/event*`, EV_KEY codes 224/225 — btw is in group `input`), so the
press/release stream is exact: no X grabs, no polling, no dependence on X's
repeat delay/rate settings. Then it adapts to how the key actually behaves:

  * tap             -> exactly one step (BRIGHT_STEP, default 1 %),
  * continuous hold -> ramp at the device limit (sysfs write ≈ 1.4 ms, <= ~700
                       steps/s),
  * pulsing hold    -> measures the pulse gap P and paces the ramp at
                       MAX_CREDIT / P steps/s, so 10 Hz pulses give ~100 %/s
                       (full sweep ~1 s) instead of 10 %/s, while the credit cap
                       bounds the overshoot after the last pulse to MAX_CREDIT
                       steps (~10 %).

Env: BRIGHT_DEV (acpi_video0), BRIGHT_STEP (percent per step, 1),
     BRIGHT_HOLD (seconds before the sweep starts, 0.2), BRIGHT_STATE,
     BRIGHT_DEBUG=1 (log every key event), BRIGHT_TRACE=1 (log ramp state at 10 Hz).
"""
import os
import pathlib
import select
import struct
import sys
import time

DEV = os.environ.get("BRIGHT_DEV", "acpi_video0")
STEP_PCT = float(os.environ.get("BRIGHT_STEP", "1"))
HOLD = float(os.environ.get("BRIGHT_HOLD", "0.2"))
SYSFS = pathlib.Path("/sys/class/backlight") / DEV
STATE = pathlib.Path(os.environ.get("BRIGHT_STATE", os.path.expanduser("~/.local/state/brightness-pct")))
INPUT_DIR = pathlib.Path("/dev/input")

KEY_DOWN = 224      # KEY_BRIGHTNESSDOWN
KEY_UP = 225        # KEY_BRIGHTNESSUP
EV_KEY = 0x01
EVENT = struct.Struct("llHHi")     # timeval + type + code + value (24 bytes on x86-64)

GRACE = 3.00           # s: a hold sequence ends when no press arrives for this long
                        #     (generous: a sequence must survive a slow EC pulse train; the
                        #      ramp itself stops on the latch below, so this adds no overshoot)
                        #     (the EC can pulse the key as slowly as ~3 Hz, so this must be
                        #      generous; a fixed 0.2 s made every pulse start a new sequence
                        #      and the ramp advanced one step per pulse = "still very slow")
PULSE_GAP = 0.02       # s: a longer gap between presses starts a new pulse measurement
STEPS_PER_PULSE = 20.0 # ramp budget per pulse = the overshoot bound after the last pulse
DEVICE_MIN_INTERVAL = 0.0012   # s between backlight writes (measured device limit)
CREDIT_RATE_CAP = 700.0        # steps/s ceiling for a continuous hold
DIRECTIONS = {KEY_UP: 1, KEY_DOWN: -1}
DEBUG = os.environ.get("BRIGHT_DEBUG") == "1"
DEBUG_LOG = pathlib.Path(os.environ.get("BRIGHT_DEBUG_LOG", "/tmp/brightd-debug.log"))
TRACE = os.environ.get("BRIGHT_TRACE") == "1"
trace_last = 0.0
trace = None


def read_int(path, default=None):
    try:
        return int(path.read_text().strip())
    except Exception:
        return default


def open_devices():
    """Every /dev/input/event* we may read (group `input`)."""
    fds = {}
    for path in sorted(INPUT_DIR.glob("event*")):
        try:
            fds[os.open(path, os.O_RDONLY | os.O_NONBLOCK)] = path
        except OSError:
            continue
    return fds


def main():
    maxb = read_int(SYSFS / "max_brightness")
    if not maxb or maxb <= 0:
        print(f"brightd: no usable backlight at {SYSFS}", file=sys.stderr)
        return 1
    step_units = max(1, int(round(maxb * STEP_PCT / 100.0)))
    bright_path = SYSFS / "brightness"
    try:
        bfd = os.open(bright_path, os.O_WRONLY)
    except OSError as exc:
        print(f"brightd: cannot open {bright_path}: {exc}", file=sys.stderr)
        return 1

    fds = open_devices()
    if not fds:
        print(f"brightd: cannot read any input device in {INPUT_DIR}", file=sys.stderr)
        return 1

    cur = read_int(bright_path, maxb // 2)
    credit = 0.0
    gaps = []
    seq_active = False
    down = False
    last_press = 0.0
    last_dir = 0
    ramp_at = 0.0
    last_step = 0.0
    last_t = time.monotonic()
    last_rescan = last_t

    def write_state():
        try:
            STATE.parent.mkdir(parents=True, exist_ok=True)
            STATE.write_text(f"{round(cur * 100 / maxb)}\n")
        except Exception:
            pass

    def step(direction):
        nonlocal cur
        new = max(1, min(maxb, cur + direction * step_units))
        if new == cur:
            return False
        try:
            os.pwrite(bfd, str(new).encode(), 0)
            cur = new
            return True
        except OSError:
            cur = read_int(bright_path, cur)
            return False

    while True:
        timeout = 0.002 if seq_active else 0.05
        ready, _, _ = select.select(list(fds), [], [], timeout)
        now = time.monotonic()

        for fd in ready:
            try:
                data = os.read(fd, EVENT.size * 64)
            except OSError:
                try:
                    os.close(fd)
                except OSError:
                    pass
                fds.pop(fd, None)
                continue
            for off in range(0, len(data) - EVENT.size + 1, EVENT.size):
                _, _, etype, code, value = EVENT.unpack_from(data, off)
                if etype != EV_KEY or code not in DIRECTIONS:
                    continue
                direction = DIRECTIONS[code]
                ev_t = time.monotonic()
                if DEBUG:
                    try:
                        with DEBUG_LOG.open("a") as fh:
                            kind = "press" if value else "release"
                            fh.write(f"{ev_t:.6f} {kind} {code} value={value}\n")
                    except Exception:
                        pass
                if value == 0:                      # release
                    down = False
                    continue
                if last_press and ev_t - last_press > 1.0:
                    # long silence inside a still-open sequence: another writer may
                    # have changed the level, so resync before ramping again
                    cur = read_int(bright_path, cur)
                    step(direction)
                    last_press = ev_t
                    last_dir = direction
                    down = True
                    continue
                if not seq_active:                  # first press of a sequence
                    seq_active = True
                    last_press = ev_t
                    last_dir = direction
                    ramp_at = ev_t + HOLD
                    credit = 0.0
                    gaps = []
                    cur = read_int(bright_path, cur)
                    step(direction)                 # a tap always moves one step
                else:
                    if last_press and ev_t - last_press > PULSE_GAP:
                        gaps.append(ev_t - last_press)
                        del gaps[:-8]
                    last_press = ev_t
                    last_dir = direction
                down = True

        # end of a hold sequence
        if seq_active and not down and now - last_press > GRACE:
            seq_active = False
            gaps = []
            credit = 0.0
            write_state()

        dt = now - last_t
        last_t = now

        held = down
        mean_gap = 0.0
        max_gap = 0.0
        stale = 0.0
        if not held and seq_active and gaps and last_press:
            recent = gaps[-3:]
            mean_gap = sum(recent) / len(recent)
            max_gap = max(recent)
            stale = now - last_press
            # The key is "held" while its pulse train keeps arriving: the EC's gaps
            # jitter (measured 0.26-0.52 s on this box), so allow 1.2x the widest
            # recent gap before declaring the hold over.
            held = stale < max(0.25, 1.2 * max_gap)

        global trace_last
        if TRACE and now - trace_last > 0.1:
            trace_last = now
            try:
                with DEBUG_LOG.open("a") as fh:
                    fh.write(f"{now:.6f} STATE down={down} seq={seq_active} gaps={len(gaps)} "
                             f"mean={mean_gap:.3f} held={held} credit={credit:.2f} cur={cur} ramp_at_ok={now >= ramp_at}\n")
            except Exception:
                pass

        if held and seq_active and now >= ramp_at:
            if down or not gaps or mean_gap <= 0:
                rate = CREDIT_RATE_CAP
            else:
                # advance STEPS_PER_PULSE steps per pulse period, spread evenly
                rate = min(CREDIT_RATE_CAP, STEPS_PER_PULSE / mean_gap)
                if stale > max_gap:
                    # a pulse is overdue: the key is probably released, so taper the
                    # ramp instead of running on — the movement then ends with the
                    # hold rather than after it
                    rate *= 0.15
            credit = min(STEPS_PER_PULSE, credit + rate * dt)
            # drain the credit: pace writes at the device limit, never faster
            while credit >= 1.0 and last_dir:
                t = time.monotonic()
                if t - last_step < DEVICE_MIN_INTERVAL:
                    time.sleep(DEVICE_MIN_INTERVAL - (t - last_step))
                    continue
                last_step = time.monotonic()
                if step(last_dir):
                    credit -= 1.0
                else:
                    credit = 0.0

        if now - last_rescan > 5.0:                 # survive device hotplug
            last_rescan = now
            for path in sorted(INPUT_DIR.glob("event*")):
                if path not in fds.values():
                    try:
                        fds[os.open(path, os.O_RDONLY | os.O_NONBLOCK)] = path
                    except OSError:
                        pass

    write_state()
    return 0


if __name__ == "__main__":
    sys.exit(main())
