# Sources

| Directory | What | Build |
|---|---|---|
| `thermald-t480/` | one daemon for fan, undervolt (MSR 0x150) and power limits | `make`, install as `/usr/local/bin/thermald-t480`, configuration `/etc/thermald.conf` |
| `slock-ly/` | lock screen with password and fingerprint | `make install` as root (setuid) |
| `gpu-power/` | turns the MX150 on and off by loading and unloading the NVIDIA driver; `prime-run` | `make install` as root (setuid) |
| `syswork/` | overlay worktrees to change `/etc` and `/usr/local` safely | copy `syswork` to `/usr/local/bin` |
| `validity-rs/` | driver for the fingerprint reader (06cb:009a), Rust | `cargo build --release`, see its README |
| `chadwm/` | window manager with my configuration and scripts | `cd chadwm && make install`; `UPSTREAM.txt` names the upstream commit |
| `third-party/` | programs from others | clone `upstream` at `commit` from `UPSTREAM.txt`, apply `local.patch` if there is one |

`thermald-t480` is the only daemon allowed to write the fan level or the undervolt MSRs;
thinkfan and throttled were removed from the system on 2026-10-01.

`chadwm/README.md` is upstream's README. This tree differs from upstream: the keymap of
HyDE (`chadwm/keybinds.txt`), its own bar modules in `scripts/bar.sh`, and no eww widget.
