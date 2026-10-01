# MX150 clock offset kit

Finds how far the MX150's graphics and memory clocks can be raised, with tests that catch
instability before it reaches daily use. The driver (580xx) exposes clock offsets through
NVML without an X screen on the NVIDIA GPU; power limits and voltages are not adjustable.

| File | Use |
|---|---|
| `nvoff.py [GPC MEM]` | sets (root) or reads the graphics and memory clock offsets in MHz |
| `gputest.sh LABEL SECONDS GPC MEM` | load test with offsets: mpv renders a synthetic 720p source upscaled to 4K with `ewa_lanczossharp` (shader-bound) on the MX150, in an off-screen window, in 20 s chunks; then 600 frames with the bilinear scaler. Logs clocks, temperature, load and throttle reasons each second. PASS = every chunk finished, no new Xid in the kernel log, offsets read back as set |
| `membw.glsl`, `memcheck.sh OFFSET...` | memory-bound benchmark: a shader that reads a 4K texture 32 times per pixel at scattered addresses; its fps follows the memory clock |
| `gpusweep.sh` | graphics offset sweep, +50 MHz steps, stops at the first FAIL or when the fps stops rising |
| `memsweep.sh` | memory offset sweep, +250 MHz steps, judged by `memcheck.sh`: GDDR5 retries failed transfers, so instability first shows as lost bandwidth |

Needs `mpv`, `python-xlib`, `bc`, the NVIDIA driver, `prime-run` from `src/gpu-power`, and
`ssh root@localhost` (or adapt the `R=` line) for the root parts. The GPU must stay on
during a run (on AC the `gpu-power` policy keeps it on). Results go to `gputest.log` next
to the scripts. The values found on this laptop are in `docs/notes/2026-10-01-mx150-tuning.md`;
`gpu-power` applies the chosen ones from `/etc/gpu-power.conf` after every driver load.

A run that pushes the clocks too far can stall the GPU; the driver usually recovers, and the
PCH watchdog covers a hang of the whole machine. Run it when nothing important is open.
