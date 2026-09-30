# CPU undervolt sweep

`uvset.py c cache gpu uncore analogio` writes the OC-mailbox offsets (mV) and reads them
back; `uvtest.sh` sets a candidate and stresses it: all-core AVX matrix products with
result verification plus GPU renders, load/idle bursts and a single-core turbo burst,
memory and cache pressure; a machine-check event, a verification failure or a changed
offset fails the run, a freeze ends in the watchdog reset and the log names the
candidate. `sweep.sh` (coarse), `fine-ac.sh` (1 mV steps), `sweep-planes.sh` (iGPU and
uncore), `sweep-batt.sh` (on battery), `soak.sh` (long run with suspend). Results on
this i7-8650U are in `docs/notes/2026-09-30-undervolt.md`; `../bench-all.sh` is
the benchmark set used before and after.
